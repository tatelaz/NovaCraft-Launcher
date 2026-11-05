use futures_util::stream::{self, StreamExt};
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::Emitter;

/// Progress information emitted to the frontend during downloads.
#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub file_name: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f32,
    pub stage: String,
}

/// Download a file from a URL to a local path (creates its own HTTP client).
/// Skips download if the file already exists with the correct SHA-1 hash.
pub async fn download_file(
    url: &str,
    path: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    download_file_client(&client, url, path, expected_sha1).await
}

/// Download a file using a shared HTTP client, so many downloads reuse the
/// same connection pool (much faster for lots of small files).
/// Skips download if the file already exists with the correct SHA-1 hash.
pub async fn download_file_client(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), String> {
    // Check if file already exists with correct hash
    if path.exists() {
        if let Some(expected) = expected_sha1 {
            if verify_sha1(path, expected)? {
                return Ok(());
            }
        } else {
            // No hash to verify, file exists — skip
            return Ok(());
        }
    }

    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
    }

    // Download the file
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} for {}", response.status(), url));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    std::fs::write(path, &bytes)
        .map_err(|e| format!("Failed to write file {:?}: {}", path, e))?;

    // Verify SHA-1 after download
    if let Some(expected) = expected_sha1 {
        if !verify_sha1(path, expected)? {
            let _ = std::fs::remove_file(path);
            return Err(format!("SHA-1 mismatch for {:?}", path));
        }
    }

    Ok(())
}

/// Download a file and emit progress events to the frontend.
/// `current` and `total` represent the overall progress (e.g., file 5 of 100).
pub async fn download_file_with_progress(
    app: &tauri::AppHandle,
    url: &str,
    path: &Path,
    expected_sha1: Option<&str>,
    file_name: &str,
    stage: &str,
    current: u64,
    total: u64,
) -> Result<(), String> {
    // Emit progress before download
    let percentage = if total > 0 {
        (current as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            file_name: file_name.to_string(),
            downloaded: current,
            total,
            percentage,
            stage: stage.to_string(),
        },
    );

    download_file(url, path, expected_sha1).await
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

/// Verify the SHA-1 hash of a file.
fn verify_sha1(path: &Path, expected: &str) -> Result<bool, String> {
    let data =
        std::fs::read(path).map_err(|e| format!("Failed to read {:?} for hash check: {}", path, e))?;
    let mut hasher = Sha1::new();
    hasher.update(&data);
    let hash = format!("{:x}", hasher.finalize());
    Ok(hash == expected)
}
