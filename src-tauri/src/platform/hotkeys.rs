//! Global hotkeys (`RegisterHotKey`). OWNER: Task 9.
//!
//! One thread owns the hotkeys and runs a message loop: `WM_HOTKEY` only sends the action to a
//! second thread (so the loop never blocks), `WM_APP` re-registers after a settings change.
//! Hotkeys: 1 = clipboard library, 2 = download the copied link, 3 = clothesline, 4 = clothesline
//! search, 5 = screen capture, 6 = copy the latest image, 7 = the main window's home page,
//! 8 = live screen translation, then Ctrl+Shift+1..9, which paste the pinned clips (off unless
//! `pin_hotkeys`), and last a bare Esc, registered only while the screen translation is on.
//! (The shelf hotkey is gone since wave 3.)
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
    /// "library" | "downloadLink" | "line" | "lineSearch" | "capture" | "copyLatestImage" |
    /// "home" | "pinned" (all nine Ctrl+Shift+1..9 keys: ok only if every one registered)
    pub name: String,
    pub keys: String,
    /// false = could not register (another app owns it, or invalid). An empty (off) hotkey is ok.
    pub ok: bool,
}

/// Hotkey names in id order (id = index + 1). The pinned-clip keys follow them.
const NAMES: [&str; 8] = [
    "library",
    "downloadLink",
    "line",
    "lineSearch",
    "capture",
    "copyLatestImage",
    "home",
    "translate",
];
/// How many pinned clips get a key (Ctrl+Shift+1..9).
pub const PIN_KEYS: usize = 9;
/// Every hotkey: the named ones, then the pinned-clip keys.
const COUNT: usize = NAMES.len() + PIN_KEYS;
/// Id of the bare Esc that ends the screen translation (registered only while it is on).
const ESC_ID: i32 = COUNT as i32 + 1;
/// Is the screen translation on (Esc registered)?
static ESCAPE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Register (or drop) the bare Esc that ends the screen translation.
pub fn set_escape(on: bool) {
    ESCAPE.store(on, std::sync::atomic::Ordering::SeqCst);
    #[cfg(windows)]
    imp::rerun();
}

/// The `n`th (1-based) pinned-clip key.
fn pin_key(n: usize) -> String {
    format!("Ctrl+Shift+{n}")
}

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

/// Keys to register in id order, each with whether it is active: the library, capture and copy
/// hotkeys are not registered while the clipboard is off (the download-link and home hotkeys
/// still work); the pinned-clip keys only with `pin_hotkeys` and the clipboard on.
fn wanted_hotkeys(s: &fc_core::settings::Settings) -> Vec<(String, bool)> {
    let mut v = vec![
        (s.hotkey_library.clone(), s.clipboard_enabled),
        (s.hotkey_download_link.clone(), true),
        (s.hotkey_line.clone(), s.line_enabled),
        (s.hotkey_line_search.clone(), s.line_enabled),
        (s.hotkey_capture.clone(), s.clipboard_enabled),
        (s.hotkey_copy_latest_image.clone(), s.clipboard_enabled),
        (s.hotkey_home.clone(), true),
        (s.hotkey_translate.clone(), true),
    ];
    let pins = s.pin_hotkeys && s.clipboard_enabled;
    v.extend((1..=PIN_KEYS).map(|n| (pin_key(n), pins)));
    v
}

