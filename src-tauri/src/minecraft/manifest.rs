use serde::{Deserialize, Serialize};

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
    #[allow(dead_code)]
    latest: LatestVersions,
    versions: Vec<VersionEntry>,
}

/// Latest release and snapshot version IDs.
#[derive(Deserialize)]
struct LatestVersions {
    #[allow(dead_code)]
    release: String,
    #[allow(dead_code)]
    snapshot: String,
}

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Fetch the version manifest from Mojang and return all version entries.
pub async fn fetch_version_manifest() -> Result<Vec<VersionEntry>, String> {
    let response = reqwest::get(VERSION_MANIFEST_URL)
        .await
        .map_err(|e| format!("Failed to fetch version manifest: {}", e))?;

    let manifest: VersionManifest = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse version manifest: {}", e))?;

    Ok(manifest.versions)
}
