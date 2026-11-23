//! Text recognition for clipboard images with the built-in Windows OCR (`Windows.Media.Ocr`:
//! offline, uses the user's installed languages). OWNER: Task 9.
//!
//! One worker thread, started on first use, takes `(item id, image path)` jobs from a channel and
//! handles one image at a time: decode, downscale to the engine's limit, recognize, store the
//! lines (joined with `\n`) with `set_ocr` and emit `clip://updated`. Nothing decoded is kept
//! after a job. Without a recognizer for the user's languages OCR is just unavailable: one log
//! line, the thread ends and later jobs are dropped.

use crate::state::AppState;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use tauri::{AppHandle, Manager};

/// The `clipboard_ocr` setting, set by [`set_enabled`] (never read from the settings on the
/// worker, so a change that is being saved right now is already seen).
static ENABLED: AtomicBool = AtomicBool::new(false);
/// Cleared when Windows has no recognizer for the user's languages.
static AVAILABLE: AtomicBool = AtomicBool::new(true);
/// Images queued by one backfill at most.
const BACKFILL_MAX: usize = 50;

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

/// Queue images for recognition (no-op while OCR is off or unavailable).
pub fn enqueue(app: &AppHandle, jobs: Vec<(u64, PathBuf)>) {
    if jobs.is_empty() || !ENABLED.load(Ordering::SeqCst) || !AVAILABLE.load(Ordering::SeqCst) {
        return;
    }
    static QUEUE: OnceLock<Sender<(u64, PathBuf)>> = OnceLock::new();
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = channel::<(u64, PathBuf)>();
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("fc-ocr".into())
            .spawn(move || {
                #[cfg(windows)]
                imp::worker(&app, rx);
                #[cfg(not(windows))]
                let _ = (app, rx);
            });
        if spawned.is_err() {
            AVAILABLE.store(false, Ordering::SeqCst);
        }
        tx
    });
    for job in jobs {
        let _ = queue.send(job);
    }
}

/// Queue up to 50 stored images that were never recognized (at start and when OCR is turned on).
pub fn backfill(app: &AppHandle) {
    if !ENABLED.load(Ordering::SeqCst) || !AVAILABLE.load(Ordering::SeqCst) {
        return;
    }
    let jobs: Vec<(u64, PathBuf)> = {
        let state = app.state::<AppState>();
        let store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
        store.needs_ocr().into_iter().take(BACKFILL_MAX).collect()
    };
    enqueue(app, jobs);
}

/// Read the text in image files right now, on the calling thread (the wheel's "Copy text", the
/// line's image cards). Each image's lines joined with `\n`, images separated by a blank line;
/// images without text are left out. Works whether or not clipboard OCR is on.
pub fn recognize_files(paths: &[PathBuf]) -> Result<String, String> {
    #[cfg(windows)]
    return imp::recognize_files(paths);
    #[cfg(not(windows))]
    {
        let _ = paths;
        Err("text recognition needs Windows".into())
    }
}

/// Size with the longest side at most `max`, aspect ratio kept (never upscales, never 0).
fn fit_within(w: u32, h: u32, max: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest <= max || max == 0 {
        return (w, h);
    }
    let scale = f64::from(max) / f64::from(longest);
    let side = |v: u32| ((f64::from(v) * scale).round() as u32).clamp(1, max);
    (side(w), side(h))
}

/// RGBA pixels -> opaque BGRA flattened onto white: transparent areas would otherwise turn
/// black and hide dark text. Opaque, so it is valid as premultiplied too.
fn bgra_on_white(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len());
    for p in rgba.as_chunks::<4>().0 {
        let a = u32::from(p[3]);
        let flat = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        out.extend_from_slice(&[flat(p[2]), flat(p[1]), flat(p[0]), 255]);
    }
    out
}

