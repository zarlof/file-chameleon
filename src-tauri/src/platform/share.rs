//! The Windows share sheet for cards of the line: files (and clip images) as storage items, text
//! and links as text. The sheet needs a top-level window of its own, so a worker thread makes an
//! invisible 1 x 1 tool window at the pointer, asks the `DataTransferManager` of that window to
//! show the sheet, and keeps the window (and its message loop) alive while the sheet may still
//! ask for the data. The line itself may hide meanwhile.

use std::path::PathBuf;

/// What to share.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShareData {
    pub files: Vec<PathBuf>,
    pub text: Option<String>,
    pub title: String,
}

impl ShareData {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.text.as_deref().is_none_or(str::is_empty)
    }
}

/// Open the share sheet for `data` (returns at once; the sheet runs on its own thread).
pub fn share(data: ShareData) -> Result<(), String> {
    if data.is_empty() {
        return Err("nothing to share".into());
    }
    #[cfg(windows)]
    {
        std::thread::Builder::new()
            .name("fc-share".into())
            .spawn(move || {
                if let Err(e) = imp::run(data) {
                    eprintln!("share: {e}");
                }
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = data;
        Err("sharing needs Windows".into())
    }
}

#[cfg(windows)]
mod imp {
    use super::ShareData;
    use windows::ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager};
    use windows::Foundation::{TypedEventHandler, Uri};
    use windows::Storage::{IStorageItem, StorageFile};
    use windows::Win32::Foundation::{COLORREF, HWND, POINT};
    use windows::Win32::System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize};
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, KillTimer,
        LWA_ALPHA, MSG, SW_SHOW, SetForegroundWindow, SetLayeredWindowAttributes, SetTimer,
        ShowWindow, TranslateMessage, WM_TIMER, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_POPUP,
    };
    use windows::core::{AgileReference, HSTRING, Interface, w};

    /// How long the helper window lives: the sheet asks for the data when it opens, but a
    /// target app may read the files later, while the sheet is still up.
    const KEEP_MS: u32 = 10 * 60 * 1000;

    pub fn run(data: ShareData) -> windows::core::Result<()> {
        // S_FALSE (already initialized) is fine.
        let _ = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) };
        let mut items: Vec<IStorageItem> = Vec::new();
        for p in &data.files {
            match StorageFile::GetFileFromPathAsync(&HSTRING::from(p.as_os_str()))
                .and_then(|op| op.join())
            {
                Ok(f) => items.push(f.cast()?),
                Err(e) => eprintln!("share {}: {e}", p.display()),
            }
        }
        if items.is_empty() && data.text.as_deref().is_none_or(str::is_empty) {
            return Ok(());
        }
        let hwnd = helper_window()?;
        let result = show(hwnd, items, data);
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        result
    }

    fn helper_window() -> windows::core::Result<HWND> {
        unsafe {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TOPMOST,
                w!("STATIC"),
                w!("File Chameleon"),
                WS_POPUP,
                pt.x,
                pt.y,
                1,
                1,
                None,
                None,
                None,
                None,
            )?;
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            Ok(hwnd)
        }
    }

    fn show(hwnd: HWND, items: Vec<IStorageItem>, data: ShareData) -> windows::core::Result<()> {
        let interop = windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()?;
        let dtm: DataTransferManager = unsafe { interop.GetForWindow(hwnd)? };
        let title = HSTRING::from(if data.title.is_empty() {
            "File Chameleon"
        } else {
            data.title.as_str()
        });
        let text = data.text.clone();
        // The handler may run on another thread: hand it agile references.
        let items: Vec<AgileReference<IStorageItem>> = items
            .iter()
            .map(AgileReference::new)
            .collect::<windows::core::Result<_>>()?;
        let token = dtm.DataRequested(&TypedEventHandler::new(
            move |_, args: windows::core::Ref<DataRequestedEventArgs>| {
                let args = args.ok()?;
                let package = args.Request()?.Data()?;
                package.Properties()?.SetTitle(&title)?;
                if !items.is_empty() {
                    let mut resolved = Vec::with_capacity(items.len());
                    for r in &items {
                        resolved.push(Some(r.resolve()?));
                    }
                    let list: windows_collections::IIterable<IStorageItem> = resolved.into();
                    package.SetStorageItemsReadOnly(&list)?;
                }
                if let Some(t) = text.as_deref().filter(|t| !t.is_empty()) {
                    package.SetText(&HSTRING::from(t))?;
                    if crate::line::is_web_url(t)
                        && let Ok(uri) = Uri::CreateUri(&HSTRING::from(t.trim()))
                    {
                        let _ = package.SetWebLink(&uri);
                    }
                }
                Ok(())
            },
        ))?;
        let shown = unsafe { interop.ShowShareUIForWindow(hwnd) };
        if shown.is_ok() {
            unsafe {
                SetTimer(Some(hwnd), 1, KEEP_MS, None);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_TIMER && msg.hwnd == hwnd {
                        break;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = KillTimer(Some(hwnd), 1);
            }
        }
        let _ = dtm.RemoveDataRequested(token);
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_is_refused() {
        assert!(ShareData::default().is_empty());
        assert!(
            ShareData {
                text: Some(String::new()),
                ..Default::default()
            }
            .is_empty()
        );
        assert!(
            !ShareData {
                text: Some("hi".into()),
                ..Default::default()
            }
            .is_empty()
        );
        assert!(share(ShareData::default()).is_err());
    }
}
