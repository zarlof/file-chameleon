//! Tauri commands (the IPC surface). Mirrored by `src/lib/ipc.ts` and `src/lib/types.ts`.
//! Errors cross IPC as `CmdError { code, message }`.

pub mod clipboard;
pub mod downloads;

use crate::events::{self, NavPayload, ToolDone, ToolProgress};
use crate::state::AppState;
use crate::windows;
use fc_core::CancelToken;
use fc_core::formats;
use fc_core::formats::planner::{self, TargetOption};
use fc_core::jobs::{JobId, JobRequest, JobSnapshot, Operation};
use fc_core::settings::Settings;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdError {
    pub code: String,
    pub message: String,
}

impl From<fc_core::Error> for CmdError {
    fn from(e: fc_core::Error) -> Self {
        Self {
            code: e.code().into(),
            message: e.to_string(),
        }
    }
}

impl CmdError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub type CmdResult<T> = Result<T, CmdError>;

// ---------------------------------------------------------------- app & settings

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    /// "tr" | "en" resolved from the OS when the setting is "system".
    pub system_language: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS.into(),
        system_language: crate::platform::system_language(),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

/// Saves and applies side effects (autostart, wheel hook, clipboard listener).
#[tauri::command]
pub fn set_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Settings {
    let old = state.settings();
    state.save_settings(settings.clone());
    crate::platform::apply_settings(&app, &old, &settings);
    settings
}

