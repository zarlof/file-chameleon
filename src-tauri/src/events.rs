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
/// Overlay: the slots for the files on the wheel (`WheelSlots`).
pub const OVERLAY_WHEEL_SLOTS: &str = "overlay://wheel-slots";
/// Overlay: flip front <-> back (mouse wheel / Tab caught by the global hooks). No payload.
pub const OVERLAY_WHEEL_FLIP: &str = "overlay://wheel-flip";
/// Overlay: small thumbnail of the first file when it is an image (`WheelThumb`).
pub const OVERLAY_WHEEL_THUMB: &str = "overlay://wheel-thumb";
/// Overlay: show a toast (`ToastPayload`).
pub const OVERLAY_TOAST: &str = "overlay://toast";
/// Overlay: hide everything.
pub const OVERLAY_HIDE: &str = "overlay://hide";
/// Overlay: a Ctrl+drag that started on the line moved (`WheelPointer`, page coordinates).
pub const OVERLAY_WHEEL_POINTER: &str = "overlay://wheel-pointer";
/// Overlay: that drag was released at a point (`WheelPointer`): run the slice there, if any.
pub const OVERLAY_WHEEL_DROP: &str = "overlay://wheel-drop";

/// Where the pointer is on the overlay page (logical pixels from its top left).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WheelPointer {
    pub x: f64,
    pub y: f64,
}

// ---- clipboard v2 (Task 9)
/// One clipboard item changed (OCR done, link title, edit, field): payload `ClipItem`.
pub const CLIP_UPDATED: &str = "clip://updated";
/// Bulk clipboard change (trash, restore, pins, folders, tags, retention): no payload, refetch.
pub const CLIP_CHANGED: &str = "clip://changed";
/// The files rope changed (recorded, removed, pruned, thumbnails made): no payload, refetch.
pub const FILES_CHANGED: &str = "files://changed";

// ---- clothesline
/// Line window: it was shown (`LineShow`).
pub const LINE_SHOW: &str = "line://show";
/// Line window: it was hidden (no payload).
pub const LINE_HIDE: &str = "line://hide";

/// `mode` is "hotkey" | "peek" | "reveal".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineShow {
    pub mode: String,
}

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
    /// A `View` of the frontend: "convert" | "download" | "clipboard" | "recent" | "line" (the İp
    /// settings page) | other settings pages | "editor".
    pub view: String,
    /// Files to preload (convert) or a URL (download).
    pub paths: Vec<String>,
    pub url: Option<String>,
    /// Clipboard item to open (clipboard view).
    #[serde(default)]
    pub clip: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelPayload {
    /// Device pixel ratio of the monitor the wheel is on.
    pub scale: f64,
    /// Click mode (context menu / "open the wheel"): no drag in progress.
    pub click: bool,
}

/// How the wheel looks (from the settings). Mirrors TS `WheelStyle`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelStyle {
    /// "auto" | "iconText" | "text" | "icon"
    pub labels: String,
    /// Percent, 40..=100.
    pub opacity: u8,
    /// Hovered-slice colour as "#RRGGBB".
    pub color: String,
    /// 0.8..=1.3
    pub scale: f64,
    pub show_detail: bool,
    pub reduce_motion: bool,
}

/// `overlay://wheel-slots`: what the wheel shows for the dragged / chosen files.
/// Mirrors TS `WheelSlots`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelSlots {
    pub paths: Vec<String>,
    pub count: usize,
    /// Lowercase extension of the first file ("jpg"), None for folders and extensionless files.
    pub first_ext: Option<String>,
    pub first_kind: Option<fc_core::formats::Kind>,
    pub profile_id: String,
    /// "" for built-in profiles.
    pub profile_name: String,
    pub front: Vec<fc_core::presets::WheelSlot>,
    pub back: Vec<fc_core::presets::WheelSlot>,
    /// Face to show first.
    pub face: fc_core::presets::Face,
    pub click: bool,
    /// Tool ids some slot needs that are not installed (those slots show a download badge).
    pub missing_tools: Vec<String>,
    pub style: WheelStyle,
}

/// `overlay://wheel-thumb`. `path` equals `WheelSlots.paths[0]`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelThumb {
    pub path: String,
    pub data_url: String,
}

/// Toasts carry data, not text: the overlay localizes and picks actions per `kind`.
/// Kinds and their `data`:
/// - "jobDone"    { jobId, outputs: [path], inputName }      actions: reveal, open
/// - "jobFailed"  { jobId, inputName, errorCode, error }     actions: details (opens Recent)
/// - "jobsBatch"  { done, failed, total }                    actions: reveal (first output)
/// - "toolNeeded" { tool, name, size, paths, target }        actions: install (then runs preset
///   `target` on `paths`), dismiss
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
