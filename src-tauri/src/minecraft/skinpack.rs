//! Makes the custom skin visible *in game* (offline / single player).
//!
//! In offline mode Minecraft can't download your skin, so it gives you one of
//! the default skins (Steve, Alex...). We generate a tiny resource pack that
//! replaces all those default skins with yours, and switch it on in
//! options.txt. When there is no custom skin the pack is removed again.

use std::path::Path;

/// Folder name of the generated pack inside `resourcepacks/`.
const PACK_DIR: &str = "NovaCraft-Skin";

/// Default skin names (1.19.3+ has nine of them, in a wide and a slim variant).
const DEFAULT_SKINS: [&str; 9] = [
    "alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri",
];

/// Install (with `Some(skin)`) or remove (with `None`) the skin pack for a game folder.
/// `legacy` = Minecraft 1.12.2 or older, which names packs differently in options.txt.
pub fn apply(game_dir: &Path, skin_png: Option<&Path>, legacy: bool) -> Result<(), String> {
    let pack_dir = game_dir.join("resourcepacks").join(PACK_DIR);
    let option_name = if legacy {
        PACK_DIR.to_string()
    } else {
        format!("file/{}", PACK_DIR)
    };

    match skin_png {
        Some(skin) => {
            let data = std::fs::read(skin).map_err(|e| format!("Skin non leggibile: {}", e))?;
            write_pack(&pack_dir, &data)?;
            update_options(game_dir, &option_name, true)
        }
        None => {
            if pack_dir.exists() {
                let _ = std::fs::remove_dir_all(&pack_dir);
            }
            update_options(game_dir, &option_name, false)
        }
    }
}

fn write_pack(pack_dir: &Path, skin: &[u8]) -> Result<(), String> {
    let textures = pack_dir.join("assets").join("minecraft").join("textures").join("entity");
    let mut targets = vec![textures.join("steve.png"), textures.join("alex.png")]; // up to 1.19.2
    for variant in ["wide", "slim"] {
        for name in DEFAULT_SKINS {
            targets.push(textures.join("player").join(variant).join(format!("{}.png", name)));
        }
    }

    for target in targets {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Impossibile creare il pacchetto skin: {}", e))?;
        }
        std::fs::write(&target, skin)
            .map_err(|e| format!("Impossibile scrivere il pacchetto skin: {}", e))?;
    }

    // The pack is accepted by every version thanks to the
    // "incompatibleResourcePacks" entry written in options.txt below.
    let meta = r#"{"pack":{"pack_format":15,"description":"La tua skin NovaCraft"}}"#;
    std::fs::write(pack_dir.join("pack.mcmeta"), meta)
        .map_err(|e| format!("Impossibile scrivere pack.mcmeta: {}", e))
}

/// Add or remove our pack from `resourcePacks` and `incompatibleResourcePacks`
/// in options.txt, leaving every other setting untouched.
fn update_options(game_dir: &Path, entry: &str, enable: bool) -> Result<(), String> {
    let path = game_dir.join("options.txt");
    let original = std::fs::read_to_string(&path).unwrap_or_default();
    if original.is_empty() && !enable {
        return Ok(());
    }
    let updated = edit_options(&original, entry, enable);
    if updated != original {
        std::fs::write(&path, updated).map_err(|e| format!("Impossibile aggiornare options.txt: {}", e))?;
    }
    Ok(())
}

fn edit_options(original: &str, entry: &str, enable: bool) -> String {
    let mut lines: Vec<String> = original.lines().map(String::from).collect();

    for key in ["resourcePacks", "incompatibleResourcePacks"] {
        let prefix = format!("{}:", key);
        match lines.iter().position(|l| l.starts_with(&prefix)) {
            Some(idx) => {
                // Don't touch a line we can't understand.
                let Ok(mut packs) = serde_json::from_str::<Vec<String>>(&lines[idx][prefix.len()..]) else {
                    continue;
                };
                let had = packs.iter().any(|p| p == entry);
                if enable && !had {
                    packs.push(entry.to_string()); // last = highest priority
                } else if !enable && had {
                    packs.retain(|p| p != entry);
                } else {
                    continue;
                }
                lines[idx] = format!("{}{}", prefix, serde_json::to_string(&packs).unwrap_or_default());
            }
            None if enable => {
                let value = if key == "resourcePacks" && !entry.starts_with("file/") {
                    vec![entry.to_string()]
                } else if key == "resourcePacks" {
                    vec!["vanilla".to_string(), entry.to_string()]
                } else {
                    vec![entry.to_string()]
                };
                lines.push(format!("{}{}", prefix, serde_json::to_string(&value).unwrap_or_default()));
            }
            None => {}
        }
    }

    // Keep the file's own line endings (Minecraft writes CRLF on Windows).
    let newline = if original.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = lines.join(newline);
    out.push_str(newline);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_removes_pack() {
        let original = "version:3953\nresourcePacks:[\"vanilla\"]\nincompatibleResourcePacks:[]\nfov:0.0\n";
        let on = edit_options(original, "file/NovaCraft-Skin", true);
        assert!(on.contains("resourcePacks:[\"vanilla\",\"file/NovaCraft-Skin\"]"));
        assert!(on.contains("incompatibleResourcePacks:[\"file/NovaCraft-Skin\"]"));
        assert!(on.contains("fov:0.0"));
        // Applying twice changes nothing
        assert_eq!(edit_options(&on, "file/NovaCraft-Skin", true), on);
        let off = edit_options(&on, "file/NovaCraft-Skin", false);
        assert_eq!(off, original);
    }

    #[test]
    fn creates_options_when_missing() {
        let on = edit_options("", "file/NovaCraft-Skin", true);
        assert!(on.contains("resourcePacks:[\"vanilla\",\"file/NovaCraft-Skin\"]"));
        let legacy = edit_options("", "NovaCraft-Skin", true);
        assert!(legacy.contains("resourcePacks:[\"NovaCraft-Skin\"]"));
    }
}
