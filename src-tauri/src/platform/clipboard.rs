//! Windows clipboard listener for the smart clipboard (v2). OWNER: Task 9.
//! `start` spawns the listener thread; `set_enabled` turns capture on/off at runtime.
//!
//! The thread blocks in `Monitor::recv` (a `WM_CLIPBOARDUPDATE` message loop), so it costs nothing
//! while idle. Disabling capture only makes it ignore events; it stays parked.
//!
//! Capture: per-kind switches, ignored apps (the clipboard owner's exe, checked before any data
//! is read), password-manager markers and our own writes are skipped; the store skips blank text
//! and refreshes duplicates. New images go to the OCR worker, new links (opt-in) to the link
//! title worker. Watched image folders import new image files. Retention runs 10 s after start,
//! then hourly, and on settings changes.

use crate::events;
use crate::state::AppState;
use fc_core::clip::history;
use fc_core::settings::Settings;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

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

/// Put a file list (`CF_HDROP`) on the clipboard (used by `clip_copy` for several images).
#[cfg(windows)]
pub fn write_files(paths: &[std::path::PathBuf]) -> Result<u32, String> {
    imp::write_files(paths)
}

/// Text currently on the clipboard (the download-link hotkey).
#[cfg(windows)]
pub fn read_text() -> Option<String> {
    imp::read_text()
}

#[cfg(not(windows))]
pub fn write_text(_text: &str) -> Result<u32, String> {
    Err("clipboard is only supported on Windows".into())
}

#[cfg(not(windows))]
pub fn write_image(_png: &[u8]) -> Result<u32, String> {
    Err("clipboard is only supported on Windows".into())
}

#[cfg(not(windows))]
pub fn write_files(_paths: &[std::path::PathBuf]) -> Result<u32, String> {
    Err("clipboard is only supported on Windows".into())
}

#[cfg(not(windows))]
pub fn read_text() -> Option<String> {
    None
}

/// Should copied text of this kind be recorded? Links (media or not) follow the links switch.
fn text_wanted(text: &str, s: &Settings) -> bool {
    match fc_core::clip::classify_text(text.trim()) {
        fc_core::clip::ClipKind::Text => s.clipboard_capture_text,
        _ => s.clipboard_capture_links,
    }
}

fn ignored_app(exe: Option<&str>, s: &Settings) -> bool {
    exe.is_some_and(|exe| {
        s.clipboard_ignore_apps
            .iter()
            .any(|a| a.trim().eq_ignore_ascii_case(exe))
    })
}

// ---------------------------------------------------------------- after an item was added

/// Background work for a freshly added item: OCR for images, a page title for links.
fn enrich(app: &AppHandle, item: &fc_core::clip::ClipItem, s: &Settings) {
    use fc_core::clip::ClipKind;
    match item.kind {
        ClipKind::Image if item.ocr.is_none() => {
            if let Some(path) = &item.image {
                super::ocr::enqueue(app, vec![(item.id, path.clone())]);
            }
        }
        ClipKind::Link | ClipKind::MediaLink
            if s.clipboard_link_titles && item.link_title.is_none() =>
        {
            let url = item
                .text
                .as_deref()
                .and_then(fc_core::download::classify_url)
                .map(|i| i.url);
            if let Some(url) = url.filter(|u| !fc_core::clip::enrich::is_private_or_secret(u)) {
                fetch_title_later(app, item.id, url);
            }
        }
        _ => {}
    }
}

/// Link titles are fetched one at a time on one lazily started worker thread.
fn fetch_title_later(app: &AppHandle, id: u64, url: String) {
    static QUEUE: OnceLock<Sender<(u64, String)>> = OnceLock::new();
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = channel::<(u64, String)>();
        let app = app.clone();
        let _ = std::thread::Builder::new()
            .name("fc-link-titles".into())
            .spawn(move || {
                for (id, url) in rx {
                    let Some(title) = fc_core::clip::enrich::fetch_link_title(&url) else {
                        continue;
                    };
                    let state = app.state::<AppState>();
                    let updated = {
                        let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
                        store
                            .set_link_title(id, title)
                            .ok()
                            .and_then(|()| store.get(id).cloned())
                    };
                    if let Some(item) = updated {
                        let _ = app.emit(events::CLIP_UPDATED, &item);
                    }
                }
            });
        tx
    });
    let _ = queue.send((id, url));
}

