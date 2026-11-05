use futures_util::StreamExt;
use std::io::Cursor;
use std::path::Path;
use tauri::Emitter;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use super::downloader::DownloadProgress;
use super::launch::VersionJson;
use crate::utils;

/// The Java major version a given Minecraft version requires.
/// Mojang declares it in the version JSON; very old versions omit it and need Java 8.
pub fn required_major(version_json: &VersionJson) -> u32 {
    if let Some(jv) = &version_json.java_version {
        if let Some(m) = jv.major_version {
            return m;
        }
    }

    // If this version inherits from another (e.g. Fabric), check parent JSON
    if let Some(parent_id) = &version_json.inherits_from {
        let parent_json_path = utils::get_novacraft_dir()
            .join("versions")
            .join(parent_id)
            .join(format!("{}.json", parent_id));
        if let Ok(data) = std::fs::read_to_string(&parent_json_path) {
            if let Ok(parent_json) = serde_json::from_str::<VersionJson>(&data) {
                if let Some(jv) = parent_json.java_version {
                    if let Some(m) = jv.major_version {
                        return m;
                    }
                }
            }
        }
    }

    8
}

/// Map a required Java major to a version actually distributed by Adoptium
/// (Temurin ships 8, 17, 21, 25 and newer LTS/feature lines).
fn adoptium_version(required: u32) -> u32 {
    match required {
        0..=8 => 8,
        9..=17 => 17,
        18..=21 => 21,
        other => other,
    }
}

/// Ensure a Java runtime suitable for `required` major is available and return
/// the path to its `javaw.exe`. Uses (in order): a previously downloaded managed
/// runtime, a compatible system Java, or a fresh Adoptium download.
pub async fn ensure_java(app: &tauri::AppHandle, required: u32) -> Result<String, String> {
    let target = adoptium_version(required);

    // 1. Already downloaded a managed runtime for this target?
    if let Some(p) = find_managed_java(target) {
        return Ok(p);
    }

    // 2. Is a compatible system Java already installed?
    //    Old Minecraft (needs 8) must use Java 8; modern versions accept any newer Java.
    if let Some(sys) = utils::detect_java() {
        if let Some(sm) = system_java_major(&sys) {
            let acceptable = if required <= 8 { sm == 8 } else { sm >= required };
            if acceptable {
                return Ok(sys);
            }
        }
    }

    // 3. Download it from Adoptium.
    download_java(app, target).await
}

/// Look for a `javaw.exe` inside a previously downloaded managed runtime.
fn find_managed_java(target: u32) -> Option<String> {
    let runtime_dir = utils::get_novacraft_dir()
        .join("runtimes")
        .join(format!("java-{}", target));
    if !runtime_dir.exists() {
        return None;
    }
    find_javaw(&runtime_dir)
}

/// Recursively search a directory tree for a Java binary (javaw.exe on Windows, java on Unix).
fn find_javaw(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_javaw(&path) {
                return Some(found);
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if (cfg!(windows) && name == "javaw.exe") || (!cfg!(windows) && name == "java") {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Run `java -version` and parse the major version number.
fn system_java_major(javaw_path: &str) -> Option<u32> {
    // Use java.exe (not javaw) on Windows so `-version` prints to stderr we can read.
    let java_exe = if cfg!(windows) {
        javaw_path.replace("javaw.exe", "java.exe")
    } else {
        javaw_path.to_string()
    };

    let mut cmd = std::process::Command::new(&java_exe);
    cmd.arg("-version");
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW — no console flash

    let output = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    parse_java_major(&text)
}

/// Parse a Java major version out of a `-version` banner
/// (handles both `"1.8.0_401"` -> 8 and `"21.0.10"` -> 21).
fn parse_java_major(text: &str) -> Option<u32> {
    let start = text.find('"')?;
    let rest = &text[start + 1..];
    let end = rest.find('"')?;
    let ver = &rest[..end];

    let mut parts = ver.split('.');
    let first = parts.next()?;
    if first == "1" {
        parts.next()?.parse::<u32>().ok()
    } else {
        first.parse::<u32>().ok()
    }
}

/// Download an Adoptium (Temurin) JRE for the given major version, extract it,
/// and return the path to its `javaw.exe`. Emits "Java" progress events.
async fn download_java(app: &tauri::AppHandle, target: u32) -> Result<String, String> {
    let runtime_dir = utils::get_novacraft_dir()
        .join("runtimes")
        .join(format!("java-{}", target));
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("Impossibile creare la cartella di Java: {}", e))?;

    let client = reqwest::Client::new();

    let (os, arch) = if cfg!(target_os = "windows") {
        ("windows", "x64")
    } else if cfg!(target_os = "macos") {
        let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };
        ("mac", arch)
    } else {
        ("linux", "x64")
    };

    // Try JRE first, fallback to JDK if JRE is not packaged for this version
    let jre_url = format!(
        "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jre/hotspot/normal/eclipse",
        target, os, arch
    );
    let mut resp = client
        .get(&jre_url)
        .send()
        .await
        .map_err(|e| format!("Impossibile scaricare Java: {}", e))?;

    if !resp.status().is_success() {
        let jdk_url = format!(
            "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/eclipse",
            target, os, arch
        );
        resp = client
            .get(&jdk_url)
            .send()
            .await
            .map_err(|e| format!("Impossibile scaricare Java: {}", e))?;
    }

    if !resp.status().is_success() {
        return Err(format!("Nessun runtime Java {} disponibile su Adoptium (HTTP {})", target, resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut buf: Vec<u8> = Vec::with_capacity(total as usize);
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Errore durante il download di Java: {}", e))?;
        buf.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;

        // Emit progress at most ~every 1 MB to avoid flooding the UI.
        if downloaded - last_emit >= 1_000_000 || (total > 0 && downloaded >= total) {
            last_emit = downloaded;
            let percentage = if total > 0 {
                (downloaded as f32 / total as f32) * 100.0
            } else {
                0.0
            };
            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    file_name: format!("Java {}", target),
                    downloaded,
                    total: total.max(1),
                    percentage,
                    stage: "Java".to_string(),
                },
            );
        }
    }

    // Extract the JRE archive into the runtime directory.
    let mut archive = zip::ZipArchive::new(Cursor::new(buf))
        .map_err(|e| format!("Archivio Java non valido: {}", e))?;
    archive
        .extract(&runtime_dir)
        .map_err(|e| format!("Errore nell'estrazione di Java: {}", e))?;

    find_managed_java(target)
        .ok_or_else(|| "javaw.exe non trovato dopo l'estrazione di Java".to_string())
}
