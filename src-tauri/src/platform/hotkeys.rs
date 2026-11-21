//! Global hotkeys (`RegisterHotKey`). OWNER: Task 9.
//!
//! One thread owns the hotkeys and runs a message loop: `WM_HOTKEY` only sends the action to a
//! second thread (so the loop never blocks), `WM_APP` re-registers after a settings change.
//! Hotkeys: 1 = clipboard shelf, 2 = clipboard library, 3 = download the copied link, 4 = clothesline.
//! A combination another app already owns (or one Windows reserves) is reported as `ok: false`
//! by [`status`]; the app keeps running.

use serde::Serialize;

/// `RegisterHotKey` modifier bits (same values as Win32 `MOD_*`).
pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;

/// TS `HotkeyStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    /// "shelf" | "library" | "downloadLink" | "line"
    pub name: String,
    pub keys: String,
    /// false = could not register (another app owns it, or invalid). An empty (off) hotkey is ok.
    pub ok: bool,
}

/// Hotkey names in id order (id = index + 1).
const NAMES: [&str; 4] = ["shelf", "library", "downloadLink", "line"];

fn modifier(name: &str) -> Option<u32> {
    Some(match name {
        "ctrl" | "control" => MOD_CONTROL,
        "alt" => MOD_ALT,
        "shift" => MOD_SHIFT,
        "win" | "meta" | "super" => MOD_WIN,
        _ => return None,
    })
}

/// Virtual-key code of a key name (lowercase).
fn key_code(name: &str) -> Option<u32> {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_alphanumeric()
    {
        // 'A'..='Z' and '0'..='9' are their own virtual-key codes.
        return Some(u32::from(c.to_ascii_uppercase()));
    }
    if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&n).then_some(0x6f + n);
    }
    for prefix in ["numpad", "num"] {
        if let Some(n) = name
            .strip_prefix(prefix)
            .and_then(|n| n.parse::<u32>().ok())
        {
            return (n <= 9).then_some(0x60 + n);
        }
    }
    Some(match name {
        "space" => 0x20,
        "enter" | "return" => 0x0d,
        "tab" => 0x09,
        "esc" | "escape" => 0x1b,
        "backspace" => 0x08,
        "insert" | "ins" => 0x2d,
        "delete" | "del" => 0x2e,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdn" => 0x22,
        "left" | "arrowleft" => 0x25,
        "up" | "arrowup" => 0x26,
        "right" | "arrowright" => 0x27,
        "down" | "arrowdown" => 0x28,
        "printscreen" | "prtsc" => 0x2c,
        "pause" => 0x13,
        _ => return None,
    })
}

/// `"Ctrl+Alt+V"` -> `(MOD_CONTROL | MOD_ALT, 0x56)`. Parts are separated by `+`, any case,
/// modifiers first (Ctrl, Alt, Shift, Win), then exactly one key: A-Z, 0-9, F1-F24, Space,
/// Enter, Tab, Esc, Backspace, Insert, Delete, Home, End, PageUp, PageDown, arrows, PrintScreen,
/// Pause, Num0-Num9. A key with no modifier or with Shift alone is only accepted for F-keys
/// (Shift+letter would eat capital letters). `MOD_NOREPEAT` is added when registering, not here.
pub fn parse_hotkey(s: &str) -> Option<(u32, u32)> {
    let mut mods = 0;
    let mut vk = None;
    for part in s.split('+') {
        let part = part.trim().to_lowercase();
        if part.is_empty() || vk.is_some() {
            return None;
        }
        match modifier(&part) {
            Some(m) => mods |= m,
            None => vk = Some(key_code(&part)?),
        }
    }
    let vk = vk?;
    let f_key = (0x70..=0x87).contains(&vk);
    (mods & !MOD_SHIFT != 0 || f_key).then_some((mods, vk))
}

/// Keys to register in id order, each with whether it is active: the shelf and library hotkeys
/// are not registered while the clipboard is off (the download-link hotkey still works).
fn wanted_hotkeys(s: &fc_core::settings::Settings) -> [(String, bool); 4] {
    [
        (s.hotkey_shelf.clone(), s.clipboard_enabled),
        (s.hotkey_library.clone(), s.clipboard_enabled),
        (s.hotkey_download_link.clone(), true),
        (s.hotkey_line.clone(), s.line_enabled),
    ]
}

/// Current registration result for each hotkey (waits briefly for a pending re-registration).
pub fn status() -> Vec<HotkeyStatus> {
    #[cfg(windows)]
    return imp::status();
    #[cfg(not(windows))]
    Vec::new()
}

