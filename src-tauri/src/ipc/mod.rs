//! Tauri commands (the IPC surface). Mirrored by `src/lib/ipc.ts` and `src/lib/types.ts`.
//! Errors cross IPC as `CmdError { code, message }`.
//!
//! Threading rule: a plain `#[tauri::command] fn` runs on the main thread, inside a WebView2
//! callback. Any command that may create a window (building a webview there deadlocks: wry waits
//! for the controller with a nested message pump that cannot complete), start processes, or do
//! image/file I/O is async: `#[tauri::command(async)]` for light work, `async fn` plus
//! [`blocking`] for heavy work.

pub mod clipboard;
pub mod downloads;
pub mod images;
pub mod line;
pub mod presets;

use crate::events::{self, NavPayload, ToolDone, ToolProgress};
use crate::state::AppState;
use crate::windows;
use fc_core::CancelToken;
use fc_core::formats;
use fc_core::formats::planner::{self, TargetOption};
use fc_core::jobs::{ConvertOptions, JobId, JobRequest, JobSnapshot, Operation};
use fc_core::settings::Settings;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

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

/// Run `f` on the blocking thread pool (never on the main thread) and wait for it.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CmdError::new("failed", e.to_string()))
}

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

/// Saves and applies side effects (autostart, wheel hook, clipboard listener, retention).
/// Returns what was actually stored: a failed autostart change is reverted, and the clipboard
/// pause state always keeps its stored value (it changes only through `clip_set_paused`).
/// Async: writes the file and the registry, and may prune the clipboard store.
#[tauri::command]
pub async fn set_settings(app: AppHandle, settings: Settings) -> CmdResult<Settings> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _tx = state.settings_transaction();
        let old = state.settings();
        let mut new = settings.clamped();
        new.clipboard_paused = old.clipboard_paused;
        let applied = crate::platform::apply_settings(&app, &old, new);
        state.save_settings(applied.clone());
        applied
    })
    .await
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
            // A folder is never a format, even when named like "photos.png".
            let f = formats::detect_path(&path).filter(|_| !path.is_dir());
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
    // Every input needs a known kind; otherwise no tool applies.
    let Some(kinds) = inputs.iter().map(|i| i.kind).collect::<Option<Vec<_>>>() else {
        return vec![];
    };
    if kinds.is_empty() {
        return vec![];
    }
    let all = |f: &dyn Fn(formats::Kind) -> bool| kinds.iter().all(|&k| f(k));
    // Input-only formats (ogv, amr, ...) cannot be re-encoded here, so the ffmpeg tools would
    // always fail on them.
    let ffmpeg_ok = !inputs
        .iter()
        .any(|i| i.format.as_deref().is_some_and(formats::is_input_only));
    let mut t = Vec::new();
    if ffmpeg_ok && all(&|k| matches!(k, Image | Video | Audio | Pdf)) {
        t.push("compress");
    }
    if ffmpeg_ok && all(&|k| matches!(k, Video | Audio)) {
        t.extend(["trim", "speed"]);
    }
    if ffmpeg_ok && all(&|k| k == Video) {
        t.push("mute");
    }
    if ffmpeg_ok && all(&|k| matches!(k, Image | Video)) && inputs.len() == 1 {
        t.push("crop");
    }
    if all(&|k| matches!(k, Pdf | Image)) && (inputs.len() > 1 || kinds[0] == Pdf) {
        t.push("mergePdf");
    }
    if all(&|k| k == Pdf) {
        t.push("splitPdf");
    }
    t.into_iter().map(String::from).collect()
}

/// The Convert view calls this for every file set it gets (drop or pick). It records nothing:
/// files join the files rope when a conversion is actually submitted (`submit_with`).
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

/// Fill in defaults from the settings, remember the target, and queue the jobs. Every way of
/// starting a conversion (Convert view, wheel, toast, install-then-convert) goes through here.
pub fn submit(state: &AppState, req: JobRequest) -> CmdResult<Vec<JobId>> {
    submit_with(state, req, true)
}

