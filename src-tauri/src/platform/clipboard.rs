//! Windows clipboard listener for the smart clipboard. OWNER: Task G.
//! `start` spawns the listener thread; `set_enabled` turns capture on/off at runtime.
//!
//! The thread blocks in `Monitor::recv` (a `WM_CLIPBOARDUPDATE` message loop), so it costs nothing
//! while idle. Disabling capture only makes it ignore events; it stays parked.

use tauri::AppHandle;

#[cfg(windows)]
pub fn start(app: &AppHandle) {
    imp::ENABLED.store(true, std::sync::atomic::Ordering::SeqCst);
    imp::spawn(app);
}

#[cfg(windows)]
pub fn set_enabled(app: &AppHandle, enabled: bool) {
    imp::ENABLED.store(enabled, std::sync::atomic::Ordering::SeqCst);
    if enabled {
        imp::spawn(app);
    }
}

#[cfg(not(windows))]
pub fn start(app: &AppHandle) {
    let _ = app;
}

#[cfg(not(windows))]
pub fn set_enabled(app: &AppHandle, enabled: bool) {
    let _ = (app, enabled);
}

/// Put text on the clipboard (used by `clip_copy`).
#[cfg(windows)]
pub fn write_text(text: &str) -> Result<u32, String> {
    imp::write_text(text)
}

/// Put an image on the clipboard as `CF_DIB` + the registered "PNG" format (used by `clip_copy`).
#[cfg(windows)]
pub fn write_image(png: &[u8]) -> Result<u32, String> {
    imp::write_image(png)
}

#[cfg(not(windows))]
pub fn write_text(_text: &str) -> Result<u32, String> {
    Err("clipboard is only supported on Windows".into())
}

#[cfg(not(windows))]
pub fn write_image(_png: &[u8]) -> Result<u32, String> {
    Err("clipboard is only supported on Windows".into())
}

#[cfg(windows)]
mod imp {
    use crate::events;
    use crate::state::AppState;
    use clipboard_win::formats::{CF_DIB, CF_DIBV5, CF_HDROP, Unicode};
    use clipboard_win::{Clipboard, Getter, Monitor, raw, register_format};
    use fc_core::clip::ClipKind;
    use fc_core::clip::history::{self, ClipItem};
    use fc_core::settings::Settings;
    use serde_json::json;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tauri::{AppHandle, Emitter, Manager};

    pub static ENABLED: AtomicBool = AtomicBool::new(false);
    static RUNNING: AtomicBool = AtomicBool::new(false);

    const DEBOUNCE: Duration = Duration::from_millis(120);
    const OPEN_RETRIES: u32 = 5;

    struct Formats {
        exclude_monitor: u32,
        viewer_ignore: u32,
        can_include_history: u32,
        can_upload_cloud: u32,
        png: u32,
    }

