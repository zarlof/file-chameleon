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

// ---------------------------------------------------------------- clipboard v2 and shelf (Task 9)

/// Lowercase exe file name of a process (`None` if it cannot be queried).
fn exe_name_of(pid: u32) -> Option<String> {
    unsafe {
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
        Some(
            full.rsplit(['\\', '/'])
                .next()
                .unwrap_or(&full)
                .to_ascii_lowercase(),
        )
    }
}

/// Lowercase exe name of the app that put the current content on the clipboard. Does not open
/// the clipboard. `None` when there is no owner window (some apps write without one).
pub fn clipboard_owner_exe() -> Option<String> {
    use windows::Win32::System::DataExchange::GetClipboardOwner;
    unsafe {
        let owner = GetClipboardOwner().ok()?;
        let mut pid = 0u32;
        GetWindowThreadProcessId(owner, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        exe_name_of(pid)
    }
}

/// The foreground top-level window (0 = none).
pub fn foreground_window() -> isize {
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow().0 as isize }
}

/// Window class name of `h` ("" when it cannot be read).
pub fn window_class(h: isize) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd(h), &mut buf) };
    String::from_utf16_lossy(&buf[..usize::try_from(n).unwrap_or(0)])
}

pub fn is_window(h: isize) -> bool {
    unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd(h))).as_bool() }
}

/// Bring a window to the front (allowed while our own window is in front). True on success.
pub fn set_foreground(h: isize) -> bool {
    unsafe { windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd(h)).as_bool() }
}

/// True when the window belongs to this process (one of our own windows).
pub fn is_own_window(h: isize) -> bool {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd(h), Some(&mut pid));
    }
    pid != 0 && pid == current_pid()
}

/// The virtual keys of `vks` that are physically down right now.
pub fn keys_down(vks: &[u16]) -> Vec<u16> {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    vks.iter()
        .copied()
        .filter(|&vk| unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000 != 0)
        .collect()
}

/// Inject `(virtual key, key up)` strokes into whatever window is in front, in one batch.
pub fn send_keys(strokes: &[(u16, bool)]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
    };
    let inputs: Vec<INPUT> = strokes
        .iter()
        .map(|&(vk, up)| {
            let mut flags = KEYBD_EVENT_FLAGS(0);
            if up {
                flags |= KEYEVENTF_KEYUP;
            }
            // Right Ctrl, right Alt and both Win keys are extended keys.
            if matches!(vk, 0xA3 | 0xA5 | 0x5B | 0x5C) {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(vk),
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            }
        })
        .collect();
    if !inputs.is_empty() {
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

// ---------------------------------------------------------------- clothesline

/// A screen rectangle `(left, top, right, bottom)` in physical pixels.
pub type Rect = (i32, i32, i32, i32);

fn rect_of(r: windows::Win32::Foundation::RECT) -> Rect {
    (r.left, r.top, r.right, r.bottom)
}

pub fn is_visible(h: isize) -> bool {
    unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd(h)).as_bool() }
}

/// Set or clear `WS_EX_NOACTIVATE`: a window with it never takes focus from a click.
pub fn set_no_activate(h: isize, on: bool) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE);
        let flag = WS_EX_NOACTIVATE.0 as isize;
        let next = if on { ex | flag } else { ex & !flag };
        if next != ex {
            SetWindowLongPtrW(hwnd(h), GWL_EXSTYLE, next);
        }
    }
}

/// Show a window raw (tao is never told, so hide it with [`hide`] too). With `activate` false
/// it is shown topmost without taking focus (`SW_SHOWNOACTIVATE` and `WS_EX_NOACTIVATE`); with
/// `activate` true it is a normal show and the caller brings it to the front.
pub fn show_window(h: isize, activate: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SW_SHOW, SW_SHOWNOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    set_no_activate(h, !activate);
    unsafe {
        if activate {
            let _ = ShowWindow(hwnd(h), SW_SHOW);
            let _ = SetWindowPos(
                hwnd(h),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        } else {
            let _ = ShowWindow(hwnd(h), SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                hwnd(h),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

/// Full rectangles of all monitors (physical pixels).
pub fn monitors() -> Vec<Rect> {
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};
    use windows::core::BOOL;

    unsafe extern "system" fn collect(_m: HMONITOR, _dc: HDC, r: *mut RECT, l: LPARAM) -> BOOL {
        let v = unsafe { &mut *(l.0 as *mut Vec<Rect>) };
        if let Some(r) = unsafe { r.as_ref() } {
            v.push(rect_of(*r));
        }
        true.into()
    }

    let mut out: Vec<Rect> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut out as *mut Vec<Rect> as isize),
        );
    }
    out
}

/// A cheap fingerprint of the monitor layout (count and virtual screen); it changes when a
/// display is plugged, unplugged, moved or resized.
pub fn display_signature() -> [i32; 5] {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CMONITORS, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };
    unsafe {
        [
            GetSystemMetrics(SM_CMONITORS),
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        ]
    }
}

