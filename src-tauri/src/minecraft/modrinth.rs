use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::utils;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModrinthSearchHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub categories: Vec<String>,
    pub client_side: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ModrinthSearchResult {
    pub hits: Vec<ModrinthSearchHit>,
    pub total_hits: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModrinthVersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModrinthVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModrinthVersionFile>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InstalledModInfo {
    pub filename: String,
    pub is_enabled: bool,
    pub size_bytes: u64,
}

/// Search projects on Modrinth (mods, shaders, resourcepacks)
pub async fn search_projects(
    query: &str,
    project_type: Option<&str>,
    loader: Option<&str>,
    game_version: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<ModrinthSearchResult, String> {
    let client = reqwest::Client::new();
    let mut facets: Vec<Vec<String>> = Vec::new();

    if let Some(pt) = project_type {
        facets.push(vec![format!("project_type:{}", pt)]);
    }
    if let Some(l) = loader {
        facets.push(vec![format!("categories:{}", l)]);
    }
    if let Some(gv) = game_version {
        facets.push(vec![format!("versions:{}", gv)]);
    }

    let facets_json = serde_json::to_string(&facets).unwrap_or_else(|_| "[]".to_string());
    let url = format!(
        "https://api.modrinth.com/v2/search?query={}&facets={}&limit={}&offset={}",
        urlencoding::encode(query),
        urlencoding::encode(&facets_json),
        limit,
        offset
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "NovaCraft-Launcher")
        .send()
        .await
        .map_err(|e| format!("Errore di rete con Modrinth: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Modrinth search HTTP {}", resp.status()));
    }

    let result = resp
        .json::<ModrinthSearchResult>()
        .await
        .map_err(|e| format!("Errore nel parsing della risposta Modrinth: {}", e))?;

    Ok(result)
}

/// Get versions for a specific Modrinth project
pub async fn get_project_versions(
    project_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<Vec<ModrinthVersion>, String> {
    let client = reqwest::Client::new();
    let mut query_params: Vec<String> = Vec::new();

    if let Some(l) = loader {
        let json_arr = format!("[\"{}\"]", l);
        query_params.push(format!("loaders={}", urlencoding::encode(&json_arr)));
    }
    if let Some(gv) = game_version {
        let json_arr = format!("[\"{}\"]", gv);
        query_params.push(format!("game_versions={}", urlencoding::encode(&json_arr)));
    }

    let query_str = if query_params.is_empty() {
        "".to_string()
    } else {
        format!("?{}", query_params.join("&"))
    };

    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version{}",
        project_id, query_str
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "NovaCraft-Launcher")
        .send()
        .await
        .map_err(|e| format!("Errore richiesta versioni Modrinth: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Modrinth version HTTP {}", resp.status()));
    }

    let mut versions = resp
        .json::<Vec<ModrinthVersion>>()
        .await
        .map_err(|e| format!("Errore parsing versioni: {}", e))?;

    // Post-filter to strictly match the requested loader
    if let Some(req_loader) = loader {
        versions.retain(|v| {
            let loader_matches = v.loaders.iter().any(|l| l.eq_ignore_ascii_case(req_loader));
            if !loader_matches {
                return false;
            }
            if req_loader == "fabric" {
                if let Some(primary) = v.files.iter().find(|f| f.primary).or_else(|| v.files.first()) {
                    let fn_lower = primary.filename.to_lowercase();
                    if fn_lower.contains("neoforge") || (fn_lower.contains("forge") && !fn_lower.contains("fabric")) {
                        return false;
                    }
                }
            }
            true
        });
    }

    Ok(versions)
}

/// Download a file from Modrinth into the target folder (mods, shaderpacks, resourcepacks)
pub async fn download_mod_file(
    download_url: &str,
    filename: &str,
    folder_type: &str,
    custom_dir: Option<&str>,
) -> Result<String, String> {
    let base = match custom_dir {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => utils::get_novacraft_dir(),
    };

    let target_dir = match folder_type {
        "shaderpacks" => base.join("shaderpacks"),
        "resourcepacks" => base.join("resourcepacks"),
        _ => base.join("mods"),
    };

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Impossibile creare cartella {}: {}", folder_type, e))?;

    let file_path = target_dir.join(filename);

    let client = reqwest::Client::new();
    let resp = client
        .get(download_url)
        .header("User-Agent", "NovaCraft-Launcher")
        .send()
        .await
        .map_err(|e| format!("Download fallito: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download HTTP {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Errore nel download del file: {}", e))?;

    std::fs::write(&file_path, bytes)
        .map_err(|e| format!("Impossibile salvare il file {}: {}", filename, e))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// List installed mods in the target game directory
pub fn list_mods(custom_dir: Option<&str>) -> Vec<InstalledModInfo> {
    let base = match custom_dir {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => utils::get_novacraft_dir(),
    };
    let mods_dir = base.join("mods");
    let mut list = Vec::new();

    if let Ok(entries) = std::fs::read_dir(mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
                        let is_enabled = name.ends_with(".jar");
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        list.push(InstalledModInfo {
                            filename: name.to_string(),
                            is_enabled,
                            size_bytes,
                        });
                    }
                }
            }
        }
    }

    list.sort_by(|a, b| a.filename.cmp(&b.filename));
    list
}

/// Toggle a mod file between .jar (enabled) and .jar.disabled (disabled)
pub fn toggle_mod(filename: &str, custom_dir: Option<&str>) -> Result<bool, String> {
    let base = match custom_dir {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => utils::get_novacraft_dir(),
    };
    let mods_dir = base.join("mods");
    let current_path = mods_dir.join(filename);

    if !current_path.exists() {
        return Err("File mod non trovato".to_string());
    }

    let (new_path, is_enabled) = if filename.ends_with(".jar") {
        (mods_dir.join(format!("{}.disabled", filename)), false)
    } else if filename.ends_with(".jar.disabled") {
        let stripped = filename.trim_end_matches(".disabled");
        (mods_dir.join(stripped), true)
    } else {
        return Err("Estensione file non valida per una mod".to_string());
    };

    std::fs::rename(current_path, new_path)
        .map_err(|e| format!("Impossibile rinominare il file mod: {}", e))?;

    Ok(is_enabled)
}

/// Delete a mod file
pub fn delete_mod(filename: &str, custom_dir: Option<&str>) -> Result<(), String> {
    let base = match custom_dir {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => utils::get_novacraft_dir(),
    };
    let path = base.join("mods").join(filename);
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("Impossibile eliminare mod: {}", e))?;
    }
    Ok(())
}

/// Open specific folder (mods, shaderpacks, resourcepacks, saves, etc.)
pub fn open_subfolder(folder_name: &str, custom_dir: Option<&str>) -> Result<(), String> {
    let base = match custom_dir {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => utils::get_novacraft_dir(),
    };
    let target = base.join(folder_name);
    let _ = std::fs::create_dir_all(&target);

    #[cfg(windows)]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";

    std::process::Command::new(program)
        .arg(&target)
        .spawn()
        .map_err(|e| format!("Impossibile aprire cartella {}: {}", folder_name, e))?;

    Ok(())
}
