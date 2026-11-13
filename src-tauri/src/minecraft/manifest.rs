use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::downloader::http_client;
use crate::utils;

/// A single version entry from Mojang's version manifest.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionEntry {
    pub id: String,
    // Mojang's JSON calls this field "type" (a reserved word in Rust), so we
    // only rename on *deserialize*. On serialize to the frontend we keep
    // "version_type", which is what app.js expects.
    #[serde(rename(deserialize = "type"))]
    pub version_type: String,
    pub url: String,
    pub sha1: Option<String>,
}

/// The top-level structure of Mojang's version_manifest_v2.json.
#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Local copy of the last manifest we managed to download.
fn cache_path() -> PathBuf {
    utils::get_novacraft_dir()
        .join("versions")
        .join("version_manifest_v2.json")
}

/// Fetch the version list from Mojang.
///
/// Without internet it falls back to the last saved copy, and if there is none,
/// to the versions already installed on disk — so you can always play offline.
pub async fn fetch_version_manifest() -> Result<Vec<VersionEntry>, String> {
    match fetch_online().await {
        Ok(text) => {
            let manifest: VersionManifest = serde_json::from_str(&text)
                .map_err(|e| format!("Lista versioni di Mojang non valida: {}", e))?;
            let path = cache_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, &text);
            Ok(manifest.versions)
        }
        Err(online_err) => {
            eprintln!("[NovaCraft] Manifest offline: {}", online_err);
            if let Ok(text) = std::fs::read_to_string(cache_path()) {
                if let Ok(manifest) = serde_json::from_str::<VersionManifest>(&text) {
                    return Ok(manifest.versions);
                }
            }
            let local = installed_vanilla_entries();
            if local.is_empty() {
                Err(online_err)
            } else {
                Ok(local)
            }
        }
    }
}

async fn fetch_online() -> Result<String, String> {
    let response = http_client()
        .get(VERSION_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("Impossibile scaricare la lista versioni: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("Lista versioni: HTTP {}", response.status()));
    }
    response
        .text()
        .await
        .map_err(|e| format!("Impossibile leggere la lista versioni: {}", e))
}

/// Build version entries from what is already installed (used when fully offline).
fn installed_vanilla_entries() -> Vec<VersionEntry> {
    #[derive(Deserialize)]
    struct LocalJson {
        id: String,
        #[serde(rename = "type")]
        version_type: Option<String>,
        #[serde(rename = "inheritsFrom")]
        inherits_from: Option<String>,
        #[serde(rename = "releaseTime")]
        release_time: Option<String>,
    }

    let mut found: Vec<(String, VersionEntry)> = Vec::new();
    let versions_dir = utils::get_novacraft_dir().join("versions");
    if let Ok(entries) = std::fs::read_dir(&versions_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let dir = entry.path();
            if !dir.join(format!("{}.jar", name)).exists() {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(dir.join(format!("{}.json", name))) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<LocalJson>(&text) else {
                continue;
            };
            if json.inherits_from.is_some() {
                continue; // Fabric profiles are not "versions" in the list
            }
            found.push((
                json.release_time.unwrap_or_default(),
                VersionEntry {
                    id: json.id,
                    version_type: json.version_type.unwrap_or_else(|| "release".to_string()),
                    url: String::new(),
                    sha1: None,
                },
            ));
        }
    }
    // Newest first, like Mojang's manifest.
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, v)| v).collect()
}
