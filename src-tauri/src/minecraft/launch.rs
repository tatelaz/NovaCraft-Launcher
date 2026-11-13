use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Instant, SystemTime};
use tauri::Emitter;

use super::assets;
use super::libraries::{is_library_allowed, library_key, maven_to_path, rules_allow, Library, Rule};
use crate::utils;

/// How many of the last log lines are kept to explain a crash.
const CRASH_LOG_LINES: usize = 80;

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

/// Download URLs for the client JAR.
#[derive(Deserialize, Clone)]
pub struct VersionDownloads {
    pub client: DownloadInfo,
}

/// Info about a downloadable file.
#[derive(Deserialize, Clone)]
pub struct DownloadInfo {
    pub sha1: String,
    pub url: String,
}

/// Asset index reference (ID, hash and download URL).
#[derive(Deserialize, Clone)]
pub struct AssetIndexInfo {
    pub id: String,
    pub sha1: Option<String>,
    pub url: String,
}

/// Modern argument structure (1.13+).
#[derive(Deserialize, Clone, Default)]
pub struct Arguments {
    pub game: Option<Vec<Value>>,
    pub jvm: Option<Vec<Value>>,
}

/// Java version requirements.
#[derive(Deserialize, Clone)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: Option<u32>,
}

impl VersionJson {
    /// Read an installed version JSON from `versions/<id>/<id>.json`.
    pub fn load(base_dir: &Path, id: &str) -> Result<VersionJson, String> {
        let path = base_dir.join("versions").join(id).join(format!("{}.json", id));
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("Versione {} non installata ({})", id, e))?;
        serde_json::from_str(&data).map_err(|e| format!("File della versione {} non valido: {}", id, e))
    }
}

