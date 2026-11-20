//! Download commands and the in-memory download list. OWNER: Task F.

use super::{CmdError, CmdResult};
use crate::events;
use crate::state::AppState;
use crate::windows;
use fc_core::download::{
    self, DlProgress, DlState, DownloadRequest, DownloadSnapshot, LinkKind, MediaInfo,
};
use fc_core::{CancelToken, Error};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

/// Progress events are sent at most this often.
const EMIT_EVERY: Duration = Duration::from_millis(100);
/// Finished entries kept for the list (oldest are dropped first).
const KEEP_FINISHED: usize = 40;

struct Entry {
    snap: DownloadSnapshot,
    cancel: CancelToken,
    last_emit: Instant,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    items: Vec<Entry>,
}

/// Active + recent downloads.
#[derive(Default)]
pub struct Downloads {
    inner: Mutex<Inner>,
    /// Cancels the previous probe when a new link is checked.
    probe: Mutex<Option<CancelToken>>,
}

fn finished(s: DlState) -> bool {
    matches!(s, DlState::Done | DlState::Failed | DlState::Cancelled)
}

impl Downloads {
    fn add(&self, url: &str, title: String) -> (DownloadSnapshot, CancelToken) {
        let mut g = self.inner.lock().unwrap();
        g.next_id += 1;
        let snap = DownloadSnapshot {
            id: g.next_id,
            url: url.to_string(),
            title,
            state: DlState::Downloading,
            progress: None,
            speed_bps: None,
            eta_secs: None,
            output: None,
            error_code: None,
            error: None,
            created_at: fc_core::now_ms(),
        };
        let cancel = CancelToken::new();
        g.items.push(Entry {
            snap: snap.clone(),
            cancel: cancel.clone(),
            last_emit: Instant::now(),
        });
        // Drop the oldest finished entries beyond the cap.
        let done = g.items.iter().filter(|e| finished(e.snap.state)).count();
        for _ in KEEP_FINISHED..done {
            if let Some(i) = g.items.iter().position(|e| finished(e.snap.state)) {
                g.items.remove(i);
            }
        }
        (snap, cancel)
    }

    /// Apply `f` to download `id` and emit the new snapshot. `throttle` skips the event when
    /// the previous one was sent less than 100 ms ago (the change itself is always kept).
    fn update(
        &self,
        app: &AppHandle,
        id: u64,
        throttle: bool,
        f: impl FnOnce(&mut DownloadSnapshot),
    ) {
        let snap = {
            let mut g = self.inner.lock().unwrap();
            let Some(e) = g.items.iter_mut().find(|e| e.snap.id == id) else {
                return;
            };
            f(&mut e.snap);
            if throttle && e.last_emit.elapsed() < EMIT_EVERY {
                return;
            }
            e.last_emit = Instant::now();
            e.snap.clone()
        };
        let _ = app.emit(events::DOWNLOAD_UPDATE, snap);
    }

    fn cancel(&self, id: u64) -> bool {
        let g = self.inner.lock().unwrap();
        match g.items.iter().find(|e| e.snap.id == id) {
            Some(e) if !finished(e.snap.state) => {
                e.cancel.cancel();
                true
            }
            _ => false,
        }
    }

    fn snapshot(&self) -> Vec<DownloadSnapshot> {
        let g = self.inner.lock().unwrap();
        g.items.iter().map(|e| e.snap.clone()).collect()
    }
}

/// UI error code and short message. A missing tool carries its manifest id as the message.
fn error_parts(e: &Error) -> (String, String) {
    match e {
        Error::MissingTool(tool) => ("missingTool".into(), tool.clone()),
        other => (other.code().into(), other.to_string()),
    }
}

fn cmd_error(e: Error) -> CmdError {
    let (code, message) = error_parts(&e);
    CmdError { code, message }
}

#[tauri::command]
pub async fn probe_url(app: AppHandle, url: String) -> CmdResult<MediaInfo> {
    let state = app.state::<AppState>();
    let engines = state.engines().clone();
    let cookies = state.settings().cookies_browser;
    let cancel = CancelToken::new();
    if let Some(prev) = state.downloads.probe.lock().unwrap().replace(cancel.clone()) {
        prev.cancel();
    }
    tauri::async_runtime::spawn_blocking(move || {
        download::probe_any(engines.as_ref(), &url, cookies.as_deref(), &cancel)
    })
    .await
    .map_err(|e| CmdError::new("failed", e.to_string()))?
    .map_err(cmd_error)
}

