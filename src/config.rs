use serde::{ Deserialize, Serialize };
use std::fs::File;
use std::io::{ Read, Write };

#[derive(Debug)]
pub enum Operation {
    Backup {
        index: Option<String>,
    },
    Restore {
        index: Option<String>,
    },
}

#[derive(Debug)]
pub struct BackupConfig {
    pub host: String,
    pub backup_dir: String,
    pub auth: Option<(String, String)>,
    pub skip_indices: Vec<String>,
    pub max_index_size_mb: Option<u64>,
    pub operation: Operation,
    pub connect_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub scroll_size: u64,
    pub scroll_time: String,
    pub max_parallel_indices: usize,
    pub buffer_size: usize,
    pub bulk_batch_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    pub elastic: ElasticConfig,
    pub backup: BackupConfigFile,
    pub restore: RestoreConfigFile,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ElasticConfig {
    pub host: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_secs: Option<u64>,
    pub connect_timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupConfigFile {
    pub backup_dir: Option<String>,
    pub scroll_size: Option<u64>,
    pub scroll_time: Option<String>,
    pub max_parallel_indices: Option<usize>,
    pub skip_indices: Option<Vec<String>>,
    pub max_index_size_mb: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreConfigFile {
    pub bulk_batch_size: Option<usize>,
}

pub const DEFAULT_BACKUP_DIR: &str = "./backups";
pub const DEFAULT_LOG_FILE: &str = "backup.log";
pub const DEFAULT_ELASTIC_HOST: &str = "http://es.example.com:9200";
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 180;
pub const DEFAULT_SCROLL_SIZE: u64 = 10000;
pub const DEFAULT_SCROLL_TIME: &str = "10m";
pub const DEFAULT_MAX_PARALLEL_INDICES: usize = 4;
pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;
pub const DEFAULT_BULK_BATCH_SIZE: usize = 5000;

pub fn load_config() -> Result<ConfigFile, Box<dyn std::error::Error>> {
    let config_path = "config.toml";
    let mut config_content = String::new();

    match File::open(config_path) {
        Ok(mut file) => {
            file.read_to_string(&mut config_content)?;
        }
        Err(_) => {
            let default_config = ConfigFile {
                elastic: ElasticConfig {
                    host: Some(DEFAULT_ELASTIC_HOST.to_string()),
                    username: Some("es_user".to_string()),
                    password: Some("securepass123".to_string()),
                    timeout_secs: Some(DEFAULT_REQUEST_TIMEOUT_SECS),
                    connect_timeout_secs: Some(DEFAULT_CONNECT_TIMEOUT_SECS),
                },
                backup: BackupConfigFile {
                    backup_dir: Some(DEFAULT_BACKUP_DIR.to_string()),
                    scroll_size: Some(DEFAULT_SCROLL_SIZE),
                    scroll_time: Some(DEFAULT_SCROLL_TIME.to_string()),
                    max_parallel_indices: Some(DEFAULT_MAX_PARALLEL_INDICES),
                    skip_indices: Some(vec![]),
                    max_index_size_mb: None,
                },
                restore: RestoreConfigFile {
                    bulk_batch_size: Some(DEFAULT_BULK_BATCH_SIZE),
                },
            };

            let toml_content = toml::to_string(&default_config)?;
            let mut file = File::create(config_path)?;
            file.write_all(toml_content.as_bytes())?;
            return Ok(default_config);
        }
    }

    let config: ConfigFile = toml::from_str(&config_content)?;
    Ok(config)
}
