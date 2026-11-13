use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::downloader::{self, http_client};
use crate::utils;

const API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModrinthSearchHit {
    pub project_id: String,
    pub project_type: Option<String>,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub categories: Vec<String>,
    pub author: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ModrinthSearchResult {
    pub hits: Vec<ModrinthSearchHit>,
    pub total_hits: u64,
}

#[derive(Debug, Deserialize, Clone)]
struct ModrinthVersion {
    project_id: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    loaders: Vec<String>,
    files: Vec<ModrinthVersionFile>,
    #[serde(default)]
    dependencies: Vec<ModrinthDependency>,
}

#[derive(Debug, Deserialize, Clone)]
struct ModrinthVersionFile {
    url: String,
    filename: String,
    primary: bool,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
struct ModrinthDependency {
    version_id: Option<String>,
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct InstalledModInfo {
    pub filename: String,
    pub is_enabled: bool,
    pub size_bytes: u64,
}

/// What an installation did, shown to the user.
#[derive(Debug, Serialize, Default)]
pub struct InstallReport {
    /// Files downloaded (the project itself + dependencies, Iris, Sodium...).
    pub installed: Vec<String>,
    /// The project was already there.
    pub already_installed: bool,
    /// Non-blocking problems (e.g. a dependency not available for this version).
    pub warnings: Vec<String>,
}

/// Folder (inside the game directory) for each kind of content.
fn folder_for(project_type: &str) -> &'static str {
    match project_type {
        "shader" => "shaderpacks",
        "resourcepack" => "resourcepacks",
        _ => "mods",
    }
}

/// Loaders we can actually run: mods need Fabric, shaders need Iris
/// (which also reads OptiFine-style shader packs).
fn loaders_for(project_type: &str) -> &'static [&'static str] {
    match project_type {
        "mod" => &["fabric"],
        "shader" => &["iris", "optifine"],
        _ => &[],
    }
}

async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let resp = http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Errore di rete con Modrinth: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Modrinth: HTTP {}", resp.status()));
    }
    resp.json::<T>()
        .await
        .map_err(|e| format!("Risposta di Modrinth non valida: {}", e))
}

/// Search projects on Modrinth (mods, shaders, resourcepacks).
/// Only content NovaCraft can run is shown (Fabric mods, Iris-compatible shaders).
pub async fn search_projects(
    query: &str,
    project_type: &str,
    game_version: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<ModrinthSearchResult, String> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", project_type)]];
    let loaders = loaders_for(project_type);
    if !loaders.is_empty() {
        // Inner list = OR
        facets.push(loaders.iter().map(|l| format!("categories:{}", l)).collect());
    }
    if let Some(gv) = game_version.filter(|g| !g.is_empty()) {
        facets.push(vec![format!("versions:{}", gv)]);
    }

    let facets_json = serde_json::to_string(&facets).unwrap_or_else(|_| "[]".to_string());
    let url = format!(
        "{}/search?query={}&facets={}&limit={}&offset={}",
        API,
        urlencoding::encode(query),
        urlencoding::encode(&facets_json),
        limit,
        offset
    );
    get_json(&url).await
}

/// Versions of a project compatible with the given loaders / game version.
async fn get_project_versions(
    project: &str,
    loaders: &[&str],
    game_version: Option<&str>,
) -> Result<Vec<ModrinthVersion>, String> {
    let mut params: Vec<String> = Vec::new();
    if !loaders.is_empty() {
        let json = serde_json::to_string(loaders).unwrap_or_default();
        params.push(format!("loaders={}", urlencoding::encode(&json)));
    }
    if let Some(gv) = game_version.filter(|g| !g.is_empty()) {
        let json = serde_json::to_string(&[gv]).unwrap_or_default();
        params.push(format!("game_versions={}", urlencoding::encode(&json)));
    }
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };
    let url = format!("{}/project/{}/version{}", API, urlencoding::encode(project), query);
    let mut versions: Vec<ModrinthVersion> = get_json(&url).await?;

    // Some projects mis-tag Forge builds: never hand a Forge jar to Fabric.
    if loaders == ["fabric"] {
        versions.retain(|v| {
            v.loaders.iter().any(|l| l.eq_ignore_ascii_case("fabric"))
                && primary_file(v).is_some_and(|f| {
                    let name = f.filename.to_lowercase();
                    !(name.contains("neoforge") || (name.contains("forge") && !name.contains("fabric")))
                })
        });
    }
    Ok(versions)
}