/// `submit`; `remember = false` (watch folders) leaves the recent targets alone.
pub fn submit_with(state: &AppState, mut req: JobRequest, remember: bool) -> CmdResult<Vec<JobId>> {
    if req.inputs.is_empty() {
        return Err(CmdError::new("unsupported", "no inputs"));
    }
    // Annotate and folderless copy/move are resolved before submitting (code "invalid").
    req.op.queueable()?;
    if remember {
        // Files the user put into a conversion by hand (not watch-folder runs), once the
        // request is known to be valid.
        crate::files::record(
            &state.app,
            req.inputs.clone(),
            fc_core::files::FileSource::Added,
        );
    }
    let s = state.settings();
    if req.options.output_dir.is_none() {
        req.options.output_dir = s.output_dir.clone();
    }
    if req.options.quality.is_none() {
        req.options.quality = Some(s.image_quality);
    }
    if let Operation::Convert { to } = &req.op
        && remember
    {
        let to = to.clone();
        state.update_settings(|s| s.touch_recent_target(&to));
    }
    let needs_downloads = req.options.output_dir.is_none()
        && req.inputs.iter().any(|p| state.paths.is_clipboard_file(p));
    let requests = if needs_downloads {
        let dir = state.default_download_dir();
        let _ = std::fs::create_dir_all(&dir);
        let name = format!("Clipboard {}", clipboard::local_stamp());
        split_clipboard_inputs(req, |p| state.paths.is_clipboard_file(p), &dir, &name)
    } else {
        vec![req]
    };
    Ok(requests
        .into_iter()
        .flat_map(|r| state.jobs().submit(r))
        .collect())
}

/// Clipboard images live in the app's hidden store under meaningless names. With no output
/// folder chosen, their results go to `dir` (the Downloads folder) named `name` (numbered when
/// taken); other inputs keep their usual place. A PDF merge or a ZIP follows its first input.
fn split_clipboard_inputs(
    req: JobRequest,
    is_clip: impl Fn(&Path) -> bool,
    dir: &Path,
    name: &str,
) -> Vec<JobRequest> {
    let clip_options = ConvertOptions {
        output_dir: Some(dir.to_path_buf()),
        output_name: Some(name.to_string()),
        ..req.options.clone()
    };
    if matches!(req.op, Operation::MergePdf | Operation::Zip) {
        let first_is_clip = req.inputs.first().is_some_and(|p| is_clip(p));
        return vec![if first_is_clip {
            JobRequest {
                options: clip_options,
                ..req
            }
        } else {
            req
        }];
    }
    let (clip, other): (Vec<PathBuf>, Vec<PathBuf>) =
        req.inputs.into_iter().partition(|p| is_clip(p));
    let mut out = Vec::with_capacity(2);
    if !other.is_empty() {
        out.push(JobRequest {
            inputs: other,
            op: req.op.clone(),
            options: req.options,
        });
    }
    if !clip.is_empty() {
        out.push(JobRequest {
            inputs: clip,
            op: req.op,
            options: clip_options,
        });
    }
    out
}

/// Async: may write the settings file (recent targets).
#[tauri::command(async)]
pub fn submit_job(state: State<'_, AppState>, req: JobRequest) -> CmdResult<Vec<JobId>> {
    submit(&state, req)
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
    if snap.state == fc_core::jobs::JobState::Done {
        crate::files::record(
            app,
            snap.outputs.clone(),
            fc_core::files::FileSource::Converted,
        );
    }
    if snap.state.is_finished() {
        windows::toast_job_finished(app, snap);
    }
}

// ---------------------------------------------------------------- engines / tools

/// Async: asks tools for their version (a process per tool; cached by file time).
#[tauri::command]
pub async fn tools_status(app: AppHandle) -> CmdResult<Vec<fc_core::engine_mgr::ToolStatus>> {
    blocking(move || app.state::<AppState>().engines().status()).await
}

