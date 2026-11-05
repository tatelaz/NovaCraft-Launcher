use md5::{Md5, Digest};
use std::env;
use std::path::PathBuf;

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

/// Detect a Java installation on the system.
/// Checks JAVA_HOME first, then searches PATH for javaw.exe (Windows) or java.
pub fn detect_java() -> Option<String> {
    // Check JAVA_HOME environment variable first
    if let Ok(java_home) = env::var("JAVA_HOME") {
        let java_home = PathBuf::from(&java_home);
        let javaw = java_home.join("bin").join(java_executable());
        if javaw.exists() {
            return Some(javaw.to_string_lossy().to_string());
        }
    }

    // Search PATH for java executable
    if let Ok(path_var) = env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(separator) {
            let java_path = PathBuf::from(dir).join(java_executable());
            if java_path.exists() {
                return Some(java_path.to_string_lossy().to_string());
            }
        }
    }

    // Check common Windows installation paths
    if cfg!(windows) {
        let common_paths = [
            "C:\\Program Files\\Java",
            "C:\\Program Files (x86)\\Java",
            "C:\\Program Files\\Eclipse Adoptium",
            "C:\\Program Files\\Microsoft\\jdk-",
        ];
        for base in &common_paths {
            let base_path = PathBuf::from(base);
            if base_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&base_path) {
                    for entry in entries.flatten() {
                        let javaw = entry.path().join("bin").join("javaw.exe");
                        if javaw.exists() {
                            return Some(javaw.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    // Check common macOS installation paths
    if cfg!(target_os = "macos") {
        let mac_base = PathBuf::from("/Library/Java/JavaVirtualMachines");
        if mac_base.exists() {
            if let Ok(entries) = std::fs::read_dir(&mac_base) {
                for entry in entries.flatten() {
                    let java_bin = entry.path().join("Contents/Home/bin/java");
                    if java_bin.exists() {
                        return Some(java_bin.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Returns the platform-specific Java executable name.
fn java_executable() -> &'static str {
    if cfg!(windows) { "javaw.exe" } else { "java" }
}
