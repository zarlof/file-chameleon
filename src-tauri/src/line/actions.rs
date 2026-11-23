//! The work behind the line's commands: copy, paste, open, share, drag out, and the wheel's
//! file actions (copy path, open a terminal).

use super::*;

/// How many folders "open a terminal here" opens at most.
const MAX_TERMINALS: usize = 5;

/// The paths as clipboard text, one per line (CRLF).
pub fn copy_path_text(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// The folders to open terminals in: a file -> its folder, a folder -> itself. Duplicates are
/// dropped (first occurrence wins) and at most [`MAX_TERMINALS`] are kept.
pub fn terminal_dirs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in paths {
        let dir = if p.is_dir() {
            Some(p.as_path())
        } else {
            p.parent()
        };
        let Some(dir) = dir.filter(|d| !d.as_os_str().is_empty()) else {
            continue;
        };
        if !out.iter().any(|d| d == dir) {
            out.push(dir.to_path_buf());
        }
        if out.len() == MAX_TERMINALS {
            break;
        }
    }
    out
}

/// The folder argument for `wt -d`: a trailing backslash is dropped (it would escape the closing
/// quote of `"D:\My Dir\"`), except for a drive root such as `C:\`.
pub fn terminal_arg(dir: &Path) -> PathBuf {
    let s = dir.to_string_lossy();
    let trimmed = s.trim_end_matches('\\');
    let is_root = trimmed.len() == 2 && trimmed.ends_with(':');
    if trimmed.len() < s.len() && !is_root && !trimmed.is_empty() {
        PathBuf::from(trimmed)
    } else {
        dir.to_path_buf()
    }
}

/// What dragging a clip out of the line carries.
#[derive(Debug, PartialEq)]
pub enum DragPlan {
    /// An image: its file (the edited copy when there is one).
    File(PathBuf),
    /// Text or a link.
    Text(String),
    Nothing,
}

pub fn drag_plan(item: &fc_core::clip::ClipItem) -> DragPlan {
    use fc_core::clip::ClipKind;
    if item.kind == ClipKind::Image {
        return item
            .edited
            .clone()
            .or_else(|| item.image.clone())
            .map_or(DragPlan::Nothing, DragPlan::File);
    }
    match item.text.as_deref() {
        Some(t) if !t.is_empty() => DragPlan::Text(t.to_string()),
        _ => DragPlan::Nothing,
    }
}

/// A file the wheel can convert for clip `item` (Ctrl+click on a card): an image clip's picture
/// (the edited copy first), a text or link clip saved as `<text_dir>/<id>.txt` (written again
/// each time, so an edited text is current). The folder is inside the clipboard store, so the
/// results go to the Downloads folder like other clipboard conversions.
pub fn wheel_file(item: &fc_core::clip::ClipItem, text_dir: &Path) -> CmdResult<PathBuf> {
    match drag_plan(item) {
        DragPlan::File(p) if p.is_file() => Ok(p),
        DragPlan::Text(t) => {
            std::fs::create_dir_all(text_dir).map_err(|e| CmdError::new("io", e.to_string()))?;
            let p = text_dir.join(format!("{}.txt", item.id));
            std::fs::write(&p, t).map_err(|e| CmdError::new("io", e.to_string()))?;
            Ok(p)
        }
        _ => Err(CmdError::new("notFound", "the clip has no file to convert")),
    }
}

/// `line_clip_files`: the wheel's files for clips `ids` (see [`wheel_file`]); clips without one
/// are left out.
pub fn clip_files(app: &AppHandle, ids: &[u64]) -> CmdResult<Vec<String>> {
    let state = app.state::<AppState>();
    let text_dir = state.paths.clipboard_dir().join("text");
    let mut out = Vec::new();
    for id in ids {
        let item = crate::ipc::clipboard::find(&state, *id)?;
        if let Ok(p) = wheel_file(&item, &text_dir) {
            out.push(p.to_string_lossy().into_owned());
        }
    }
    if out.is_empty() {
        return Err(CmdError::new(
            "notFound",
            "the clips have no file to convert",
        ));
    }
    Ok(out)
}

/// `line_wheel` (Ctrl+click on cards): open the click-mode wheel for `paths` at the cursor. The
/// line stays open and keeps the keyboard: its page sends Tab and Esc on (`line_wheel_key`).
pub fn open_wheel(app: &AppHandle, paths: Vec<String>) {
    crate::diag::log(|| format!("line_wheel {} file(s)", paths.len()));
    // The user is probably still holding Ctrl: Tab and Esc must work anyway.
    crate::platform::allow_ctrl_keys();
    crate::windows::open_wheel_click(app, paths);
}

