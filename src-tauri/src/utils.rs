use md5::{Digest, Md5};
use std::env;
use std::path::{Path, PathBuf};

/// Generate an offline-mode UUID v3 from a username.
/// Uses the same algorithm as Minecraft servers with online-mode=false:
/// UUID = MD5("OfflinePlayer:<username>") with version/variant bits set.
pub fn offline_uuid(username: &str) -> String {
    let input = format!("OfflinePlayer:{}", username);
    let hash = Md5::digest(input.as_bytes());
    let mut bytes = hash.to_vec();

    // Set UUID version to 3 (MD5-based)
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    // Set UUID variant to IETF (RFC 4122)
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Get the NovaCraft data directory (%APPDATA%/NovaCraft on Windows).
/// Creates the directory if it doesn't exist.
pub fn get_novacraft_dir() -> PathBuf {
    let base = if cfg!(windows) {
        env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")))
    } else {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
    };

    let novacraft_dir = base.join("NovaCraft");
    if !novacraft_dir.exists() {
        let _ = std::fs::create_dir_all(&novacraft_dir);
    }
    novacraft_dir
}

/// The game directory of a profile: its own folder if it has one,
/// otherwise the main NovaCraft folder (the "Predefinito" profile).
pub fn game_dir(custom_dir: Option<&str>) -> PathBuf {
    match custom_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d),
        _ => get_novacraft_dir(),
    }
}

/// Keep only characters that are safe in a file name ([A-Za-z0-9_-]).
pub fn safe_file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if cleaned.is_empty() {
        "_".to_string()
    } else {
        cleaned
    }
}

/// Reduce a file name coming from outside (API, frontend) to a bare name,
/// so it can never point outside the intended folder.
pub fn bare_file_name(name: &str) -> Result<String, String> {
    let bare = Path::new(name)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    if bare.is_empty() || bare == "." || bare == ".." || bare != name {
        return Err(format!("Nome file non valido: {}", name));
    }
    Ok(bare)
}

/// Open a folder in the OS file manager (Explorer / Finder / file manager).
pub fn open_in_file_manager(path: &Path) -> Result<(), String> {
    let _ = std::fs::create_dir_all(path);

    #[cfg(windows)]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";

    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map_err(|e| format!("Impossibile aprire la cartella: {}", e))?;
    Ok(())
}

/// Every Java installation we can find on this computer (system ones, not the
/// runtimes NovaCraft downloads), without duplicates.
pub fn find_java_candidates() -> Vec<String> {
    let mut found: Vec<PathBuf> = Vec::new();
    let exe = java_executable();

    // JAVA_HOME first
    if let Ok(java_home) = env::var("JAVA_HOME") {
        found.push(PathBuf::from(java_home).join("bin").join(exe));
    }

    // Then everything on PATH
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(exe);
            // On macOS /usr/bin/java is only a stub that pops up an
            // "install Java" window when no JDK is present: skip it.
            if cfg!(target_os = "macos") && candidate == Path::new("/usr/bin/java") {
                continue;
            }
            found.push(candidate);
        }
    }

    // Common install folders (each vendor puts one sub-folder per version)
    let roots: &[&str] = if cfg!(windows) {
        &[
            "C:\\Program Files\\Java",
            "C:\\Program Files (x86)\\Java",
            "C:\\Program Files\\Eclipse Adoptium",
            "C:\\Program Files\\Microsoft",
            "C:\\Program Files\\Zulu",
            "C:\\Program Files\\BellSoft",
            "C:\\Program Files\\Amazon Corretto",
        ]
    } else if cfg!(target_os = "macos") {
        &["/Library/Java/JavaVirtualMachines"]
    } else {
        &["/usr/lib/jvm"]
    };
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let base = entry.path();
                found.push(base.join("bin").join(exe));
                found.push(base.join("Contents/Home/bin").join(exe)); // macOS layout
            }
        }
    }

    let mut result: Vec<String> = Vec::new();
    for p in found {
        if p.is_file() {
            let s = p.to_string_lossy().to_string();
            if !result.iter().any(|r| r.eq_ignore_ascii_case(&s)) {
                result.push(s);
            }
        }
    }
    result
}

/// Returns the platform-specific Java executable name.
fn java_executable() -> &'static str {
    if cfg!(windows) {
        "javaw.exe"
    } else {
        "java"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_minecraft() {
        // Value produced by Minecraft servers in offline mode for "Notch".
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
    }

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(safe_file_stem("Steve_01"), "Steve_01");
        assert_eq!(safe_file_stem("../evil"), "___evil");
        assert!(bare_file_name("sodium.jar").is_ok());
        assert!(bare_file_name("../sodium.jar").is_err());
        assert!(bare_file_name("a/b.jar").is_err());
        assert!(bare_file_name("").is_err());
    }
}