/// Register the hotkeys of `s` (starts the hotkey thread on first use). Call with the new
/// settings on every change: the thread does not read the settings itself, so it never sees a
/// value that is not saved yet.
pub fn configure(app: &tauri::AppHandle, s: &fc_core::settings::Settings) {
    let wanted = wanted_hotkeys(s);
    #[cfg(windows)]
    imp::configure(app, wanted);
    #[cfg(not(windows))]
    let _ = (app, wanted);
}

#[cfg(windows)]
mod imp {
    use super::{HotkeyStatus, NAMES, parse_hotkey};
    use crate::events::NavPayload;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::mpsc::{Sender, channel};
    use std::sync::{Condvar, Mutex, MutexGuard};
    use std::time::Duration;
    use tauri::AppHandle;
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_APP, WM_HOTKEY, WM_USER,
    };

    struct Shared {
        /// Wanted key strings in id order, with whether each is active.
        wanted: [(String, bool); 4],
        wanted_gen: u64,
        done_gen: u64,
        status: Vec<HotkeyStatus>,
    }

    static SHARED: Mutex<Shared> = Mutex::new(Shared {
        wanted: [
            (String::new(), false),
            (String::new(), false),
            (String::new(), false),
            (String::new(), false),
        ],
        wanted_gen: 0,
        done_gen: 0,
        status: Vec::new(),
    });
    static DONE: Condvar = Condvar::new();
    static THREAD_ID: AtomicU32 = AtomicU32::new(0);

    fn shared() -> MutexGuard<'static, Shared> {
        SHARED.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn configure(app: &AppHandle, wanted: [(String, bool); 4]) {
        {
            let mut s = shared();
            s.wanted = wanted;
            s.wanted_gen += 1;
        }
        start(app);
        let tid = THREAD_ID.load(Ordering::SeqCst);
        // Before the thread published its id it has not registered yet: it reads `wanted` then.
        if tid != 0 {
            let _ = unsafe { PostThreadMessageW(tid, WM_APP, WPARAM(0), LPARAM(0)) };
        }
    }

    pub fn status() -> Vec<HotkeyStatus> {
        let guard = shared();
        let (guard, _) = DONE
            .wait_timeout_while(guard, Duration::from_millis(500), |s| {
                s.done_gen < s.wanted_gen
            })
            .unwrap_or_else(|e| e.into_inner());
        guard.status.clone()
    }

    #[derive(Clone, Copy)]
    enum Action {
        Shelf,
        Library,
        DownloadLink,
        Line,
    }

    fn start(app: &AppHandle) {
        static STARTED: AtomicBool = AtomicBool::new(false);
        if STARTED.swap(true, Ordering::SeqCst) {
            return;
        }
        let (tx, rx) = channel::<Action>();
        let spawned = std::thread::Builder::new()
            .name("fc-hotkeys".into())
            .spawn(move || message_loop(&tx));
        if spawned.is_err() {
            eprintln!("hotkeys: cannot start the hotkey thread");
            return;
        }
        let app = app.clone();
        let _ = std::thread::Builder::new()
            .name("fc-hotkey-actions".into())
            .spawn(move || {
                for action in rx {
                    run(&app, action);
                }
            });
    }

    fn message_loop(tx: &Sender<Action>) {
        let mut msg = MSG::default();
        unsafe {
            // Create this thread's message queue before anyone posts to it.
            let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
            THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);
        }
        register_wanted();
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
            match msg.message {
                WM_HOTKEY => {
                    let action = match msg.wParam.0 {
                        1 => Action::Shelf,
                        2 => Action::Library,
                        3 => Action::DownloadLink,
                        4 => Action::Line,
                        _ => continue,
                    };
                    let _ = tx.send(action);
                }
                WM_APP => register_wanted(),
                _ => {}
            }
        }
    }

    /// (Re-)register every hotkey from `wanted`. Runs on the hotkey thread only. All of ours are
    /// unregistered first, so swapping two of them is not reported as a conflict. An empty or
    /// inactive hotkey is not registered and reported as ok.
    fn register_wanted() {
        let (wanted, generation) = {
            let s = shared();
            (s.wanted.clone(), s.wanted_gen)
        };
        for id in 1..=NAMES.len() as i32 {
            unsafe {
                let _ = UnregisterHotKey(None, id);
            }
        }
        let mut status = Vec::with_capacity(NAMES.len());
        for (i, (keys, active)) in wanted.iter().enumerate() {
            let id = i as i32 + 1;
            let ok = if keys.trim().is_empty() || !active {
                true
            } else {
                parse_hotkey(keys).is_some_and(|(mods, vk)| unsafe {
                    RegisterHotKey(None, id, HOT_KEY_MODIFIERS(mods) | MOD_NOREPEAT, vk).is_ok()
                })
            };
            status.push(HotkeyStatus {
                name: NAMES[i].into(),
                keys: keys.clone(),
                ok,
            });
        }
        let mut s = shared();
        s.status = status;
        s.done_gen = s.done_gen.max(generation);
        DONE.notify_all();
    }

    fn run(app: &AppHandle, action: Action) {
        match action {
            Action::Shelf => crate::shelf::toggle(app),
            Action::Library => crate::windows::show_main(
                app,
                Some(NavPayload {
                    view: "clipboard".into(),
                    paths: vec![],
                    url: None,
                    clip: None,
                }),
            ),
            Action::DownloadLink => download_copied_link(app),
            Action::Line => crate::line::toggle_hotkey(app),
        }
    }

    /// A media link on the clipboard opens the Download view with it (the view probes it
    /// itself); anything else shows the `noLink` toast.
    fn download_copied_link(app: &AppHandle) {
        let link = crate::platform::clipboard::read_text()
            .and_then(|t| fc_core::download::classify_url(&t))
            .filter(|i| i.kind != fc_core::download::LinkKind::Other);
        match link {
            Some(info) => crate::windows::show_main(
                app,
                Some(NavPayload {
                    view: "download".into(),
                    paths: vec![],
                    url: Some(info.url),
                    clip: None,
                }),
            ),
            None => crate::windows::show_toast(app, "noLink", serde_json::json!({}), Some(4000)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hotkey() {
        use super::parse_hotkey as p;
        assert_eq!(p("Ctrl+Alt+V"), Some((MOD_CONTROL | MOD_ALT, 0x56)));
        assert_eq!(p("Ctrl+Shift+F5"), Some((MOD_CONTROL | MOD_SHIFT, 0x74)));
        assert_eq!(p("win + shift + s"), Some((MOD_WIN | MOD_SHIFT, 0x53)));
        assert_eq!(p("Alt+1"), Some((MOD_ALT, 0x31)));
        assert_eq!(p("Ctrl+Alt+Delete"), Some((MOD_CONTROL | MOD_ALT, 0x2e)));
        assert_eq!(p("Ctrl+Space"), Some((MOD_CONTROL, 0x20)));
        assert_eq!(p("Ctrl+PageDown"), Some((MOD_CONTROL, 0x22)));
        // An F-key needs no modifier; other keys do.
        assert_eq!(p("F9"), Some((0, 0x78)));
        assert_eq!(p("F24"), Some((0, 0x87)));
        assert_eq!(p("V"), None);
        assert_eq!(p(""), None);
        assert_eq!(p("Ctrl+"), None);
        assert_eq!(p("Ctrl+Alt"), None);
        assert_eq!(p("Ctrl+V+B"), None);
        assert_eq!(p("Ctrl+F25"), None);
        assert_eq!(p("Ctrl+Banana"), None);
        // Shift alone is not enough for a normal key (it would eat capital letters); F-keys ok.
        assert_eq!(p("Shift+V"), None);
        assert_eq!(p("Shift+1"), None);
        assert_eq!(p("Shift+F5"), Some((MOD_SHIFT, 0x74)));
        assert_eq!(p("Ctrl+Shift+V"), Some((MOD_CONTROL | MOD_SHIFT, 0x56)));
    }

    #[test]
    fn clipboard_hotkeys_are_off_while_the_clipboard_is() {
        let mut s = fc_core::settings::Settings::default();
        assert_eq!(
            wanted_hotkeys(&s),
            [
                ("Ctrl+Alt+V".to_string(), true),
                (String::new(), true),
                ("Ctrl+Alt+D".to_string(), true),
                ("Ctrl+Alt+L".to_string(), true),
            ]
        );
        s.clipboard_enabled = false;
        s.hotkey_library = "Ctrl+Alt+L".into();
        assert_eq!(
            wanted_hotkeys(&s),
            [
                ("Ctrl+Alt+V".to_string(), false),
                ("Ctrl+Alt+L".to_string(), false),
                ("Ctrl+Alt+D".to_string(), true),
                ("Ctrl+Alt+L".to_string(), true),
            ]
        );
        // The clothesline hotkey follows its own switch, not the clipboard's.
        s.line_enabled = false;
        assert_eq!(wanted_hotkeys(&s)[3], ("Ctrl+Alt+L".to_string(), false));
    }

    #[test]
    fn hotkey_line_parses() {
        assert_eq!(
            super::parse_hotkey("Ctrl+Alt+L"),
            Some((MOD_CONTROL | MOD_ALT, 0x4c))
        );
        assert_eq!(NAMES[3], "line");
    }

    #[cfg(windows)]
    #[test]
    fn modifier_bits_match_win32() {
        use windows::Win32::UI::Input::KeyboardAndMouse as k;
        assert_eq!(
            (MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN),
            (k::MOD_ALT.0, k::MOD_CONTROL.0, k::MOD_SHIFT.0, k::MOD_WIN.0)
        );
    }
}
