use serde::Deserialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use tauri::Emitter;

use super::libraries::{Library, is_library_allowed};
use crate::utils;

/// The complete version JSON structure from Mojang / Fabric.
#[derive(Deserialize, Clone)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
    #[serde(rename = "type")]
    pub version_type: Option<String>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex")]
    pub asset_index: Option<AssetIndexInfo>,
    pub assets: Option<String>,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
}

/// Download URLs for the client/server JARs.
#[derive(Deserialize, Clone)]
pub struct VersionDownloads {
    pub client: DownloadInfo,
}

/// Info about a downloadable file.
#[derive(Deserialize, Clone)]
pub struct DownloadInfo {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Asset index reference (ID and download URL).
#[derive(Deserialize, Clone)]
pub struct AssetIndexInfo {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

/// Modern argument structure (1.13+).
#[derive(Deserialize, Clone, Default)]
pub struct Arguments {
    pub game: Option<Vec<serde_json::Value>>,
    pub jvm: Option<Vec<serde_json::Value>>,
}

/// Java version requirements.
#[derive(Deserialize, Clone)]
pub struct JavaVersion {
    pub component: Option<String>,
    #[serde(rename = "majorVersion")]
    pub major_version: Option<u32>,
}

/// Build and execute the Minecraft launch command.
///
/// Constructs the full Java command line with:
/// - JVM arguments (memory, native paths, classpath)
/// - Main class
/// - Game arguments (username, version, directories, offline auth)
pub fn launch_minecraft(
    app: &tauri::AppHandle,
    java_path: &str,
    version_json: &VersionJson,
    username: &str,
    base_dir: &Path,
    custom_game_dir: Option<&Path>,
    ram_mb: u32,
) -> Result<(), String> {
    let version_id = &version_json.id;

    // Check if this version inherits from a base version (e.g., Fabric)
    let parent_json: Option<VersionJson> = if let Some(parent_id) = &version_json.inherits_from {
        let parent_path = base_dir
            .join("versions")
            .join(parent_id)
            .join(format!("{}.json", parent_id));
        if parent_path.exists() {
            let data = std::fs::read_to_string(&parent_path).ok();
            data.and_then(|d| serde_json::from_str(&d).ok())
        } else {
            None
        }
    } else {
        None
    };

    // The primary version ID used for client.jar and natives
    let base_version_id = version_json
        .inherits_from
        .as_deref()
        .unwrap_or(version_id);

    let versions_dir = base_dir.join("versions").join(base_version_id);
    let natives_dir = versions_dir.join("natives");
    let assets_dir = base_dir.join("assets");
    let libraries_dir = base_dir.join("libraries");

    // The game directory where saves, mods, options live
    let target_game_dir = custom_game_dir.unwrap_or(base_dir);
    let _ = std::fs::create_dir_all(target_game_dir);

    // Merge libraries: parent libraries + profile libraries
    let mut all_libraries: Vec<Library> = Vec::new();
    if let Some(parent) = &parent_json {
        all_libraries.extend(parent.libraries.clone());
    }
    all_libraries.extend(version_json.libraries.clone());

    // Build classpath: all allowed libraries + client.jar
    let classpath = build_classpath(
        &all_libraries,
        &libraries_dir,
        &versions_dir,
        base_version_id,
    );

    // Generate offline UUID
    let uuid = utils::offline_uuid(username);

    let assets_index_name = version_json
        .assets
        .as_deref()
        .or_else(|| parent_json.as_ref().and_then(|p| p.assets.as_deref()))
        .unwrap_or("legacy");

    let version_type = version_json
        .version_type
        .as_deref()
        .or_else(|| parent_json.as_ref().and_then(|p| p.version_type.as_deref()))
        .unwrap_or("release");

    // Build variable substitution map
    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert("auth_player_name".to_string(), username.to_string());
    vars.insert("version_name".to_string(), version_id.to_string());
    vars.insert("game_directory".to_string(), target_game_dir.to_string_lossy().to_string());
    vars.insert("assets_root".to_string(), assets_dir.to_string_lossy().to_string());
    vars.insert("assets_index_name".to_string(), assets_index_name.to_string());
    vars.insert("auth_uuid".to_string(), uuid);
    vars.insert("auth_access_token".to_string(), "0".to_string());
    vars.insert("user_type".to_string(), "legacy".to_string());
    vars.insert("version_type".to_string(), version_type.to_string());
    vars.insert("natives_directory".to_string(), natives_dir.to_string_lossy().to_string());
    vars.insert("launcher_name".to_string(), "NovaCraft".to_string());
    vars.insert("launcher_version".to_string(), "1.0.0".to_string());
    vars.insert("classpath".to_string(), classpath.clone());
    vars.insert("user_properties".to_string(), "{}".to_string());
    vars.insert("game_assets".to_string(), assets_dir.to_string_lossy().to_string());

    // Build the command arguments
    let mut args: Vec<String> = Vec::new();

    // --- JVM arguments ---
    args.push("-Xms512M".to_string());
    args.push(format!("-Xmx{}M", ram_mb));
    args.push(format!("-Djava.library.path={}", natives_dir.to_string_lossy()));
    args.push(format!("-Djna.tmpdir={}", natives_dir.to_string_lossy()));
    args.push(format!(
        "-Dorg.lwjgl.system.SharedLibraryExtractPath={}",
        natives_dir.to_string_lossy()
    ));
    args.push(format!("-Dio.netty.native.workdir={}", natives_dir.to_string_lossy()));
    args.push("-Dminecraft.launcher.brand=NovaCraft".to_string());
    args.push("-Dminecraft.launcher.version=1.0.0".to_string());

    // Collect JVM args from parent and child
    let mut combined_jvm_args: Vec<serde_json::Value> = Vec::new();
    if let Some(parent) = &parent_json {
        if let Some(args) = &parent.arguments {
            if let Some(j) = &args.jvm {
                combined_jvm_args.extend(j.clone());
            }
        }
    }
    if let Some(args) = &version_json.arguments {
        if let Some(j) = &args.jvm {
            combined_jvm_args.extend(j.clone());
        }
    }

    for arg in combined_jvm_args {
        if let Some(s) = arg.as_str() {
            if s == "-cp" || s == "${classpath}" {
                continue;
            }
            args.push(substitute_vars(s, &vars));
        }
    }

    // Classpath
    args.push("-cp".to_string());
    args.push(classpath);

    // Main class
    args.push(version_json.main_class.clone());

    // --- Game arguments ---
    let mut combined_game_args: Vec<serde_json::Value> = Vec::new();
    if let Some(parent) = &parent_json {
        if let Some(args) = &parent.arguments {
            if let Some(g) = &args.game {
                combined_game_args.extend(g.clone());
            }
        }
    }
    if let Some(args) = &version_json.arguments {
        if let Some(g) = &args.game {
            combined_game_args.extend(g.clone());
        }
    }

    if !combined_game_args.is_empty() {
        for arg in combined_game_args {
            if let Some(s) = arg.as_str() {
                args.push(substitute_vars(s, &vars));
            }
        }
    } else if let Some(legacy_args) = version_json
        .minecraft_arguments
        .as_deref()
        .or_else(|| parent_json.as_ref().and_then(|p| p.minecraft_arguments.as_deref()))
    {
        for arg in legacy_args.split_whitespace() {
            args.push(substitute_vars(arg, &vars));
        }
    }

    eprintln!("[NovaCraft] Java: {}", java_path);
    eprintln!("[NovaCraft] Launch directory: {}", target_game_dir.display());

    let mut child = Command::new(java_path)
        .args(&args)
        .current_dir(target_game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Impossibile avviare Minecraft: {}", e))?;

    let pid = child.id();
    eprintln!("[NovaCraft] Minecraft process spawned (pid {}).", pid);
    let _ = app.emit("game-started", pid);

    let crash_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Stream stdout to UI console
    if let Some(out) = child.stdout.take() {
        let app_handle = app.clone();
        thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = app_handle.emit(
                    "game-log",
                    serde_json::json!({
                        "stream": "stdout",
                        "text": line
                    }),
                );
            }
        });
    }