/// What is reported per hotkey: one entry per named hotkey, and one ("pinned") for the nine
/// pinned-clip keys, ok only when every active one registered.
fn status_of(wanted: &[(String, bool)], ok: &[bool]) -> Vec<HotkeyStatus> {
    let mut out: Vec<HotkeyStatus> = NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| HotkeyStatus {
            name: (*name).into(),
            keys: wanted[i].0.clone(),
            ok: ok[i],
        })
        .collect();
    out.push(HotkeyStatus {
        name: "pinned".into(),
        keys: format!("{}..{PIN_KEYS}", pin_key(1)),
        ok: ok[NAMES.len()..].iter().all(|&o| o),
    });
    out
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
    use super::{COUNT, ESC_ID, ESCAPE, HotkeyStatus, NAMES, parse_hotkey, status_of};
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
        wanted: Vec<(String, bool)>,
        wanted_gen: u64,
        done_gen: u64,
        status: Vec<HotkeyStatus>,
    }

    static SHARED: Mutex<Shared> = Mutex::new(Shared {
        wanted: Vec::new(),
        wanted_gen: 0,
        done_gen: 0,
        status: Vec::new(),
    });
    static DONE: Condvar = Condvar::new();
    static THREAD_ID: AtomicU32 = AtomicU32::new(0);

    fn shared() -> MutexGuard<'static, Shared> {
        SHARED.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn configure(app: &AppHandle, wanted: Vec<(String, bool)>) {
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

    /// Re-register now (the Esc of the screen translation changed).
    pub fn rerun() {
        let tid = THREAD_ID.load(Ordering::SeqCst);
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
        Library,
        DownloadLink,
        Line,
        LineSearch,
        Capture,
        CopyLatestImage,
        Home,
        Translate,
        /// Esc while the screen translation is on.
        TranslateOff,
        /// Paste the `n`th pinned clip (1-based).
        Pinned(usize),
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
                        1 => Action::Library,
                        2 => Action::DownloadLink,
                        3 => Action::Line,
                        4 => Action::LineSearch,
                        5 => Action::Capture,
                        6 => Action::CopyLatestImage,
                        7 => Action::Home,
                        8 => Action::Translate,
                        n if n > NAMES.len() && n <= COUNT => Action::Pinned(n - NAMES.len()),
                        n if n == ESC_ID as usize => Action::TranslateOff,
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
        for id in 1..=ESC_ID {
            unsafe {
                let _ = UnregisterHotKey(None, id);
            }
        }
        if ESCAPE.load(Ordering::SeqCst) {
            // Bare Esc (no modifier): only while the translation is on screen.
            let _ = unsafe { RegisterHotKey(None, ESC_ID, MOD_NOREPEAT, 0x1b) };
        }
        let mut wanted = wanted;
        wanted.resize(COUNT, (String::new(), false));
        let mut ok = vec![true; COUNT];
        for (i, (keys, active)) in wanted.iter().enumerate() {
            let id = i as i32 + 1;
            if !keys.trim().is_empty() && *active {
                ok[i] = parse_hotkey(keys).is_some_and(|(mods, vk)| unsafe {
                    RegisterHotKey(None, id, HOT_KEY_MODIFIERS(mods) | MOD_NOREPEAT, vk).is_ok()
                });
            }
        }
        let status = status_of(&wanted, &ok);
        let mut s = shared();
        s.status = status;
        s.done_gen = s.done_gen.max(generation);
        DONE.notify_all();
    }

    fn run(app: &AppHandle, action: Action) {
        match action {
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
            Action::LineSearch => crate::line::toggle_search(app),
            Action::Capture => crate::hang::capture(app),
            Action::CopyLatestImage => crate::hang::copy_latest_image(app),
            Action::Home => crate::windows::show_main(
                app,
                Some(NavPayload {
                    view: "convert".into(),
                    paths: vec![],
                    url: None,
                    clip: None,
                }),
            ),
            Action::Translate => crate::screen_translate::toggle(app),
            Action::TranslateOff => crate::screen_translate::stop(app),
            Action::Pinned(n) => crate::line::paste_pinned(app, n),
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
        // Hotkeys set here, so a change of the defaults does not touch this test.
        let mut s = fc_core::settings::Settings {
            hotkey_library: "Ctrl+2".into(),
            hotkey_download_link: "Ctrl+3".into(),
            hotkey_line: "Ctrl+4".into(),
            hotkey_line_search: "Ctrl+Alt+F".into(),
            hotkey_capture: String::new(),
            hotkey_copy_latest_image: String::new(),
            ..Default::default()
        };
        s.hotkey_home = "Ctrl+5".into();
        s.hotkey_translate = "Ctrl+6".into();
        let all_on = |lib: bool, line: bool| {
            let mut v = vec![
                ("Ctrl+2".to_string(), lib),
                ("Ctrl+3".to_string(), true),
                ("Ctrl+4".to_string(), line),
                ("Ctrl+Alt+F".to_string(), line),
                (String::new(), lib),
                (String::new(), lib),
                ("Ctrl+5".to_string(), true),
                ("Ctrl+6".to_string(), true),
            ];
            v.extend((1..=9).map(|n| (format!("Ctrl+Shift+{n}"), false)));
            v
        };
        assert_eq!(wanted_hotkeys(&s), all_on(true, true));
        s.clipboard_enabled = false;
        assert_eq!(wanted_hotkeys(&s), all_on(false, true));
        // The clothesline hotkeys follow their own switch, not the clipboard's.
        s.line_enabled = false;
        assert_eq!(wanted_hotkeys(&s), all_on(false, false));
    }

    #[test]
    fn shelf_hotkey_not_registered() {
        assert!(!NAMES.contains(&"shelf"));
        assert_eq!(NAMES.len(), 8);
        // A leftover shelf hotkey in old settings registers nothing.
        let s = fc_core::settings::Settings {
            hotkey_shelf: "Ctrl+Alt+9".into(),
            ..Default::default()
        };
        assert!(wanted_hotkeys(&s).iter().all(|(k, _)| k != "Ctrl+Alt+9"));
    }

    #[test]
    fn hotkey_line_parses() {
        assert_eq!(
            super::parse_hotkey("Ctrl+Alt+L"),
            Some((MOD_CONTROL | MOD_ALT, 0x4c))
        );
        assert_eq!(NAMES[2], "line");
    }

    #[test]
    fn wave2_hotkeys_parse() {
        assert_eq!(
            super::parse_hotkey("Ctrl+Alt+F"),
            Some((MOD_CONTROL | MOD_ALT, 0x46))
        );
        assert_eq!(&NAMES[3..6], ["lineSearch", "capture", "copyLatestImage"]);
    }

    #[test]
    fn pinned_keys_follow_their_switch_and_report_as_one() {
        let mut s = fc_core::settings::Settings {
            pin_hotkeys: false,
            ..Default::default()
        };
        let pins = |s: &fc_core::settings::Settings| wanted_hotkeys(s)[NAMES.len()..].to_vec();
        assert_eq!(pins(&s).len(), PIN_KEYS);
        assert!(pins(&s).iter().all(|(_, on)| !on));
        assert_eq!(pins(&s)[0].0, "Ctrl+Shift+1");
        assert_eq!(pins(&s)[8].0, "Ctrl+Shift+9");
        assert!(
            pins(&s)
                .iter()
                .all(|(k, _)| super::parse_hotkey(k).is_some())
        );
        s.pin_hotkeys = true;
        assert!(pins(&s).iter().all(|(_, on)| *on));
        // They paste clips: off with the clipboard.
        s.clipboard_enabled = false;
        assert!(pins(&s).iter().all(|(_, on)| !on));

        let wanted = wanted_hotkeys(&s);
        let mut ok = vec![true; COUNT];
        let st = status_of(&wanted, &ok);
        assert_eq!(st.len(), NAMES.len() + 1);
        assert_eq!(st[6].name, "home");
        assert_eq!(st[7].name, "translate");
        assert_eq!(st[8].name, "pinned");
        assert!(st[8].ok);
        ok[COUNT - 1] = false; // Ctrl+Shift+9 is taken
        assert!(!status_of(&wanted, &ok)[8].ok);
        assert_eq!(ESC_ID as usize, COUNT + 1);
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
