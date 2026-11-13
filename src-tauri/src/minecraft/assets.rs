use futures_util::stream::{self, StreamExt};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::Emitter;

use super::downloader::{download_file_client, DownloadProgress};

/// The asset index JSON structure — maps asset names to hash/size.
#[derive(Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    /// 1.6 – 1.7.2: the game reads assets by name from assets/virtual/<id>/.
    #[serde(rename = "virtual", default)]
    pub is_virtual: bool,
    /// Before 1.6: the game reads assets from <game dir>/resources/.
    #[serde(default)]
    pub map_to_resources: bool,
}

/// A single asset object (texture, sound, etc).
#[derive(Deserialize)]
pub struct AssetObject {
    pub hash: String,
}

const ASSET_BASE_URL: &str = "https://resources.download.minecraft.net";
const CONCURRENCY: usize = 16;

/// Download all assets for a Minecraft version.
///
/// 1. Downloads the asset index JSON (verified with its SHA-1)
/// 2. Downloads all objects concurrently using hash-based storage
/// 3. Skips files that already exist
pub async fn download_assets(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    asset_index_url: &str,
    asset_index_id: &str,
    asset_index_sha1: Option<&str>,
    base_dir: &Path,
) -> Result<(), String> {
    let assets_dir = base_dir.join("assets");
    let indexes_dir = assets_dir.join("indexes");
    let objects_dir = assets_dir.join("objects");

    std::fs::create_dir_all(&indexes_dir)
        .map_err(|e| format!("Impossibile creare la cartella degli indici: {}", e))?;
    std::fs::create_dir_all(&objects_dir)
        .map_err(|e| format!("Impossibile creare la cartella delle risorse: {}", e))?;

    // Download asset index JSON (re-downloaded if Mojang updated it)
    let index_path = indexes_dir.join(format!("{}.json", asset_index_id));
    if let Err(e) = download_file_client(client, asset_index_url, &index_path, asset_index_sha1).await {
        // Offline but we already have an index: go on with it.
        if !index_path.exists() {
            return Err(e);
        }
        eprintln!("[NovaCraft] Indice risorse non aggiornato (uso quello locale): {}", e);
    }

    let index_data = std::fs::read_to_string(&index_path)
        .map_err(|e| format!("Impossibile leggere l'indice delle risorse: {}", e))?;
    let index: AssetIndex = serde_json::from_str(&index_data)
        .map_err(|e| format!("Indice delle risorse non valido: {}", e))?;

    // Many assets share identical content (= same hash): download each only once.
    let mut seen = HashSet::new();
    let hashes: Vec<String> = index
        .objects
        .into_values()
        .map(|o| o.hash)
        .filter(|h| h.len() > 2 && seen.insert(h.clone()))
        .collect();

    let total = hashes.len() as u64;
    let done = Arc::new(AtomicU64::new(0));

    let results: Vec<Result<(), String>> = stream::iter(hashes)
        .map(|hash| {
            let client = client.clone();
            let app = app.clone();
            let done = done.clone();
            let objects_dir = objects_dir.clone();
            async move {
                let hash_prefix = &hash[..2];
                let object_path = objects_dir.join(hash_prefix).join(&hash);

                // Skip if already downloaded (assets are content-addressed by hash).
                if !object_path.exists() {
                    let url = format!("{}/{}/{}", ASSET_BASE_URL, hash_prefix, hash);
                    download_file_client(&client, &url, &object_path, Some(&hash)).await?;
                }

                let c = done.fetch_add(1, Ordering::Relaxed) + 1;
                let percentage = (c as f32 / total as f32) * 100.0;
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        file_name: hash[..8].to_string(),
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

/// Very old versions (up to 1.7.2) read sounds and textures by *name* instead
/// of by hash. Copy them where those versions expect them and return the folder
/// to pass as `${game_assets}`. Returns `None` for modern versions.
pub fn prepare_legacy_assets(base_dir: &Path, index_id: &str, game_dir: &Path) -> Option<PathBuf> {
    let assets_dir = base_dir.join("assets");
    let index_path = assets_dir.join("indexes").join(format!("{}.json", index_id));
    let text = std::fs::read_to_string(&index_path).ok()?;
    let index: AssetIndex = serde_json::from_str(&text).ok()?;
    if !index.is_virtual && !index.map_to_resources {
        return None;
    }

    let target = if index.map_to_resources {
        game_dir.join("resources")
    } else {
        assets_dir.join("virtual").join(index_id)
    };

    for (name, obj) in &index.objects {
        // Names come from Mojang, but never allow escaping the target folder.
        if name.split(['/', '\\']).any(|part| part == "..") || obj.hash.len() < 2 {
            continue;
        }
        let src = assets_dir.join("objects").join(&obj.hash[..2]).join(&obj.hash);
        let dst = target.join(name);
        if dst.exists() || !src.exists() {
            continue;
        }
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::copy(&src, &dst);
    }

    Some(target)
}