/// Install tool `id` on the calling thread, reporting `tool://progress` and `tool://done`.
pub fn install_now(app: &AppHandle, id: &str) -> fc_core::Result<()> {
    let engines = app.state::<AppState>().engines().clone();
    let cancel = CancelToken::new();
    let last = std::sync::atomic::AtomicU64::new(0);
    let res = engines.install(
        id,
        &|done, total| {
            // ~1% steps to keep IPC light.
            let prev = last.load(std::sync::atomic::Ordering::Relaxed);
            if total == 0 || done == total || done.saturating_sub(prev) * 100 >= total {
                last.store(done, std::sync::atomic::Ordering::Relaxed);
                let _ = app.emit(
                    events::TOOL_PROGRESS,
                    ToolProgress {
                        id: id.to_string(),
                        done,
                        total,
                    },
                );
            }
        },
        &cancel,
    );
    let (ok, code, msg) = match &res {
        Ok(()) => (true, None, None),
        Err(e) => (false, Some(e.code().to_string()), Some(e.to_string())),
    };
    let _ = app.emit(
        events::TOOL_DONE,
        ToolDone {
            id: id.to_string(),
            ok,
            error_code: code,
            error: msg,
        },
    );
    res
}

/// Installs in the background; progress via `tool://progress`, result via `tool://done`.
#[tauri::command]
pub fn install_tool(app: AppHandle, id: String) {
    std::thread::spawn(move || {
        let _ = install_now(&app, &id);
    });
}

/// Install the missing `tools`, then submit `req` (Convert's "Download X and convert").
/// Runs in Rust so the conversion still starts if the window is closed during the download.
/// An install failure is reported with code "installFailed".
#[tauri::command]
pub async fn install_and_submit(
    app: AppHandle,
    tools: Vec<String>,
    req: JobRequest,
) -> CmdResult<Vec<JobId>> {
    blocking(move || {
        let state = app.state::<AppState>();
        for id in &tools {
            if !state.engines().is_available(id) {
                install_now(&app, id).map_err(|e| CmdError::new("installFailed", e.to_string()))?;
            }
        }
        submit(&state, req)
    })
    .await?
}

/// Check for a newer yt-dlp now and install it. Returns true when it was updated.
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> CmdResult<bool> {
    blocking(move || {
        app.state::<AppState>()
            .engines()
            .update_ytdlp(true, &CancelToken::new())
            .map_err(CmdError::from)
    })
    .await?
}

/// Async: deletes the tool's folder (hundreds of MB for FFmpeg).
#[tauri::command]
pub async fn uninstall_tool(app: AppHandle, id: String) -> CmdResult<()> {
    blocking(move || {
        app.state::<AppState>()
            .engines()
            .uninstall(&id)
            .map_err(CmdError::from)
    })
    .await?
}

// Downloads: see `downloads.rs` (Task F). Clipboard: see `clipboard.rs` (Task G).

// ---------------------------------------------------------------- shell helpers

