//! Small Win32 helpers (no state). HWNDs cross module boundaries as `isize`.

use windows::Win32::Foundation::{CloseHandle, HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GWL_EXSTYLE, GetAncestor, GetCursorPos, GetWindowLongPtrW, GetWindowThreadProcessId,
    HWND_TOPMOST, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WindowFromPoint,
};
use windows::core::PWSTR;

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut core::ffi::c_void)
}

/// Overlay never takes focus and never appears in Alt+Tab.
pub fn make_overlay_style(h: isize) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE);
        let add = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;
        SetWindowLongPtrW(hwnd(h), GWL_EXSTYLE, ex | add);
    }
}

/// Move/resize and show topmost without activating (focus stays in Explorer).
pub fn show_no_activate(h: isize, x: i32, y: i32, w: i32, hgt: i32) {
    unsafe {
        let _ = SetWindowPos(
            hwnd(h),
            Some(HWND_TOPMOST),
            x,
            y,
            w,
            hgt,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn hide(h: isize) {
    unsafe {
        let _ = ShowWindow(hwnd(h), SW_HIDE);
    }
}

pub fn cursor_pos() -> (i32, i32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

/// Work area (left, top, right, bottom) of the monitor nearest to the physical point.
pub fn work_area_at(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let mon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(mon, &mut info).as_bool() {
            let r = info.rcWork;
            Some((r.left, r.top, r.right, r.bottom))
        } else {
            None
        }
    }
}

/// (lowercase exe file name, pid) of the top-level window under a physical point.
pub fn process_at(x: i32, y: i32) -> Option<(String, u32)> {
    unsafe {
        let w = WindowFromPoint(POINT { x, y });
        if w.0.is_null() {
            return None;
        }
        let root = GetAncestor(w, GA_ROOT);
        let mut pid = 0u32;
        GetWindowThreadProcessId(if root.0.is_null() { w } else { root }, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
                .is_ok();
        let _ = CloseHandle(proc);
        if !ok {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        let name = full
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&full)
            .to_ascii_lowercase();
        Some((name, pid))
    }
}

/// Give every descendant window of the webview window `top` the drop target wry registered.
///
/// wry registers its OLE drop target only on the child windows that exist when the webview is
/// created. The overlay is the first webview, created hidden at startup, so WebView2 creates its
/// browser-side windows (`Chrome_WidgetWin_1`, `Chrome_RenderWidgetHostHWND`, ...) afterwards and
/// they never get a target. OLE hands a drag to the deepest window under the cursor, finds no
/// target there and the wheel never sees the files. Registering wry's own target on those windows
/// (as wry does for windows that already exist) fixes it. Idempotent and cheap; must run on the UI
/// thread, where OLE is initialised.
pub fn share_drop_target(top: isize) {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::System::Ole::{IDropTarget, RegisterDragDrop};
    use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetPropW};
    use windows::core::{BOOL, Interface, w};

    unsafe extern "system" fn collect(h: HWND, l: LPARAM) -> BOOL {
        let v = unsafe { &mut *(l.0 as *mut Vec<HWND>) };
        v.push(h);
        true.into()
    }

    let mut children: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumChildWindows(
            Some(hwnd(top)),
            Some(collect),
            LPARAM(&mut children as *mut Vec<HWND> as isize),
        );
        let me = GetCurrentProcessId();
        // The target wry registered sits on a child window of our own process.
        let raw = children.iter().find_map(|&c| {
            let mut pid = 0u32;
            GetWindowThreadProcessId(c, Some(&mut pid));
            let p = GetPropW(c, w!("OleDropTargetInterface"));
            (pid == me && !p.0.is_null()).then_some(p.0)
        });
        let Some(raw) = raw else { return };
        let Some(target) = IDropTarget::from_raw_borrowed(&raw) else {
            return;
        };
        for &c in &children {
            if GetPropW(c, w!("OleDropTargetInterface")).0.is_null() {
                let _ = RegisterDragDrop(c, target);
            }
        }
    }
}

pub fn current_pid() -> u32 {
    unsafe { GetCurrentProcessId() }
}

/// "tr" when the Windows UI language is Turkish, else "en".
pub fn system_language() -> String {
    let lang = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    if lang & 0x3ff == 0x1f {
        "tr".into()
    } else {
        "en".into()
    }
}
