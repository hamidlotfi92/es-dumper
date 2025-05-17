use crate::config::{ BackupConfig };
use crate::http_client::build_http_client;
use crate::utils::{ log, reduce_document_size, compress_file };
use indicatif::{ MultiProgress, ProgressBar, ProgressStyle };
use rayon::prelude::*;

use serde_json::Value;
use std::fs::{ self, File };
use std::io::{ BufWriter, Write };
use std::path::Path;
use std::sync::{ Arc, Mutex };

pub fn run_backup(
    config: &BackupConfig,
    log_file: &Arc<Mutex<File>>,
    specific_index: Option<&str>
) -> Result<(), Box<dyn std::error::Error>> {
    log(log_file, "Starting Elasticsearch backup process")?;

    let indices = match specific_index {
        Some(index) => {
            let client = build_http_client(config)?;
            let url = format!("{}/{}/_count", config.host, index);
            let response = client.get(&url).send()?;
            if !response.status().is_success() {
                let pb = ProgressBar::new_spinner();
                pb.set_style(ProgressStyle::default_spinner().template("{spinner} {msg}").unwrap());
                pb.set_message(format!("Index '{}' does not exist", index));
                pb.finish_and_clear();
                return Err(format!("Index '{}' does not exist", index).into());
            }
            vec![index.to_string()]
        }
        None => fetch_indices(config)?,
    };

    if indices.is_empty() {
        log(log_file, "No indices found to backup")?;
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner().template("{spinner} {msg}").unwrap());
        pb.set_message("No indices found to backup");
        pb.finish_and_clear();
        return Ok(());
    }

    log(log_file, &format!("Found {} indices to backup", indices.len()))?;

    let multi = Arc::new(MultiProgress::new());
    let pb_main = multi.add(ProgressBar::new(indices.len() as u64));
    pb_main.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) Indices"
            )
            .unwrap()
            .progress_chars("#>-")
    );

    let start_time = std::time::Instant::now();

    let completed_indices = Arc::new(Mutex::new(0));

    indices.par_chunks(config.max_parallel_indices).for_each(|chunk| {
        for index in chunk {
            let pb_index = multi.add(ProgressBar::new(0));
            pb_index.set_style(
                ProgressStyle::default_bar()
                    .template(
                        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}"
                    )
                    .unwrap()
                    .progress_chars("#>-")
            );
            pb_index.set_message(index.to_string());

            let result = backup_index(config, index, log_file, &pb_index);
            if let Err(e) = result {
                let _ = log(log_file, &format!("Error backing up index {}: {}", index, e));
                pb_index.abandon_with_message(format!("Error: {}", e));
            } else {
                pb_index.finish_and_clear();
            }

            let mut completed = completed_indices.lock().unwrap();
            *completed += 1;
            pb_main.set_position(*completed);
        }
    });

    let duration = start_time.elapsed();
    pb_main.finish_with_message(format!("Completed in {:.2} seconds", duration.as_secs_f64()));
    log(
        log_file,
        &format!("Backup completed successfully in {:.2} seconds", duration.as_secs_f64())
    )?;

    Ok(())
}

fn backup_index(
    config: &BackupConfig,
    index: &str,
    log_file: &Arc<Mutex<File>>,
    pb_index: &ProgressBar
) -> Result<(), Box<dyn std::error::Error>> {
    log(log_file, &format!("Starting backup for index: {}", index))?;

    let index_dir = Path::new(&config.backup_dir).join(index);
    fs::create_dir_all(&index_dir)?;

    backup_mapping(config, index, &index_dir, log_file)?;
    backup_data(config, index, &index_dir, log_file, pb_index)?;

    log(log_file, &format!("Backup completed for index: {}", index))?;
    Ok(())
}