    fn formats() -> &'static Formats {
        static F: OnceLock<Formats> = OnceLock::new();
        F.get_or_init(|| {
            let reg = |n: &str| register_format(n).map_or(0, |v| v.get());
            Formats {
                exclude_monitor: reg("ExcludeClipboardContentFromMonitorProcessing"),
                viewer_ignore: reg("Clipboard Viewer Ignore"),
                can_include_history: reg("CanIncludeInClipboardHistory"),
                can_upload_cloud: reg("CanUploadToCloudClipboard"),
                png: reg("PNG"),
            }
        })
    }

    pub fn spawn(app: &AppHandle) {
        if RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("clipboard-listener".into())
            .spawn(move || {
                listen(&app);
                RUNNING.store(false, Ordering::SeqCst);
            });
        if spawned.is_err() {
            RUNNING.store(false, Ordering::SeqCst);
        }
    }

    fn listen(app: &AppHandle) {
        let Ok(mut monitor) = Monitor::new() else {
            eprintln!("clipboard: cannot create listener window");
            return;
        };
        while let Ok(true) = monitor.recv() {
            let state = app.state::<AppState>();
            if !ENABLED.load(Ordering::SeqCst) || state.clip_paused.load(Ordering::SeqCst) {
                continue;
            }
            // Debounce: many apps publish several updates for one copy.
            std::thread::sleep(DEBOUNCE);
            while let Ok(true) = monitor.try_recv() {}
            // Our own write (any number of updates it caused) ends at the stored sequence number.
            let own = state.clip_self_write.load(Ordering::SeqCst);
            if own != 0 && seq() == own {
                continue;
            }
            // A panic must never kill capture for the rest of the session.
            let _ = catch_unwind(AssertUnwindSafe(|| handle(app, &state)));
        }
    }

    enum Captured {
        Text(String),
        Image { png: Vec<u8>, w: u32, h: u32 },
    }

    fn open_with_retry() -> Option<Clipboard> {
        let mut delay = 15u64;
        for attempt in 0..=OPEN_RETRIES {
            if let Ok(c) = Clipboard::new() {
                return Some(c);
            }
            if attempt < OPEN_RETRIES {
                std::thread::sleep(Duration::from_millis(delay));
                delay *= 2;
            }
        }
        None
    }

    /// True if the content owner asked not to be recorded (password managers, etc.).
    /// Clipboard must be open.
    fn excluded() -> bool {
        let f = formats();
        for fmt in [f.exclude_monitor, f.viewer_ignore] {
            if fmt != 0 && raw::is_format_avail(fmt) {
                return true;
            }
        }
        for fmt in [f.can_include_history, f.can_upload_cloud] {
            if fmt != 0 && raw::is_format_avail(fmt) {
                let mut v = Vec::new();
                if raw::get_vec(fmt, &mut v).is_ok() && v.len() >= 4 && v[..4] == [0, 0, 0, 0] {
                    return true;
                }
            }
        }
        false
    }

    fn read_image(images: bool) -> Option<Captured> {
        if !images {
            return None;
        }
        let f = formats();
        if f.png != 0 && raw::is_format_avail(f.png) {
            let mut png = Vec::new();
            if raw::get_vec(f.png, &mut png).is_ok()
                && let Some((w, h)) = history::png_size(&png)
            {
                if u64::from(w) * u64::from(h) > history::MAX_IMAGE_PIXELS {
                    return None;
                }
                return Some(Captured::Image { png, w, h });
            }
        }
        for fmt in [CF_DIBV5, CF_DIB] {
            if !raw::is_format_avail(fmt) {
                continue;
            }
            let mut dib = Vec::new();
            if raw::get_vec(fmt, &mut dib).is_err() {
                continue;
            }
            match history::dib_to_png(&dib) {
                Ok((png, w, h)) => return Some(Captured::Image { png, w, h }),
                Err(fc_core::Error::TooLarge(_)) => return None,
                Err(_) => {}
            }
        }
        None
    }

    fn read(images: bool) -> Option<Captured> {
        let _guard = open_with_retry()?;
        if excluded() || raw::is_format_avail(CF_HDROP) {
            return None;
        }
        if raw::is_format_avail(CF_UNICODETEXT) {
            let mut s = String::new();
            if Unicode.read_clipboard(&mut s).is_ok() && !s.trim().is_empty() {
                return Some(Captured::Text(s));
            }
        }
        read_image(images)
    }

    const CF_UNICODETEXT: u32 = clipboard_win::formats::CF_UNICODETEXT;

    fn handle(app: &AppHandle, state: &AppState) {
        let settings = state.settings();
        if !settings.clipboard_enabled {
            return;
        }
        let Some(captured) = read(settings.clipboard_images) else {
            return;
        };
        let now = fc_core::now_ms();
        let item = {
            let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
            let first_before = store.items().first().map(|i| i.id);
            let added = match &captured {
                Captured::Text(t) => store.add_text(t, now),
                Captured::Image { png, w, h } => store.add_image_png(png, *w, *h, now),
            };
            let Ok(Some(item)) = added else { return };
            let _ = store.prune(
                now,
                settings.clipboard_retention_days,
                settings.clipboard_max_items as usize,
            );
            // The same content copied again while it is already on top: nothing new to announce.
            if first_before == Some(item.id) {
                return;
            }
            item
        };
        let _ = app.emit(events::CLIP_ADDED, &item);
        suggest(app, &item, &settings);
    }

    /// "Download?" for media links, "Save/Convert?" for images; never while the app is in front.
    fn suggest(app: &AppHandle, item: &ClipItem, settings: &Settings) {
        let main_focused = app
            .get_webview_window("main")
            .is_some_and(|w| w.is_focused().unwrap_or(false));
        if main_focused {
            return;
        }
        let (kind, data) = match item.kind {
            ClipKind::MediaLink if settings.clipboard_suggest_links => {
                ("clipLink", json!({ "url": item.text, "site": item.site }))
            }
            ClipKind::Image if settings.clipboard_suggest_images => (
                "clipImage",
                json!({
                    "clipId": item.id,
                    "width": item.width,
                    "height": item.height,
                    "thumb": item.thumb,
                    "image": item.image,
                }),
            ),
            _ => return,
        };
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            crate::windows::show_toast(&app2, kind, data, Some(8000));
        });
    }

    /// Clipboard sequence number (0 if unavailable).
    pub fn seq() -> u32 {
        raw::seq_num().map_or(0, |n| n.get())
    }

    /// Both writers return the sequence number after the write completed (clipboard closed).
    pub fn write_text(text: &str) -> Result<u32, String> {
        {
            let _guard = Clipboard::new_attempts(10).map_err(|e| e.to_string())?;
            raw::set_string(text).map_err(|e| e.to_string())?;
        }
        Ok(seq())
    }

    pub fn write_image(png: &[u8]) -> Result<u32, String> {
        let dib = history::png_to_dib(png).map_err(|e| e.to_string())?;
        {
            let _guard = Clipboard::new_attempts(10).map_err(|e| e.to_string())?;
            raw::set(CF_DIB, &dib).map_err(|e| e.to_string())?;
            let f = formats();
            if f.png != 0 {
                raw::set_without_clear(f.png, png).map_err(|e| e.to_string())?;
            }
        }
        Ok(seq())
    }
}
