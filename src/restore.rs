use crate::config::{ BackupConfig, DEFAULT_LOG_FILE };
use crate::http_client::build_http_client;
use crate::utils::log;
use indicatif::{ MultiProgress, ProgressBar, ProgressStyle };
use rayon::prelude::*;
use reqwest::header;
use serde_json::Value;
use std::fs::{ self, File };
use std::io::{ BufReader, Read };
use std::path::Path;
use std::process::Command;
use std::sync::{ Arc, Mutex };
use std::time::Duration;

pub fn run_restore(
    config: &BackupConfig,
    log_file: &Arc<Mutex<File>>,
    specific_index: Option<&str>
) -> Result<(), Box<dyn std::error::Error>> {
    log(log_file, "Starting Elasticsearch restore process")?;

    let backup_dir_path = Path::new(&config.backup_dir);

    let indices = match specific_index {
        Some(index) => {
            let index_path = backup_dir_path.join(index);
            if !index_path.exists() || !index_path.is_dir() {
                return Err(format!("Backup for index '{}' not found", index).into());
            }
            vec![index.to_string()]
        }
        None =>
            fs
                ::read_dir(backup_dir_path)?
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    if path.is_dir() && path.file_name()?.to_str()?.starts_with('.') == false {
                        Some(path.file_name()?.to_str()?.to_string())
                    } else {
                        None
                    }
                })
                .collect(),
    };

    if indices.is_empty() {
        log(log_file, "No backups found to restore")?;
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner().template("{spinner} {msg}").unwrap());
        pb.set_message("No backups found to restore");
        pb.finish_and_clear();
        return Ok(());
    }

    log(log_file, &format!("Found {} indices to restore", indices.len()))?;

    let multi = Arc::new(MultiProgress::new());
    let pb_main = multi.add(ProgressBar::new(indices.len() as u64));
    pb_main.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.magenta/purple}] {pos}/{len} ({eta}) Indices"
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
                        "{spinner:.green} [{elapsed_precise}] [{bar:40.magenta/purple}] {pos}/{len} ({eta}) {msg}"
                    )
                    .unwrap()
                    .progress_chars("#>-")
            );
            pb_index.set_message(index.to_string());

            let result = restore_index(config, index, log_file, &pb_index);
            if let Err(e) = result {
                let _ = log(log_file, &format!("Error restoring index {}: {}", index, e));
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
        &format!("Restore completed successfully in {:.2} seconds", duration.as_secs_f64())
    )?;

    Ok(())
}

fn restore_index(
    config: &BackupConfig,
    index: &str,
    log_file: &Arc<Mutex<File>>,
    pb_index: &ProgressBar
) -> Result<(), Box<dyn std::error::Error>> {
    log(log_file, &format!("Starting restore for index: {}", index))?;

    let index_dir = Path::new(&config.backup_dir).join(index);
    if !index_dir.exists() || !index_dir.is_dir() {
        return Err(format!("Backup directory for index '{}' not found", index).into());
    }

    restore_mapping(config, index, &index_dir, log_file)?;
    restore_data(config, index, &index_dir, log_file, pb_index)?;

    log(log_file, &format!("Restore completed for index: {}", index))?;
    Ok(())
}

