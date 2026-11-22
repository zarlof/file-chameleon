//! "Hang on the line": the Explorer verb "İpe as" (`--hang "<file>"`), the screen capture and
//! "copy the latest image" hotkeys. Everything here ends in the clipboard store, so the clip
//! shows up on the line like any other copy.

use crate::events;
use crate::state::AppState;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};

/// Command-line switch of the verb: the paths after it are hung on the line.
pub const HANG_ARG: &str = "--hang";

/// Largest image file the verb imports.
const MAX_IMAGE_BYTES: u64 = 50 * 1024 * 1024;
/// Largest text file the verb imports.
const MAX_TEXT_BYTES: u64 = 1024 * 1024;

const IMAGE_EXTS: [&str; 9] = [
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff", "ico",
];
const TEXT_EXTS: [&str; 6] = ["txt", "md", "markdown", "log", "csv", "json"];

/// What the verb makes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HangKind {
    Image,
    Text,
    Reject,
}

/// The paths after `--hang` (empty when the switch is missing).
pub fn paths_from_args(args: &[String]) -> Vec<String> {
    args.iter()
        .skip_while(|a| *a != HANG_ARG)
        .skip(1)
        .filter(|a| !a.trim().is_empty())
        .cloned()
        .collect()
}

/// Is `head` (the first bytes of a file) UTF-8? A character cut off at the end of `head` is fine.
fn utf8_prefix_ok(head: &[u8]) -> bool {
    match std::str::from_utf8(head) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none(),
    }
}

/// Decide by extension, size (`len` bytes) and the first bytes (`head`). Folders, unknown
/// extensions, empty or oversized files and text that is not UTF-8 are rejected.
pub fn classify(path: &Path, len: u64, head: &[u8]) -> HangKind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if len == 0 || path.is_dir() {
        return HangKind::Reject;
    }
    if IMAGE_EXTS.contains(&ext.as_str()) {
        return if len <= MAX_IMAGE_BYTES {
            HangKind::Image
        } else {
            HangKind::Reject
        };
    }
    if TEXT_EXTS.contains(&ext.as_str()) {
        return if len <= MAX_TEXT_BYTES && !head.contains(&0) && utf8_prefix_ok(head) {
            HangKind::Text
        } else {
            HangKind::Reject
        };
    }
    HangKind::Reject
}

/// Hang each file on the line. One worker thread per call; a rejected file is skipped.
pub fn import_paths(app: &AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("fc-hang".into())
        .spawn(move || {
            for p in paths {
                import_one(&app, Path::new(&p));
            }
        });
}

fn import_one(app: &AppHandle, path: &Path) {
    use std::io::Read;
    let Ok(meta) = std::fs::metadata(path) else {
        return reject(app);
    };
    let mut head = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(8192).read_to_end(&mut head);
    }
    let now = fc_core::now_ms();
    let state = app.state::<AppState>();
    let added = match classify(path, meta.len(), &head) {
        HangKind::Reject => return reject(app),
        HangKind::Image => {
            let Some((png, w, h)) = crate::platform::clipboard::image_file_as_png(path) else {
                return reject(app);
            };
            let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
            store.add_image_png(&png, w, h, now)
        }
        HangKind::Text => {
            let Ok(text) = std::fs::read_to_string(path) else {
                return reject(app);
            };
            let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
            store.add_text(&text, now)
        }
    };
    announce(app, added);
}

/// The toast text for a file that cannot be hung, in the UI language.
fn reject_message(lang: &str) -> &'static str {
    if lang == "tr" {
        "Bu dosya ipe asılamaz"
    } else {
        "This file can't be hung on the line"
    }
}

/// A rejected file is not silent: a short toast (the overlay shows an unknown toast kind as its
/// title, so the kind carries the localized message).
fn reject(app: &AppHandle) {
    let lang = crate::platform::ui_language(&app.state::<AppState>().settings().language);
    crate::windows::show_toast(
        app,
        reject_message(&lang),
        serde_json::json!({}),
        Some(4000),
    );
}