    // Stream stderr to UI console and save recent lines for crash detection
    if let Some(err) = child.stderr.take() {
        let app_handle = app.clone();
        let buf = crash_log.clone();
        thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = app_handle.emit(
                    "game-log",
                    serde_json::json!({
                        "stream": "stderr",
                        "text": &line
                    }),
                );
                if let Ok(mut b) = buf.lock() {
                    b.push(line);
                    let len = b.len();
                    if len > 60 {
                        b.drain(0..len - 60);
                    }
                }
            }
        });
    }

    // Wait for the game to exit
    let app = app.clone();
    let started = Instant::now();
    thread::spawn(move || {
        let mut child = child;
        let code = child.wait().ok().and_then(|s| s.code());
        let crashed = code != Some(0);
        let secs = started.elapsed().as_secs();
        let log = crash_log.lock().map(|b| b.join("\n")).unwrap_or_default();
        eprintln!("[NovaCraft] Minecraft exited (code {:?}, after {}s).", code, secs);
        let _ = app.emit(
            "game-exited",
            serde_json::json!({
                "code": code,
                "crashed": crashed,
                "durationSecs": secs,
                "log": log,
            }),
        );
    });

    Ok(())
}

/// Build the classpath string from all allowed libraries + client JAR.
fn build_classpath(
    libraries: &[Library],
    libraries_dir: &Path,
    versions_dir: &Path,
    version_id: &str,
) -> String {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let mut entries: Vec<String> = Vec::new();

    for lib in libraries {
        if !is_library_allowed(&lib.rules) {
            continue;
        }

        if let Some(downloads) = &lib.downloads {
            if let Some(artifact) = &downloads.artifact {
                let path = libraries_dir.join(&artifact.path);
                entries.push(path.to_string_lossy().to_string());
                continue;
            }
        }

        // Support maven artifact naming (used by Fabric)
        if let Some(rel_path) = maven_name_to_path(&lib.name) {
            let path = libraries_dir.join(rel_path);
            entries.push(path.to_string_lossy().to_string());
        }
    }

    // Append client.jar
    let client_jar = versions_dir.join(format!("{}.jar", version_id));
    entries.push(client_jar.to_string_lossy().to_string());

    entries.join(separator)
}

fn maven_name_to_path(coord: &str) -> Option<String> {
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

/// Replace ${placeholder} variables in a string with their values.
fn substitute_vars(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in vars {
        let placeholder = format!("${{{}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}
