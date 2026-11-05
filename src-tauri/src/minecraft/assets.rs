use futures_util::stream::{self, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::Emitter;

use super::downloader::{download_file, download_file_client, DownloadProgress};

/// The asset index JSON structure — maps asset names to hash/size.
#[derive(Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

/// A single asset object (texture, sound, etc).
#[derive(Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

const ASSET_BASE_URL: &str = "https://resources.download.minecraft.net";
const CONCURRENCY: usize = 16;

/// Download all assets for a Minecraft version.
///
/// 1. Downloads the asset index JSON
/// 2. Downloads all objects concurrently using hash-based storage
/// 3. Skips files that already exist
pub async fn download_assets(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    asset_index_url: &str,
    asset_index_id: &str,
    base_dir: &Path,
) -> Result<(), String> {
    let assets_dir = base_dir.join("assets");
    let indexes_dir = assets_dir.join("indexes");
    let objects_dir = assets_dir.join("objects");

    std::fs::create_dir_all(&indexes_dir)
        .map_err(|e| format!("Failed to create indexes dir: {}", e))?;
    std::fs::create_dir_all(&objects_dir)
        .map_err(|e| format!("Failed to create objects dir: {}", e))?;

    // Download asset index JSON
    let index_path = indexes_dir.join(format!("{}.json", asset_index_id));
    download_file(asset_index_url, &index_path, None).await?;

    // Parse asset index
    let index_data = std::fs::read_to_string(&index_path)
        .map_err(|e| format!("Failed to read asset index: {}", e))?;
    let index: AssetIndex =
        serde_json::from_str(&index_data).map_err(|e| format!("Failed to parse asset index: {}", e))?;

    let total = index.objects.len() as u64;
    let done = Arc::new(AtomicU64::new(0));

    // Download all objects concurrently.
    let results: Vec<Result<(), String>> = stream::iter(index.objects.into_values())
        .map(|obj| {
            let client = client.clone();
            let app = app.clone();
            let done = done.clone();
            let objects_dir = objects_dir.clone();
            async move {
                let hash_prefix = obj.hash[..2].to_string();
                let object_dir = objects_dir.join(&hash_prefix);
                let object_path = object_dir.join(&obj.hash);

                // Skip if already downloaded (assets are content-addressed by hash).
                if !object_path.exists() {
                    std::fs::create_dir_all(&object_dir)
                        .map_err(|e| format!("Failed to create asset dir: {}", e))?;
                    let url = format!("{}/{}/{}", ASSET_BASE_URL, hash_prefix, obj.hash);
                    download_file_client(&client, &url, &object_path, Some(&obj.hash)).await?;
                }

                let c = done.fetch_add(1, Ordering::Relaxed) + 1;
                let percentage = (c as f32 / total as f32) * 100.0;
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        file_name: obj.hash[..8].to_string(),
                        downloaded: c,
                        total,
                        percentage,
                        stage: "Assets".to_string(),
                    },
                );
                Ok::<(), String>(())
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    for r in results {
        r?;
    }

    Ok(())
}
