pub mod auth;
pub mod backdrop;
mod commands;
pub mod game;
pub mod java;
pub mod jre;
pub mod maven;
pub mod memory;
pub mod modpack;
pub mod modpack_export;
pub mod modrinth;
pub mod net;
pub mod online_mod;
pub mod skin;
pub mod sysmon;
pub mod task;
pub mod ygg;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::Installs::default())
        .manage(game::launch::Games::default())
        .manage(auth::Logins::default())
        .setup(|app| {
            if let Ok(dir) = tauri::Manager::path(app).app_data_dir() {
                jre::set_root(dir.join("java"));
                std::thread::spawn(move || {
                    let _ = ygg::load_key(&dir);
                });
            }
            sysmon::spawn(app.handle().clone());
            // 窗口一开始是隐藏的，等页面画好、云母打开后由前端显示（见 +page.svelte）；
            // 万一前端出错没显示，3 秒后这里兜底显示
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(3));
                if let Some(w) = tauri::Manager::get_webview_window(&handle, "main") {
                    if !w.is_visible().unwrap_or(true) {
                        let _ = w.show();
                    }
                }
            });
            Ok(())
        })
        // 开着云母时，窗口状态一变就确认系统按钮没有冒出来（见 backdrop.rs）
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(_)) {
                backdrop::refresh(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::default_game_dir,
            commands::memory_plan,
            commands::set_backdrop,
            commands::version_manifest,
            commands::loader_versions,
            commands::optifine_versions,
            commands::find_java,
            commands::java_offers,
            commands::download_java,
            commands::install_version,
            commands::cancel_install,
            commands::list_installed,
            commands::delete_version,
            commands::online_mod_status,
            commands::install_online_mod,
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
            commands::modrinth_deps,
            commands::install_modpack,
            commands::change_loader,
            commands::modrinth_updates,
            commands::read_modpack,
            commands::export_entries,
            commands::export_modpack,
            commands::modrinth_install,
            commands::modrinth_identify,
            commands::ms_login,
            commands::ms_login_cancel,
            commands::ms_accounts,
            commands::ms_remove,
            commands::default_skins,
            commands::set_background_image,
            commands::background_image,
            commands::is_win11,
            commands::sysmon_watch,
            commands::ms_skin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
