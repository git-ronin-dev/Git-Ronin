//! IPC commands for settings, profiles, config sharing, SSH keys,
//! workspaces and the terminal panel.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ronin_config::bundle::{self, ImportMode};
use ronin_config::merge::Change;
use ronin_config::sync::SyncStatus;
use ronin_config::{CheckUpdates, GitPrefs, Profile, SyncSettings, UiPrefs, Workspace};
use ronin_git::{Progress, RepoSummary};
use serde::Serialize;
use tauri::AppHandle;
use tauri::ipc::Channel;
use ts_rs::TS;

use crate::commands::{blocking, reporter};
use crate::ssh::{self, SshKey};
use crate::state::{AppState, CmdResult, err, lock};
use crate::terminal::TerminalEvent;

#[tauri::command]
pub async fn config_set_ui(app: AppHandle, ui: UiPrefs) -> CmdResult<()> {
    blocking(&app, move |_, s| s.update_portable(|p| p.ui = ui)).await
}

#[tauri::command]
pub async fn config_set_git(app: AppHandle, git: GitPrefs) -> CmdResult<()> {
    blocking(&app, move |_, s| s.update_portable(|p| p.git = git)).await
}

/// Replaces the shortcut overrides (command id → shortcut, "" unbinds).
#[tauri::command]
pub async fn config_set_keybindings(
    app: AppHandle,
    keybindings: BTreeMap<String, String>,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.update_portable(|p| p.keybindings = keybindings)
    })
    .await
}

/// Replaces the profiles. Returns a warning if the active one could not be
/// applied.
#[tauri::command]
pub async fn config_set_profiles(
    app: AppHandle,
    profiles: Vec<Profile>,
) -> CmdResult<Option<String>> {
    blocking(&app, move |_, s| {
        let mut portable = s.config().get().portable.clone();
        portable.profiles = profiles;
        let portable = portable.normalized();
        s.update_portable(|p| *p = portable)?;
        Ok(s.apply_profile())
    })
    .await
}

#[tauri::command]
pub async fn config_set_terminal_shell(app: AppHandle, shell: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.config()
            .update_local(|l| l.terminal_shell = shell)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn config_set_check_updates(app: AppHandle, check: bool) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.config()
            .update_local(|l| l.check_updates = CheckUpdates(check))
            .map_err(err)
    })
    .await
}

/// Switches profile. Every repository is closed; the UI then restores the
/// new profile's tabs. Returns a warning if the profile could not be applied.
#[tauri::command]
pub async fn profile_activate(app: AppHandle, id: String) -> CmdResult<Option<String>> {
    blocking(&app, move |_, s| {
        s.config().switch_profile(&id).map_err(err)?;
        lock(&s.repos).clear();
        Ok(s.apply_profile())
    })
    .await
}

/// Who git commits as without a profile, to show what empty fields mean.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Identity {
    pub name: Option<String>,
    pub email: Option<String>,
    pub signing_key: Option<String>,
}

#[tauri::command]
pub async fn git_identity(app: AppHandle) -> CmdResult<Identity> {
    blocking(&app, move |_, s| {
        let git = s.own_git()?;
        let home = lock(&s.profile).home().to_owned();
        let get = |key: &str| {
            git.run(&home, ["config", "--global", "--get", key])
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        Ok(Identity {
            name: get("user.name"),
            email: get("user.email"),
            signing_key: get("user.signingKey"),
        })
    })
    .await
}

#[tauri::command]
pub async fn settings_export(app: AppHandle, path: PathBuf) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let text = bundle::export(&s.config().get().portable).map_err(err)?;
        std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
    })
    .await
}

fn read_bundle(path: &PathBuf) -> CmdResult<ronin_config::Portable> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    bundle::parse(&text).map_err(err)
}

/// What importing the bundle at `path` would change.
#[tauri::command]
pub async fn settings_import_preview(
    app: AppHandle,
    path: PathBuf,
    mode: ImportMode,
) -> CmdResult<Vec<Change>> {
    blocking(&app, move |_, s| {
        let incoming = read_bundle(&path)?;
        bundle::preview(&s.config().get().portable, &incoming, mode).map_err(err)
    })
    .await
}

/// Imports the bundle at `path`. Returns a warning if the active profile
/// could not be applied afterwards.
#[tauri::command]
pub async fn settings_import(
    app: AppHandle,
    path: PathBuf,
    mode: ImportMode,
) -> CmdResult<Option<String>> {
    blocking(&app, move |_, s| {
        let incoming = read_bundle(&path)?;
        let current = s.config().get().portable.clone();
        let result = bundle::apply(&current, &incoming, mode).map_err(err)?;
        s.update_portable(|p| *p = result)?;
        Ok(s.apply_profile())
    })
    .await
}

#[tauri::command]
pub fn sync_status(app: AppHandle, state: tauri::State<'_, AppState>) -> SyncStatus {
    state.sync.status(&app)
}

