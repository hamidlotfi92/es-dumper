use crate::config::BackupConfig;
use reqwest::blocking::Client;
use reqwest::header::{ self, HeaderMap, HeaderValue };
use std::time::Duration;
use base64::encode;

pub fn build_http_client(config: &BackupConfig) -> Result<Client, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));

    // Add Basic Authentication header if credentials are provided
    if let Some((username, password)) = &config.auth {
        let auth = encode(format!("{}:{}", username, password));
        let auth_header = HeaderValue::from_str(&format!("Basic {}", auth))?;
        headers.insert(header::AUTHORIZATION, auth_header);
    }

    let client = Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(config.request_timeout_secs))
        .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
        .danger_accept_invalid_certs(true)
        .build()?;

    Ok(client)
}