/// Tell the webviews about a new item and let the line peek (when peeking on copy is on).
fn announce(app: &AppHandle, added: fc_core::Result<Option<fc_core::clip::ClipItem>>) {
    if let Ok(Some(item)) = added {
        let s = app.state::<AppState>().settings();
        let _ = app.emit(events::CLIP_ADDED, &item);
        crate::line::refit(app);
        crate::line::peek(app);
        crate::platform::clipboard::enrich(app, &item, &s);
    }
}

/// The capture hotkey: the monitor under the cursor becomes an image clip ("File Chameleon").
pub fn capture(app: &AppHandle) {
    #[cfg(windows)]
    {
        let app = app.clone();
        let _ = std::thread::Builder::new()
            .name("fc-capture".into())
            .spawn(move || {
                let (x, y) = crate::platform::win::cursor_pos();
                let Some((rgba, w, h)) = crate::platform::win::capture_monitor_at(x, y) else {
                    return;
                };
                let Some(img) = image::RgbaImage::from_raw(w, h, rgba) else {
                    return;
                };
                let mut png = Vec::new();
                if img
                    .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                    .is_err()
                {
                    return;
                }
                drop(img);
                let state = app.state::<AppState>();
                let added = {
                    let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
                    let added = store.add_image_png(&png, w, h, fc_core::now_ms());
                    if let Ok(Some(item)) = &added {
                        let _ = store.set_source_app(item.id, Some("File Chameleon".into()));
                    }
                    added.map(|o| o.and_then(|i| store.get(i.id).cloned()))
                };
                announce(&app, added);
            });
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// The "copy latest image" hotkey: the newest live image (its edited copy when there is one)
/// goes to the clipboard.
pub fn copy_latest_image(app: &AppHandle) {
    use fc_core::clip::{ClipQuery, Shelf};
    let state = app.state::<AppState>();
    let id = state
        .clips()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .query(&ClipQuery {
            shelf: Some(Shelf::Images),
            limit: Some(1),
            ..Default::default()
        })
        .first()
        .map(|i| i.id);
    let lang = crate::platform::ui_language(&state.settings().language);
    let message = match id {
        None => none_message(&lang),
        Some(id) => match crate::ipc::clipboard::copy_items(&state, &[id]) {
            Ok(_) => copied_message(&lang),
            Err(e) => {
                eprintln!("copy latest image: {e:?}");
                return;
            }
        },
    };
    crate::windows::show_toast(app, message, serde_json::json!({}), Some(2000));
}

fn copied_message(lang: &str) -> &'static str {
    if lang == "tr" {
        "Kopyalandı"
    } else {
        "Copied"
    }
}

fn none_message(lang: &str) -> &'static str {
    if lang == "tr" {
        "Kopyalanacak görsel yok"
    } else {
        "No image to copy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn reject_message_follows_the_language() {
        assert_eq!(reject_message("tr"), "Bu dosya ipe asılamaz");
        assert_eq!(reject_message("en"), "This file can't be hung on the line");
    }

    #[test]
    fn hang_args() {
        assert_eq!(
            paths_from_args(&s(&["exe", "--hang", "C:\\a.png"])),
            ["C:\\a.png"]
        );
        assert!(paths_from_args(&s(&["exe", "--wheel", "a.png"])).is_empty());
        assert!(paths_from_args(&[]).is_empty());
    }

    #[test]
    fn hang_import_rejects() {
        const MB: u64 = 1024 * 1024;
        let c = |name: &str, len: u64, head: &[u8]| classify(Path::new(name), len, head);
        assert_eq!(c("a.txt", 2 * MB, b"hello"), HangKind::Reject);
        assert_eq!(c("a.txt", 10, &[0xff, 0xfe, 0x00, 0x41]), HangKind::Reject);
        assert_eq!(c("a.txt", 10, &[b'a', 0xc3, 0x28]), HangKind::Reject);
        assert_eq!(c("a.png", 51 * MB, &[]), HangKind::Reject);
        assert_eq!(c("a.exe", 10, b"MZ"), HangKind::Reject);
        assert_eq!(c("a.md", MB / 2, "merhaba ş".as_bytes()), HangKind::Text);
        assert_eq!(c("a.jpg", 5 * MB, &[]), HangKind::Image);
        // A folder and an empty file are never imported.
        assert_eq!(classify(&std::env::temp_dir(), 10, b""), HangKind::Reject);
        assert_eq!(c("a.png", 0, &[]), HangKind::Reject);
    }
}
