# Elasticsearch Backup and Restore Tool

This Rust application provides a robust solution for backing up and restoring Elasticsearch indices. It supports backing up all indices or a specific index, restoring individual or all indices, and features a clean progress bar interface for real-time monitoring. Backups are stored as JSON files (with optional gzip compression) in a configurable directory, leveraging parallel processing for efficiency.

## Features
- **Backup All Indices**: Save all non-system Elasticsearch indices to a specified directory.
- **Backup Specific Index**: Back up a single index by name.
- **Restore Indices**: Restore all indices or a specific index from backup files.
- **Clean Progress Bars**: Cyan/blue bars for backup and magenta/purple for restore, with completed bars cleared to reduce clutter.
- **Detailed Logging**: Logs written to a file for debugging and verification.
- **Configurable Settings**: Customize batch sizes, parallelism, and skipped indices via `config.toml`.
- **Robust Error Handling**: Clear console and log messages for network, file, and Elasticsearch errors.

## Prerequisites
- **Rust** (for source code usage): Install Rust and Cargo (https://www.rust-lang.org/tools/install).
- **Elasticsearch**: A running Elasticsearch cluster (e.g., `http://es.example.com:9200`) with valid credentials.
- **Dependencies** (source code): Included in `Cargo.toml` (`indicatif`, `rayon`, `reqwest`, `serde`, etc.).
- **Optional**: `gunzip` for compressed backups (`sudo apt-get install gzip` on Debian/Ubuntu).
- **Binary Usage**: No Rust installation needed; download the precompiled binary (see Released File Usage).

## Setup
### Source Code Setup
1. **Clone the Repository**:
   ```bash
   git clone <repository-url>
   cd <repository-directory>
   ```

2. **Install Dependencies**:
   ```bash
   cargo build
   ```

3. **Configure `config.toml`**:
   - A default `config.toml` is created on first run. Edit it to match your Elasticsearch setup:
     ```toml
     [elastic]
     host = "http://es.example.com:9200"
     username = "es_user"
     password = "securepass123"
     timeout_secs = 180
     connect_timeout_secs = 60

     [backup]
     backup_dir = "./backups"
     scroll_size = 10000
     scroll_time = "10m"
     max_parallel_indices = 4
     skip_indices = []
     max_index_size_mb = null

     [restore]
     bulk_batch_size = 5000
     ```
   - **Key Settings**:
     - `host`: Elasticsearch URL (use `http` to avoid certificate issues).
     - `username`/`password`: Remove if authentication is not required.
     - `backup_dir`: Directory for backups (must be writable).
     - `scroll_size`: Documents per backup batch (reduce for memory constraints).
     - `bulk_batch_size`: Documents per restore batch (reduce for large indices).
     - `max_parallel_indices`: Concurrent indices processed (default 4; reduce for less clutter).

4. **Create Backup Directory**:
   ```bash
   mkdir -p ./backups
   chmod u+w ./backups
   ```

5. **Test Elasticsearch Connectivity**:
   ```bash
   curl -u es_user:securepass123 http://es.example.com:9200
   ```

### Released File Usage
1. **Download the Binary**:
   - Obtain the precompiled binary for your platform from the repository’s Releases page (`<repository-url>/releases`).
   - Example filenames:
     - Linux: `es-backup-linux-x86_64`
     - macOS: `es-backup-macos-x86_64`
     - Windows: `es-backup-windows-x86_64.exe`

2. **Make Executable (Linux/macOS)**:
   ```bash
   chmod +x es-backup-linux-x86_64
   ```

3. **Place Binary**:
   - Move the binary to a directory in your PATH (e.g., `/usr/local/bin`) or run it from its current location:
     ```bash
     mv es-backup-linux-x86_64 /usr/local/bin/es-backup
     ```
   - For Windows, place `es-backup-windows-x86_64.exe` in a convenient directory (e.g., `C:\Tools`).

4. **Create `config.toml`**:
   - Create a `config.toml` file in the same directory as the binary or in `./backups/` with the content above.
   - Ensure `backup_dir` exists:
     ```bash
     mkdir -p ./backups
     chmod u+w ./backups
     ```

5. **Test Connectivity**:
   - Same as source code setup (use `curl` to verify).

## Usage
### Source Code Commands
Run with `cargo run` from the project directory.

- **Back Up All Indices**:
  ```bash
  cargo run
  ```
  Or:
  ```bash
  cargo run -- backup
  ```

- **Back Up a Specific Index**:
  ```bash
  cargo run -- backup sample-index-2025-01-01
  ```

- **Restore All Indices**:
  ```bash
  cargo run -- restore
  ```

- **Restore a Specific Index**:
  ```bash
  cargo run -- restore sample-index-2025-01-01
  ```

### Binary Commands
Run the binary directly (replace `es-backup` with the binary name for your platform, e.g., `es-backup-windows-x86_64.exe` on Windows).

- **Back Up All Indices**:
  ```bash
  ./es-backup backup
  ```
  Or (Windows):
  ```cmd
  es-backup-windows-x86_64.exe backup
  ```

- **Back Up a Specific Index**:
  ```bash
  ./es-backup backup sample-index-2025-01-01
  ```

- **Restore All Indices**:
  ```bash
  ./es-backup restore
  ```

- **Restore a Specific Index**:
  ```bash
  ./es-backup restore sample-index-2025-01-01
  ```

### Output Example
- **Backup** (cyan/blue bars):
  ```
  ⠁ [00:00:00] [----------------------------------------] 0/50 (0s) Indices
  ⠂ [00:00:01] [>---------------------------------------] 10000/1000000 (2m) sample-index-2025-01-01
  ⠠ [00:00:01] [>---------------------------------------] 5000/2000000 (10m) logs-2025-01-01
  ...
  [00:00:30] [########################################] 50/50 (0s) Completed in 30.00 seconds
  ```

- **Restore** (magenta/purple bars):
  ```
  ⠁ [00:00:00] [----------------------------------------] 0/50 (0s) Indices
  ⠂ [00:00:01] [>---------------------------------------] 1/200 (2m) sample-index-2025-01-01
  ...
  [00:00:30] [########################################] 50/50 (0s) Completed in 30.00 seconds
  ```

### Logs
- Logs are written to `backup_dir/backup.log` (e.g., `./backups/backup.log`).
- Example:
  ```
  [2025-01-01 09:00:00] Starting Elasticsearch backup process
  [2025-01-01 09:00:01] Starting backup for index: sample-index-2025-01-01
  [2025-01-01 09:00:30] Completed data export for index: sample-index-2025-01-01. Total documents: 1000000
  ```

## Verify Operations
- **Check Backup Files**:
  ```bash
  ls ./backups/sample-index-2025-01-01/
  ```
  Expected: `sample-index-2025-01-01_mapping.json`, `sample-index-2025-01-01_data.json` (or `.gz`).

- **Verify Compressed Files**:
  ```bash
  gunzip -t ./backups/sample-index-2025-01-01/sample-index-2025-01-01_data.json.gz
  ```

- **Confirm Restored Indices**:
  ```bash
  curl -u es_user:securepass123 http://es.example.com:9200/sample-index-2025-01-01/_count
  ```

## Troubleshooting
- **DNS Error (e.g., “api.certprovider.com does not exist”)**:
  - Caused by HTTPS certificate issues or a misconfigured proxy.
  - **Fix**:
    1. Use `http` in `config.toml`:
       ```toml
       host = "http://es.example.com:9200"
       ```
    2. Disable proxy:
       ```bash
       unset http_proxy
       unset https_proxy
       ```
    3. Set DNS resolver:
       ```bash
       echo "nameserver 8.8.8.8" | sudo tee /etc/resolv.conf
       ```
    4. Test:
       ```bash
       curl -u es_user:securepass123 http://es.example.com:9200
       ```

- **Error: “Index ‘{index}’ does not exist”**:
  - Index not in Elasticsearch.
  - List indices:
    ```bash
    curl -u es_user:securepass123 http://es.example.com:9200/_cat/indices
    ```

- **Error: “Backup directory for index ‘{index}’ not found”**:
  - Missing backup directory.
  - Check:
    ```bash
    ls ./backups
    ```

- **Error: “Failed to uncompress data file”**:
  - Missing `gunzip` or corrupted file.
  - Install:
    ```bash
    sudo apt-get install gzip
    ```
  - Test:
    ```bash
    gunzip -t ./backups/sample-index-2025-01-01/sample-index-2025-01-01_data.json.gz
    ```

- **Error: “Bulk upload failed”**:
  - Elasticsearch rejected a restore batch.
  - Check `backup.log`.
  - Reduce `bulk_batch_size`:
    ```toml
    bulk_batch_size = 1000
    ```

- **Slow Backup/Restore**:
  - Large indices or high batch sizes.
  - Adjust `config.toml`:
    ```toml
    scroll_size = 5000
    bulk_batch_size = 1000
    max_parallel_indices = 2
    ```

- **Index Already Exists**:
  - Restore skips mapping if index exists.
  - Delete to overwrite:
    ```bash
    curl -u es_user:securepass123 -X DELETE http://es.example.com:9200/sample-index-2025-01-01
    ```

## Customization
- **Fewer Progress Bars**: Set `max_parallel_indices = 2` in `config.toml`.
- **Performance Tuning**: Lower `scroll_size` or `bulk_batch_size`.
- **Skip Indices**: Add to `skip_indices`:
  ```toml
  skip_indices = ["system_index", "old_logs"]
  ```

## Example Workflow
1. **Back Up a Specific Index**:
   ```bash
   ./es-backup backup sample-index-2025-01-01
   ```
   Or (source code):
   ```bash
   cargo run -- backup sample-index-2025-01-01
   ```

2. **Restore the Index**:
   ```bash
   ./es-backup restore sample-index-2025-01-01
   ```

3. **Verify**:
   ```bash
   curl -u es_user:securepass123 http://es.example.com:9200/sample-index-2025-01-01/_count
   ```

## Notes
- **Security**: Use environment variables for credentials:
  ```bash
  export ES_USERNAME=es_user
  export ES_PASSWORD=securepass123
  ```
- **Compression**: Enable with the `compression` feature for `.gz` files.
- **Testing**: Start with small indices to verify setup.
- **Support**: Check `backup.log` or open a repository issue for help.

Last updated: January 2025