//! The shortcuts rope: the apps the user can pick (Start menu, taskbar pins, desktop), their
//! icons (cached PNGs made by the shell) and starting one.

use crate::ipc::{CmdError, CmdResult};
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// An app the user can put on the rope.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub name: String,
    pub path: String,
    /// "taskbar", "start" or "desktop".
    pub place: String,
}

/// Size of the cached icons (px).
const ICON_SIDE: i32 = 64;

/// Shortcut-like files worth listing.
fn is_launchable(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["lnk", "url", "appref-ms", "exe"].contains(&e.to_ascii_lowercase().as_str())
    })
}

/// Start-menu entries that are not apps (uninstallers, help files, web pages of the vendor).
fn is_noise(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "uninstall",
        "kaldır",
        "readme",
        "help",
        "documentation",
        "website",
        "release notes",
    ]
    .iter()
    .any(|w| n.contains(w))
}

/// Walk `dir` (at most `depth` levels) and push its shortcut files.
fn walk(dir: &Path, depth: usize, place: &str, out: &mut Vec<AppEntry>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            if depth > 0 {
                walk(&p, depth - 1, place, out);
            }
        } else if is_launchable(&p) {
            let name = p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !name.is_empty() && !is_noise(&name) {
                out.push(AppEntry {
                    name,
                    path: p.to_string_lossy().into_owned(),
                    place: place.to_string(),
                });
            }
        }
    }
}

/// Keep the first entry of each name (case-insensitive), then sort by name.
pub fn dedupe_sorted(mut list: Vec<AppEntry>) -> Vec<AppEntry> {
    let mut seen = std::collections::HashSet::new();
    list.retain(|a| seen.insert(a.name.to_lowercase()));
    list.sort_by_key(|a| a.name.to_lowercase());
    list
}

/// `apps_list`: taskbar pins first (they win the name dedupe), then the Start menu (user and
/// all users), then desktop shortcuts.
pub fn list() -> Vec<AppEntry> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut out = Vec::new();
    if let Some(appdata) = env("APPDATA") {
        walk(
            &appdata.join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar"),
            0,
            "taskbar",
            &mut out,
        );
        walk(
            &appdata.join(r"Microsoft\Windows\Start Menu\Programs"),
            3,
            "start",
            &mut out,
        );
    }
    if let Some(pd) = env("ProgramData") {
        walk(
            &pd.join(r"Microsoft\Windows\Start Menu\Programs"),
            3,
            "start",
            &mut out,
        );
    }
    if let Some(profile) = env("USERPROFILE") {
        walk(&profile.join("Desktop"), 1, "desktop", &mut out);
    }
    if let Some(public) = env("PUBLIC") {
        walk(&public.join("Desktop"), 0, "desktop", &mut out);
    }
    dedupe_sorted(out)
}

/// Is `path` one of the shortcuts the user put on the rope?
fn on_rope(app: &AppHandle, path: &str) -> bool {
    let key = |p: &str| p.trim().to_lowercase().replace('/', "\\");
    app.state::<AppState>()
        .settings()
        .line_shortcuts
        .iter()
        .any(|s| key(&s.path) == key(path))
}

/// `line_shortcut_launch`: start a shortcut of the rope (only those).
pub fn launch(app: &AppHandle, path: &str) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    if !on_rope(app, path) {
        return Err(CmdError::new("notFound", "not on the shortcuts rope"));
    }
    if !Path::new(path).exists() {
        return Err(CmdError::new("notFound", "the shortcut's file is gone"));
    }
    crate::line::hide(app);
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

/// `line_shortcut_reveal`: show a shortcut of the rope in Explorer.
pub fn reveal(app: &AppHandle, path: &str) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    if !on_rope(app, path) {
        return Err(CmdError::new("notFound", "not on the shortcuts rope"));
    }
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

/// Folder of the cached app icons (allowed for the asset protocol at start-up).
pub fn icons_dir(app: &AppHandle) -> PathBuf {
    app.state::<AppState>()
        .paths
        .files_thumbs_dir()
        .join("apps")
}

