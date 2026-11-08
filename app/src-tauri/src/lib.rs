pub mod commands;
pub mod config;
pub mod describe;
pub mod detect;
pub mod edition;
pub mod errors;
pub mod games;
pub mod job;
pub mod library;
pub mod presets;
pub mod settings;
pub mod share;
pub mod spoiler;
pub mod state;

use tauri::Manager;

use crate::state::{AppDirs, AppState};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = app.path();
            let dirs = AppDirs {
                config_file: paths.app_config_dir()?.join("config.json"),
                app_data: paths.app_data_dir()?,
                appdata: std::env::var_os("APPDATA").map(Into::into),
            };
            app.manage(AppState::new(dirs));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_environment,
            commands::save_config,
            commands::get_registry,
            commands::normalize_settings,
            commands::describe_changes,
            commands::load_settings_file,
            commands::save_settings_file,
            commands::encode_share_code,
            commands::decode_share_code,
            commands::list_species,
            commands::pool_counts,
            commands::randomize,
            commands::cancel_randomize,
            commands::list_seeds,
            commands::check_switch,
            commands::activate_seed,
            commands::deactivate,
            commands::delete_seed,
            commands::open_path,
            commands::launch_eden,
        ])
        .run(tauri::generate_context!())
        .expect("the app window failed to start");
}
