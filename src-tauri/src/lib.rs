mod commands;
mod minecraft;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_versions,
            commands::download_version,
            commands::launch_game,
            commands::get_skin_url,
            commands::detect_java_cmd,
            commands::get_game_dir,
            commands::open_game_folder,
            commands::list_installed_versions,
            commands::close_launcher,
            commands::fetch_fabric_loaders_cmd,
            commands::install_fabric_cmd,
            commands::search_modrinth_cmd,
            commands::get_modrinth_versions_cmd,
            commands::download_mod_file_cmd,
            commands::list_installed_mods_cmd,
            commands::toggle_mod_cmd,
            commands::delete_mod_cmd,
            commands::open_subfolder_cmd,
            commands::save_custom_skin_cmd,
            commands::get_custom_skin_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