/// `app_icons`: a cached PNG per path (None when the shell has no picture for it). Icons are made
/// once and kept until the shortcut file changes.
pub fn icons(app: &AppHandle, paths: &[String]) -> Vec<Option<String>> {
    let dir = icons_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    paths
        .iter()
        .map(|p| {
            let src = Path::new(p);
            let dst = dir.join(crate::files::thumb_name(src));
            let fresh = match (mtime(src), mtime(&dst)) {
                (Some(s), Some(d)) => d >= s,
                (None, Some(_)) => true,
                _ => false,
            };
            if fresh || make_icon(src, &dst) {
                Some(dst.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect()
}

fn mtime(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

#[cfg(windows)]
fn make_icon(src: &Path, dst: &Path) -> bool {
    match imp::shell_icon_rgba(src, ICON_SIDE) {
        Some((w, h, rgba)) => image::RgbaImage::from_raw(w, h, rgba)
            .is_some_and(|img| img.save_with_format(dst, image::ImageFormat::Png).is_ok()),
        None => false,
    }
}

#[cfg(not(windows))]
fn make_icon(_src: &Path, _dst: &Path) -> bool {
    false
}

#[cfg(windows)]
mod imp {
    use std::path::Path;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
    };
    use windows::core::HSTRING;

    /// The shell's icon of `path` at `side` px as straight RGBA (top row first).
    pub fn shell_icon_rgba(path: &Path, side: i32) -> Option<(u32, u32, Vec<u8>)> {
        unsafe {
            // S_FALSE / RPC_E_CHANGED_MODE: COM is already up on this thread, which is fine.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None).ok()?;
            let hbm: HBITMAP = factory
                .GetImage(SIZE { cx: side, cy: side }, SIIGBF_ICONONLY)
                .ok()?;
            let out = read_bitmap(hbm);
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            out
        }
    }

    unsafe fn read_bitmap(hbm: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
        unsafe {
            let mut bm = BITMAP::default();
            if GetObjectW(
                HGDIOBJ(hbm.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some((&raw mut bm).cast()),
            ) == 0
            {
                return None;
            }
            let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
            if w <= 0 || h <= 0 {
                return None;
            }
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let dc = CreateCompatibleDC(None);
            let lines = GetDIBits(
                dc,
                hbm,
                0,
                h as u32,
                Some(buf.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(dc);
            if lines == 0 {
                return None;
            }
            Some((w as u32, h as u32, super::bgra_to_rgba(buf)))
        }
    }
}

/// The shell hands out premultiplied BGRA: make it straight RGBA. A bitmap without any alpha is
/// treated as opaque.
pub fn bgra_to_rgba(mut buf: Vec<u8>) -> Vec<u8> {
    let has_alpha = buf.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
    for p in buf.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        } else if p[3] != 0 && p[3] != 255 {
            let a = u16::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, place: &str) -> AppEntry {
        AppEntry {
            name: name.into(),
            path: format!("C:\\{name}.lnk"),
            place: place.into(),
        }
    }

    #[test]
    fn first_of_a_name_wins_and_the_list_is_sorted() {
        let out = dedupe_sorted(vec![
            e("Zed", "taskbar"),
            e("paint", "taskbar"),
            e("Paint", "start"),
            e("Alpha", "start"),
        ]);
        let names: Vec<(&str, &str)> = out
            .iter()
            .map(|a| (a.name.as_str(), a.place.as_str()))
            .collect();
        assert_eq!(
            names,
            [("Alpha", "start"), ("paint", "taskbar"), ("Zed", "taskbar")]
        );
    }

    #[test]
    fn noise_and_kinds() {
        assert!(is_noise("Uninstall Foo"));
        assert!(is_noise("Foo'yu kaldır"));
        assert!(!is_noise("Discord"));
        assert!(is_launchable(Path::new("a.LNK")));
        assert!(is_launchable(Path::new("b.url")));
        assert!(!is_launchable(Path::new("c.txt")));
    }

    #[test]
    fn walk_finds_shortcuts() {
        let root = std::env::temp_dir().join(format!("fc-apps-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Sub")).unwrap();
        struct Gone(PathBuf);
        impl Drop for Gone {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let d = Gone(root);
        std::fs::write(d.0.as_path().join("One.lnk"), b"x").unwrap();
        std::fs::write(d.0.as_path().join("Sub").join("Two.url"), b"x").unwrap();
        std::fs::write(d.0.as_path().join("note.txt"), b"x").unwrap();
        let mut out = Vec::new();
        walk(d.0.as_path(), 0, "start", &mut out);
        assert_eq!(out.len(), 1);
        let mut out = Vec::new();
        walk(d.0.as_path(), 1, "start", &mut out);
        assert_eq!(dedupe_sorted(out).len(), 2);
    }

    #[test]
    fn pixels_become_straight_rgba() {
        // one half-transparent pixel (premultiplied BGRA) and one transparent one
        let out = bgra_to_rgba(vec![0, 0, 100, 128, 0, 0, 0, 0]);
        assert_eq!(&out[..4], &[199, 0, 0, 128]);
        // no alpha at all: opaque
        assert_eq!(bgra_to_rgba(vec![1, 2, 3, 0]), vec![3, 2, 1, 255]);
    }

    #[cfg(windows)]
    #[test]
    fn the_shell_draws_an_icon() {
        let (w, h, rgba) = imp::shell_icon_rgba(Path::new(r"C:\Windows\explorer.exe"), ICON_SIDE)
            .expect("explorer has an icon");
        assert_eq!((w, h), (64, 64));
        assert_eq!(rgba.len(), 64 * 64 * 4);
        assert!(rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0));
    }
}