/// Starts syncing the portable settings through `remote`, and syncs once
/// (asking for credentials if needed).
#[tauri::command]
pub async fn sync_enable(
    app: AppHandle,
    remote: String,
    branch: String,
    push_minutes: u32,
) -> CmdResult<SyncStatus> {
    blocking(&app, move |app, s| {
        let dir = s.config().dir().to_owned();
        let branch = branch.trim().to_owned();
        ronin_config::sync::enable(&s.git()?, &dir, &remote, &branch).map_err(err)?;
        s.config()
            .update_local(|l| {
                l.sync = Some(SyncSettings {
                    remote: remote.trim().to_owned(),
                    branch,
                    push_minutes,
                })
            })
            .map_err(err)?;
        // A failed first sync is shown in the status; sync stays on.
        let _ = s.sync.run(app, false);
        s.sync.restart();
        Ok(s.sync.status(app))
    })
    .await
}

/// Stops syncing. The remote repository is left as it is.
#[tauri::command]
pub async fn sync_disable(app: AppHandle) -> CmdResult<SyncStatus> {
    blocking(&app, move |app, s| {
        s.config().update_local(|l| l.sync = None).map_err(err)?;
        let dir = s.config().dir().to_owned();
        ronin_config::sync::disable(&dir).map_err(err)?;
        s.sync.clear();
        Ok(s.sync.status(app))
    })
    .await
}

#[tauri::command]
pub async fn sync_now(app: AppHandle) -> CmdResult<SyncStatus> {
    blocking(&app, move |app, s| {
        let _ = s.sync.run(app, false);
        Ok(s.sync.status(app))
    })
    .await
}

/// Settles conflicting settings: for each conflict in the status, `true`
/// takes the other machine's value.
#[tauri::command]
pub async fn sync_resolve(app: AppHandle, take_theirs: Vec<bool>) -> CmdResult<SyncStatus> {
    blocking(&app, move |app, s| {
        s.sync.resolve(app, &take_theirs)?;
        Ok(s.sync.status(app))
    })
    .await
}

#[tauri::command]
pub async fn ssh_keys(app: AppHandle) -> CmdResult<Vec<SshKey>> {
    blocking(&app, move |_, s| {
        let dir = lock(&s.profile).home().join(".ssh");
        Ok(ssh::list(&dir))
    })
    .await
}

#[tauri::command]
pub async fn ssh_generate(
    app: AppHandle,
    name: String,
    comment: String,
    passphrase: String,
) -> CmdResult<SshKey> {
    blocking(&app, move |_, s| {
        let dir = lock(&s.profile).home().join(".ssh");
        ssh::generate(&dir, &name, &comment, &passphrase)
    })
    .await
}

#[tauri::command]
pub async fn workspaces_set(app: AppHandle, workspaces: Vec<Workspace>) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.config()
            .update_local(|l| l.workspaces = workspaces)
            .map_err(err)
    })
    .await
}

/// The state of a repository that need not be open.
#[tauri::command]
pub async fn repo_summary(app: AppHandle, path: PathBuf) -> CmdResult<RepoSummary> {
    blocking(&app, move |_, s| {
        ronin_git::summary(&s.git()?, &path).map_err(err)
    })
    .await
}

/// Fetches all remotes of a repository that need not be open. Progress is
/// reported under `path`.
#[tauri::command]
pub async fn fetch_path(app: AppHandle, path: String) -> CmdResult<()> {
    blocking(&app, move |app, s| {
        let git = s.git()?;
        let mut report = reporter(app, &path);
        let progress: &mut dyn FnMut(Progress) = &mut report;
        match s.session(&path) {
            // An open repository serialises with its other ref-moving commands.
            Ok(session) => {
                let _serialised = lock(&session.journal);
                let result = ronin_git::fetch(&git, &session.path, None, progress);
                s.refs_moved(&path);
                result.map_err(err)
            }
            Err(_) => ronin_git::fetch(&git, path.as_ref(), None, progress).map_err(err),
        }
    })
    .await
}

/// Starts a shell in `cwd`; its output and exit arrive on `on_event`.
#[tauri::command]
pub async fn terminal_open(
    app: AppHandle,
    cwd: PathBuf,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
) -> CmdResult<u32> {
    blocking(&app, move |_, s| {
        let shell = s.config().get().local.terminal_shell.clone();
        let env = lock(&s.profile).env();
        s.terminals
            .open(&shell, &cwd, (cols, rows), env, move |event| {
                let _ = on_event.send(event);
            })
    })
    .await
}

#[tauri::command]
pub fn terminal_write(state: tauri::State<'_, AppState>, id: u32, data: String) -> CmdResult<()> {
    state.terminals.write(id, &data)
}

#[tauri::command]
pub fn terminal_resize(
    state: tauri::State<'_, AppState>,
    id: u32,
    cols: u16,
    rows: u16,
) -> CmdResult<()> {
    state.terminals.resize(id, cols, rows)
}

#[tauri::command]
pub fn terminal_close(state: tauri::State<'_, AppState>, id: u32) {
    state.terminals.close(id);
}
