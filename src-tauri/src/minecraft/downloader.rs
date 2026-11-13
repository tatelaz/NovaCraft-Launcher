use futures_util::stream::{self, StreamExt};
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::Emitter;

/// User-Agent sent with every request (Modrinth and Adoptium ask for one).
pub const USER_AGENT: &str = concat!("NovaCraft-Launcher/", env!("CARGO_PKG_VERSION"));

/// How many times a single file is attempted before giving up.
const MAX_ATTEMPTS: u32 = 3;

/// Progress information emitted to the frontend during downloads.
#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub file_name: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f32,
    pub stage: String,
}

/// Shared HTTP client: one connection pool for the whole app, a User-Agent,
/// and timeouts so a dead connection can never hang the launcher forever.
pub fn http_client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(Duration::from_secs(15))
                .read_timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

/// Download a file using the shared client.
/// Skips download if the file already exists with the correct SHA-1 hash.
pub async fn download_file(url: &str, path: &Path, expected_sha1: Option<&str>) -> Result<(), String> {
    download_file_client(&http_client(), url, path, expected_sha1).await
}

/// Download a file, retrying temporary failures (network hiccups, server errors,
/// corrupted data) up to `MAX_ATTEMPTS` times.
/// Skips download if the file already exists with the correct SHA-1 hash.
pub async fn download_file_client(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), String> {
    // Already present and valid? Nothing to do (this is also what makes offline play work).
    if path.exists() {
        match expected_sha1 {
            Some(expected) => {
                if verify_sha1(path, expected)? {
                    return Ok(());
                }
            }
            None => return Ok(()),
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Impossibile creare la cartella {:?}: {}", parent, e))?;
    }

    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match try_download(client, url, path, expected_sha1).await {
            Ok(()) => return Ok(()),
            Err(Attempt::Fatal(e)) => return Err(e),
            Err(Attempt::Retry(e)) => {
                last_err = e;
                if attempt < MAX_ATTEMPTS {
                    // 0.5s, then 2s: gives a flaky connection time to recover.
                    let wait = 500 * u64::from(attempt * attempt);
                    tokio::time::sleep(Duration::from_millis(wait)).await;
                }
            }
        }
    }
    Err(last_err)
}

enum Attempt {
    /// Worth trying again (network error, 5xx, corrupted body).
    Retry(String),
    /// Retrying won't help (404, 403...).
    Fatal(String),
}

async fn try_download(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), Attempt> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| Attempt::Retry(format!("Download fallito ({}): {}", url, e)))?;

    let status = response.status();
    if !status.is_success() {
        let msg = format!("HTTP {} per {}", status, url);
        let retryable = status.is_server_error()
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::REQUEST_TIMEOUT;
        return Err(if retryable { Attempt::Retry(msg) } else { Attempt::Fatal(msg) });
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| Attempt::Retry(format!("Download interrotto ({}): {}", url, e)))?;

    if let Some(expected) = expected_sha1 {
        if !sha1_hex(&bytes).eq_ignore_ascii_case(expected) {
            return Err(Attempt::Retry(format!("SHA-1 mismatch: file danneggiato ({})", url)));
        }
    }

    // Write to a temporary file first, then rename: a crash or a full disk can
    // never leave a half-written file that looks complete.
    write_atomic(path, &bytes).map_err(Attempt::Fatal)
}

/// Write `data` to `path` atomically (temp file + rename).
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let tmp = path.with_file_name(format!("{}.part{}", file_name, n));

    std::fs::write(&tmp, data).map_err(|e| format!("Impossibile scrivere {:?}: {}", tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Impossibile salvare {:?}: {}", path, e)
    })
}

/// A single file to download as part of a batch.
pub struct DownloadItem {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub name: String,
}

/// Download many files concurrently (up to `concurrency` at once), reusing the
/// shared client and emitting a progress event as each file completes.
pub async fn download_many(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    items: Vec<DownloadItem>,
    stage: &str,
    concurrency: usize,
) -> Result<(), String> {
    let total = items.len() as u64;
    if total == 0 {
        return Ok(());
    }
    let done = Arc::new(AtomicU64::new(0));

    let results: Vec<Result<(), String>> = stream::iter(items)
        .map(|item| {
            let client = client.clone();
            let app = app.clone();
            let done = done.clone();
            let stage = stage.to_string();
            async move {
                download_file_client(&client, &item.url, &item.path, item.sha1.as_deref()).await?;

                let c = done.fetch_add(1, Ordering::Relaxed) + 1;
                let percentage = (c as f32 / total as f32) * 100.0;
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        file_name: item.name,
                        downloaded: c,
                        total,
                        percentage,
                        stage,
                    },
                );
                Ok::<(), String>(())
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    for r in results {
        r?;
    }
    Ok(())
}

/// Hex SHA-1 of a byte slice.
pub fn sha1_hex(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Verify the SHA-1 hash of a file.
pub fn verify_sha1(path: &Path, expected: &str) -> Result<bool, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Impossibile leggere {:?} per il controllo: {}", path, e))?;
    Ok(sha1_hex(&data).eq_ignore_ascii_case(expected))
}