/// `line_wheel_key`: Tab (`flip`) or Esc (`close`) pressed on the line while its wheel is up.
/// Returns whether a wheel was open (if not, the page handles the key itself).
pub fn wheel_key(app: &AppHandle, key: &str) -> bool {
    let open = crate::windows::wheel_is_open();
    crate::diag::log(|| format!("line_wheel_key {key} open={open}"));
    if open {
        match key {
            "flip" => crate::windows::wheel_flip_from(app, crate::windows::FlipFrom::Line),
            "close" => crate::windows::wheel_close(app),
            _ => {}
        }
    }
    open
}

/// `line_wheel_start` (Ctrl+drag on cards): open the drag wheel under the cursor with the cards'
/// files on it (clips give the files of [`wheel_file`]). No OLE drag: the line page keeps the
/// pointer and reports its moves and the release (`line_wheel_pointer`), so the UI thread is never
/// stuck in a drag loop and the wheel follows live (hover, Tab, the mouse wheel, Esc).
pub fn wheel_start(app: &AppHandle, ids: &[u64], paths: &[String], shift: bool) -> CmdResult<()> {
    let mut files: Vec<PathBuf> = if paths.is_empty() {
        Vec::new()
    } else {
        existing_files(app, paths)?
    };
    if !ids.is_empty() {
        files.extend(clip_files(app, ids)?.into_iter().map(PathBuf::from));
    }
    // The line stays open: a focus move to the wheel must not count as leaving an activated line
    // (it then closes when the pointer leaves it, as a peek does).
    ACTIVE.store(false, Ordering::SeqCst);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let a = app.clone();
    app.run_on_main_thread(move || {
        #[cfg(windows)]
        let (x, y) = crate::platform::win::cursor_pos();
        #[cfg(not(windows))]
        let (x, y) = (0, 0);
        crate::diag::log(|| format!("line_wheel_start cursor=({x},{y}) files={}", names.len()));
        crate::windows::show_wheel_at(&a, x, y, shift);
        // We know the files: the slots go out at once, no drag-enter needed.
        crate::windows::files_entered(&a, names);
    })
    .map_err(|e| CmdError::new("failed", e.to_string()))
}

/// Only web links are opened.
pub fn is_web_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

// ---------------------------------------------------------------- commands' work

/// `line_paste`: copy the items, hide the line and paste into the window it came from.
pub fn paste(app: &AppHandle, ids: &[u64]) -> CmdResult<()> {
    crate::ipc::clipboard::copy_items(&app.state::<AppState>(), ids)?;
    hide(app);
    let prev = app.state::<AppState>().line_prev.load(Ordering::SeqCst);
    paste_keys_into(prev);
    Ok(())
}

/// `line_file_paste`: copy the files (a file list), hide the line and paste into the window it
/// came from.
pub fn paste_files(app: &AppHandle, paths: &[String]) -> CmdResult<()> {
    copy_files(app, paths)?;
    hide(app);
    let prev = app.state::<AppState>().line_prev.load(Ordering::SeqCst);
    paste_keys_into(prev);
    Ok(())
}

/// Text the user made (OCR, a text action on a card) on the clipboard as a normal copy: the
/// history records it, so it also hangs on the line as a new card.
pub fn copy_new_text(text: &str) -> CmdResult<()> {
    crate::platform::clipboard::write_text(text)
        .map(|_| ())
        .map_err(|e| CmdError::new("clipboard", e))
}

/// "Copy text": `text` (already recognized) copied as a new clip with a toast, or the "no text"
/// toast. Returns the number of characters copied.
fn copy_recognized(app: &AppHandle, text: &str) -> CmdResult<usize> {
    let text = text.trim();
    if text.is_empty() {
        crate::windows::show_toast(app, "noText", serde_json::json!({}), Some(3000));
        return Ok(0);
    }
    copy_new_text(text)?;
    let chars = text.chars().count();
    crate::windows::show_toast(
        app,
        "textCopied",
        serde_json::json!({ "chars": chars }),
        Some(3000),
    );
    Ok(chars)
}

/// The wheel's "Copy text": read the text in the images and copy it.
pub fn copy_image_text(app: &AppHandle, paths: &[PathBuf]) -> CmdResult<usize> {
    let text = crate::platform::ocr::recognize_files(paths).map_err(|e| CmdError::new("ocr", e))?;
    copy_recognized(app, &text)
}

