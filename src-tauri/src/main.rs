// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod askpass;
mod commands;
mod hosting;
mod profile;
mod secrets;
mod settings;
mod ssh;
mod state;
mod sync;
mod terminal;
mod watcher;

use ronin_config::ConfigStore;
use tauri::{Manager, RunEvent};

use crate::profile::ProfileGit;
use crate::state::{AppState, lock};

fn main() {
    // Started by git or ssh to ask for a credential: answer and exit.
    if let Some(code) = askpass::client() {
        std::process::exit(code);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let (config, warnings) = ConfigStore::load(dir);
            let askpass = askpass::Askpass::start(app.handle().clone()).ok();
            let profile = ProfileGit::new(app.path().home_dir()?);
            app.manage(AppState::new(config, warnings, askpass, profile));
            let state = app.state::<AppState>();
            if let Some(warning) = state.apply_profile() {
                lock(&state.config_warnings).push(warning);
            }
            state.sync.start(app.handle().clone());
            if let Some(askpass) = &state.askpass {
                // Account tokens answer git's HTTPS prompts before the user is asked.
                let handle = app.handle().clone();
                askpass.set_resolver(Box::new(move |prompt| {
                    handle.state::<AppState>().git_credential(prompt)
                }));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::git_version,
            commands::config_get,
            commands::config_take_warnings,
            settings::config_set_ui,
            settings::config_set_git,
            settings::config_set_keybindings,
            settings::config_set_profiles,
            settings::config_set_terminal_shell,
            settings::profile_activate,
            settings::git_identity,
            settings::settings_export,
            settings::settings_import_preview,
            settings::settings_import,
            settings::sync_status,
            settings::sync_enable,
            settings::sync_disable,
            settings::sync_now,
            settings::sync_resolve,
            settings::ssh_keys,
            settings::ssh_generate,
            settings::workspaces_set,
            settings::repo_summary,
            settings::fetch_path,
            settings::terminal_open,
            settings::terminal_write,
            settings::terminal_resize,
            settings::terminal_close,
            hosting::hosting_accounts,
            hosting::hosting_client_id,
            hosting::hosting_sign_in,
            hosting::hosting_device_start,
            hosting::hosting_device_wait,
            hosting::hosting_device_cancel,
            hosting::hosting_sign_out,
            hosting::hosting_move_account,
            hosting::hosting_repos,
            hosting::hosting_fork,
            hosting::hosting_links,
            hosting::hosting_pull_requests,
            hosting::hosting_pull_request,
            hosting::hosting_create_pull_request,
            hosting::hosting_comment,
            hosting::hosting_approve,
            hosting::hosting_merge,
            hosting::hosting_issues,
            hosting::hosting_create_issue,
            hosting::hosting_ci_status,
            hosting::hosting_add_ssh_key,
            hosting::hosting_launchpad,
            hosting::pr_fetch,
            hosting::range_files,
            hosting::branch_issue,
            hosting::set_branch_issue,
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
            commands::working_status,
            commands::working_diff,
            commands::working_blob,
            commands::stage_files,
            commands::unstage_files,
            commands::discard_files,
            commands::apply_lines,
            commands::commit,
            commands::head_message,
            commands::stash_push,
            commands::stash_apply,
            commands::stash_drop,
            commands::add_to_gitignore,
            commands::clone_repo,
            commands::clone_name,
            commands::init_repo,
            commands::journal_state,
            commands::undo,
            commands::create_branch,
            commands::checkout_branch,
            commands::checkout_remote_branch,
            commands::checkout_detached,
            commands::rename_branch,
            commands::delete_branch,
            commands::move_branch,
            commands::set_upstream,
            commands::create_tag,
            commands::delete_tag,
            commands::add_remote,
            commands::edit_remote,
            commands::remove_remote,
            commands::fetch,
            commands::pull,
            commands::push_branch,
            commands::push_tag,
            commands::delete_remote_ref,
            commands::merge,
            commands::rebase,
            commands::cherry_pick,
            commands::revert,
            commands::reset,
            commands::resolve_operation,
            commands::pending_message,
            commands::conflict,
            commands::resolve_conflict,
            commands::rebase_plan,
            commands::interactive_rebase,
            commands::blame,
            commands::file_log,
            commands::add_submodule,
            commands::update_submodules,
            commands::list_worktrees,
            commands::add_worktree,
            commands::remove_worktree,
            commands::prune_worktrees,
            commands::lfs_status,
            commands::lfs_init,
            commands::lfs_track,
            commands::lfs_untrack,
            commands::lfs_locks,
            commands::lfs_lock,
            commands::lfs_unlock,
            commands::flow_config,
            commands::flow_init,
            commands::flow_start,
            commands::flow_finish,
            commands::bisect_state,
            commands::bisect_mark,
            commands::credential_respond,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Git Ronin")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<AppState>().sync.shutdown(app);
            }
        });
}