fn primary_file(v: &ModrinthVersion) -> Option<&ModrinthVersionFile> {
    v.files.iter().find(|f| f.primary).or_else(|| v.files.first())
}

/// Best version to install: the newest stable release, otherwise the newest one.
async fn best_version(
    project: &str,
    loaders: &[&str],
    game_version: Option<&str>,
) -> Result<ModrinthVersion, String> {
    let versions = get_project_versions(project, loaders, game_version).await?;
    versions
        .iter()
        .find(|v| v.version_type == "release")
        .or_else(|| versions.first())
        .cloned()
        .ok_or_else(|| match game_version {
            Some(gv) => format!("Nessun file compatibile con Minecraft {} per \"{}\".", gv, project),
            None => format!("Nessun file compatibile per \"{}\".", project),
        })
}

/// Download the main file of a version into `dir` (SHA-1 verified).
async fn download_version_file(version: &ModrinthVersion, dir: &Path) -> Result<String, String> {
    let file = primary_file(version).ok_or("Questa versione non contiene file")?;
    // The name comes from the internet: make sure it can't escape the folder.
    let filename = utils::bare_file_name(&file.filename)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Impossibile creare {:?}: {}", dir, e))?;
    downloader::download_file(
        &file.url,
        &dir.join(&filename),
        file.hashes.get("sha1").map(String::as_str),
    )
    .await?;
    Ok(filename)
}

/// Which Modrinth projects are already in a folder (identified by file hash,
/// so it works whatever the file is called, enabled or disabled).
/// Returns project_id -> file name. Without internet it returns an empty map.
async fn installed_projects(dir: &Path) -> HashMap<String, String> {
    let dir = dir.to_path_buf();
    let hashes: HashMap<String, String> = tauri::async_runtime::spawn_blocking(move || {
        let mut map = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if let Ok(data) = std::fs::read(&path) {
                    map.insert(downloader::sha1_hex(&data), name);
                }
            }
        }
        map
    })
    .await
    .unwrap_or_default();

    if hashes.is_empty() {
        return HashMap::new();
    }

    let body = serde_json::json!({
        "hashes": hashes.keys().collect::<Vec<_>>(),
        "algorithm": "sha1",
    });
    let resp = match http_client()
        .post(format!("{}/version_files", API))
        .json(&body)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => return HashMap::new(),
    };
    let found: HashMap<String, ModrinthVersion> = resp.json().await.unwrap_or_default();
    found
        .into_iter()
        .filter_map(|(hash, v)| hashes.get(&hash).map(|name| (v.project_id, name.clone())))
        .collect()
}

/// Project ids installed in the folder of the given content type (for the
/// "✓ Installato" badges in the UI).
pub async fn installed_project_ids(project_type: &str, custom_dir: Option<&str>) -> Vec<String> {
    let dir = utils::game_dir(custom_dir).join(folder_for(project_type));
    installed_projects(&dir).await.into_keys().collect()
}

enum Wanted {
    Project(String),
    Version(String),
}

/// Install a Fabric mod and, recursively, every mod it requires
/// (e.g. Fabric API), skipping what is already installed.
async fn install_mod_tree(
    root: &str,
    game_version: &str,
    mods_dir: &Path,
    installed: &mut HashMap<String, String>,
    report: &mut InstallReport,
    is_main_project: bool,
) -> Result<(), String> {
    let mut queue = VecDeque::from([Wanted::Project(root.to_string())]);
    let mut visited: HashSet<String> = HashSet::new();
    let mut first = true;

    while let Some(wanted) = queue.pop_front() {
        let result = match &wanted {
            Wanted::Project(id) => best_version(id, &["fabric"], Some(game_version)).await,
            Wanted::Version(id) => get_json::<ModrinthVersion>(&format!("{}/version/{}", API, id)).await,
        };
        let is_root = std::mem::replace(&mut first, false);
        let version = match result {
            Ok(v) => v,
            Err(e) if is_root => return Err(e),
            Err(e) => {
                report.warnings.push(format!("Dipendenza non installata: {}", e));
                continue;
            }
        };

        if !visited.insert(version.project_id.clone()) {
            continue;
        }
        if installed.contains_key(&version.project_id) {
            if is_root && is_main_project {
                report.already_installed = true;
            }
            continue;
        }

        let filename = download_version_file(&version, mods_dir).await?;
        installed.insert(version.project_id.clone(), filename.clone());
        report.installed.push(filename);

        for dep in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
            match (&dep.project_id, &dep.version_id) {
                // Prefer the project: we pick the build matching *our* game version.
                (Some(pid), _) => {
                    if !installed.contains_key(pid) && !visited.contains(pid) {
                        queue.push_back(Wanted::Project(pid.clone()));
                    }
                }
                (None, Some(vid)) => queue.push_back(Wanted::Version(vid.clone())),
                _ => {}
            }
        }
    }
    Ok(())
}