/// `line_image_text`: "Copy text" on an image card: its stored OCR text when there is some, else
/// read now (OCR off, or not done yet).
pub fn copy_clip_text(app: &AppHandle, id: u64) -> CmdResult<usize> {
    let item = {
        let state = app.state::<AppState>();
        let store = state.clips().lock().unwrap_or_else(|e| e.into_inner());
        store.get(id).cloned()
    }
    .ok_or_else(|| CmdError::new("notFound", "no such clip"))?;
    if let Some(text) = item.ocr.as_deref().filter(|t| !t.trim().is_empty()) {
        return copy_recognized(app, text);
    }
    let path = item
        .edited
        .or(item.image)
        .ok_or_else(|| CmdError::new("notFound", "not an image"))?;
    copy_image_text(app, &[path])
}

/// `line_copy_text`: text on the clipboard as our own write, so it is neither recorded as a clip
/// nor peeked at.
pub fn copy_text(app: &AppHandle, text: &str) -> CmdResult<()> {
    match crate::platform::clipboard::write_text(text) {
        Ok(seq) => {
            app.state::<AppState>()
                .clip_self_write
                .store(seq, Ordering::SeqCst);
            Ok(())
        }
        Err(e) => Err(CmdError::new("clipboard", e)),
    }
}

/// `line_open_item`: the clipboard library, with that clip selected.
pub fn open_item(app: &AppHandle, id: u64) {
    hide(app);
    crate::windows::show_main(
        app,
        Some(NavPayload {
            view: "clipboard".into(),
            paths: vec![],
            url: None,
            clip: Some(id),
        }),
    );
}

/// `line_open_clip`: a click on a card. An image opens in the default image viewer, text in the
/// library (where it can be edited), a link in the browser.
pub fn open_clip(app: &AppHandle, id: u64) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let item = crate::ipc::clipboard::find(&app.state::<AppState>(), id)?;
    match (
        item.kind == fc_core::clip::ClipKind::Image,
        drag_plan(&item),
    ) {
        (true, DragPlan::File(path)) if path.is_file() => {
            hide(app);
            app.opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|e| CmdError::new("io", e.to_string()))
        }
        (false, DragPlan::Text(t)) if is_web_url(&t) => {
            hide(app);
            open_url(t.trim())
        }
        _ => {
            open_item(app, id);
            Ok(())
        }
    }
}

/// `line_share`: the Windows share sheet for clips (`ids`) and files of the files rope (`paths`).
pub fn share(app: &AppHandle, ids: &[u64], paths: &[String]) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let mut data = crate::platform::share::ShareData {
        files: if paths.is_empty() {
            Vec::new()
        } else {
            existing_files(app, paths)?
        },
        text: None,
        title: "File Chameleon".into(),
    };
    let mut texts = Vec::new();
    for id in ids {
        match drag_plan(&crate::ipc::clipboard::find(&state, *id)?) {
            DragPlan::File(p) => data.files.push(p),
            DragPlan::Text(t) => texts.push(t),
            DragPlan::Nothing => {}
        }
    }
    if !texts.is_empty() {
        data.text = Some(texts.join(
            "
",
        ));
    }
    if let [one] = data.files.as_slice()
        && let Some(name) = one.file_name()
    {
        data.title = name.to_string_lossy().into_owned();
    }
    crate::platform::share::share(data).map_err(|e| CmdError::new("share", e))
}

/// `line_drag_out`: start an OLE drag of clip `id` (copy only) and wait for the drop to end. The
/// drag runs on the main (UI) thread, which blocks inside `SHDoDragDrop` until then.
pub async fn drag_out(app: &AppHandle, id: u64) -> CmdResult<()> {
    let item = crate::ipc::clipboard::find(&app.state::<AppState>(), id)?;
    run_drag(app, drag_plan(&item)).await
}

/// `line_file_drag_out`: the same drag for a file of the files rope.
pub async fn drag_out_file(app: &AppHandle, path: PathBuf) -> CmdResult<()> {
    let found = existing_files(app, &[path.to_string_lossy().into_owned()])?;
    run_drag(app, DragPlan::File(found[0].clone())).await
}