#[cfg(windows)]
mod imp {
    use super::{AVAILABLE, ENABLED, bgra_on_white, fit_within};
    use crate::events;
    use crate::state::AppState;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::Receiver;
    use tauri::{AppHandle, Emitter, Manager};
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

    pub fn worker(app: &AppHandle, rx: Receiver<(u64, PathBuf)>) {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() else {
            AVAILABLE.store(false, Ordering::SeqCst);
            eprintln!("ocr: Windows has no text recognizer for the user's languages");
            return;
        };
        let max = OcrEngine::MaxImageDimension().unwrap_or(2600).max(64);
        let state = app.state::<AppState>();
        for (id, path) in rx {
            if !ENABLED.load(Ordering::SeqCst) {
                continue;
            }
            // Queued twice (capture + backfill), already done, deleted or trashed: skip.
            let pending = {
                let store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
                store
                    .get(id)
                    .is_some_and(|i| i.ocr.is_none() && i.trashed_at.is_none())
            };
            if !pending {
                continue;
            }
            // A failure is stored as "nothing found", so the image is not retried on every start.
            let text = recognize(&engine, &path, max).unwrap_or_else(|e| {
                eprintln!("ocr {}: {e}", path.display());
                String::new()
            });
            let updated = {
                let mut store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
                store
                    .set_ocr(id, text)
                    .ok()
                    .and_then(|()| store.get(id).cloned())
            };
            if let Some(item) = updated {
                let _ = app.emit(events::CLIP_UPDATED, &item);
            }
        }
    }

    pub fn recognize_files(paths: &[PathBuf]) -> Result<String, String> {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| "Windows has no text recognizer for your languages".to_string())?;
        let max = OcrEngine::MaxImageDimension().unwrap_or(2600).max(64);
        let mut parts = Vec::new();
        for path in paths {
            let text = recognize(&engine, path, max)?;
            if !text.trim().is_empty() {
                parts.push(text);
            }
        }
        Ok(parts.join("\n\n"))
    }

    fn recognize(engine: &OcrEngine, path: &Path, max: u32) -> Result<String, String> {
        let win = |e: windows::core::Error| e.message();
        let img = image::open(path).map_err(|e| e.to_string())?;
        let (w, h) = fit_within(img.width(), img.height(), max);
        let img = if (w, h) == (img.width(), img.height()) {
            img
        } else {
            img.resize_exact(w, h, image::imageops::FilterType::Triangle)
        };
        let bgra = bgra_on_white(img.into_rgba8().as_raw());
        let buffer = {
            let writer = DataWriter::new().map_err(win)?;
            writer.WriteBytes(&bgra).map_err(win)?;
            writer.DetachBuffer().map_err(win)?
        };
        drop(bgra);
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            w as i32,
            h as i32,
            BitmapAlphaMode::Premultiplied,
        )
        .map_err(win)?;
        drop(buffer);
        let result = engine
            .RecognizeAsync(&bitmap)
            .and_then(|op| op.join())
            .map_err(win)?;
        let mut lines = Vec::new();
        for line in &result.Lines().map_err(win)? {
            lines.push(line.Text().map_err(win)?.to_string_lossy());
        }
        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{bgra_on_white, fit_within};

    #[test]
    fn fit_within_keeps_aspect_and_never_upscales() {
        assert_eq!(fit_within(800, 600, 2600), (800, 600));
        assert_eq!(fit_within(5200, 2600, 2600), (2600, 1300));
        assert_eq!(fit_within(1000, 4000, 2000), (500, 2000));
        assert_eq!(fit_within(10_000, 3, 2600), (2600, 1));
    }

    #[test]
    fn bgra_is_opaque_on_white() {
        let rgba = [10, 20, 30, 255, 0, 0, 0, 0, 0, 0, 0, 128];
        assert_eq!(
            bgra_on_white(&rgba),
            [30, 20, 10, 255, 255, 255, 255, 255, 127, 127, 127, 255]
        );
    }
}