/// Install a mod / shader / resource pack from Modrinth into a profile.
///
/// - mods: also installs their required dependencies (Fabric API & co.)
/// - shaders: also installs Iris + Sodium, needed to load them
pub async fn install_project(
    project_id: &str,
    project_type: &str,
    game_version: &str,
    custom_dir: Option<&str>,
) -> Result<InstallReport, String> {
    let game_dir = utils::game_dir(custom_dir);
    let mods_dir = game_dir.join("mods");
    let mut report = InstallReport::default();

    if project_type == "mod" {
        let mut installed = installed_projects(&mods_dir).await;
        install_mod_tree(project_id, game_version, &mods_dir, &mut installed, &mut report, true).await?;
        return Ok(report);
    }

    // Shader or resource pack
    let dir = game_dir.join(folder_for(project_type));
    let loaders = loaders_for(project_type);
    let version = match best_version(project_id, loaders, Some(game_version)).await {
        Ok(v) => v,
        // Some shader packs don't declare a loader at all: accept them too.
        Err(_) if !loaders.is_empty() => best_version(project_id, &[], Some(game_version)).await?,
        Err(e) => return Err(e),
    };
    let already = installed_projects(&dir).await;
    if already.contains_key(&version.project_id) {
        report.already_installed = true;
    } else {
        report.installed.push(download_version_file(&version, &dir).await?);
    }

    if project_type == "shader" {
        let mut installed = installed_projects(&mods_dir).await;
        for needed in ["iris", "sodium"] {
            if let Err(e) =
                install_mod_tree(needed, game_version, &mods_dir, &mut installed, &mut report, false).await
            {
                report
                    .warnings
                    .push(format!("{} non installato: {}", needed, e));
            }
        }
    }

    Ok(report)
}

/// List installed mods in the target game directory
pub fn list_mods(custom_dir: Option<&str>) -> Vec<InstalledModInfo> {
    let mods_dir = utils::game_dir(custom_dir).join("mods");
    let mut list = Vec::new();

    if let Ok(entries) = std::fs::read_dir(mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
                list.push(InstalledModInfo {
                    is_enabled: name.ends_with(".jar"),
                    size_bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
                    filename: name,
                });
            }
        }
    }

    list.sort_by_key(|m| m.filename.to_lowercase());
    list
}

/// Toggle a mod file between .jar (enabled) and .jar.disabled (disabled)
pub fn toggle_mod(filename: &str, custom_dir: Option<&str>) -> Result<bool, String> {
    let filename = utils::bare_file_name(filename)?;
    let mods_dir = utils::game_dir(custom_dir).join("mods");
    let current_path = mods_dir.join(&filename);

    if !current_path.exists() {
        return Err("File della mod non trovato".to_string());
    }

    let (new_path, is_enabled) = if let Some(stripped) = filename.strip_suffix(".disabled") {
        (mods_dir.join(stripped), true)
    } else if filename.ends_with(".jar") {
        (mods_dir.join(format!("{}.disabled", filename)), false)
    } else {
        return Err("Questo file non è una mod".to_string());
    };

    std::fs::rename(current_path, new_path)
        .map_err(|e| format!("Impossibile rinominare la mod (il gioco è aperto?): {}", e))?;

    Ok(is_enabled)
}

/// Delete a mod file
pub fn delete_mod(filename: &str, custom_dir: Option<&str>) -> Result<(), String> {
    let filename = utils::bare_file_name(filename)?;
    let path = utils::game_dir(custom_dir).join("mods").join(filename);
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|e| format!("Impossibile eliminare la mod (il gioco è aperto?): {}", e))?;
    }
    Ok(())
}

/// Open a profile folder ("" = the profile itself, or mods, shaderpacks, saves...)
pub fn open_subfolder(folder_name: &str, custom_dir: Option<&str>) -> Result<(), String> {
    let base = utils::game_dir(custom_dir);
    let target: PathBuf = if folder_name.is_empty() {
        base
    } else {
        base.join(utils::bare_file_name(folder_name)?)
    };
    utils::open_in_file_manager(&target)
}
