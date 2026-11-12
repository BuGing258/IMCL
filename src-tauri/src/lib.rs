pub mod auth;
mod commands;
pub mod game;
pub mod java;
pub mod maven;
pub mod modrinth;
pub mod net;
pub mod skin;
mod sysmon;
pub mod task;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::Installs::default())
        .manage(game::launch::Games::default())
        .manage(auth::Logins::default())
        .setup(|app| {
            sysmon::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::default_game_dir,
            commands::version_manifest,
            commands::loader_versions,
            commands::optifine_versions,
            commands::find_java,
            commands::install_version,
            commands::cancel_install,
            commands::list_installed,
            commands::version_files,
            commands::set_mod_enabled,
            commands::trash_version_file,
            commands::probe_java,
            commands::launch_game,
            commands::stop_game,
            commands::inspect_game_dir,
            commands::version_settings,
            commands::set_version_settings,
            commands::modrinth_search,
            commands::modrinth_versions,
            commands::modrinth_install,
            commands::modrinth_identify,
            commands::ms_login,
            commands::ms_login_cancel,
            commands::ms_accounts,
            commands::ms_remove,
            commands::default_skins,
            commands::ms_skin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
