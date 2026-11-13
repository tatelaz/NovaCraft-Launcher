use std::path::PathBuf;
use tauri::Emitter;

use crate::minecraft::{
    assets, downloader, fabric, java, launch, libraries, manifest, modrinth, natives, skinpack,
};
use crate::utils;

/// Fetch all available Minecraft versions (works offline too, see manifest.rs).
#[tauri::command]
pub async fn get_versions() -> Result<Vec<manifest::VersionEntry>, String> {
    manifest::fetch_version_manifest().await
}

/// Download everything needed to play a (vanilla) Minecraft version, Java included.
/// Files already present and intact are skipped, so this also works offline
/// for an installed version, and repairs damaged installs.
#[tauri::command]
pub async fn download_version(app: tauri::AppHandle, version_id: String) -> Result<(), String> {
    let base_dir = utils::get_novacraft_dir();
    let client = downloader::http_client();
    const CONCURRENCY: usize = 16;

    // 1. Version JSON (from Mojang, or the local copy when offline)
    let version_dir = base_dir.join("versions").join(&version_id);
    std::fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Impossibile creare la cartella della versione: {}", e))?;
    let version_json_path = version_dir.join(format!("{}.json", version_id));

    let versions = manifest::fetch_version_manifest().await?;
    match versions.iter().find(|v| v.id == version_id && !v.url.is_empty()) {
        Some(entry) => {
            if let Err(e) =
                downloader::download_file(&entry.url, &version_json_path, entry.sha1.as_deref()).await
            {
                if !version_json_path.exists() {
                    return Err(e);
                }
                eprintln!("[NovaCraft] Uso il file della versione già scaricato: {}", e);
            }
        }
        None if version_json_path.exists() => {}
        None => return Err(format!("La versione {} non esiste nella lista di Mojang", version_id)),
    }

    let version_json = launch::VersionJson::load(&base_dir, &version_id)?;

    // 2. client.jar
    if let Some(downloads) = &version_json.downloads {
        let _ = app.emit(
            "download-progress",
            downloader::DownloadProgress {
                file_name: format!("{}.jar", version_id),
                downloaded: 0,
                total: 1,
                percentage: 0.0,
                stage: "Client".to_string(),
            },
        );
        let client_jar_path = version_dir.join(format!("{}.jar", version_id));
        downloader::download_file_client(
            &client,
            &downloads.client.url,
            &client_jar_path,
            Some(&downloads.client.sha1),
        )
        .await?;
    }

    // 3. Libraries (in parallel)
    let lib_items: Vec<downloader::DownloadItem> =
        libraries::get_required_libraries(&version_json.libraries, &base_dir)
            .into_iter()
            .map(|(artifact, local_path)| downloader::DownloadItem {
                url: artifact.url,
                path: local_path,
                sha1: Some(artifact.sha1),
                name: artifact.path,
            })
            .collect();
    downloader::download_many(&app, &client, lib_items, "Libraries", CONCURRENCY).await?;

    // 4. Native libraries (in parallel) + extraction
    let native_libs = libraries::get_native_libraries(&version_json.libraries, &base_dir);
    let native_paths: Vec<PathBuf> = native_libs.iter().map(|(_, p)| p.clone()).collect();
    let native_items: Vec<downloader::DownloadItem> = native_libs
        .into_iter()
        .map(|(artifact, local_path)| downloader::DownloadItem {
            url: artifact.url,
            path: local_path,
            sha1: Some(artifact.sha1),
            name: artifact.path,
        })
        .collect();
    downloader::download_many(&app, &client, native_items, "Natives", CONCURRENCY).await?;
    natives::extract_natives(&native_paths, &version_dir.join("natives"))?;

    // 5. Assets (in parallel)
    if let Some(asset_index) = &version_json.asset_index {
        assets::download_assets(
            &app,
            &client,
            &asset_index.url,
            &asset_index.id,
            asset_index.sha1.as_deref(),
            &base_dir,
        )
        .await?;
    }

    // 6. The right Java for this version (downloaded only if missing), so the
    //    progress bar is still visible while it downloads.
    java::ensure_java(&app, java::required_major(&version_json)).await?;

    let _ = app.emit(
        "download-progress",
        downloader::DownloadProgress {
            file_name: "Completato".to_string(),
            downloaded: 1,
            total: 1,
            percentage: 100.0,
            stage: "Pronto".to_string(),
        },
    );

    Ok(())
}

