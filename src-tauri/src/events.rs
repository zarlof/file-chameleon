//! Event names and payloads pushed from Rust to the webviews. Mirrored in `src/lib/types.ts`.

use serde::Serialize;

pub const JOB_UPDATE: &str = "job://update";
pub const DOWNLOAD_UPDATE: &str = "download://update";
pub const TOOL_PROGRESS: &str = "tool://progress";
pub const TOOL_DONE: &str = "tool://done";
pub const CLIP_ADDED: &str = "clip://added";
pub const SETTINGS_CHANGED: &str = "settings://changed";
/// Main window: navigate to a view (`NavPayload`).
pub const NAV_GOTO: &str = "nav://goto";
/// Overlay: switch to wheel mode (`WheelPayload`).
pub const OVERLAY_WHEEL: &str = "overlay://wheel";
/// Overlay: targets for the files that entered the wheel (`WheelTargets`).
pub const OVERLAY_WHEEL_TARGETS: &str = "overlay://wheel-targets";
/// Overlay: show a toast (`ToastPayload`).
pub const OVERLAY_TOAST: &str = "overlay://toast";
/// Overlay: hide everything.
pub const OVERLAY_HIDE: &str = "overlay://hide";

// ---- clipboard v2 and shelf (Task 9)
/// One clipboard item changed (OCR done, link title, edit, field): payload `ClipItem`.
pub const CLIP_UPDATED: &str = "clip://updated";
/// Bulk clipboard change (trash, restore, pins, folders, tags, retention): no payload, refetch.
pub const CLIP_CHANGED: &str = "clip://changed";
/// Shelf window: it was just shown (no payload).
pub const SHELF_OPEN: &str = "shelf://open";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolProgress {
    pub id: String,
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDone {
    pub id: String,
    pub ok: bool,
    pub error_code: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavPayload {
    /// "convert" | "download" | "clipboard" | "recent" | "settings"
    pub view: String,
    /// Files to preload (convert) or a URL (download).
    pub paths: Vec<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelPayload {
    /// Device pixel ratio of the monitor the wheel is on.
    pub scale: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelTargets {
    pub paths: Vec<String>,
    /// Kind of the first dragged file (centre icon), None if unknown.
    pub first_kind: Option<fc_core::formats::Kind>,
    pub targets: Vec<fc_core::formats::planner::TargetOption>,
    /// Recently used target ids that are valid here, newest first (max 7).
    pub recent: Vec<String>,
    /// Tool ids some target needs that are not installed (those slices show a download glyph).
    pub missing_tools: Vec<String>,
}

/// Toasts carry data, not text: the overlay localizes and picks actions per `kind`.
/// Kinds and their `data`:
/// - "jobDone"    { jobId, outputs: [path], inputName }      actions: reveal, open
/// - "jobFailed"  { jobId, inputName, errorCode, error }     actions: details (opens Recent)
/// - "jobsBatch"  { done, failed, total }                    actions: reveal (first output)
/// - "toolNeeded" { tool, name, size, paths, target }        actions: install (then converts
///   `paths` to `target`), dismiss
/// - "toolInstalling" { tool, name }                         no actions
/// - "toolFailed" { tool, name, errorCode, error }           no actions
/// - "clipLink"   { url, site }                              actions: download, dismiss
/// - "clipImage"  { clipId, width, height, thumb, image }    actions: savePng, convert
/// - "imageSaved" { output }                                 actions: reveal, open
/// - "downloadDone" { downloadId, output, title }            actions: reveal, open
/// - "noLink"     {}  (download-link hotkey, no media link copied)  no actions
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToastPayload {
    /// Unique per toast; sent back with `overlay_toast_action`.
    pub id: u64,
    pub kind: String,
    pub data: serde_json::Value,
    /// 0.0..=1.0 shows a progress bar.
    pub progress: Option<f32>,
    /// Auto-hide after this many ms (None = stays until dismissed or replaced).
    pub timeout_ms: Option<u64>,
}