fn restore_data(
    config: &BackupConfig,
    index: &str,
    index_dir: &Path,
    log_file: &Arc<Mutex<File>>,
    pb_index: &ProgressBar
) -> Result<(), Box<dyn std::error::Error>> {
    let data_file = index_dir.join(format!("{}_data.json", index));
    let gz_data_file = index_dir.join(format!("{}_data.json.gz", index));

    let data_path = if data_file.exists() {
        data_file
    } else if gz_data_file.exists() {
        log(log_file, &format!("Uncompressing data file for index: {}", index))?;
        let status = Command::new("gunzip").arg("-k").arg(&gz_data_file).status()?;

        if !status.success() {
            pb_index.abandon_with_message("Failed to uncompress data file");
            return Err(format!("Failed to uncompress data file for index '{}'", index).into());
        }

        data_file
    } else {
        pb_index.abandon_with_message("Data file not found");
        return Err(format!("Data file for index '{}' not found", index).into());
    };

    log(log_file, &format!("Reading data file for index: {}", index))?;

    let file = File::open(&data_path)?;
    let reader = BufReader::with_capacity(config.buffer_size, file);
    let documents: Vec<Value> = serde_json::from_reader(reader)?;

    let doc_count = documents.len() as u64;
    if doc_count == 0 {
        log(log_file, &format!("Index {} has no documents, skipping restore", index))?;
        pb_index.set_message(format!("{} (empty)", index));
        pb_index.finish_and_clear();
        return Ok(());
    }

    log(log_file, &format!("Found {} documents to restore for index: {}", doc_count, index))?;

    let batch_count = ((doc_count as f64) / (config.bulk_batch_size as f64)).ceil() as u64;
    pb_index.set_length(batch_count);
    pb_index.set_message(index.to_string());

    let client = build_http_client(config)?;
    let bulk_url = format!("{}/_bulk", config.host);

    for (batch_num, chunk) in documents.chunks(config.bulk_batch_size).enumerate() {
        let mut bulk_body = String::with_capacity(config.buffer_size);

        for doc in chunk {
            let doc_id = doc["_id"].as_str().unwrap_or("");
            let action = format!(
                "{{ \"index\": {{ \"_index\": \"{}\", \"_id\": \"{}\" }} }}\n",
                index,
                doc_id
            );
            bulk_body.push_str(&action);

            if let Some(source) = doc["_source"].as_object() {
                let source_line = serde_json::to_string(source)?;
                bulk_body.push_str(&source_line);
                bulk_body.push('\n');
            }
        }

        log(
            log_file,
            &format!(
                "Uploading batch {} for index: {} ({} documents)",
                batch_num + 1,
                index,
                chunk.len()
            )
        )?;

        let response = client
            .post(&bulk_url)
            .header(header::CONTENT_TYPE, "application/x-ndjson")
            .body(bulk_body)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text()?;
            pb_index.abandon_with_message(format!("Bulk upload failed: {}", status));
            return Err(
                format!(
                    "Bulk upload failed for index '{}': {} - {}",
                    index,
                    status,
                    error_text
                ).into()
            );
        }

        let response_text = response.text()?;
        let response_json: Value = serde_json::from_str(&response_text)?;
        if response_json["errors"].as_bool().unwrap_or(false) {
            log(
                log_file,
                &format!("Warning: Some errors occurred during bulk upload for index: {}", index)
            )?;

            if let Some(items) = response_json["items"].as_array() {
                let errors: Vec<_> = items
                    .iter()
                    .filter_map(|item| {
                        if let Some(error) = item["index"]["error"].as_object() {
                            Some(
                                format!(
                                    "{}: {}",
                                    error["type"].as_str().unwrap_or("unknown"),
                                    error["reason"].as_str().unwrap_or("unknown reason")
                                )
                            )
                        } else {
                            None
                        }
                    })
                    .take(5)
                    .collect();

                if !errors.is_empty() {
                    log(log_file, &format!("First few errors: {}", errors.join(", ")))?;
                }
            }
        }

        pb_index.inc(1);
    }

    log(
        log_file,
        &format!("Data restoration completed for index: {}. Total documents: {}", index, doc_count)
    )?;
    Ok(())
}

fn restore_mapping(
    config: &BackupConfig,
    index: &str,
    index_dir: &Path,
    log_file: &Arc<Mutex<File>>
) -> Result<(), Box<dyn std::error::Error>> {
    let client = build_http_client(config)?;
    let mapping_file = index_dir.join(format!("{}_mapping.json", index));

    let file = File::open(&mapping_file)?;
    let reader = BufReader::new(file);
    let mapping_json: Value = serde_json::from_reader(reader)?;

    let create_index_url = format!("{}/{}", config.host, index);
    let response = client.put(&create_index_url).json(&mapping_json).send()?;

    if !response.status().is_success() {
        let error_text = response.text()?;
        return Err(format!("Failed to create index '{}': {}", index, error_text).into());
    }

    log(log_file, &format!("Mapping restored for index: {}", index))?;
    Ok(())
}