/// Path of the custom skin saved for a player name.
fn skin_path(username: &str) -> PathBuf {
    utils::get_novacraft_dir()
        .join("skins")
        .join(format!("{}.png", utils::safe_file_stem(username)))
}

/// Launch Minecraft with the specified version, username, and RAM allocation.
#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    version_id: String,
    username: String,
    ram_mb: u32,
    custom_dir: Option<String>,
) -> Result<(), String> {
    let base_dir = utils::get_novacraft_dir();
    let version_json = launch::VersionJson::load(&base_dir, &version_id)?;

    // Safety net: the base version of a Fabric profile must be installed.
    if let Some(parent_id) = &version_json.inherits_from {
        let parent_jar = base_dir.join("versions").join(parent_id).join(format!("{}.jar", parent_id));
        if !parent_jar.exists() {
            download_version(app.clone(), parent_id.clone()).await?;
        }
    }

    // The correct Java runtime for this version (already ensured by download_version)
    let java_major = java::required_major(&version_json);
    let java_path = java::ensure_java(&app, java_major).await?;

    let game_dir = utils::game_dir(custom_dir.as_deref());

    // Custom skin: make it visible in game through a generated resource pack
    // (or remove that pack if this player has no custom skin).
    let skin = skin_path(&username);
    let legacy = launch::is_legacy_version(&base_dir, &version_json);
    if let Err(e) = skinpack::apply(&game_dir, skin.exists().then_some(skin.as_path()), legacy) {
        eprintln!("[NovaCraft] Skin personalizzata non applicata: {}", e);
    }

    launch::launch_minecraft(
        &app,
        &java_path,
        &version_json,
        &username,
        &base_dir,
        Some(game_dir.as_path()),
        ram_mb,
    )
}

/// Fetch available Fabric loaders for a version (stable first).
#[tauri::command]
pub async fn fetch_fabric_loaders_cmd(game_version: String) -> Result<Vec<String>, String> {
    fabric::fetch_fabric_loaders(&game_version).await
}

/// Install (or reuse) Fabric for a game version; returns the id to launch.
#[tauri::command]
pub async fn ensure_fabric_cmd(app: tauri::AppHandle, game_version: String) -> Result<String, String> {
    fabric::ensure_fabric(&app, &game_version).await
}

/// Search projects on Modrinth
#[tauri::command]
pub async fn search_modrinth_cmd(
    query: String,
    project_type: String,
    game_version: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<modrinth::ModrinthSearchResult, String> {
    modrinth::search_projects(
        &query,
        &project_type,
        game_version.as_deref(),
        limit.unwrap_or(20),
        offset.unwrap_or(0),
    )
    .await
}

/// Install a mod (with its dependencies), shader (with Iris + Sodium) or resource pack.
#[tauri::command]
pub async fn install_modrinth_project_cmd(
    project_id: String,
    project_type: String,
    game_version: String,
    custom_dir: Option<String>,
) -> Result<modrinth::InstallReport, String> {
    modrinth::install_project(&project_id, &project_type, &game_version, custom_dir.as_deref()).await
}

/// Modrinth project ids already installed in the profile (for the "Installato" badges).
#[tauri::command]
pub async fn installed_project_ids_cmd(project_type: String, custom_dir: Option<String>) -> Vec<String> {
    modrinth::installed_project_ids(&project_type, custom_dir.as_deref()).await
}

/// List installed mods
#[tauri::command]
pub fn list_installed_mods_cmd(custom_dir: Option<String>) -> Vec<modrinth::InstalledModInfo> {
    modrinth::list_mods(custom_dir.as_deref())
}

/// Toggle a mod file
#[tauri::command]
pub fn toggle_mod_cmd(filename: String, custom_dir: Option<String>) -> Result<bool, String> {
    modrinth::toggle_mod(&filename, custom_dir.as_deref())
}

/// Delete a mod file
#[tauri::command]
pub fn delete_mod_cmd(filename: String, custom_dir: Option<String>) -> Result<(), String> {
    modrinth::delete_mod(&filename, custom_dir.as_deref())
}

/// Open a profile folder ("" = the profile itself, or mods, shaderpacks, ...)
#[tauri::command]
pub fn open_subfolder_cmd(folder_name: String, custom_dir: Option<String>) -> Result<(), String> {
    modrinth::open_subfolder(&folder_name, custom_dir.as_deref())
}

/// Save a custom skin (PNG as base64 / data URL) for a player name.
#[tauri::command]
pub fn save_custom_skin_cmd(username: String, base64_png: String) -> Result<(), String> {
    let clean_b64 = match base64_png.find("base64,") {
        Some(idx) => &base64_png[idx + 7..],
        None => &base64_png,
    };

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(clean_b64.trim())
        .map_err(|e| format!("Immagine non valida: {}", e))?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("La skin deve essere un'immagine PNG".to_string());
    }

    let path = skin_path(&username);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Impossibile creare la cartella delle skin: {}", e))?;
    }
    downloader::write_atomic(&path, &bytes)
}