async fn run_drag(app: &AppHandle, plan: DragPlan) -> CmdResult<()> {
    if plan == DragPlan::Nothing {
        return Err(CmdError::new("notFound", "nothing to drag"));
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(CmdError::new("unsupported", "drag out needs Windows"))
    }
    #[cfg(windows)]
    {
        use crate::platform::win;
        let owner = app
            .get_webview_window(LINE)
            .and_then(|w| hwnd_of(&w))
            .ok_or_else(|| CmdError::new("failed", "line window is not open"))?;
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
        app.run_on_main_thread(move || {
            let result = match plan {
                DragPlan::File(p) => win::file_data_object(&[p]),
                DragPlan::Text(t) => win::text_data_object(&t),
                DragPlan::Nothing => Err("nothing to drag".into()),
            }
            .and_then(|data| win::do_drag(owner, &data));
            let _ = tx.send(result);
        })
        .map_err(|e| CmdError::new("failed", e.to_string()))?;
        crate::ipc::blocking(move || rx.recv())
            .await?
            .map_err(|e| CmdError::new("failed", e.to_string()))?
            .map_err(|e| CmdError::new("drag", e))
    }
}

/// The files of `paths` that are on the files rope (and still exist); `notFound` when there is
/// none. The file commands never act on a path that is not a current entry of the rope.
fn existing_files(app: &AppHandle, paths: &[String]) -> CmdResult<Vec<PathBuf>> {
    let found = crate::files::listed(app, paths);
    if found.is_empty() {
        return Err(CmdError::new("notFound", "not on the files rope or gone"));
    }
    Ok(found)
}

/// `line_file_copy`: the files as a file list on the clipboard (copy). Our own write is not
/// recorded as a new clip.
pub fn copy_files(app: &AppHandle, paths: &[String]) -> CmdResult<()> {
    let files = existing_files(app, paths)?;
    match crate::platform::clipboard::write_files(&files) {
        Ok(seq) => {
            app.state::<AppState>()
                .clip_self_write
                .store(seq, Ordering::SeqCst);
            Ok(())
        }
        Err(e) => Err(CmdError::new("clipboard", e)),
    }
}

/// `line_file_open`: the file in its default app.
pub fn open_file(app: &AppHandle, path: &str) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    existing_files(app, &[path.to_string()])?;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

/// `line_file_reveal`: the file selected in Explorer.
pub fn reveal_file(app: &AppHandle, path: &str) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    existing_files(app, &[path.to_string()])?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| CmdError::new("io", e.to_string()))
}

/// `open_url`: web links only.
pub fn open_url(url: &str) -> CmdResult<()> {
    if !is_web_url(url) {
        return Err(CmdError::new("invalid", "only http and https links open"));
    }
    #[cfg(windows)]
    if crate::platform::win::shell_open(url.trim()) {
        return Ok(());
    }
    Err(CmdError::new("io", "could not open the link"))
}

// ---------------------------------------------------------------- file actions (wheel)

/// The `copyPath` preset: the paths, one per line, on the clipboard. Our own write is not
/// recorded as a new clip.
pub fn copy_paths(app: &AppHandle, paths: &[PathBuf]) -> CmdResult<()> {
    let text = copy_path_text(paths);
    match crate::platform::clipboard::write_text(&text) {
        Ok(seq) => {
            app.state::<AppState>()
                .clip_self_write
                .store(seq, Ordering::SeqCst);
            Ok(())
        }
        Err(e) => Err(CmdError::new("clipboard", e)),
    }
}

/// The `openTerminal` preset: Windows Terminal in each folder (at most five), PowerShell when
/// `wt.exe` is not there.
pub fn open_terminal(paths: &[PathBuf]) -> CmdResult<()> {
    let dirs = terminal_dirs(paths);
    if dirs.is_empty() {
        return Err(CmdError::new("unsupported", "no folder to open"));
    }
    let mut failed = None;
    for dir in &dirs {
        if let Err(e) = spawn_terminal(dir) {
            failed = Some(e);
        }
    }
    failed.map_or(Ok(()), |e| Err(CmdError::new("io", e.to_string())))
}

#[cfg(windows)]
fn spawn_terminal(dir: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    // wt.exe makes its own window: no console flash for the launcher.
    let wt = Command::new("wt.exe")
        .arg("-d")
        .arg(terminal_arg(dir))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    match wt {
        Ok(_) => Ok(()),
        Err(_) => Command::new("powershell.exe")
            .arg("-NoExit")
            .current_dir(dir)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map(|_| ()),
    }
}

#[cfg(not(windows))]
fn spawn_terminal(_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("terminals need Windows"))
}
