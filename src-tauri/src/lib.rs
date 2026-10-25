mod commands;
mod config;
mod github;
mod install;
mod launch;
mod self_update;
mod settings;

use tauri::Manager;

pub fn run() {
    let config = config::LauncherConfig::load();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(self_update::PendingUpdate::default())
        .setup(move |app| {
            let paths = app.path();
            let settings_path = paths.app_config_dir()?.join("settings.json");
            let default_root = paths.app_data_dir()?;
            app.manage(commands::AppState::new(config, settings_path, default_root));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_info,
            commands::list_releases,
            commands::get_status,
            commands::install,
            commands::play,
            commands::uninstall,
            commands::set_install_dir,
            commands::set_launch_options,
            commands::open_install_folder,
            commands::open_repo_page,
            commands::open_developer_page,
            self_update::check_launcher_update,
            self_update::install_launcher_update,
        ])
        .run(tauri::generate_context!())
        .expect("FlySim Launcher failed to start");
}
