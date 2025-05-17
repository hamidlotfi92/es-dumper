mod backup;
mod config;
mod http_client;
mod restore;
mod utils;

use config::{ BackupConfig, Operation };
use std::env;
use std::fs::File;
use std::path::Path;
use std::sync::{ Arc, Mutex };
use utils::{ setup_backup_dir };

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let operation = match args.get(1).map(|s| s.as_str()) {
        Some("restore") =>
            Operation::Restore {
                index: args.get(2).cloned(),
            },
        Some("backup") =>
            Operation::Backup {
                index: args.get(2).cloned(),
            },
        _ => Operation::Backup { index: None },
    };

    // Load config from file
    let config_file = config::load_config()?;

    let backup_dir = config_file.backup.backup_dir.unwrap_or(
        config::DEFAULT_BACKUP_DIR.to_string()
    );
    let host = config_file.elastic.host.unwrap_or(config::DEFAULT_ELASTIC_HOST.to_string());

    let config = BackupConfig {
        host,
        backup_dir,
        auth: match (config_file.elastic.username, config_file.elastic.password) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        },
        skip_indices: config_file.backup.skip_indices.unwrap_or_default(),
        max_index_size_mb: config_file.backup.max_index_size_mb,
        operation,
        connect_timeout_secs: config_file.elastic.connect_timeout_secs.unwrap_or(
            config::DEFAULT_CONNECT_TIMEOUT_SECS
        ),
        request_timeout_secs: config_file.elastic.timeout_secs.unwrap_or(
            config::DEFAULT_REQUEST_TIMEOUT_SECS
        ),
        scroll_size: config_file.backup.scroll_size.unwrap_or(config::DEFAULT_SCROLL_SIZE),
        scroll_time: config_file.backup.scroll_time.unwrap_or_else(||
            config::DEFAULT_SCROLL_TIME.to_string()
        ),
        max_parallel_indices: config_file.backup.max_parallel_indices.unwrap_or(
            config::DEFAULT_MAX_PARALLEL_INDICES
        ),
        buffer_size: config::DEFAULT_BUFFER_SIZE,
        bulk_batch_size: config_file.restore.bulk_batch_size.unwrap_or(
            config::DEFAULT_BULK_BATCH_SIZE
        ),
    };

    setup_backup_dir(&config.backup_dir)?;

    let log_path = Path::new(&config.backup_dir).join(config::DEFAULT_LOG_FILE);
    let log_file = File::options().append(true).create(true).open(&log_path)?;
    let log_file = Arc::new(Mutex::new(log_file));

    match &config.operation {
        Operation::Backup { index } => backup::run_backup(&config, &log_file, index.as_deref())?,
        Operation::Restore { index } => restore::run_restore(&config, &log_file, index.as_deref())?,
    }

    Ok(())
}