// ---------------------------------------------------------------- retention

/// Apply the retention of `s` now; tells the webviews when something was removed.
pub fn prune(app: &AppHandle, s: &Settings) {
    let removed = {
        let state = app.state::<AppState>();
        let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
        store
            .prune(
                fc_core::now_ms(),
                &fc_core::clip::Retention::from_settings(s),
            )
            .unwrap_or(0)
    };
    if removed > 0 {
        let _ = app.emit(events::CLIP_CHANGED, ());
    }
}

/// One sleeping thread: 10 s after start (keeps startup light) apply retention and queue images
/// that still need OCR, then apply retention every hour. Nothing happens while the clipboard is
/// off (the store is not even opened).
pub fn start_maintenance(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("fc-clip-maintenance".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(10));
            let mut first = true;
            loop {
                let s = app.state::<AppState>().settings();
                if s.clipboard_enabled {
                    prune(&app, &s);
                    if first && s.clipboard_ocr {
                        super::ocr::backfill(&app);
                    }
                }
                first = false;
                std::thread::sleep(Duration::from_secs(60 * 60));
            }
        });
}

// ---------------------------------------------------------------- watched image folders

const MAX_WATCH_FOLDERS: usize = 3;
const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "bmp", "gif"];

/// Start, update or stop the image-folder watcher for `s` (on while the clipboard is on and
/// some folders are set; at most 3 are watched).
pub fn configure_folders(app: &AppHandle, s: &Settings) {
    use fc_core::watch::{WatchSpec, Watcher};
    let specs: Vec<WatchSpec> = if s.clipboard_enabled {
        s.clipboard_watch_folders
            .iter()
            .filter(|d| !d.as_os_str().is_empty())
            .take(MAX_WATCH_FOLDERS)
            .enumerate()
            .map(|(i, dir)| WatchSpec {
                id: format!("clip{i}"),
                dir: dir.clone(),
                recursive: false,
                exclude: Vec::new(),
                extensions: Some(IMAGE_EXTENSIONS.iter().map(|e| e.to_string()).collect()),
            })
            .collect()
    } else {
        Vec::new()
    };
    let state = app.state::<AppState>();
    let mut slot = state.clip_watcher.lock().unwrap_or_else(|e| e.into_inner());
    if specs.is_empty() {
        // Dropping joins the watcher thread: do it after releasing the lock.
        let old = slot.take();
        drop(slot);
        drop(old);
        return;
    }
    let watcher = slot.get_or_insert_with(|| {
        let app = app.clone();
        Watcher::start(
            Duration::from_secs(2),
            Duration::from_secs(2),
            move |_, path| import_image_file(&app, &path),
        )
    });
    watcher.set_specs(specs);
}

/// PNG bytes and size of an image file (other formats are converted). `None` if it is not a
/// readable image or larger than the clipboard accepts.
fn image_file_as_png(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let bytes = std::fs::read(path).ok()?;
    let fits = |w: u32, h: u32| u64::from(w) * u64::from(h) <= history::MAX_IMAGE_PIXELS;
    if let Some((w, h)) = history::png_size(&bytes) {
        return fits(w, h).then_some((bytes, w, h));
    }
    let reader = || {
        image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .ok()
    };
    let (w, h) = reader()?.into_dimensions().ok()?;
    if !fits(w, h) {
        return None;
    }
    let img = reader()?.decode().ok()?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some((png, img.width(), img.height()))
}

