use futures_util::StreamExt;
use serde::Serialize;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::Emitter;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use super::downloader::{http_client, DownloadProgress};
use super::launch::VersionJson;
use crate::utils;

/// The Java major version a given Minecraft version requires.
/// Mojang declares it in the version JSON; very old versions omit it and need Java 8.
pub fn required_major(version_json: &VersionJson) -> u32 {
    if let Some(m) = version_json.java_version.as_ref().and_then(|j| j.major_version) {
        return m;
    }

    // If this version inherits from another (e.g. Fabric), check parent JSON
    if let Some(parent_id) = &version_json.inherits_from {
        let parent_json_path = utils::get_novacraft_dir()
            .join("versions")
            .join(parent_id)
            .join(format!("{}.json", parent_id));
        if let Ok(data) = std::fs::read_to_string(&parent_json_path) {
            if let Ok(parent_json) = serde_json::from_str::<VersionJson>(&data) {
                if let Some(m) = parent_json.java_version.and_then(|j| j.major_version) {
                    return m;
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

/// Is a Java of version `found` good for a game that needs `required`?
/// Old Minecraft (needs 8) must use exactly Java 8; modern versions accept newer Java.
fn is_compatible(found: u32, required: u32) -> bool {
    if required <= 8 {
        found == 8
    } else {
        found >= required
    }
}

/// Ensure a Java runtime suitable for `required` major is available and return
/// the path to its executable. Uses (in order): a previously downloaded managed
/// runtime, a compatible system Java, or a fresh Adoptium download.
pub async fn ensure_java(app: &tauri::AppHandle, required: u32) -> Result<String, String> {
    let target = adoptium_version(required);

    // 1. Already downloaded a managed runtime for this target?
    if let Some(p) = find_managed_java(target) {
        return Ok(p);
    }

    // 2. Is a compatible Java already installed on the system? (check all of them,
    //    not just the first one found)
    let candidates = tauri::async_runtime::spawn_blocking(utils::find_java_candidates)
        .await
        .unwrap_or_default();
    for sys in candidates {
        if let Some(found) = system_java_major(&sys) {
            if is_compatible(found, required) {
                return Ok(sys);
            }
        }
    }

    // 3. Download it from Adoptium.
    download_java(app, target).await
}

/// What the Settings page shows about Java.
#[derive(Serialize)]
pub struct JavaStatus {
    /// First Java found on the system: (path, major version).
    pub system_path: Option<String>,
    pub system_major: Option<u32>,
    /// Java versions NovaCraft already downloaded (e.g. [8, 21]).
    pub managed: Vec<u32>,
}

pub fn java_status() -> JavaStatus {
    let mut system_path = None;
    let mut system_major = None;
    for candidate in utils::find_java_candidates() {
        if let Some(major) = system_java_major(&candidate) {
            system_path = Some(candidate);
            system_major = Some(major);
            break;
        }
    }

    let mut managed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(utils::get_novacraft_dir().join("runtimes")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(n) = name.strip_prefix("java-").and_then(|v| v.parse::<u32>().ok()) {
                if find_java_binary(&entry.path()).is_some() {
                    managed.push(n);
                }
            }
        }
    }
    managed.sort_unstable();

    JavaStatus { system_path, system_major, managed }
}

fn managed_dir(target: u32) -> PathBuf {
    utils::get_novacraft_dir()
        .join("runtimes")
        .join(format!("java-{}", target))
}

/// Look for the Java executable inside a previously downloaded managed runtime.
fn find_managed_java(target: u32) -> Option<String> {
    let runtime_dir = managed_dir(target);
    if !runtime_dir.exists() {
        return None;
    }
    find_java_binary(&runtime_dir)
}

/// Recursively search a directory tree for the Java executable
/// (`bin/javaw.exe` on Windows, `bin/java` elsewhere).
fn find_java_binary(dir: &Path) -> Option<String> {
    let wanted = if cfg!(windows) { "javaw.exe" } else { "java" };
    let entries = std::fs::read_dir(dir).ok()?;
    let mut sub_dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sub_dirs.push(path);
        } else if path.file_name().and_then(|n| n.to_str()) == Some(wanted)
            && path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) == Some("bin")
        {
            return Some(path.to_string_lossy().to_string());
        }
    }
    sub_dirs.iter().find_map(|d| find_java_binary(d))
}

/// Run `java -version` and parse the major version number.
fn system_java_major(java_path: &str) -> Option<u32> {
    // Use java.exe (not javaw) on Windows so `-version` prints to stderr we can read.
    let java_exe = if cfg!(windows) {
        java_path.replace("javaw.exe", "java.exe")
    } else {
        java_path.to_string()
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

    let mut parts = ver.split(['.', '-', '+', '_']);
    let first = parts.next()?;
    if first == "1" {
        parts.next()?.parse::<u32>().ok()
    } else {
        first.parse::<u32>().ok()
    }
}

/// Operating system and CPU names as Adoptium's API expects them.
fn adoptium_platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };
    (os, arch)
}

