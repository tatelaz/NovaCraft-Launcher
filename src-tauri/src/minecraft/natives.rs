use std::fs;
use std::io::Read;
use std::path::Path;

/// Extract native libraries from JAR files (which are ZIP archives).
///
/// Extracts all files except META-INF/ entries to the specified natives directory.
/// These are the .dll/.so/.dylib files that LWJGL and other native libraries need.
pub fn extract_natives(native_jars: &[std::path::PathBuf], natives_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(natives_dir)
        .map_err(|e| format!("Failed to create natives dir: {}", e))?;

    for jar_path in native_jars {
        if !jar_path.exists() {
            continue;
        }

        let file = fs::File::open(jar_path)
            .map_err(|e| format!("Failed to open native jar {:?}: {}", jar_path, e))?;

        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Failed to read zip {:?}: {}", jar_path, e))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read zip entry: {}", e))?;

            let name = entry.name().to_string();

            // Skip directories and META-INF entries
            if entry.is_dir() || name.starts_with("META-INF") {
                continue;
            }

            // Extract only binary/native files
            let out_path = natives_dir.join(&name);

            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|e| format!("Failed to read entry {}: {}", name, e))?;

            fs::write(&out_path, &data)
                .map_err(|e| format!("Failed to write native {:?}: {}", out_path, e))?;
        }
    }

    Ok(())
}
