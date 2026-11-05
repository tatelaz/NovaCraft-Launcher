use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A library entry from the version JSON.
#[derive(Debug, Deserialize, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
}

/// Download information for a library.
#[derive(Debug, Deserialize, Clone)]
pub struct LibraryDownloads {
    pub artifact: Option<LibraryArtifact>,
    pub classifiers: Option<HashMap<String, LibraryArtifact>>,
}

/// A downloadable library artifact (JAR file).
#[derive(Debug, Deserialize, Clone)]
pub struct LibraryArtifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// A rule that determines whether a library is included for the current OS.
#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
}

/// OS-specific conditions for a rule.
#[derive(Debug, Deserialize, Clone)]
pub struct OsRule {
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

/// Get the current OS name in Mojang's format.
fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Evaluate library rules to determine if a library should be included.
/// Rules are processed sequentially — the last matching rule's action wins.
/// If there are no rules, the library is always included.
pub fn is_library_allowed(rules: &Option<Vec<Rule>>) -> bool {
    let rules = match rules {
        Some(r) => r,
        None => return true, // No rules = always allowed
    };

    let mut allowed = false;
    let os_name = current_os_name();

    for rule in rules {
        let mut matches = true;

        if let Some(os_rule) = &rule.os {
            if let Some(name) = &os_rule.name {
                if name != os_name {
                    matches = false;
                }
            }
            // We skip version/arch matching for simplicity in v1
        }

        if matches {
            allowed = rule.action == "allow";
        }
    }

    allowed
}

/// Get all regular (non-native) libraries that should be downloaded for the current OS.
/// Returns pairs of (artifact info, local file path).
pub fn get_required_libraries(
    libraries: &[Library],
    base_dir: &Path,
) -> Vec<(LibraryArtifact, PathBuf)> {
    let mut result = Vec::new();
    let libraries_dir = base_dir.join("libraries");

    for lib in libraries {
        if !is_library_allowed(&lib.rules) {
            continue;
        }

        if let Some(downloads) = &lib.downloads {
            if let Some(artifact) = &downloads.artifact {
                let local_path = libraries_dir.join(&artifact.path);
                result.push((artifact.clone(), local_path));
            }
        }
    }

    result
}

/// Get native libraries for the current OS.
/// Returns pairs of (artifact info, local file path).
pub fn get_native_libraries(
    libraries: &[Library],
    base_dir: &Path,
) -> Vec<(LibraryArtifact, PathBuf)> {
    let mut result = Vec::new();
    let libraries_dir = base_dir.join("libraries");
    let os_name = current_os_name();

    for lib in libraries {
        if !is_library_allowed(&lib.rules) {
            continue;
        }

        // Check if this library has natives for the current OS
        if let Some(natives) = &lib.natives {
            if let Some(classifier_key) = natives.get(os_name) {
                if let Some(downloads) = &lib.downloads {
                    if let Some(classifiers) = &downloads.classifiers {
                        if let Some(native_artifact) = classifiers.get(classifier_key) {
                            let local_path = libraries_dir.join(&native_artifact.path);
                            result.push((native_artifact.clone(), local_path));
                        }
                    }
                }
            }
        }
    }

    result
}