/// Async: the shell may block (COM, Explorer).
#[tauri::command(async)]
pub fn reveal_path(app: AppHandle, path: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

/// Async: starts the default app for the file.
#[tauri::command(async)]
pub fn open_path(app: AppHandle, path: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

/// Bring up the main window, optionally navigating (used by the overlay and tray).
/// Async: it may create the main window.
#[tauri::command(async)]
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
        clip: None,
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
/// Async: actions may create the main window, save images or install tools.
#[tauri::command(async)]
pub fn overlay_toast_action(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    action: String,
    data: serde_json::Value,
) {
    windows::toast_action(&app, &state, &kind, &action, &data);
}

/// Files were dropped on (or clicked in) a wheel slot: run preset `target` on them (see
/// `presets::run_preset`: copy/move without a folder asks for one, annotate opens the editor,
/// a missing tool shows the `toolNeeded` toast). Async: it may create the main window or show
/// a folder picker.
#[tauri::command(async)]
pub fn wheel_drop(app: AppHandle, paths: Vec<String>, target: String) -> CmdResult<Vec<JobId>> {
    windows::hide_overlay(&app);
    presets::run_preset(
        &app,
        paths.into_iter().map(PathBuf::from).collect(),
        &target,
        true,
        None,
    )
}

/// Run a preset on files from the app (Convert view). Same rules as `wheel_drop`.
#[tauri::command(async)]
pub fn run_preset(app: AppHandle, paths: Vec<String>, preset_id: String) -> CmdResult<Vec<JobId>> {
    presets::run_preset(
        &app,
        paths.into_iter().map(PathBuf::from).collect(),
        &preset_id,
        true,
        None,
    )
}

/// Open the wheel in click mode at the cursor for these files. Async: builds the slots (file
/// checks) off the main thread.
#[tauri::command(async)]
pub fn wheel_open(app: AppHandle, paths: Vec<String>) {
    windows::open_wheel_click(&app, paths);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(inputs: &[&str], op: Operation) -> JobRequest {
        JobRequest {
            inputs: inputs.iter().map(PathBuf::from).collect(),
            op,
            options: ConvertOptions {
                quality: Some(80),
                ..Default::default()
            },
        }
    }

    fn input(format: &str, kind: formats::Kind) -> InputInfo {
        InputInfo {
            path: format!("a.{format}"),
            name: format!("a.{format}"),
            format: Some(format.into()),
            kind: Some(kind),
            size: 1,
        }
    }

    #[test]
    fn input_only_formats_get_no_ffmpeg_tools() {
        let ogv = tools_for(&[input("ogv", formats::Kind::Video)]);
        assert!(ogv.is_empty(), "{ogv:?}");
        let amr = tools_for(&[input("amr", formats::Kind::Audio)]);
        assert!(amr.is_empty(), "{amr:?}");
        let mp4 = tools_for(&[input("mp4", formats::Kind::Video)]);
        assert!(mp4.contains(&"compress".to_string()) && mp4.contains(&"mute".to_string()));
    }

    #[test]
    fn clipboard_inputs_go_to_downloads_with_a_readable_name() {
        let is_clip = |p: &Path| p.starts_with("C:/data/clipboard");
        let dl = Path::new("C:/Users/me/Downloads");
        let convert = Operation::Convert { to: "webp".into() };
        let out = split_clipboard_inputs(
            req(
                &["C:/x/a.png", "C:/data/clipboard/img/3.png", "C:/x/b.png"],
                convert.clone(),
            ),
            is_clip,
            dl,
            "Clipboard 2026-11-20 10.11.12",
        );
        assert_eq!(out.len(), 2);
        // Other files keep their place and name.
        assert_eq!(
            out[0].inputs,
            [PathBuf::from("C:/x/a.png"), "C:/x/b.png".into()]
        );
        assert_eq!(out[0].options.output_dir, None);
        assert_eq!(out[0].options.output_name, None);
        // The clipboard image goes to Downloads with a readable name; other options are kept.
        assert_eq!(
            out[1].inputs,
            [PathBuf::from("C:/data/clipboard/img/3.png")]
        );
        assert_eq!(out[1].options.output_dir.as_deref(), Some(dl));
        assert_eq!(
            out[1].options.output_name.as_deref(),
            Some("Clipboard 2026-11-20 10.11.12")
        );
        assert_eq!(out[1].options.quality, Some(80));
        assert_eq!(out[1].op, convert);

        // A merge is one job: it follows its first input.
        let merged = split_clipboard_inputs(
            req(
                &["C:/data/clipboard/img/3.png", "C:/x/a.png"],
                Operation::MergePdf,
            ),
            is_clip,
            dl,
            "Clipboard",
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].inputs.len(), 2);
        assert_eq!(merged[0].options.output_dir.as_deref(), Some(dl));
        let merged = split_clipboard_inputs(
            req(
                &["C:/x/a.png", "C:/data/clipboard/img/3.png"],
                Operation::MergePdf,
            ),
            is_clip,
            dl,
            "Clipboard",
        );
        assert_eq!(merged[0].options.output_dir, None);
        // So is a ZIP.
        let zipped = split_clipboard_inputs(
            req(
                &["C:/x/a.png", "C:/data/clipboard/img/3.png"],
                Operation::Zip,
            ),
            is_clip,
            dl,
            "Clipboard",
        );
        assert_eq!(zipped.len(), 1);
        assert_eq!(zipped[0].inputs.len(), 2);
    }
}
