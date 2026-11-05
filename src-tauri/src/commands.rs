use tauri::Emitter;
use std::path::PathBuf;
use crate::minecraft::{assets, downloader, fabric, java, launch, libraries, manifest, modrinth, natives};
use crate::utils;

/// Fetch all available Minecraft versions from Mojang's manifest.
#[tauri::command]
pub async fn get_versions() -> Result<Vec<manifest::VersionEntry>, String> {
    manifest::fetch_version_manifest().await
}

/// Download all files needed for a specific Minecraft version.
#[tauri::command]
pub async fn download_version(app: tauri::AppHandle, version_id: String) -> Result<(), String> {
    let base_dir = utils::get_novacraft_dir();
    let client = reqwest::Client::new();
    const CONCURRENCY: usize = 16;

    // 1. Get the version URL from manifest
    let versions = manifest::fetch_version_manifest().await?;
    let version_entry = versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| format!("Version {} not found in manifest", version_id))?;

    // 2. Download and parse version JSON
    let version_dir = base_dir.join("versions").join(&version_id);
    std::fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Failed to create version dir: {}", e))?;

    let version_json_path = version_dir.join(format!("{}.json", version_id));
    downloader::download_file(&version_entry.url, &version_json_path, version_entry.sha1.as_deref())
        .await?;

    let version_data = std::fs::read_to_string(&version_json_path)
        .map_err(|e| format!("Failed to read version JSON: {}", e))?;
    let version_json: launch::VersionJson =
        serde_json::from_str(&version_data).map_err(|e| format!("Failed to parse version JSON: {}", e))?;

    // 3. Download client.jar
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
        downloader::download_file(
            &downloads.client.url,
            &client_jar_path,
            Some(&downloads.client.sha1),
        )
        .await?;
    }

    // 4. Download libraries (in parallel)
    let required_libs = libraries::get_required_libraries(&version_json.libraries, &base_dir);
    let lib_items: Vec<downloader::DownloadItem> = required_libs
        .into_iter()
        .map(|(artifact, local_path)| downloader::DownloadItem {
            url: artifact.url,
            path: local_path,
            sha1: Some(artifact.sha1),
            name: artifact.path,
        })
        .collect();
    downloader::download_many(&app, &client, lib_items, "Libraries", CONCURRENCY).await?;

    // 5. Download native libraries (in parallel)
    let native_libs = libraries::get_native_libraries(&version_json.libraries, &base_dir);
    let native_paths: Vec<_> = native_libs.iter().map(|(_, p)| p.clone()).collect();
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

    // 6. Extract natives
    let natives_dir = version_dir.join("natives");
    natives::extract_natives(&native_paths, &natives_dir)?;

    // 7. Download assets (in parallel)
    if let Some(asset_index) = &version_json.asset_index {
        assets::download_assets(
            &app,
            &client,
            &asset_index.url,
            &asset_index.id,
            &base_dir,
        )
        .await?;
    }

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

    // Read the cached version JSON
    let version_json_path = base_dir
        .join("versions")
        .join(&version_id)
        .join(format!("{}.json", version_id));

    let version_data = std::fs::read_to_string(&version_json_path)
        .map_err(|e| format!("Failed to read version JSON: {}", e))?;
    let version_json: launch::VersionJson =
        serde_json::from_str(&version_data).map_err(|e| format!("Failed to parse version JSON: {}", e))?;

    // Ensure base version is downloaded if this is an inherited version (e.g. Fabric)
    if let Some(parent_id) = &version_json.inherits_from {
        let parent_jar = base_dir.join("versions").join(parent_id).join(format!("{}.jar", parent_id));
        if !parent_jar.exists() {
            download_version(app.clone(), parent_id.clone()).await?;
        }
    }

    // Ensure the correct Java runtime for this version (downloads it if missing)
    let java_major = java::required_major(&version_json);
    let java_path = java::ensure_java(&app, java_major).await?;

    let custom_path = custom_dir.filter(|s| !s.is_empty()).map(PathBuf::from);

    // Launch the game
    launch::launch_minecraft(
        &app,
        &java_path,
        &version_json,
        &username,
        &base_dir,
        custom_path.as_deref(),
        ram_mb,
    )
}

/// Fetch available Fabric loaders for a version
#[tauri::command]
pub async fn fetch_fabric_loaders_cmd(game_version: String) -> Result<Vec<String>, String> {
    fabric::fetch_fabric_loaders(&game_version).await
}