/// The custom skin of a player as a data URL, or None.
#[tauri::command]
pub fn get_custom_skin_cmd(username: String) -> Option<String> {
    use base64::Engine;
    let bytes = std::fs::read(skin_path(&username)).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Remove the custom skin of a player (back to the online/default skin).
#[tauri::command]
pub fn delete_custom_skin_cmd(username: String) -> Result<(), String> {
    let path = skin_path(&username);
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("Impossibile rimuovere la skin: {}", e))?;
    }
    Ok(())
}

/// Java found on the system + runtimes already downloaded by NovaCraft.
#[tauri::command]
pub async fn java_status_cmd() -> java::JavaStatus {
    tauri::async_runtime::spawn_blocking(java::java_status)
        .await
        .unwrap_or(java::JavaStatus {
            system_path: None,
            system_major: None,
            managed: Vec::new(),
        })
}

/// Return the NovaCraft game directory as a string.
#[tauri::command]
pub fn get_game_dir() -> String {
    utils::get_novacraft_dir().to_string_lossy().to_string()
}

/// Folder of a profile: <NovaCraft>/instances/<id> (created if missing).
#[tauri::command]
pub fn instance_dir_cmd(instance_id: String) -> Result<String, String> {
    let dir = utils::get_novacraft_dir()
        .join("instances")
        .join(utils::safe_file_stem(&instance_id));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Impossibile creare la cartella del profilo: {}", e))?;
    Ok(dir.to_string_lossy().to_string())
}

/// Open a NovaCraft folder in the OS file manager.
#[tauri::command]
pub fn open_game_folder(which: Option<String>) -> Result<(), String> {
    let base = utils::get_novacraft_dir();
    let path = match which.as_deref() {
        Some("runtimes") => base.join("runtimes"),
        Some("instances") => base.join("instances"),
        _ => base,
    };
    utils::open_in_file_manager(&path)
}

/// Installed version ids: vanilla versions with their game file, and Fabric
/// profiles (which reuse the game file of their base version).
#[tauri::command]
pub fn list_installed_versions() -> Vec<String> {
    let versions_dir = utils::get_novacraft_dir().join("versions");
    let mut result = Vec::new();
    let Ok(entries) = std::fs::read_dir(&versions_dir) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let has_jar = path.join(format!("{}.jar", name)).exists();
        let is_profile = std::fs::read_to_string(path.join(format!("{}.json", name)))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .is_some_and(|j| j.get("inheritsFrom").is_some());
        if has_jar || is_profile {
            result.push(name);
        }
    }
    result
}

/// Close the launcher.
#[tauri::command]
pub fn close_launcher(app: tauri::AppHandle) {
    app.exit(0);
}