/// Is this a pre-1.13 version (old-style arguments)? Looks at the parent too,
/// so it also works for Fabric profiles.
pub fn is_legacy_version(base_dir: &Path, version_json: &VersionJson) -> bool {
    if version_json.arguments.is_some() {
        return false;
    }
    match &version_json.inherits_from {
        Some(parent_id) => VersionJson::load(base_dir, parent_id)
            .map(|p| p.arguments.is_none())
            .unwrap_or(false),
        None => true,
    }
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
    let parent_json: Option<VersionJson> = match &version_json.inherits_from {
        Some(parent_id) => Some(VersionJson::load(base_dir, parent_id)?),
        None => None,
    };

    // The primary version ID used for client.jar and natives
    let base_version_id = version_json.inherits_from.as_deref().unwrap_or(version_id);

    let versions_dir = base_dir.join("versions").join(base_version_id);
    let natives_dir = versions_dir.join("natives");
    let assets_dir = base_dir.join("assets");
    let libraries_dir = base_dir.join("libraries");

    // The game directory where saves, mods, options live
    let target_game_dir = custom_game_dir.unwrap_or(base_dir);
    std::fs::create_dir_all(target_game_dir)
        .map_err(|e| format!("Impossibile creare la cartella di gioco: {}", e))?;

    // Merge libraries: parent libraries + profile libraries
    let mut all_libraries: Vec<Library> = Vec::new();
    if let Some(parent) = &parent_json {
        all_libraries.extend(parent.libraries.clone());
    }
    all_libraries.extend(version_json.libraries.clone());

    let client_jar = versions_dir.join(format!("{}.jar", base_version_id));
    if !client_jar.exists() {
        return Err(format!("Il file del gioco {} non è stato scaricato", base_version_id));
    }
    let classpath = build_classpath(&all_libraries, &libraries_dir, &client_jar);

    let assets_index_name = version_json
        .assets
        .as_deref()
        .or_else(|| parent_json.as_ref().and_then(|p| p.assets.as_deref()))
        .unwrap_or("legacy");

    // Very old versions read assets by name from a special folder.
    let game_assets = assets::prepare_legacy_assets(base_dir, assets_index_name, target_game_dir)
        .unwrap_or_else(|| assets_dir.clone());

    let version_type = version_json
        .version_type
        .as_deref()
        .or_else(|| parent_json.as_ref().and_then(|p| p.version_type.as_deref()))
        .unwrap_or("release");

    let separator = if cfg!(windows) { ";" } else { ":" };
    let launcher_version = env!("CARGO_PKG_VERSION");

    // Build variable substitution map
    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", username.to_string());
    vars.insert("version_name", version_id.to_string());
    vars.insert("game_directory", target_game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", assets_dir.to_string_lossy().to_string());
    vars.insert("game_assets", game_assets.to_string_lossy().to_string());
    vars.insert("assets_index_name", assets_index_name.to_string());
    vars.insert("auth_uuid", utils::offline_uuid(username));
    vars.insert("auth_access_token", "0".to_string());
    vars.insert("auth_session", "0".to_string());
    vars.insert("auth_xuid", "0".to_string());
    vars.insert("clientid", "0".to_string());
    vars.insert("user_type", "legacy".to_string());
    vars.insert("version_type", version_type.to_string());
    vars.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    vars.insert("launcher_name", "NovaCraft".to_string());
    vars.insert("launcher_version", launcher_version.to_string());
    vars.insert("classpath", classpath.clone());
    vars.insert("classpath_separator", separator.to_string());
    vars.insert("library_directory", libraries_dir.to_string_lossy().to_string());
    vars.insert("user_properties", "{}".to_string());

    // Build the command arguments
    let mut args: Vec<String> = Vec::new();

    // --- JVM arguments ---
    args.push(format!("-Xms{}M", ram_mb.min(512)));
    args.push(format!("-Xmx{}M", ram_mb));
    args.push(format!("-Djava.library.path={}", natives_dir.to_string_lossy()));
    args.push(format!("-Djna.tmpdir={}", natives_dir.to_string_lossy()));
    args.push(format!(
        "-Dorg.lwjgl.system.SharedLibraryExtractPath={}",
        natives_dir.to_string_lossy()
    ));
    args.push(format!("-Dio.netty.native.workdir={}", natives_dir.to_string_lossy()));
    args.push("-Dminecraft.launcher.brand=NovaCraft".to_string());
    args.push(format!("-Dminecraft.launcher.version={}", launcher_version));

    // JVM args from parent and child. Conditional ones are kept only when their
    // rules match this computer (e.g. -XstartOnFirstThread, required on macOS).
    let mut jvm_args: Vec<String> = Vec::new();
    for json in parent_json.iter().chain(std::iter::once(version_json)) {
        if let Some(list) = json.arguments.as_ref().and_then(|a| a.jvm.as_ref()) {
            jvm_args.extend(expand_args(list));
        }
    }
    for arg in jvm_args {
        // We add the classpath ourselves below.
        if arg == "-cp" || arg == "${classpath}" {
            continue;
        }
        args.push(substitute_vars(&arg, &vars));
    }

    // Classpath
    args.push("-cp".to_string());
    args.push(classpath);

    // Main class
    args.push(version_json.main_class.clone());

    // --- Game arguments ---
    let mut game_args: Vec<String> = Vec::new();
    for json in parent_json.iter().chain(std::iter::once(version_json)) {
        if let Some(list) = json.arguments.as_ref().and_then(|a| a.game.as_ref()) {
            game_args.extend(expand_args(list));
        }
    }

    if !game_args.is_empty() {
        args.extend(game_args.iter().map(|a| substitute_vars(a, &vars)));
    } else if let Some(legacy_args) = version_json
        .minecraft_arguments
        .as_deref()
        .or_else(|| parent_json.as_ref().and_then(|p| p.minecraft_arguments.as_deref()))
    {
        args.extend(legacy_args.split_whitespace().map(|a| substitute_vars(a, &vars)));
    }

    eprintln!("[NovaCraft] Java: {}", java_path);
    eprintln!("[NovaCraft] Cartella di gioco: {}", target_game_dir.display());

    let started_at = SystemTime::now();
    let mut child = Command::new(java_path)
        .args(&args)
        .current_dir(target_game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Impossibile avviare Minecraft: {}", e))?;

    let pid = child.id();
    eprintln!("[NovaCraft] Minecraft avviato (pid {}).", pid);
    let _ = app.emit("game-started", pid);

    // Last lines of BOTH streams: Minecraft writes most of its log (and Fabric
    // its errors) on stdout, Java crashes go to stderr.
    let recent_log: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        readers.push(spawn_log_reader(app.clone(), out, "stdout", recent_log.clone()));
    }
    if let Some(err) = child.stderr.take() {
        readers.push(spawn_log_reader(app.clone(), err, "stderr", recent_log.clone()));
    }

    // Wait for the game to exit
    let app = app.clone();
    let started = Instant::now();
    let game_dir = target_game_dir.to_path_buf();
    thread::spawn(move || {
        let mut child = child;
        let code = child.wait().ok().and_then(|s| s.code());
        // Let the readers flush the last lines before we report.
        for r in readers {
            let _ = r.join();
        }
        let crashed = code != Some(0);
        let secs = started.elapsed().as_secs();
        let log = recent_log
            .lock()
            .map(|b| b.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();
        let crash_report = if crashed {
            find_crash_report(&game_dir, started_at).map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };
        eprintln!("[NovaCraft] Minecraft terminato (codice {:?}, dopo {}s).", code, secs);
        let _ = app.emit(
            "game-exited",
            serde_json::json!({
                "code": code,
                "crashed": crashed,
                "durationSecs": secs,
                "log": log,
                "crashReport": crash_report,
            }),
        );
    });

    Ok(())
}

/// Forward each line of a game output stream to the UI console and keep the
/// most recent ones for the crash message.
fn spawn_log_reader<R: std::io::Read + Send + 'static>(
    app: tauri::AppHandle,
    stream: R,
    name: &'static str,
    recent: Arc<Mutex<VecDeque<String>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let _ = app.emit("game-log", serde_json::json!({ "stream": name, "text": &line }));
            if let Ok(mut buf) = recent.lock() {
                buf.push_back(line);
                while buf.len() > CRASH_LOG_LINES {
                    buf.pop_front();
                }
            }
        }
    })
}

