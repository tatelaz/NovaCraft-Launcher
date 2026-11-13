use serde::Deserialize;
use tauri::Emitter;

use super::downloader::{self, http_client, DownloadItem, DownloadProgress};
use super::libraries::maven_to_path;
use crate::utils;

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const FABRIC_MAVEN: &str = "https://maven.fabricmc.net/";

#[derive(Debug, Deserialize, Clone)]
struct FabricLoaderInfo {
    loader: FabricLoaderDetails,
}

#[derive(Debug, Deserialize, Clone)]
struct FabricLoaderDetails {
    version: String,
    stable: Option<bool>,
}

/// Fetch available Fabric loader versions for a Minecraft version,
/// stable ones first (newest first within each group).
pub async fn fetch_fabric_loaders(game_version: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/versions/loader/{}", FABRIC_META, game_version);
    let resp = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Impossibile connettersi a Fabric: {}", e))?;

    if !resp.status().is_success() {
        return Ok(Vec::new()); // No Fabric for this version
    }

    let loaders: Vec<FabricLoaderInfo> = resp
        .json()
        .await
        .map_err(|e| format!("Risposta di Fabric non valida: {}", e))?;

    let (stable, unstable): (Vec<_>, Vec<_>) =
        loaders.into_iter().partition(|l| l.loader.stable.unwrap_or(true));
    Ok(stable
        .into_iter()
        .chain(unstable)
        .map(|l| l.loader.version)
        .collect())
}

/// Make sure Fabric is installed for `game_version` and return the version id
/// to launch (e.g. "fabric-loader-0.16.10-1.20.1").
///
/// Online: installs/updates the newest stable loader. Offline: falls back to a
/// Fabric already installed for that version, so you can still play.
pub async fn ensure_fabric(app: &tauri::AppHandle, game_version: &str) -> Result<String, String> {
    match fetch_fabric_loaders(game_version).await {
        Ok(loaders) => match loaders.first() {
            Some(loader) => install_fabric(app, game_version, loader).await,
            None => Err(format!(
                "Fabric non è ancora disponibile per Minecraft {}. Scegli un'altra versione o usa Vanilla.",
                game_version
            )),
        },
        Err(network_err) => installed_fabric_for(game_version).ok_or(network_err),
    }
}

/// Download and configure Fabric for the given game version and loader version.
/// Returns the generated version ID (e.g., "fabric-loader-0.16.10-1.20.1").
pub async fn install_fabric(
    app: &tauri::AppHandle,
    game_version: &str,
    loader_version: &str,
) -> Result<String, String> {
    let base_dir = utils::get_novacraft_dir();
    let client = http_client();
    let expected_id = format!("fabric-loader-{}-{}", loader_version, game_version);

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            file_name: "Profilo Fabric".to_string(),
            downloaded: 0,
            total: 1,
            percentage: 5.0,
            stage: "Fabric".to_string(),
        },
    );

    // 1. Get the Fabric profile JSON (or reuse the saved one when offline)
    let profile_url = format!(
        "{}/versions/loader/{}/{}/profile/json",
        FABRIC_META, game_version, loader_version
    );
    let local_json_path = base_dir
        .join("versions")
        .join(&expected_id)
        .join(format!("{}.json", expected_id));

    let profile_json: serde_json::Value = match fetch_profile(&client, &profile_url).await {
        Ok(json) => json,
        Err(e) => std::fs::read_to_string(&local_json_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .ok_or(e)?,
    };

    let version_id = profile_json["id"]
        .as_str()
        .ok_or("Profilo Fabric non valido (manca l'id)")?
        .to_string();
    if version_id.contains(['/', '\\']) || version_id.contains("..") {
        return Err(format!("Profilo Fabric non valido: {}", version_id));
    }

    // 2. Download Fabric libraries from maven (in parallel, verified, with retries)
    let libraries = profile_json["libraries"]
        .as_array()
        .ok_or("Profilo Fabric non valido (mancano le librerie)")?;
    let libraries_dir = base_dir.join("libraries");

    let mut items = Vec::new();
    for lib in libraries {
        let name = lib["name"].as_str().unwrap_or_default();
        let rel_path = maven_to_path(name)
            .ok_or_else(|| format!("Libreria Fabric non valida: {}", name))?;
        let mut maven_base = lib["url"].as_str().unwrap_or(FABRIC_MAVEN).to_string();
        if !maven_base.ends_with('/') {
            maven_base.push('/');
        }
        items.push(DownloadItem {
            url: format!("{}{}", maven_base, rel_path),
            path: libraries_dir.join(&rel_path),
            sha1: lib["sha1"].as_str().map(String::from),
            name: name.to_string(),
        });
    }
    // Any failure here is reported: a missing library would only show up later
    // as an incomprehensible crash.
    downloader::download_many(app, &client, items, "Librerie Fabric", 8).await?;

    // 3. Save the profile JSON only now that everything it needs is on disk.
    let version_dir = base_dir.join("versions").join(&version_id);
    std::fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Impossibile creare la cartella di Fabric: {}", e))?;
    let json_text = serde_json::to_string_pretty(&profile_json).map_err(|e| e.to_string())?;
    downloader::write_atomic(&version_dir.join(format!("{}.json", version_id)), json_text.as_bytes())?;

    Ok(version_id)
}

async fn fetch_profile(client: &reqwest::Client, url: &str) -> Result<serde_json::Value, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Impossibile scaricare il profilo Fabric: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Profilo Fabric: HTTP {}", resp.status()));
    }
    resp.json()
        .await
        .map_err(|e| format!("Profilo Fabric non valido: {}", e))
}

/// A Fabric profile already installed for `game_version` whose libraries are
/// all on disk (used when there is no internet).
fn installed_fabric_for(game_version: &str) -> Option<String> {
    let base_dir = utils::get_novacraft_dir();
    let suffix = format!("-{}", game_version);
    let mut candidates: Vec<String> = std::fs::read_dir(base_dir.join("versions"))
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|id| id.starts_with("fabric-loader-") && id.ends_with(&suffix))
        .collect();
    candidates.sort();
    candidates.into_iter().rev().find(|id| {
        let json_path = base_dir.join("versions").join(id).join(format!("{}.json", id));
        let Ok(text) = std::fs::read_to_string(json_path) else { return false };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { return false };
        json["libraries"].as_array().is_some_and(|libs| {
            libs.iter().all(|l| {
                maven_to_path(l["name"].as_str().unwrap_or_default())
                    .is_some_and(|p| base_dir.join("libraries").join(p).exists())
            })
        })
    })
}
