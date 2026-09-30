//! Repositories named when the app is started: `git-ronin [path…]`.
//!
//! Only one instance runs. Starting the app again hands its paths to the
//! running one (through `tauri-plugin-single-instance`), which opens them
//! and comes to the front. macOS hands over files opened from Finder as an
//! `Opened` event instead.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::lock;

/// Paths from this instance's own command line, until the UI takes them.
#[derive(Default)]
pub struct LaunchPaths(Mutex<Vec<String>>);

impl LaunchPaths {
    pub fn from_args() -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self(Mutex::new(paths_from(std::env::args().skip(1), &cwd)))
    }
}

/// The paths among command-line arguments (options are skipped), made
/// absolute against the directory the command ran in.
pub fn paths_from(args: impl IntoIterator<Item = String>, cwd: &Path) -> Vec<String> {
    args.into_iter()
        .filter(|a| !a.is_empty() && !a.starts_with('-'))
        .map(|a| {
            let path = PathBuf::from(&a);
            let path = if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            };
            path.to_string_lossy().into_owned()
        })
        .collect()
}

/// The paths the app was started with; empty after the first call.
#[tauri::command]
pub fn take_launch_paths(paths: State<'_, LaunchPaths>) -> Vec<String> {
    std::mem::take(&mut *lock(&paths.0))
}

/// Another start of the app: open its paths here and show the window.
pub fn second_instance(app: &AppHandle, args: Vec<String>, cwd: String) {
    open(app, paths_from(args.into_iter().skip(1), Path::new(&cwd)));
}

/// Asks the UI to open `paths` (it reports failures) and brings the window up.
pub fn open(app: &AppHandle, paths: Vec<String>) {
    if !paths.is_empty() {
        let _ = app.emit("open-paths", paths);
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_paths_and_resolves_them_against_the_working_directory() {
        let cwd = Path::new("/work");
        let args = ["--verbose", "ronin", "/abs/repo", "", "-v", "../other"];
        let paths = paths_from(args.map(String::from), cwd);
        let expected = [
            Path::new("/work").join("ronin"),
            PathBuf::from("/abs/repo"),
            Path::new("/work").join("../other"),
        ];
        assert_eq!(paths, expected.map(|p| p.to_string_lossy().into_owned()));
    }
}
