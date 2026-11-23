//! Updates: the app asks GitHub for the newest release (`latest.json`, made by the release
//! workflow), offers it, and installs it with its signed NSIS installer (tauri-plugin-updater;
//! the public key is in `tauri.conf.json`, so only builds signed with our private key install).
//!
//! - At start (release builds, `update_check` on) one quiet check after [`FIRST_CHECK`]; a newer
//!   version shows the `updateAvailable` toast. Failing (offline, no release yet) says nothing.
//! - Settings > About: `update_check` (check now) and `update_install`.
//!
//! Installing runs the installer in passive mode (a small progress window) and the app exits;
//! the installer starts the new version.

use crate::ipc::{CmdError, CmdResult};
use crate::state::AppState;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// How long after start the automatic check waits (the app settles first).
const FIRST_CHECK: Duration = Duration::from_secs(20);

/// The update found by the last check, kept for "install".
static FOUND: Mutex<Option<Update>> = Mutex::new(None);

/// TS `UpdateInfo`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// The new version ("0.5.1").
    pub version: String,
    /// Release notes, if the release has any.
    pub notes: Option<String>,
}

fn found() -> std::sync::MutexGuard<'static, Option<Update>> {
    FOUND.lock().unwrap_or_else(|e| e.into_inner())
}

/// Ask for a newer version; remembers it for [`install`].
async fn check(app: &AppHandle) -> CmdResult<Option<UpdateInfo>> {
    let updater = app
        .updater()
        .map_err(|e| CmdError::new("update", e.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|e| CmdError::new("network", e.to_string()))?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
    });
    *found() = update;
    Ok(info)
}

/// Download and run the installer of the update found last (checking again if there is none).
/// On Windows the app exits while the installer runs.
async fn install(app: &AppHandle) -> CmdResult<()> {
    let pending = found().take();
    let update = match pending {
        Some(u) => u,
        None => {
            check(app).await?;
            found()
                .take()
                .ok_or_else(|| CmdError::new("notFound", "no update available"))?
        }
    };
    crate::windows::show_toast(
        app,
        "updateInstalling",
        serde_json::json!({ "version": update.version }),
        None,
    );
    let result = update.download_and_install(|_, _| {}, || {}).await;
    if let Err(e) = result {
        crate::windows::hide_overlay(app);
        return Err(CmdError::new("update", e.to_string()));
    }
    // Not reached on Windows (the installer closes the app); elsewhere start the new version.
    app.restart();
}

/// The quiet check at start: a toast when there is a newer version, nothing otherwise.
pub fn check_at_start(app: &AppHandle) {
    if cfg!(debug_assertions) || !app.state::<AppState>().settings().update_check {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        if let Ok(Some(info)) = tauri::async_runtime::block_on(check(&app)) {
            crate::windows::show_toast(
                &app,
                "updateAvailable",
                serde_json::json!({ "version": info.version }),
                Some(20_000),
            );
        }
    });
}

/// The `updateAvailable` toast's "Install" button.
pub fn install_from_toast(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = install(&app).await {
            eprintln!("update: {}", e.message);
            crate::windows::show_toast(&app, "updateFailed", serde_json::json!({}), Some(6000));
        }
    });
}

/// Settings > About: is there a newer version?
#[tauri::command]
pub async fn update_check(app: AppHandle) -> CmdResult<Option<UpdateInfo>> {
    check(&app).await
}

/// Settings > About: install the newer version (the app closes and the new one starts).
#[tauri::command]
pub async fn update_install(app: AppHandle) -> CmdResult<()> {
    install(&app).await
}