// ---------------------------------------------------------------- convert

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputInfo {
    pub path: String,
    pub name: String,
    /// Detected format id, None if unknown.
    pub format: Option<String>,
    pub kind: Option<formats::Kind>,
    pub size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetsResponse {
    pub inputs: Vec<InputInfo>,
    pub targets: Vec<TargetOption>,
    /// Tools-panel operations valid for these inputs ("compress", "trim", "mute", "speed",
    /// "crop", "mergePdf", "splitPdf").
    pub tools: Vec<String>,
}

pub fn describe_inputs(paths: &[String]) -> Vec<InputInfo> {
    paths
        .iter()
        .map(|p| {
            let path = PathBuf::from(p);
            let f = formats::detect_path(&path);
            InputInfo {
                path: p.clone(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                format: f.map(|f| f.id.to_string()),
                kind: f.map(|f| f.kind),
                size: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            }
        })
        .collect()
}

pub fn targets_for_paths(paths: &[String]) -> (Vec<InputInfo>, Vec<TargetOption>) {
    let inputs = describe_inputs(paths);
    let ids: Vec<&str> = inputs.iter().filter_map(|i| i.format.as_deref()).collect();
    let targets = if ids.len() == inputs.len() {
        planner::targets_for(&ids)
    } else {
        Vec::new()
    };
    (inputs, targets)
}

fn tools_for(inputs: &[InputInfo]) -> Vec<String> {
    use formats::Kind::*;
    let kinds: Vec<_> = inputs.iter().map(|i| i.kind).collect();
    if kinds.is_empty() || kinds.iter().any(|k| k.is_none()) {
        return vec![];
    }
    let all = |f: &dyn Fn(formats::Kind) -> bool| kinds.iter().all(|k| f(k.unwrap()));
    let mut t = Vec::new();
    if all(&|k| matches!(k, Image | Video | Audio | Pdf)) {
        t.push("compress");
    }
    if all(&|k| matches!(k, Video | Audio)) {
        t.extend(["trim", "speed"]);
    }
    if all(&|k| k == Video) {
        t.push("mute");
    }
    if all(&|k| matches!(k, Image | Video)) && inputs.len() == 1 {
        t.push("crop");
    }
    if all(&|k| matches!(k, Pdf | Image)) && (inputs.len() > 1 || kinds[0] == Some(Pdf)) {
        t.push("mergePdf");
    }
    if all(&|k| k == Pdf) {
        t.push("splitPdf");
    }
    t.into_iter().map(String::from).collect()
}

#[tauri::command]
pub fn list_targets(paths: Vec<String>) -> TargetsResponse {
    let (inputs, targets) = targets_for_paths(&paths);
    let tools = tools_for(&inputs);
    TargetsResponse {
        inputs,
        targets,
        tools,
    }
}

#[tauri::command]
pub fn submit_job(state: State<'_, AppState>, mut req: JobRequest) -> CmdResult<Vec<JobId>> {
    if req.inputs.is_empty() {
        return Err(CmdError::new("unsupported", "no inputs"));
    }
    let mut s = state.settings();
    if req.options.output_dir.is_none() {
        req.options.output_dir = s.output_dir.clone();
    }
    if req.options.quality.is_none() {
        req.options.quality = Some(s.image_quality);
    }
    if let Operation::Convert { to } = &req.op {
        s.touch_recent_target(to);
        state.save_settings(s);
    }
    Ok(state.jobs().submit(req))
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, id: JobId) -> bool {
    state.jobs().cancel(id)
}

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> Vec<JobSnapshot> {
    state.jobs().snapshot()
}

#[tauri::command]
pub fn clear_jobs(state: State<'_, AppState>) {
    state.jobs().clear_finished()
}

/// Job listener: forward to the UI and, when the main window is not visible, toast results.
pub fn on_job_update(app: &AppHandle, snap: &JobSnapshot) {
    let _ = app.emit(events::JOB_UPDATE, snap);
    if snap.state.is_finished() {
        windows::toast_job_finished(app, snap);
    }
}

// ---------------------------------------------------------------- engines / tools

#[tauri::command]
pub fn tools_status(state: State<'_, AppState>) -> Vec<fc_core::engine_mgr::ToolStatus> {
    state.engines().status()
}

/// Installs in the background; progress via `tool://progress`, result via `tool://done`.
#[tauri::command]
pub fn install_tool(app: AppHandle, state: State<'_, AppState>, id: String) {
    let engines = state.engines().clone();
    std::thread::spawn(move || {
        let cancel = CancelToken::new();
        let last = std::sync::atomic::AtomicU64::new(0);
        let res = engines.install(
            &id,
            &|done, total| {
                // ~1% steps to keep IPC light.
                let prev = last.load(std::sync::atomic::Ordering::Relaxed);
                if total == 0 || done == total || done.saturating_sub(prev) * 100 >= total {
                    last.store(done, std::sync::atomic::Ordering::Relaxed);
                    let _ = app.emit(
                        events::TOOL_PROGRESS,
                        ToolProgress {
                            id: id.clone(),
                            done,
                            total,
                        },
                    );
                }
            },
            &cancel,
        );
        let (ok, code, msg) = match res {
            Ok(()) => (true, None, None),
            Err(e) => (false, Some(e.code().to_string()), Some(e.to_string())),
        };
        let _ = app.emit(
            events::TOOL_DONE,
            ToolDone {
                id,
                ok,
                error_code: code,
                error: msg,
            },
        );
    });
}

#[tauri::command]
pub fn uninstall_tool(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.engines().uninstall(&id).map_err(Into::into)
}

// Downloads: see `downloads.rs` (Task F). Clipboard: see `clipboard.rs` (Task G).

// ---------------------------------------------------------------- shell helpers

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

/// Bring up the main window, optionally navigating (used by the overlay and tray).
#[tauri::command]
pub fn show_main(
    app: AppHandle,
    view: Option<String>,
    paths: Option<Vec<String>>,
    url: Option<String>,
) {
    let nav = view.map(|view| NavPayload {
        view,
        paths: paths.unwrap_or_default(),
        url,
    });
    windows::show_main(&app, nav);
}

/// Navigation queued before the main window existed; the page reads it once on mount.
#[tauri::command]
pub fn take_pending_nav() -> Option<NavPayload> {
    windows::take_pending_nav()
}

// ---------------------------------------------------------------- overlay

#[tauri::command]
pub fn overlay_ready(app: AppHandle) {
    windows::overlay_ready(&app);
}

#[tauri::command]
pub fn overlay_hide(app: AppHandle) {
    windows::hide_overlay(&app);
}

/// A toast button was pressed. `action` ids are listed on `events::ToastPayload`.
#[tauri::command]
pub fn overlay_toast_action(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    action: String,
    data: serde_json::Value,
) {
    windows::toast_action(&app, &state, &kind, &action, &data);
}

/// Files were dropped on a wheel slice: convert them to `target` ("more" opens the main window).
#[tauri::command]
pub fn wheel_drop(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
    target: String,
) -> CmdResult<Vec<JobId>> {
    windows::hide_overlay(&app);
    if target == "more" {
        windows::show_main(
            &app,
            Some(NavPayload {
                view: "convert".into(),
                paths,
                url: None,
            }),
        );
        return Ok(vec![]);
    }
    let req = JobRequest {
        inputs: paths.into_iter().map(PathBuf::from).collect(),
        op: Operation::Convert { to: target },
        options: Default::default(),
    };
    submit_job(state, req)
}