/// A new file in a watched folder: add it as an image item (not while capture is paused).
fn import_image_file(app: &AppHandle, path: &Path) {
    let state = app.state::<AppState>();
    if state.clip_paused.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some((png, w, h)) = image_file_as_png(path) else {
        return;
    };
    let added = {
        let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
        store.add_image_png(&png, w, h, fc_core::now_ms())
    };
    drop(png);
    if let Ok(Some(item)) = added {
        let _ = app.emit(events::CLIP_ADDED, &item);
        enrich(app, &item, &state.settings());
    }
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

    /// Text of a kind the settings record, else an image (when images are on).
    fn read(s: &Settings) -> Option<Captured> {
        let _guard = open_with_retry()?;
        if excluded() || raw::is_format_avail(CF_HDROP) {
            return None;
        }
        if raw::is_format_avail(CF_UNICODETEXT) {
            let mut text = String::new();
            if Unicode.read_clipboard(&mut text).is_ok()
                && !text.trim().is_empty()
                && super::text_wanted(&text, s)
            {
                return Some(Captured::Text(text));
            }
        }
        read_image(s.clipboard_images)
    }

    const CF_UNICODETEXT: u32 = clipboard_win::formats::CF_UNICODETEXT;

    fn handle(app: &AppHandle, state: &AppState) {
        let settings = state.settings();
        if !settings.clipboard_enabled {
            return;
        }
        // Ignored apps are checked before any clipboard data is read.
        let source = crate::platform::win::clipboard_owner_exe();
        if super::ignored_app(source.as_deref(), &settings) {
            return;
        }
        let Some(captured) = read(&settings) else {
            return;
        };
        let now = fc_core::now_ms();
        let (item, pruned) = {
            let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
            let first_before = store
                .query(&fc_core::clip::ClipQuery {
                    limit: Some(1),
                    ..Default::default()
                })
                .first()
                .map(|i| i.id);
            let added = match &captured {
                Captured::Text(t) => store.add_text(t, now),
                Captured::Image { png, w, h } => store.add_image_png(png, *w, *h, now),
            };
            let Ok(Some(mut item)) = added else { return };
            if item.source_app != source && store.set_source_app(item.id, source.clone()).is_ok() {
                item.source_app = source;
            }
            let pruned = store
                .prune(now, &fc_core::clip::Retention::from_settings(&settings))
                .unwrap_or(0);
            // The same content copied again while it is already on top: nothing new to announce.
            if first_before == Some(item.id) {
                return;
            }
            (item, pruned)
        };
        drop(captured);
        if pruned > 0 {
            let _ = app.emit(events::CLIP_CHANGED, ());
        }
        let _ = app.emit(events::CLIP_ADDED, &item);
        super::enrich(app, &item, &settings);
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

    /// The writers return the sequence number after the write completed (clipboard closed).
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

    pub fn write_files(paths: &[std::path::PathBuf]) -> Result<u32, String> {
        let names: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        {
            let _guard = Clipboard::new_attempts(10).map_err(|e| e.to_string())?;
            raw::set_file_list_with(&names, clipboard_win::options::DoClear)
                .map_err(|e| e.to_string())?;
        }
        Ok(seq())
    }

    pub fn read_text() -> Option<String> {
        let _guard = open_with_retry()?;
        let mut text = String::new();
        Unicode.read_clipboard(&mut text).ok()?;
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_switches_and_ignored_apps() {
        let mut s = Settings {
            clipboard_capture_text: false,
            clipboard_ignore_apps: vec!["keepass.exe".into()],
            ..Settings::default()
        };
        assert!(!text_wanted("just some words", &s));
        assert!(text_wanted("https://youtube.com/watch?v=1", &s));
        assert!(text_wanted(" https://example.com/page ", &s));
        s.clipboard_capture_text = true;
        s.clipboard_capture_links = false;
        assert!(text_wanted("just some words", &s));
        assert!(!text_wanted("https://example.com/page", &s));
        assert!(ignored_app(Some("keepass.exe"), &s));
        assert!(ignored_app(Some("KeePass.EXE"), &s));
        assert!(!ignored_app(Some("notepad.exe"), &s));
        assert!(!ignored_app(None, &s));
    }

    #[test]
    fn image_files_become_png() {
        let d = tempfile_dir();
        let jpg = d.join("shot.jpg");
        image::RgbImage::from_pixel(30, 20, image::Rgb([200, 10, 10]))
            .save(&jpg)
            .unwrap();
        let (png, w, h) = image_file_as_png(&jpg).unwrap();
        assert_eq!((w, h), (30, 20));
        assert_eq!(history::png_size(&png), Some((30, 20)));
        // A PNG is taken as is.
        let png_path = d.join("same.png");
        std::fs::write(&png_path, &png).unwrap();
        assert_eq!(image_file_as_png(&png_path).unwrap().0, png);
        // Not an image.
        let txt = d.join("notes.png");
        std::fs::write(&txt, b"hello").unwrap();
        assert!(image_file_as_png(&txt).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A fresh folder in the temp dir (`tempfile` is not a dependency of this crate).
    fn tempfile_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "fc-clip-test-{}-{}",
            std::process::id(),
            fc_core::now_ms()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
