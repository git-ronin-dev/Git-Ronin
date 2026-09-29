// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;
mod watcher;

use ronin_config::ConfigStore;
use tauri::Manager;

use crate::state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let (config, warnings) = ConfigStore::load(dir);
            app.manage(AppState::new(config, warnings));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::git_version,
            commands::config_get,
            commands::config_take_warnings,
            commands::config_set_ui,
            commands::open_repo,
            commands::restore_tabs,
            commands::close_repo,
            commands::set_active_tab,
            commands::repo_info,
            commands::list_refs,
            commands::graph_page,
            commands::graph_search,
            commands::graph_locate,
            commands::set_graph_filter,
            commands::commit_detail,
            commands::file_diff,
            commands::blob,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Git Ronin");
}
