use crate::config::DEFAULT_LOG_FILE;
use chrono::Local;
use serde_json::Value;
use std::fs::{ self, File };
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::{ Arc, Mutex };
use reqwest::blocking::Client;

pub fn setup_backup_dir(backup_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(backup_dir)?;
    Ok(())
}

pub fn log(log_file: &Arc<Mutex<File>>, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let log_message = format!("[{}] {}\n", timestamp, message);
    let mut file = log_file.lock().unwrap();
    file.write_all(log_message.as_bytes())?;
    file.flush()?;
    Ok(())
}

pub fn reduce_document_size(doc: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    let mut reduced = doc.clone();
    if let Some(obj) = reduced.as_object_mut() {
        obj.remove("_index");
        obj.remove("_type");
        obj.remove("_score");
    }
    Ok(reduced)
}

#[cfg(feature = "compression")]
pub fn compress_file(file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("gzip").arg("-k").arg(file_path).status()?;
    if !status.success() {
        return Err("Failed to compress file".into());
    }
    Ok(())
}

pub fn get_elasticsearch_version(
    client: &Client,
    host: &str,
    log_file: &Arc<Mutex<File>>
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{}/", host);
    let response = client.get(&url).send()?;
    let status = response.status();
    let response_text = response.text()?;

    log(log_file, &format!("Response from version check (status: {}): {}", status, response_text))?;

    if !status.is_success() {
        return Err(format!("Failed to fetch version: {}", status).into());
    }

    let json: Value = serde_json::from_str(&response_text)?;
    let version = json["version"]["number"]
        .as_str()
        .ok_or("No version number found in response")?
        .to_string();

    Ok(version)
}
