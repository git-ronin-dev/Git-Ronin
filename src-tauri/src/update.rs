//! Self-updates through the Tauri updater: signed bundles plus a
//! `latest.json` on GitHub Releases, so there is still no server of our own.
//!
//! Only the release workflow sets the updater's endpoint, so development
//! builds and builds from source never update themselves. Nor does the
//! Flatpak, which Flathub updates.

use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::utils::config::BundleType;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use ts_rs::TS;

use crate::state::{CmdResult, err, lock};

/// The update found by the last check, ready to install.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

/// This build and how it is updated.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    /// How the app was installed, e.g. "AppImage" or "Flatpak".
    pub install: String,
    /// Why the app can't update itself; `None` when it can.
    pub updates_unavailable: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AvailableUpdate {
    pub version: String,
    /// Release notes.
    pub notes: String,
    /// RFC 3339.
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum UpdateProgress {
    #[serde(rename_all = "camelCase")]
    Downloading {
        #[ts(type = "number")]
        downloaded: u64,
        #[ts(type = "number | null")]
        total: Option<u64>,
    },
    /// Download finished; the installer is running.
    Installing,
}

fn in_flatpak() -> bool {
    std::env::var_os("FLATPAK_ID").is_some() || Path::new("/.flatpak-info").exists()
}

fn install_kind() -> &'static str {
    if cfg!(debug_assertions) {
        return "Development build";
    }
    if in_flatpak() {
        return "Flatpak";
    }
    match tauri::utils::platform::bundle_type() {
        Some(BundleType::AppImage) => "AppImage",
        Some(BundleType::Deb) => "Debian package",
        Some(BundleType::Rpm) => "RPM package",
        Some(BundleType::Nsis) => "Windows installer",
        Some(BundleType::Msi) => "Windows Installer package",
        Some(BundleType::App | BundleType::Dmg) => "macOS app",
        None => "Built from source",
    }
}

fn unavailable(app: &AppHandle) -> Option<String> {
    if in_flatpak() {
        return Some("Flatpak installs are updated by Flathub or your software center.".into());
    }
    if tauri::utils::platform::bundle_type().is_none() || cfg!(debug_assertions) {
        return Some("This build was not installed from a release package.".into());
    }
    match app.updater() {
        Ok(_) => None,
        Err(tauri_plugin_updater::Error::EmptyEndpoints) => {
            Some("This build has no release channel to update from.".into())
        }
        Err(e) => Some(e.to_string()),
    }
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        install: install_kind().into(),
        updates_unavailable: unavailable(&app),
    }
}

/// Asks the release channel for a newer version.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> CmdResult<Option<AvailableUpdate>> {
    if let Some(reason) = unavailable(&app) {
        return Err(reason);
    }
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    let found = update.as_ref().map(|u| AvailableUpdate {
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
        date: u.date.and_then(|d| {
            d.format(&time::format_description::well_known::Rfc3339)
                .ok()
        }),
    });
    *lock(&app.state::<PendingUpdate>().0) = update;
    Ok(found)
}

/// Downloads and installs the update found by the last check, then
/// restarts. On Windows the installer closes the app itself.
#[tauri::command]
pub async fn update_install(app: AppHandle, on_progress: Channel<UpdateProgress>) -> CmdResult<()> {
    let update = lock(&app.state::<PendingUpdate>().0)
        .clone()
        .ok_or("Check for updates first.")?;
    let mut downloaded = 0u64;
    let mut reported = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                // About every 256 KiB, so the channel isn't flooded.
                if downloaded - reported >= 1 << 18 || Some(downloaded) == total {
                    reported = downloaded;
                    let _ = on_progress.send(UpdateProgress::Downloading { downloaded, total });
                }
            },
            || {
                let _ = on_progress.send(UpdateProgress::Installing);
            },
        )
        .await
        .map_err(err)?;
    // Linux packages ask for an administrator password (pkexec) here, and
    // the installer blocks the thread meanwhile.
    tauri::async_runtime::spawn_blocking(move || update.install(bytes))
        .await
        .map_err(err)?
        .map_err(err)?;
    app.restart()
}