#[tauri::command]
pub fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    req: DownloadRequest,
) -> CmdResult<u64> {
    let link = download::classify_url(&req.url)
        .ok_or_else(|| CmdError::new("unsupported", "not a web link"))?;
    let mut req = req;
    req.url = link.url.clone();
    if req.out_dir.as_os_str().is_empty() {
        req.out_dir = state.default_download_dir();
    }

    // Tell the UI up front which tool to offer for installation.
    let uses_ytdlp = !matches!(link.kind, LinkKind::DirectImage | LinkKind::DirectMedia);
    if uses_ytdlp {
        let engines = state.engines();
        if fc_core::engines::ToolLocator::locate(engines.as_ref(), "yt-dlp", "yt-dlp.exe").is_none()
        {
            return Err(CmdError::new("missingTool", "yt-dlp"));
        }
        if fc_core::engines::ToolLocator::locate(engines.as_ref(), "ffmpeg", "ffmpeg.exe").is_none()
        {
            return Err(CmdError::new("missingTool", "ffmpeg"));
        }
    }

    let (snap, cancel) = state.downloads.add(&req.url, req.url.clone());
    let id = snap.id;
    let _ = app.emit(events::DOWNLOAD_UPDATE, snap);

    std::thread::spawn(move || run(app, id, req, uses_ytdlp, cancel));
    Ok(id)
}

fn run(app: AppHandle, id: u64, req: DownloadRequest, uses_ytdlp: bool, cancel: CancelToken) {
    let state = app.state::<AppState>();
    if uses_ytdlp {
        // Keep yt-dlp current without delaying this download; failures are not interesting.
        let engines = state.engines().clone();
        let c = cancel.clone();
        std::thread::spawn(move || {
            let _ = engines.update_ytdlp_if_stale(&c);
        });
    }
    let staging = state.paths.staging_dir().join(format!("dl-{id}"));
    let cookies = state.settings().cookies_browser;
    let engines = state.engines().clone();
    let downloads = &state.downloads;

    let mut on_progress = |p: DlProgress| {
        downloads.update(&app, id, true, |s| {
            s.state = if p.processing {
                DlState::Processing
            } else {
                DlState::Downloading
            };
            s.progress = p.fraction.or(s.progress);
            s.speed_bps = p.speed_bps;
            s.eta_secs = p.eta_secs;
        });
    };
    let result = download::download_any(
        engines.as_ref(),
        &req,
        &staging,
        cookies.as_deref(),
        &mut on_progress,
        &cancel,
    );

    match result {
        Ok(path) => {
            let title = title_of(&path);
            downloads.update(&app, id, false, |s| {
                s.state = DlState::Done;
                s.progress = Some(1.0);
                s.speed_bps = None;
                s.eta_secs = None;
                s.title = title.clone();
                s.output = Some(path.clone());
            });
            if !windows::main_is_active(&app) {
                windows::show_toast(
                    &app,
                    "downloadDone",
                    json!({ "downloadId": id, "output": path, "title": title }),
                    Some(6000),
                );
            }
        }
        Err(Error::Cancelled) => downloads.update(&app, id, false, |s| {
            s.state = DlState::Cancelled;
            s.speed_bps = None;
            s.eta_secs = None;
        }),
        Err(e) => {
            let (code, message) = error_parts(&e);
            downloads.update(&app, id, false, |s| {
                s.state = DlState::Failed;
                s.speed_bps = None;
                s.eta_secs = None;
                s.error_code = Some(code);
                s.error = Some(message);
            });
        }
    }
}

fn title_of(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| PathBuf::from(path).display().to_string())
}

#[tauri::command]
pub fn cancel_download(state: State<'_, AppState>, id: u64) -> bool {
    state.downloads.cancel(id)
}

#[tauri::command]
pub fn list_downloads(state: State<'_, AppState>) -> Vec<DownloadSnapshot> {
    state.downloads.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tool_message_is_the_tool_id() {
        let (code, msg) = error_parts(&Error::MissingTool("yt-dlp".into()));
        assert_eq!((code.as_str(), msg.as_str()), ("missingTool", "yt-dlp"));
        let (code, _) = error_parts(&Error::Network("x".into()));
        assert_eq!(code, "network");
    }

    #[test]
    fn cap_keeps_recent_finished_entries() {
        let d = Downloads::default();
        for i in 0..(KEEP_FINISHED + 5) {
            let (snap, _) = d.add("https://x.com/a.mp4", format!("t{i}"));
            let mut g = d.inner.lock().unwrap();
            g.items
                .iter_mut()
                .find(|e| e.snap.id == snap.id)
                .unwrap()
                .snap
                .state = DlState::Done;
        }
        d.add("https://x.com/b.mp4", "active".into());
        let snaps = d.snapshot();
        assert!(snaps.len() <= KEEP_FINISHED + 1);
        assert_eq!(snaps.last().unwrap().title, "active");
    }

    #[test]
    fn cancel_only_active() {
        let d = Downloads::default();
        let (a, token) = d.add("https://x.com/a.mp4", "a".into());
        assert!(d.cancel(a.id));
        assert!(token.is_cancelled());
        assert!(!d.cancel(999));
    }
}