fn backup_data(
    config: &BackupConfig,
    index: &str,
    index_dir: &Path,
    log_file: &Arc<Mutex<File>>,
    pb_index: &ProgressBar
) -> Result<(), Box<dyn std::error::Error>> {
    let client = build_http_client(config)?;

    let count_url = format!("{}/{}/_count", config.host, index);
    let count_response = client.get(&count_url).send()?;
    let count_json: Value = count_response.json()?;
    let doc_count = count_json["count"].as_u64().unwrap_or(0);

    if doc_count == 0 {
        log(log_file, &format!("Index {} is empty, skipping data backup", index))?;
        pb_index.set_message(format!("{} (empty)", index));
        pb_index.finish_and_clear();
        return Ok(());
    }

    pb_index.set_length(doc_count);

    let scroll_url = format!("{}/{}/_search?scroll={}", config.host, index, config.scroll_time);

    let scroll_body =
        serde_json::json!({
        "size": config.scroll_size,
        "query": { "match_all": {} },
        "_source": true,
        "sort": ["_doc"]
    });

    log(log_file, &format!("Starting data export for index: {} ({} documents)", index, doc_count))?;

    let response = client.post(&scroll_url).json(&scroll_body).send()?;

    if !response.status().is_success() {
        pb_index.abandon_with_message(format!("Scroll failed: {}", response.status()));
        return Err(
            format!("Failed to initialize scroll for {}: {}", index, response.status()).into()
        );
    }

    let response_json: Value = response.json()?;
    let mut scroll_id = response_json["_scroll_id"]
        .as_str()
        .ok_or("No scroll ID returned")?
        .to_string();

    let data_file = index_dir.join(format!("{}_data.json", index));
    let file = File::create(&data_file)?;
    let mut writer = BufWriter::with_capacity(config.buffer_size, file);

    let hits = response_json["hits"]["hits"].as_array().ok_or("Invalid hits format")?;

    writer.write_all(b"[")?;

    let mut total_docs = 0;
    let mut is_first = true;

    for hit in hits {
        if !is_first {
            writer.write_all(b",")?;
        }

        let reduced_doc = reduce_document_size(hit)?;
        serde_json::to_writer(&mut writer, &reduced_doc)?;

        is_first = false;
        total_docs += 1;
        pb_index.inc(1);
    }

    let mut batch_hits: Vec<Value>;

    while !hits.is_empty() {
        let scroll_continue_url = format!("{}/_search/scroll", config.host);
        let continue_body =
            serde_json::json!({
            "scroll": config.scroll_time,
            "scroll_id": scroll_id
        });

        let continue_response = client.post(&scroll_continue_url).json(&continue_body).send()?;

        if !continue_response.status().is_success() {
            let _ = client
                .delete(&format!("{}/_search/scroll", config.host))
                .json(&serde_json::json!({"scroll_id": [scroll_id]}))
                .send();

            pb_index.abandon_with_message(format!("Scroll failed: {}", continue_response.status()));
            return Err(format!("Failed to continue scroll: {}", continue_response.status()).into());
        }

        let continue_json: Value = continue_response.json()?;
        scroll_id = continue_json["_scroll_id"]
            .as_str()
            .ok_or("No scroll ID returned")?
            .to_string();

        batch_hits = continue_json["hits"]["hits"].as_array().ok_or("Invalid hits format")?.clone();

        if batch_hits.is_empty() {
            break;
        }

        for hit in &batch_hits {
            writer.write_all(b",")?;
            let reduced_doc = reduce_document_size(hit)?;
            serde_json::to_writer(&mut writer, &reduced_doc)?;
            total_docs += 1;
            pb_index.inc(1);
        }

        writer.flush()?;
    }

    writer.write_all(b"]")?;
    writer.flush()?;

    let _ = client
        .delete(&format!("{}/_search/scroll", config.host))
        .json(&serde_json::json!({"scroll_id": [scroll_id]}))
        .send();

    log(
        log_file,
        &format!("Completed data export for index: {}. Total documents: {}", index, total_docs)
    )?;

    #[cfg(feature = "compression")]
    compress_file(&data_file)?;

    Ok(())
}

fn backup_mapping(
    config: &BackupConfig,
    index: &str,
    index_dir: &Path,
    log_file: &Arc<Mutex<File>>
) -> Result<(), Box<dyn std::error::Error>> {
    let client = build_http_client(config)?;
    let mapping_url = format!("{}/{}/_mapping", config.host, index);
    let mapping_response = client.get(&mapping_url).send()?;
    let mapping_json: Value = mapping_response.json()?;

    let mapping_file = index_dir.join(format!("{}_mapping.json", index));
    let file = File::create(&mapping_file)?;
    serde_json::to_writer_pretty(file, &mapping_json)?;

    log(log_file, &format!("Mapping backed up for index: {}", index))?;
    Ok(())
}

fn fetch_indices(config: &BackupConfig) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = build_http_client(config)?;
    let cat_indices_url = format!("{}/_cat/indices?format=json", config.host);
    let response = client.get(&cat_indices_url).send()?;
    let indices: Vec<Value> = response.json()?;

    let mut result = indices
        .into_iter()
        .filter_map(|index| {
            let index_name = index["index"].as_str()?;
            if index_name.starts_with('.') || config.skip_indices.contains(&index_name.to_string()) {
                None
            } else {
                Some(index_name.to_string())
            }
        })
        .collect::<Vec<String>>();

    if let Some(max_size_mb) = config.max_index_size_mb {
        result.retain(|index| {
            let size_url = format!("{}/{}/_stats/store", config.host, index);
            if let Ok(response) = client.get(&size_url).send() {
                if let Ok(json) = response.json::<Value>() {
                    if
                        let Some(size_bytes) =
                            json["indices"][index]["total"]["store"]["size_in_bytes"].as_u64()
                    {
                        let size_mb = size_bytes / (1024 * 1024);
                        return size_mb <= max_size_mb;
                    }
                }
            }
            true
        });
    }

    result.sort();
    Ok(result)
}