/// Download an Adoptium (Temurin) JRE for the given major version, extract it,
/// and return the path to its executable. Emits "Java" progress events.
async fn download_java(app: &tauri::AppHandle, target: u32) -> Result<String, String> {
    let client = http_client();
    let (os, arch) = adoptium_platform();

    // Try the JRE first, then the JDK. On Apple Silicon old Java (8) only exists
    // for Intel: fall back to the x64 build, which runs through Rosetta.
    let mut attempts: Vec<(&str, &str)> = vec![(arch, "jre"), (arch, "jdk")];
    if arch == "aarch64" && os == "mac" {
        attempts.push(("x64", "jre"));
        attempts.push(("x64", "jdk"));
    }

    let mut resp = None;
    let mut last_status = String::new();
    for (a, image) in attempts {
        let url = format!(
            "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/{}/hotspot/normal/eclipse",
            target, os, a, image
        );
        let r = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Impossibile scaricare Java {}: {}", target, e))?;
        if r.status().is_success() {
            resp = Some(r);
            break;
        }
        last_status = r.status().to_string();
    }
    let resp = resp.ok_or_else(|| {
        format!("Nessun runtime Java {} disponibile su Adoptium (HTTP {})", target, last_status)
    })?;

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut buf: Vec<u8> = Vec::with_capacity(total as usize);
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download di Java interrotto: {}", e))?;
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

    // Extract into a temporary folder and only then move it into place, so an
    // interrupted extraction never leaves a broken Java behind.
    let final_dir = managed_dir(target);
    let tmp_dir = final_dir.with_file_name(format!("java-{}.tmp", target));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("Impossibile creare la cartella di Java: {}", e))?;

    let extract_result = tauri::async_runtime::spawn_blocking({
        let tmp_dir = tmp_dir.clone();
        move || extract_archive(buf, &tmp_dir)
    })
    .await
    .map_err(|e| format!("Errore nell'estrazione di Java: {}", e))?;
    if let Err(e) = extract_result {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }

    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&tmp_dir, &final_dir)
        .map_err(|e| format!("Impossibile installare Java: {}", e))?;

    find_managed_java(target)
        .ok_or_else(|| "Eseguibile di Java non trovato dopo l'estrazione".to_string())
}

/// Adoptium ships .zip on Windows and .tar.gz on macOS/Linux: handle both.
fn extract_archive(data: Vec<u8>, dest: &Path) -> Result<(), String> {
    let is_gzip = data.len() > 2 && data[0] == 0x1f && data[1] == 0x8b;
    if is_gzip {
        let decoder = flate2::read::GzDecoder::new(Cursor::new(data));
        tar::Archive::new(decoder)
            .unpack(dest)
            .map_err(|e| format!("Errore nell'estrazione di Java: {}", e))
    } else {
        zip::ZipArchive::new(Cursor::new(data))
            .map_err(|e| format!("Archivio Java non valido: {}", e))?
            .extract(dest)
            .map_err(|e| format!("Errore nell'estrazione di Java: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_java_versions() {
        assert_eq!(parse_java_major(r#"java version "1.8.0_401""#), Some(8));
        assert_eq!(parse_java_major(r#"openjdk version "21.0.10" 2026-01-20"#), Some(21));
        assert_eq!(parse_java_major(r#"openjdk version "25" 2025-09-16"#), Some(25));
        assert_eq!(parse_java_major(r#"openjdk version "17-ea""#), Some(17));
        assert_eq!(parse_java_major("garbage"), None);
    }

    #[test]
    fn java_compatibility() {
        assert!(is_compatible(8, 8));
        assert!(!is_compatible(17, 8));
        assert!(is_compatible(21, 17));
        assert!(!is_compatible(17, 21));
    }
}