/// The foreground window's rectangle, its monitor's rectangle and its class name.
fn foreground_geometry() -> Option<(Rect, Rect, String)> {
    use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let fg = foreground_window();
    if fg == 0 {
        return None;
    }
    unsafe {
        let mut wr = windows::Win32::Foundation::RECT::default();
        GetWindowRect(hwnd(fg), &mut wr).ok()?;
        let mon = MonitorFromWindow(hwnd(fg), MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(mon, &mut info).as_bool() {
            return None;
        }
        Some((rect_of(wr), rect_of(info.rcMonitor), window_class(fg)))
    }
}

/// True when the foreground window covers a whole monitor (a game, a video): the clothesline must
/// not pop up over it.
pub fn fullscreen_foreground() -> bool {
    foreground_geometry()
        .is_some_and(|(win, mon, class)| crate::line::fullscreen_guard(win, mon, &class))
}

/// A point inside the foreground window (its centre): the monitor a peek appears on.
pub fn foreground_point() -> Option<(i32, i32)> {
    let (r, _, _) = foreground_geometry()?;
    Some(((r.0 + r.2) / 2, (r.1 + r.3) / 2))
}

/// `ShellExecuteW "open"` for a URL (the caller checks the scheme).
pub fn shell_open(url: &str) -> bool {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, w};
    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let r = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success.
    r.0 as isize > 32
}

// ---- OLE drag-out

use windows::Win32::System::Com::IDataObject;

/// A shell data object holding the files (`CF_HDROP` and the shell's other file formats).
pub fn file_data_object(paths: &[std::path::PathBuf]) -> Result<IDataObject, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::Com::{CoTaskMemFree, IBindCtx};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        BHID_DataObject, SHCreateShellItemArrayFromIDLists, SHParseDisplayName,
    };
    use windows::core::PCWSTR;

    if paths.is_empty() {
        return Err("no files".into());
    }
    let mut pidls: Vec<*mut ITEMIDLIST> = Vec::new();
    let result = (|| unsafe {
        for p in paths {
            let wide: Vec<u16> = p
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(wide.as_ptr()), None::<&IBindCtx>, &mut pidl, 0, None)
                .map_err(|e| e.to_string())?;
            pidls.push(pidl);
        }
        let ptrs: Vec<*const ITEMIDLIST> = pidls.iter().map(|p| *p as *const _).collect();
        let items = SHCreateShellItemArrayFromIDLists(&ptrs).map_err(|e| e.to_string())?;
        items
            .BindToHandler::<_, IDataObject>(None::<&IBindCtx>, &BHID_DataObject)
            .map_err(|e| e.to_string())
    })();
    for p in pidls {
        unsafe { CoTaskMemFree(Some(p as *const _)) };
    }
    result
}

/// A shell data object holding `text` as `CF_UNICODETEXT`.
pub fn text_data_object(text: &str) -> Result<IDataObject, String> {
    use windows::Win32::Foundation::{GlobalFree, HGLOBAL};
    use windows::Win32::System::Com::{
        DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::UI::Shell::SHCreateDataObject;

    const CF_UNICODETEXT: u16 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let data: IDataObject =
            SHCreateDataObject(None, None, None::<&IDataObject>).map_err(|e| e.to_string())?;
        let hmem: HGLOBAL =
            GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).map_err(|e| e.to_string())?;
        let dst = GlobalLock(hmem) as *mut u16;
        if dst.is_null() {
            let _ = GlobalFree(Some(hmem));
            return Err("GlobalLock failed".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
        let _ = GlobalUnlock(hmem);
        let fmt = FORMATETC {
            cfFormat: CF_UNICODETEXT,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: hmem },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        };
        // The data object owns the memory from here on (`frelease`).
        if let Err(e) = data.SetData(&fmt, &medium, true) {
            // Ownership did not transfer.
            let _ = GlobalFree(Some(hmem));
            return Err(e.to_string());
        }
        Ok(data)
    }
}

/// Run an OLE drag with the shell's default drop source (copy only). Blocks until the drop ends
/// or is cancelled. Must run on the UI thread, where OLE is initialised.
pub fn do_drag(owner: isize, data: &IDataObject) -> Result<(), String> {
    use windows::Win32::System::Ole::{DROPEFFECT_COPY, IDropSource};
    use windows::Win32::UI::Shell::SHDoDragDrop;
    unsafe {
        SHDoDragDrop(
            Some(hwnd(owner)),
            data,
            None::<&IDropSource>,
            DROPEFFECT_COPY,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod drag_tests {
    use super::*;
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, CoInitializeEx, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL,
    };

    fn fmt(cf: u16) -> FORMATETC {
        FORMATETC {
            cfFormat: cf,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        }
    }

    #[test]
    fn data_objects_carry_the_formats() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let text = text_data_object("merhaba").expect("text object");
        assert!(
            unsafe { text.QueryGetData(&fmt(13)) }.is_ok(),
            "CF_UNICODETEXT"
        );
        let file = std::env::temp_dir().join(format!("fc-drag-{}.txt", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let files = file_data_object(std::slice::from_ref(&file)).expect("file object");
        assert!(unsafe { files.QueryGetData(&fmt(15)) }.is_ok(), "CF_HDROP");
        let _ = std::fs::remove_file(&file);
        assert!(file_data_object(&[]).is_err());
    }
}
