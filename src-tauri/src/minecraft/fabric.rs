use serde::{Deserialize, Serialize};
use tauri::Emitter;
use crate::minecraft::downloader::DownloadProgress;
use crate::utils;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FabricLoaderInfo {
    pub loader: FabricLoaderDetails,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FabricLoaderDetails {
    pub version: String,
    pub stable: Option<bool>,
}

/// Fetch available Fabric loader versions for a specific Minecraft version.
pub async fn fetch_fabric_loaders(game_version: &str) -> Result<Vec<String>, String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{}", game_version);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "NovaCraft-Launcher")
        .send()
        .await
        .map_err(|e| format!("Impossibile connettersi all'API di Fabric: {}", e))?;

    if !resp.status().is_success() {
        return Ok(Vec::new()); // No fabric available for this version
    }

    let loaders: Vec<FabricLoaderInfo> = resp
        .json()
        .await
        .map_err(|e| format!("Errore nel parsing dei loader Fabric: {}", e))?;

    let versions: Vec<String> = loaders.into_iter().map(|l| l.loader.version).collect();
    Ok(versions)
}

/// Download and configure Fabric for the given game version and loader version.
/// Returns the generated version ID (e.g., "fabric-loader-0.16.10-1.20.1").
pub async fn install_fabric(
    app: &tauri::AppHandle,
    game_version: &str,
    loader_version: &str,
) -> Result<String, String> {
    let base_dir = utils::get_novacraft_dir();
    let client = reqwest::Client::new();

    // 1. Get the Fabric profile JSON
    let profile_url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
        game_version, loader_version
    );

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            file_name: "Profilo Fabric".to_string(),
            downloaded: 0,
            total: 1,
            percentage: 10.0,
            stage: "Fabric".to_string(),
        },
    );

    let resp = client
        .get(&profile_url)
        .header("User-Agent", "NovaCraft-Launcher")
        .send()
        .await
        .map_err(|e| format!("Impossibile scaricare il profilo Fabric: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Fabric profile HTTP {}", resp.status()));
    }

    let profile_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Errore nel parsing del profilo Fabric: {}", e))?;

    let version_id = profile_json["id"]
        .as_str()
        .ok_or("ID profilo Fabric non valido")?
        .to_string();

    // 2. Save profile JSON in versions/fabric-loader-...
    let version_dir = base_dir.join("versions").join(&version_id);
    std::fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Impossibile creare cartella versione Fabric: {}", e))?;

    let version_json_path = version_dir.join(format!("{}.json", version_id));
    std::fs::write(
        &version_json_path,
        serde_json::to_string_pretty(&profile_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Impossibile salvare il JSON di Fabric: {}", e))?;

    // 3. Download Fabric libraries from maven
    let libraries = profile_json["libraries"]
        .as_array()
        .ok_or("Librerie Fabric mancanti")?;

    let libraries_dir = base_dir.join("libraries");
    let total_libs = libraries.len();

    for (idx, lib) in libraries.iter().enumerate() {
        let name = lib["name"].as_str().unwrap_or_default();
        let maven_base = lib["url"].as_str().unwrap_or("https://maven.fabricmc.net/");

        // Convert maven coordinate (e.g. "org.ow2.asm:asm:9.7.1") to relative path
        if let Some(rel_path) = maven_to_path(name) {
            let local_path = libraries_dir.join(&rel_path);
            if !local_path.exists() {
                if let Some(parent) = local_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let download_url = format!("{}{}", maven_base, rel_path.replace('\\', "/"));
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        file_name: name.to_string(),
                        downloaded: (idx + 1) as u64,
                        total: total_libs as u64,
                        percentage: 20.0 + ((idx + 1) as f32 / total_libs as f32) * 80.0,
                        stage: "Librerie Fabric".to_string(),
                    },
                );

                if let Ok(res) = client.get(&download_url).send().await {
                    if res.status().is_success() {
                        if let Ok(bytes) = res.bytes().await {
                            let _ = std::fs::write(&local_path, bytes);
                        }
                    }
                }
            }
        }
    }

    Ok(version_id)
}

/// Convert maven coordinate "group:name:version" into "group/path/name/version/name-version.jar"
fn maven_to_path(coord: &str) -> Option<String> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let name = parts[1];
    let version = parts[2];

    let classifier = if parts.len() >= 4 {
        format!("-{}", parts[3])
    } else {
        "".to_string()
    };

    Some(format!(
        "{}/{}/{}/{}-{}{}.jar",
        group, name, version, name, version, classifier
    ))
}
