use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Extract native libraries from JAR files (which are ZIP archives).
///
/// Extracts all files except META-INF/ entries to the specified natives directory.
/// These are the .dll/.so/.dylib files that LWJGL and other native libraries need.
pub fn extract_natives(native_jars: &[PathBuf], natives_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(natives_dir)
        .map_err(|e| format!("Impossibile creare la cartella dei componenti nativi: {}", e))?;

    for jar_path in native_jars {
        if !jar_path.exists() {
            continue;
        }

        let file = fs::File::open(jar_path)
            .map_err(|e| format!("Impossibile aprire {:?}: {}", jar_path, e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Archivio non valido {:?}: {}", jar_path, e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Impossibile leggere l'archivio {:?}: {}", jar_path, e))?;

            // `enclosed_name` rejects paths like "../../evil.dll".
            let Some(rel_path) = entry.enclosed_name() else { continue };
            if entry.is_dir() || rel_path.starts_with("META-INF") {
                continue;
            }
            let out_path = natives_dir.join(&rel_path);

            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|e| format!("Impossibile estrarre {:?}: {}", rel_path, e))?;

            // A running game may lock an existing .dll: if it's already there
            // with the same size, keep it.
            if let Ok(meta) = fs::metadata(&out_path) {
                if meta.len() == data.len() as u64 {
                    continue;
                }
            }
            fs::write(&out_path, &data)
                .map_err(|e| format!("Impossibile scrivere {:?}: {}", out_path, e))?;
        }
    }

    Ok(())
}