/// The crash report Minecraft wrote during this session, if any.
fn find_crash_report(game_dir: &Path, since: SystemTime) -> Option<PathBuf> {
    let entries = std::fs::read_dir(game_dir.join("crash-reports")).ok()?;
    entries
        .flatten()
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            (modified >= since).then(|| (modified, e.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// Expand a JSON argument list: plain strings are kept, objects are kept only
/// if their rules match this computer (and never for optional features such
/// as demo mode or a custom window size).
fn expand_args(list: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for arg in list {
        match arg {
            Value::String(s) => out.push(s.clone()),
            Value::Object(obj) => {
                let allowed = match obj.get("rules") {
                    None => true,
                    Some(r) => serde_json::from_value::<Vec<Rule>>(r.clone())
                        .map(|rules| rules_allow(&rules))
                        .unwrap_or(false),
                };
                if !allowed {
                    continue;
                }
                match obj.get("value") {
                    Some(Value::String(s)) => out.push(s.clone()),
                    Some(Value::Array(values)) => {
                        out.extend(values.iter().filter_map(|v| v.as_str().map(String::from)))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

/// Build the classpath string from all allowed libraries + client JAR.
///
/// When the same library appears twice (vanilla + Fabric), the later one wins,
/// so two versions of the same library are never loaded together.
fn build_classpath(libraries: &[Library], libraries_dir: &Path, client_jar: &Path) -> String {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let mut order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, PathBuf> = HashMap::new();

    for lib in libraries {
        if !is_library_allowed(&lib.rules) {
            continue;
        }
        let path = match &lib.downloads {
            // Natives-only entries (old versions) have no jar for the classpath.
            Some(downloads) => match &downloads.artifact {
                Some(artifact) => libraries_dir.join(&artifact.path),
                None => continue,
            },
            // Fabric libraries only have a maven name.
            None => match maven_to_path(&lib.name) {
                Some(rel) => libraries_dir.join(rel),
                None => continue,
            },
        };
        let key = library_key(&lib.name);
        if !by_key.contains_key(&key) {
            order.push(key.clone());
        }
        by_key.insert(key, path);
    }

    let mut entries: Vec<String> = Vec::new();
    for key in &order {
        let path = &by_key[key];
        if path.exists() {
            entries.push(path.to_string_lossy().to_string());
        } else {
            eprintln!("[NovaCraft] Libreria mancante, ignorata: {}", path.display());
        }
    }
    entries.push(client_jar.to_string_lossy().to_string());
    entries.join(separator)
}

/// Replace ${placeholder} variables in a string with their values.
fn substitute_vars(input: &str, vars: &HashMap<&str, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in vars {
        let placeholder = format!("${{{}}}", key);
        if result.contains(&placeholder) {
            result = result.replace(&placeholder, value);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_arguments() {
        let list: Vec<Value> = serde_json::from_str(
            r#"[
                "--username", "${auth_player_name}",
                {"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"},
                {"rules":[{"action":"allow","os":{"name":"osx"}}],"value":["-XstartOnFirstThread"]},
                {"rules":[{"action":"allow","os":{"name":"windows"}}],"value":"-XX:HeapDumpPath=x"}
            ]"#,
        )
        .unwrap();
        let args = expand_args(&list);
        assert_eq!(&args[..2], &["--username".to_string(), "${auth_player_name}".to_string()]);
        assert!(!args.contains(&"--demo".to_string()));
        assert_eq!(args.contains(&"-XstartOnFirstThread".to_string()), cfg!(target_os = "macos"));
        assert_eq!(args.contains(&"-XX:HeapDumpPath=x".to_string()), cfg!(windows));
    }

    #[test]
    fn variables() {
        let mut vars = HashMap::new();
        vars.insert("auth_player_name", "Manuel".to_string());
        assert_eq!(substitute_vars("--name=${auth_player_name}", &vars), "--name=Manuel");
        assert_eq!(substitute_vars("${unknown}", &vars), "${unknown}");
    }
}