/// Install Fabric loader for a version
#[tauri::command]
pub async fn install_fabric_cmd(
    app: tauri::AppHandle,
    game_version: String,
    loader_version: String,
) -> Result<String, String> {
    fabric::install_fabric(&app, &game_version, &loader_version).await
}

/// Search projects on Modrinth
#[tauri::command]
pub async fn search_modrinth_cmd(
    query: String,
    project_type: Option<String>,
    loader: Option<String>,
    game_version: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<modrinth::ModrinthSearchResult, String> {
    modrinth::search_projects(
        &query,
        project_type.as_deref(),
        loader.as_deref(),
        game_version.as_deref(),
        limit.unwrap_or(20),
        offset.unwrap_or(0),
    )
    .await
}

/// Get versions of a Modrinth project
#[tauri::command]
pub async fn get_modrinth_versions_cmd(
    project_id: String,
    loader: Option<String>,
    game_version: Option<String>,
) -> Result<Vec<modrinth::ModrinthVersion>, String> {
    modrinth::get_project_versions(&project_id, loader.as_deref(), game_version.as_deref()).await
}

/// Download a mod/shader/resourcepack file
#[tauri::command]
pub async fn download_mod_file_cmd(
    download_url: String,
    filename: String,
    folder_type: String,
    custom_dir: Option<String>,
) -> Result<String, String> {
    modrinth::download_mod_file(&download_url, &filename, &folder_type, custom_dir.as_deref()).await
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

/// Open specific subfolder (mods, shaderpacks, resourcepacks, etc.)
#[tauri::command]
pub fn open_subfolder_cmd(folder_name: String, custom_dir: Option<String>) -> Result<(), String> {
    modrinth::open_subfolder(&folder_name, custom_dir.as_deref())
}

/// Save custom skin PNG (base64) locally
#[tauri::command]
pub fn save_custom_skin_cmd(username: String, base64_png: String) -> Result<String, String> {
    let skins_dir = utils::get_novacraft_dir().join("skins");
    std::fs::create_dir_all(&skins_dir)
        .map_err(|e| format!("Impossibile creare cartella skin: {}", e))?;

    let clean_b64 = if let Some(idx) = base64_png.find("base64,") {
        &base64_png[idx + 7..]
    } else {
        &base64_png
    };

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(clean_b64.trim())
        .map_err(|e| format!("Base64 non valido: {}", e))?;

    let skin_path = skins_dir.join(format!("{}.png", username));
    std::fs::write(&skin_path, bytes)
        .map_err(|e| format!("Impossibile salvare la skin: {}", e))?;

    Ok(skin_path.to_string_lossy().to_string())
}

/// Check if custom skin exists for user and return path
#[tauri::command]
pub fn get_custom_skin_cmd(username: String) -> Option<String> {
    let skin_path = utils::get_novacraft_dir().join("skins").join(format!("{}.png", username));
    if skin_path.exists() {
        Some(skin_path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Get the skin avatar URL for a username from mc-heads.net.
#[tauri::command]
pub fn get_skin_url(username: String) -> String {
    format!("https://mc-heads.net/avatar/{}/100", username)
}

/// Detect if Java is installed on the system.
#[tauri::command]
pub fn detect_java_cmd() -> Option<String> {
    utils::detect_java()
}

/// Return the NovaCraft game directory as a string.
#[tauri::command]
pub fn get_game_dir() -> String {
    utils::get_novacraft_dir().to_string_lossy().to_string()
}

/// Open a folder in the OS file manager.
#[tauri::command]
pub fn open_game_folder(which: Option<String>) -> Result<(), String> {
    let base = utils::get_novacraft_dir();
    let path = match which.as_deref() {
        Some("runtimes") => base.join("runtimes"),
        Some("instances") => base.join("instances"),
        _ => base,
    };
    let _ = std::fs::create_dir_all(&path);

    #[cfg(windows)]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";

    std::process::Command::new(program)
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Impossibile aprire la cartella: {}", e))?;
    Ok(())
}

/// List installed version ids (those with a downloaded client JAR or valid json).
#[tauri::command]
pub fn list_installed_versions() -> Vec<String> {
    let versions_dir = utils::get_novacraft_dir().join("versions");
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&versions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) {
                    // Either has jar or is a fabric-loader / custom profile with json
                    if path.join(format!("{}.jar", name)).exists()
                        || path.join(format!("{}.json", name)).exists()
                    {
                        result.push(name);
                    }
                }
            }
        }
    }
    result
}

/// Close the launcher.
#[tauri::command]
pub fn close_launcher(app: tauri::AppHandle) {
    app.exit(0);
}
