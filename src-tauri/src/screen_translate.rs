//! Live screen translation (hotkey, default Ctrl+5). The monitor under the cursor is captured,
//! read with Windows OCR (lines with positions), the lines are grouped into paragraphs and
//! translated with Google Translate (`fc_core::translate`), and the translations are drawn over
//! the original text by a transparent, click-through window (`translate.html`, label
//! `translate`). While it is on, the screen is read again every [`EVERY`]: only new text is
//! translated (a cache), and nothing is sent when the screen did not change. The window is left
//! out of screen captures, so it never reads its own translations. The hotkey again or Esc ends it.
//!
//! Only text is sent to Google, and only while the user has it on.

use crate::state::AppState;
use fc_core::translate::{Block, Target, group_blocks};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const TRANSLATE: &str = "translate";
/// Event with the current [`View`].
const EVENT: &str = "translate://view";
/// How often the screen is read again while it is on.
const EVERY: Duration = Duration::from_millis(1200);
/// Translations remembered in one session.
const CACHE_MAX: usize = 3000;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static SESSION: AtomicU64 = AtomicU64::new(0);
/// The last view sent, for a page that loads after it was sent.
static LAST: Mutex<Option<View>> = Mutex::new(None);

/// One translated paragraph, in the page's CSS pixels.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub line_h: f64,
    pub text: String,
}

/// What the page shows. `status`: "working" (first read running), "live", "empty" (no text on
/// screen), "error" (translation failed: offline?).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub status: &'static str,
    pub blocks: Vec<Shown>,
    /// Language the text is translated into ("tr" / "en"); `None` in auto mode (both ways).
    pub target: Option<String>,
}

fn send(app: &AppHandle, view: View) {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(view.clone());
    if let Some(w) = app.get_webview_window(TRANSLATE) {
        let _ = w.emit(EVENT, view);
    }
}

/// `translate_view`: the page asks for the current view once it is listening.
#[tauri::command]
pub fn translate_view() -> Option<View> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `translate_toggle`: the same as the hotkey (from the page or a script).
#[tauri::command]
pub fn translate_toggle(app: AppHandle) {
    std::thread::spawn(move || toggle(&app));
}

/// The hotkey: on when off, off when on.
pub fn toggle(app: &AppHandle) {
    if ACTIVE.load(Ordering::SeqCst) {
        stop(app);
    } else {
        start(app);
    }
}

/// Turn it off: hide the window and end the loop.
pub fn stop(app: &AppHandle) {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    SESSION.fetch_add(1, Ordering::SeqCst);
    crate::platform::hotkeys::set_escape(false);
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
    #[cfg(windows)]
    if let Some(w) = app.get_webview_window(TRANSLATE)
        && let Ok(h) = w.hwnd()
    {
        crate::platform::win::hide(h.0 as isize);
    }
    #[cfg(not(windows))]
    let _ = app;
}

