//! Global low-level mouse hook that detects "Ctrl + drag" and asks for the wheel.
//!
//! The callback only does atomic bookkeeping and a channel send (Windows silently removes
//! LL hooks that block ~300 ms) and always calls `CallNextHookEx`, so normal drags are
//! never affected. Filtering (which app the drag started in) happens on the consumer thread.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering::Relaxed};
use std::sync::mpsc::{Sender, channel};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, GetSystemMetrics, MSG, MSLLHOOKSTRUCT,
    SM_CXDRAG, SM_CYDRAG, SetWindowsHookExW, TranslateMessage, WH_MOUSE_LL, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE,
};

#[derive(Debug, Clone, Copy)]
pub enum HookEvent {
    /// Ctrl held while dragging past the system drag threshold. `down_*` = where the press began.
    DragStart {
        x: i32,
        y: i32,
        down_x: i32,
        down_y: i32,
    },
    /// Left button released after a `DragStart`.
    Released,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static ANY_APP: AtomicBool = AtomicBool::new(false);
static DOWN: AtomicBool = AtomicBool::new(false);
static TRIGGERED: AtomicBool = AtomicBool::new(false);
static DOWN_X: AtomicI32 = AtomicI32::new(0);
static DOWN_Y: AtomicI32 = AtomicI32::new(0);
static TH_X: AtomicI32 = AtomicI32::new(4);
static TH_Y: AtomicI32 = AtomicI32::new(4);
static SENDER: OnceLock<Sender<HookEvent>> = OnceLock::new();

pub fn set_config(enabled: bool, any_app: bool) {
    ENABLED.store(enabled, Relaxed);
    ANY_APP.store(any_app, Relaxed);
}

pub fn any_app() -> bool {
    ANY_APP.load(Relaxed)
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && ENABLED.load(Relaxed) {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let (x, y) = (info.pt.x, info.pt.y);
        match wparam.0 as u32 {
            WM_LBUTTONDOWN => {
                DOWN.store(true, Relaxed);
                TRIGGERED.store(false, Relaxed);
                DOWN_X.store(x, Relaxed);
                DOWN_Y.store(y, Relaxed);
            }
            WM_MOUSEMOVE if DOWN.load(Relaxed) && !TRIGGERED.load(Relaxed) => {
                let (dx, dy) = (
                    (x - DOWN_X.load(Relaxed)).abs(),
                    (y - DOWN_Y.load(Relaxed)).abs(),
                );
                // Use a slightly larger threshold than the system's so clicks never trigger.
                if dx > TH_X.load(Relaxed) * 2 || dy > TH_Y.load(Relaxed) * 2 {
                    let ctrl =
                        unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } as u16 & 0x8000 != 0;
                    if ctrl {
                        TRIGGERED.store(true, Relaxed);
                        if let Some(tx) = SENDER.get() {
                            let _ = tx.send(HookEvent::DragStart {
                                x,
                                y,
                                down_x: DOWN_X.load(Relaxed),
                                down_y: DOWN_Y.load(Relaxed),
                            });
                        }
                    }
                }
            }
            WM_LBUTTONUP => {
                if TRIGGERED.swap(false, Relaxed)
                    && let Some(tx) = SENDER.get()
                {
                    let _ = tx.send(HookEvent::Released);
                }
                DOWN.store(false, Relaxed);
            }
            _ => {}
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Install the hook once (idempotent). `on_event` runs on a dedicated consumer thread.
pub fn ensure_started(on_event: impl Fn(HookEvent) + Send + 'static) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Relaxed) {
        return;
    }
    let (tx, rx) = channel::<HookEvent>();
    let _ = SENDER.set(tx);
    std::thread::Builder::new()
        .name("fc-mouse-hook".into())
        .spawn(|| unsafe {
            TH_X.store(GetSystemMetrics(SM_CXDRAG).max(1), Relaxed);
            TH_Y.store(GetSystemMetrics(SM_CYDRAG).max(1), Relaxed);
            let module = GetModuleHandleW(None).ok().map(|m| m.into());
            let Ok(_hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), module, 0) else {
                eprintln!("mouse hook: install failed");
                return;
            };
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .expect("spawn hook thread");
    std::thread::Builder::new()
        .name("fc-mouse-events".into())
        .spawn(move || {
            for ev in rx {
                on_event(ev);
            }
        })
        .expect("spawn hook consumer");
}

/// Should a drag that started at this point open the wheel?
/// Default: only Explorer windows and the desktop. Never our own windows.
pub fn source_allowed(down_x: i32, down_y: i32) -> bool {
    match super::win::process_at(down_x, down_y) {
        Some((_, pid)) if pid == super::win::current_pid() => false,
        Some((name, _)) => any_app() || name == "explorer.exe",
        None => any_app(),
    }
}
