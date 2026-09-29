//! IPC commands exposed to the UI. Errors cross the boundary as plain messages.

use std::path::PathBuf;

use ronin_git::{GitCli, GitVersion, RepoInfo};
use tauri::State;

pub struct AppState {
    /// Resolved once at startup so the UI can show a banner if git is missing.
    git: Result<GitCli, String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            git: GitCli::discover().map_err(|e| e.to_string()),
        }
    }
}

#[tauri::command]
pub fn git_version(state: State<'_, AppState>) -> Result<GitVersion, String> {
    state
        .git
        .as_ref()
        .map(GitCli::version)
        .map_err(Clone::clone)
}

#[tauri::command]
pub async fn open_repo(path: PathBuf) -> Result<RepoInfo, String> {
    // Discovery touches the disk; keep it off the main thread.
    tauri::async_runtime::spawn_blocking(move || ronin_git::open_repo(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
