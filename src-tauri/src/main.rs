// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::git_version,
            commands::open_repo,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Git Ronin");
}
