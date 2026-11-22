//! Pasting into the window the line was opened from: wait for the hotkey keys to be released,
//! mask Alt/Win, then send Ctrl+V.


const VK_CONTROL: u16 = 0x11;
const VK_V: u16 = 0x56;
/// Left/right Shift, left/right Alt, left/right Win: still held, they turn our Ctrl+V into
/// Ctrl+Shift+V, Ctrl+Alt+V (our own line hotkey, Paste Special in Word) or a Win shortcut.
const PASTE_BLOCKERS: [u16; 6] = [0xA0, 0xA1, 0xA4, 0xA5, 0x5B, 0x5C];
/// An unassigned virtual key. Pressed before a lone Alt or Win key-up so the target app does not
/// open its menu bar (or Windows the Start menu).
const VK_MASK: u16 = 0xE8;
/// How long the user gets to let go of Shift/Alt/Win before their key-ups are injected.
const RELEASE_WAIT: std::time::Duration = std::time::Duration::from_millis(300);

fn is_alt_or_win(vk: u16) -> bool {
    matches!(vk, 0xA4 | 0xA5 | 0x5B | 0x5C)
}

/// The masking key press, needed when Alt or Win is among `held`.
fn mask_strokes(held: &[u16]) -> Vec<(u16, bool)> {
    if held.iter().any(|&k| is_alt_or_win(k)) {
        vec![(VK_MASK, false), (VK_MASK, true)]
    } else {
        Vec::new()
    }
}

/// Keystrokes `(virtual key, key up)` for the paste into the previous app: key-ups for the paste
/// blockers in `held` (after a masking key when Alt or Win is among them), then Ctrl+V. When the
/// user still holds Ctrl (Ctrl+Enter in the line), Ctrl is left alone and only V is typed.
fn paste_sequence(held: &[u16], ctrl_held: bool) -> Vec<(u16, bool)> {
    let held: Vec<u16> = held
        .iter()
        .copied()
        .filter(|k| PASTE_BLOCKERS.contains(k))
        .collect();
    let mut out = mask_strokes(&held);
    out.extend(held.iter().map(|&k| (k, true)));
    if !ctrl_held {
        out.push((VK_CONTROL, false));
    }
    out.extend([(VK_V, false), (VK_V, true)]);
    if !ctrl_held {
        out.push((VK_CONTROL, true));
    }
    out
}

/// Shell chrome that takes focus when the line is opened from the tray (the taskbar, the
/// hidden-icons flyout): pasting "back" into it is meaningless, so it counts as no window.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn is_taskbar_class(class: &str) -> bool {
    matches!(
        class,
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
    )
}

/// Bring `prev` (an HWND, 0 = none) to the front and type Ctrl+V into it. Used by the
/// clothesline; the caller hides its own window first.
pub fn paste_keys_into(prev: isize) {
    #[cfg(not(windows))]
    let _ = prev;
    #[cfg(windows)]
    {
        use crate::platform::win;
        if prev != 0 && win::is_window(prev) && win::set_foreground(prev) {
            std::thread::sleep(std::time::Duration::from_millis(60));
            // Mask right away: Alt or Win released by hand while we wait must not open a menu.
            win::send_keys(&mask_strokes(&win::keys_down(&PASTE_BLOCKERS)));
            let deadline = std::time::Instant::now() + RELEASE_WAIT;
            let mut held = win::keys_down(&PASTE_BLOCKERS);
            while !held.is_empty() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(15));
                held = win::keys_down(&PASTE_BLOCKERS);
            }
            let ctrl_held = !win::keys_down(&[VK_CONTROL]).is_empty();
            win::send_keys(&paste_sequence(&held, ctrl_held));
        }
    }
}

#[cfg(test)]
mod paste_tests {
    use super::{is_taskbar_class, paste_sequence};

    #[test]
    fn taskbar_windows_are_not_a_paste_target() {
        for c in [
            "Shell_TrayWnd",
            "NotifyIconOverflowWindow",
            "TopLevelWindowForOverflowXamlIsland",
        ] {
            assert!(is_taskbar_class(c), "{c}");
        }
        assert!(!is_taskbar_class("Notepad"));
        assert!(!is_taskbar_class("Chrome_WidgetWin_1"));
    }

    #[test]
    fn paste_releases_held_modifiers_first() {
        const MASK: u16 = 0xE8;
        let ctrl_v = |held_ctrl: bool| {
            if held_ctrl {
                vec![(0x56, false), (0x56, true)]
            } else {
                vec![(0x11, false), (0x56, false), (0x56, true), (0x11, true)]
            }
        };
        // Nothing held: plain Ctrl+V.
        assert_eq!(paste_sequence(&[], false), ctrl_v(false));
        // Ctrl still held (Ctrl+Enter in the line): only V, Ctrl stays as the user holds it.
        assert_eq!(paste_sequence(&[], true), ctrl_v(true));
        // Shift held: released first, no mask needed.
        let mut want = vec![(0xA0, true)];
        want.extend(ctrl_v(true));
        assert_eq!(paste_sequence(&[0xA0], true), want);
        // Ctrl+Alt held after Ctrl+Alt+V, Enter: a masking key, then Alt up, then V.
        let mut want = vec![(MASK, false), (MASK, true), (0xA4, true)];
        want.extend(ctrl_v(true));
        assert_eq!(paste_sequence(&[0xA4], true), want);
        // Right Win + right Shift: masked (Start menu), both released.
        let mut want = vec![(MASK, false), (MASK, true), (0xA1, true), (0x5C, true)];
        want.extend(ctrl_v(false));
        assert_eq!(paste_sequence(&[0xA1, 0x5C], false), want);
        // Keys that are not paste blockers are ignored.
        assert_eq!(paste_sequence(&[0x41], false), ctrl_v(false));
    }
}
