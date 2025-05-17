use chrono::Local;
use serde_json::Value;
use std::fs::{ self, File };
use std::io::Write;

use std::sync::{ Arc, Mutex };

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
