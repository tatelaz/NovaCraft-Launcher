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
    pub url: String,
}

/// A rule that determines whether a library (or a launch argument) applies.
#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    /// Launcher features (demo mode, custom resolution, quick play...).
    pub features: Option<HashMap<String, bool>>,
}

/// OS-specific conditions for a rule.
#[derive(Debug, Deserialize, Clone)]
pub struct OsRule {
    pub name: Option<String>,
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

/// Does a rule's `arch` condition match this computer?
/// Unknown values are treated as matching, so we never drop a library by mistake.
fn arch_matches(arch: &str) -> bool {
    match arch {
        "x86" => cfg!(target_arch = "x86"),
        "arm64" | "aarch64" => cfg!(target_arch = "aarch64"),
        "x86_64" | "amd64" | "x64" => cfg!(target_arch = "x86_64"),
        _ => true,
    }
}

fn rule_matches(rule: &Rule) -> bool {
    if let Some(os_rule) = &rule.os {
        if let Some(name) = &os_rule.name {
            if name != current_os_name() {
                return false;
            }
        }
        if let Some(arch) = &os_rule.arch {
            if !arch_matches(arch) {
                return false;
            }
        }
    }
    // NovaCraft enables none of the optional launcher features (demo mode,
    // custom resolution, quick play): a rule asking for one doesn't match.
    if let Some(features) = &rule.features {
        if features.values().any(|wanted| *wanted) {
            return false;
        }
    }
    true
}

/// Evaluate a list of rules. Rules are processed in order and the last
/// matching one wins; an empty list means "always allowed".
pub fn rules_allow(rules: &[Rule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule) {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

/// Evaluate library rules to determine if a library should be included.
/// If there are no rules, the library is always included.
pub fn is_library_allowed(rules: &Option<Vec<Rule>>) -> bool {
    match rules {
        Some(r) => rules_allow(r),
        None => true,
    }
}

/// Get all regular (non-native) libraries that should be downloaded for the current OS.
/// Returns pairs of (artifact info, local file path).
pub fn get_required_libraries(libraries: &[Library], base_dir: &Path) -> Vec<(LibraryArtifact, PathBuf)> {
    let libraries_dir = base_dir.join("libraries");
    libraries
        .iter()
        .filter(|lib| is_library_allowed(&lib.rules))
        .filter_map(|lib| lib.downloads.as_ref()?.artifact.clone())
        .map(|artifact| {
            let local_path = libraries_dir.join(&artifact.path);
            (artifact, local_path)
        })
        .collect()
}

/// Get native libraries (old-style "classifiers") for the current OS.
/// Returns pairs of (artifact info, local file path).
pub fn get_native_libraries(libraries: &[Library], base_dir: &Path) -> Vec<(LibraryArtifact, PathBuf)> {
    let libraries_dir = base_dir.join("libraries");
    let os_name = current_os_name();
    let pointer_width = if cfg!(target_pointer_width = "64") { "64" } else { "32" };
    let mut result = Vec::new();

    for lib in libraries {
        if !is_library_allowed(&lib.rules) {
            continue;
        }
        let Some(natives) = &lib.natives else { continue };
        let Some(classifier_key) = natives.get(os_name) else { continue };
        // Some old libraries use "natives-windows-${arch}" (32 / 64).
        let classifier_key = classifier_key.replace("${arch}", pointer_width);
        let native_artifact = lib
            .downloads
            .as_ref()
            .and_then(|d| d.classifiers.as_ref())
            .and_then(|c| c.get(&classifier_key));
        if let Some(native_artifact) = native_artifact {
            let local_path = libraries_dir.join(&native_artifact.path);
            result.push((native_artifact.clone(), local_path));
        }
    }

    result
}

/// Convert a maven coordinate into its relative jar path:
/// "group.id:name:version[:classifier][@ext]" -> "group/id/name/version/name-version[-classifier].ext"
pub fn maven_to_path(coord: &str) -> Option<String> {
    let (coord, ext) = match coord.split_once('@') {
        Some((c, e)) => (c, e),
        None => (coord, "jar"),
    };
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 || parts.iter().any(|p| p.is_empty() || p.contains("..")) {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let name = parts[1];
    let version = parts[2];
    let classifier = if parts.len() >= 4 {
        format!("-{}", parts[3])
    } else {
        String::new()
    };
    Some(format!(
        "{}/{}/{}/{}-{}{}.{}",
        group, name, version, name, version, classifier, ext
    ))
}

/// Identity of a library regardless of its version:
/// "group:name:version[:classifier]" -> "group:name[:classifier]".
pub fn library_key(coord: &str) -> String {
    let parts: Vec<&str> = coord.split('@').next().unwrap_or(coord).split(':').collect();
    match parts.len() {
        0..=2 => coord.to_string(),
        3 => format!("{}:{}", parts[0], parts[1]),
        _ => format!("{}:{}:{}", parts[0], parts[1], parts[3..].join(":")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_to_path("org.ow2.asm:asm:9.7.1").as_deref(),
            Some("org/ow2/asm/asm/9.7.1/asm-9.7.1.jar")
        );
        assert_eq!(
            maven_to_path("org.lwjgl:lwjgl:3.3.1:natives-windows").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar")
        );
        assert_eq!(maven_to_path("a:b:1@zip").as_deref(), Some("a/b/1/b-1.zip"));
        assert_eq!(maven_to_path("bad"), None);
        assert_eq!(maven_to_path("a:..:1"), None);
        assert_eq!(library_key("org.ow2.asm:asm:9.7.1"), "org.ow2.asm:asm");
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.3.1:natives-windows"),
            "org.lwjgl:lwjgl:natives-windows"
        );
    }

    fn rule(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn rules_os_and_features() {
        assert!(rules_allow(&[]));
        let other_os = if cfg!(windows) { "osx" } else { "windows" };
        assert!(!rules_allow(&rule(&format!(
            r#"[{{"action":"allow","os":{{"name":"{}"}}}}]"#,
            other_os
        ))));
        assert!(rules_allow(&rule(&format!(
            r#"[{{"action":"allow","os":{{"name":"{}"}}}}]"#,
            current_os_name()
        ))));
        // "allow everything except osx"
        let r = rule(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        assert_eq!(rules_allow(&r), current_os_name() != "osx");
        // Demo mode / custom resolution are never enabled
        assert!(!rules_allow(&rule(r#"[{"action":"allow","features":{"is_demo_user":true}}]"#)));
        assert!(!rules_allow(&rule(
            r#"[{"action":"allow","features":{"has_custom_resolution":true}}]"#
        )));
    }

    #[test]
    fn rules_arch() {
        let r = rule(r#"[{"action":"allow","os":{"arch":"x86"}}]"#);
        assert_eq!(rules_allow(&r), cfg!(target_arch = "x86"));
    }
}