fn start(app: &AppHandle) {
    #[cfg(windows)]
    {
        let (cx, cy) = crate::platform::win::cursor_pos();
        let Some(rect) = crate::platform::win::monitor_rect_at(cx, cy) else {
            return;
        };
        let window = match window(app) {
            Ok(w) => w,
            Err(e) => {
                crate::diag::log(|| format!("translate window: {e}"));
                eprintln!("translate window: {e}");
                return;
            }
        };
        ACTIVE.store(true, Ordering::SeqCst);
        let session = SESSION.fetch_add(1, Ordering::SeqCst) + 1;
        send(
            app,
            View {
                status: "working",
                blocks: Vec::new(),
                target: None,
            },
        );
        if let Ok(h) = window.hwnd() {
            let (l, t, r, b) = rect;
            crate::platform::win::show_no_activate(h.0 as isize, l, t, r - l, b - t);
        }
        crate::platform::hotkeys::set_escape(true);
        let app = app.clone();
        let _ = std::thread::Builder::new()
            .name("fc-translate".into())
            .spawn(move || run(&app, session, (cx, cy)));
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// The translation window, made on first use: transparent, click-through, never focused, left
/// out of screen captures.
#[cfg(windows)]
fn window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(TRANSLATE) {
        return Ok(w);
    }
    let state = app.state::<AppState>();
    let w = WebviewWindowBuilder::new(app, TRANSLATE, WebviewUrl::App("translate.html".into()))
        .title("File Chameleon Translate")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .focusable(false)
        .data_directory(state.paths.webview_dir())
        // The same browser arguments as every other window: WebView2 refuses a second set for
        // one data folder.
        .additional_browser_args(&crate::windows::browser_args())
        .build()
        .map_err(|e| e.to_string())?;
    let _ = w.set_ignore_cursor_events(true);
    if let Ok(h) = w.hwnd() {
        crate::platform::win::make_overlay_style(h.0 as isize);
        crate::platform::win::exclude_from_capture(h.0 as isize);
    }
    Ok(w)
}

fn alive(session: u64) -> bool {
    ACTIVE.load(Ordering::SeqCst) && SESSION.load(Ordering::SeqCst) == session
}

/// What changed between two reads: the texts and (roughly) where they are.
fn screen_key(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(|b| format!("{}@{},{}", b.text, (b.x / 4.0).round(), (b.y / 4.0).round()))
        .collect::<Vec<_>>()
        .join("\u{1}")
}

/// Paragraphs as the page draws them: CSS pixels (`scale` = device pixels per CSS pixel), only
/// those whose translation differs from the original.
fn shown(blocks: &[Block], cache: &HashMap<String, String>, scale: f64) -> Vec<Shown> {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    blocks
        .iter()
        .filter_map(|b| {
            let text = cache.get(&b.text)?;
            (text.trim() != b.text.trim() && !text.trim().is_empty()).then(|| Shown {
                x: b.x / scale,
                y: b.y / scale,
                w: b.w / scale,
                h: b.h / scale,
                line_h: b.line_h / scale,
                text: text.clone(),
            })
        })
        .collect()
}

#[cfg(windows)]
fn run(app: &AppHandle, session: u64, at: (i32, i32)) {
    let reader = match crate::platform::ocr::LineReader::new() {
        Ok(r) => r,
        Err(e) => {
            crate::diag::log(|| format!("translate ocr: {e}"));
            eprintln!("translate: {e}");
            crate::windows::show_toast(
                app,
                "translateFailed",
                serde_json::json!({ "why": "ocr" }),
                Some(5000),
            );
            stop(app);
            return;
        }
    };
    let target = Target::from_setting(&app.state::<AppState>().settings().translate_target);
    let scale = app
        .get_webview_window(TRANSLATE)
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);
    let mut cache: HashMap<String, String> = HashMap::new();
    let mut source: Option<String> = None;
    let mut last_key = String::new();
    let mut first = true;
    while alive(session) {
        if let Some((rgba, w, h)) = crate::platform::win::capture_monitor_at(at.0, at.1) {
            let lines = reader.read(rgba, w, h).unwrap_or_default();
            let blocks = group_blocks(&lines);
            let key = screen_key(&blocks);
            if key != last_key {
                let mut missing: Vec<String> = blocks
                    .iter()
                    .map(|b| b.text.clone())
                    .filter(|t| !cache.contains_key(t))
                    .collect();
                missing.sort();
                missing.dedup();
                let mut failed = false;
                if !missing.is_empty() {
                    match fc_core::translate::translate(&missing, target, &mut source) {
                        Ok(out) => {
                            if cache.len() > CACHE_MAX {
                                cache.clear();
                            }
                            cache.extend(missing.into_iter().zip(out));
                        }
                        Err(e) => {
                            crate::diag::log(|| format!("translate request: {e}"));
                            eprintln!("translate: {e}");
                            failed = true;
                        }
                    }
                }
                if !alive(session) {
                    break;
                }
                if failed && first {
                    send(
                        app,
                        View {
                            status: "error",
                            blocks: Vec::new(),
                            target: None,
                        },
                    );
                } else if !failed {
                    let into = fc_core::translate::target_code(target, source.as_deref());
                    let status = if blocks.is_empty() { "empty" } else { "live" };
                    send(
                        app,
                        View {
                            status,
                            blocks: shown(&blocks, &cache, scale),
                            // Auto mode goes both ways on a mixed screen: no single language.
                            target: (target != Target::Auto).then(|| into.to_string()),
                        },
                    );
                    last_key = key;
                }
            }
        }
        first = false;
        // Wait, but notice "off" quickly.
        for _ in 0..(EVERY.as_millis() / 100) {
            if !alive(session) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str, x: f64, y: f64) -> Block {
        Block {
            text: text.into(),
            x,
            y,
            w: 100.0,
            h: 20.0,
            line_h: 20.0,
            lines: 1,
        }
    }

    #[test]
    fn only_changed_text_is_shown_in_css_pixels() {
        let blocks = vec![
            block("Hello", 200.0, 100.0),
            block("2024", 10.0, 10.0),
            block("New", 0.0, 0.0),
        ];
        let cache: HashMap<String, String> = [
            ("Hello".to_string(), "Merhaba".to_string()),
            ("2024".to_string(), "2024".to_string()),
        ]
        .into();
        let s = shown(&blocks, &cache, 2.0);
        // "2024" is the same in both languages, "New" has no translation yet.
        assert_eq!(s.len(), 1);
        assert_eq!(
            (s[0].x, s[0].y, s[0].w, s[0].h, s[0].line_h),
            (100.0, 50.0, 50.0, 10.0, 10.0)
        );
        assert_eq!(s[0].text, "Merhaba");
    }

    #[test]
    fn screen_key_ignores_tiny_moves() {
        let a = screen_key(&[block("Hi", 100.0, 100.0)]);
        assert_eq!(a, screen_key(&[block("Hi", 101.0, 100.4)]));
        assert_ne!(a, screen_key(&[block("Hi", 100.0, 140.0)]));
        assert_ne!(a, screen_key(&[block("Ho", 100.0, 100.0)]));
    }
}
