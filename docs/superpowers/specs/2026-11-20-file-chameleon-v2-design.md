# File Chameleon v2 — Design

Builds on v1 (`2026-11-19-file-chameleon-design.md`). Same stack: Tauri 2 + Rust (`crates/fc-core`,
`src-tauri`) + Svelte 5. Original code only; Filee, Nashi and Clipboardline are references for
features and feel, never for code or assets.

## Why

The user (Turkish, student) wants the app to look like a native Windows app and to match what
Filee, Nashi and Clipboardline offer:

1. Their File Chameleon shortcut now lives in `Desktop\Klasör\ZARLOF\File Chameleon.lnk`.
   Reinstalls must keep it there and must not leave a Desktop copy.
2. Remove the macOS traffic lights. Put Windows 11 caption buttons (– □ ✕) at the top right.
3. New app icon: a **green chameleon** on a **brown Windows-style folder**.
4. Fill the empty sidebar space under "Ayarlar" with Filee/Nashi-style settings pages. The most
   important one is a **wheel editor** (Filee's "Donut toolbar"). The wheel itself should look
   and behave like Filee's donut (screenshot: light slices, purple hovered slice, white centre
   "1 JPG / Esc to close", file chip at the cursor). Add Filee's newer features where they fit.
5. Implement Clipboardline's feature set (not just v1's light "smart clipboard"), with its own
   settings page.

The user opted out of approval questions ("benden izin isteme"). Calls below marked **(call)**
are mine and are stated to the user.

## Goals / non-goals / anti-goals

- Goals: everything in "Why"; keep RAM and size low (idle in tray stays ≈ 100 MB, installer
  stays under ~6 MB); TR + EN for every new string; tests for all new pure logic.
- Non-goals (**call**, too large or needs huge engines): LibreOffice/Calibre/Pandoc/Ghostscript
  formats (documents, spreadsheets, presentations, e-books, CAD, fonts, 3D, geospatial), AI
  upscale/translation/Whisper, Filee's preset gallery and phone connect, Clipboardline's visual
  (CLIP) search and screen-capture hotkey (Windows' own Win+Shift+S already lands in our shelf),
  Windows 11 snap-layout flyout on the maximize button, auto-update.
- Anti-goals: anything that copies Filee/Clipboardline code, icons or text; network access at
  copy time unless the user opts in; global hooks that block or swallow input outside the wheel.

## 1. Window chrome and sidebar

- `TitleBar.svelte`: full-width drag region (`data-tauri-drag-region`, double-click maximizes),
  three caption buttons at the **top right**, 46×32 px each, glyphs from the system font
  `"Segoe Fluent Icons", "Segoe MDL2 Assets"` (E921 minimize, E922 maximize, E923 restore,
  E8BB close). Hover: subtle fill (`rgba(0,0,0,.06)` light / `rgba(255,255,255,.08)` dark);
  close hover `#C42B1C` with a white glyph; pressed states; glyphs dim while the window is
  unfocused. Maximize/restore glyph follows `isMaximized()` (updated on resize).
- Sidebar: logo + name at the very top (no room reserved for traffic lights any more). Two
  sections:
  - main: Dönüştür, İndir, Pano, Son işlemler
  - **AYARLAR** (a small caption, not clickable): Genel, Çark, Hazır ayarlar, Kısayollar,
    İzlenen klasörler, Pano ayarları, Görünüm, Motorlar, Hakkında
  - bottom: GitHub button (opens the repo) and the version.
  The nav list scrolls when the window is short. Collapsed mode (< 860 px) shows icons, and the
  section caption becomes a thin separator.
- `View` grows to: `convert | download | clipboard | recent | general | wheel | presets |
  shortcuts | watch | clipboardSettings | appearance | engines | about | editor`. `editor` is
  the image editor (not in the sidebar). `Ctrl+1..4` = main views, `Ctrl+,` = Genel.
- Dönüştür (Filee "Home"): under the drop card a hint "Explorer'da Ctrl'ye basılı tutarak
  sürükleyin; çark imlecin yanında açılır" and a **Son dönüşümler** list (last 6 finished jobs,
  open / show in folder, "Temizle").

## 2. Icon

Hand-written SVG `assets/icon.svg` (**call**: SVG instead of the old PIL script, so the art is
editable): a brown Windows-11-style folder (darker back with tab, lighter front panel, soft
gradient) and a green chameleon on the front panel (curled tail, crest, big eye, feet on a
twig-like edge). Rendered to `assets/icon-source.png` (1024²) by
`cargo run -p fc-core --example svg_to_png`, then `npx tauri icon assets/icon-source.png -o
src-tauri/icons`. Sidebar logo and tray use the new icons automatically.

## 3. Presets, profiles and the wheel (Filee "Donut toolbar")

### Model (`fc_core::presets`, stored in `<config>/wheel.json`)

```
WheelConfig { version, presets: Vec<Preset>, groups: Vec<Group>, profiles: Vec<Profile> }
Preset  { id, name /* "" = built-in, UI localizes by id */, builtin, op: Operation, options: ConvertOptions }
Group   { id, name, presets: Vec<String> /* ≤ 6 */ }
Profile { id, name, builtin, extensions: Vec<String>, mixed: bool,
          front: Vec<String> /* ≤ 8 slot ids */, back: Vec<String> /* ≤ 12 */ }
slot id = preset id | "group:<group id>"
```

- Built-in presets have stable ids: `to-<format>` (convert; `to-jpg-70` = JPG quality 70),
  image edits `grayscale rotate-left rotate-right rotate-180 flip-h flip-v trim pixelate blur
  resize-50 fit-1920 annotate`, media `compress mute speed-2x speed-half crop-16-9 crop-9-16
  crop-1-1 crop-4-3`, pdf `merge-pdf split-pdf`, files `zip extract copy-to move-to`.
- Built-in profiles: Görseller, Vektör, Video, Ses, PDF, Metin ve Markdown, Veri, Altyazılar,
  Arşivler, **Karışık** (`mixed: true`) and **Diğer dosyalar** (`id: "other"`, no extensions:
  any unknown file gets ZIP / copy / move). Users can add profiles (any extensions, e.g.
  `docx` → ZIP / copy), presets and groups; built-ins can be duplicated, re-arranged and reset.
- Loading merges new built-ins into an older file without touching the user's arrangement.
  Invalid references are dropped on load; limits are enforced on save.
- Resolution for dropped files: all files in one profile → that profile; several profiles → the
  first `mixed` profile; unknown extensions → `other`. Slots whose preset cannot handle *every*
  input are hidden (convert: planner route exists; edits: raster images; crop: image or video;
  merge: pdf/images; files ops: anything).

### New operations (`fc_core::jobs::Operation`)

`Edit { ops: Vec<ImageOp>, to: Option<String> }` (Grayscale, RotateLeft, RotateRight,
Rotate180, FlipH, FlipV, Trim (transparent borders), Pixelate{block}, Blur{sigma},
Scale{percent}; max size via options), `CropAspect { w, h }` (centre crop; images in-process,
video with ffmpeg), `CopyTo { dir: Option<PathBuf>, moveFiles }` (`None` → folder picker at drop
time), `Zip` (all inputs → one `.zip` next to the first), `Extract` (`.zip` → folder), `Annotate`
(never queued: opens the image editor).

New formats: `m4r` (iPhone ringtone, ≤ 40 s AAC), `tsv`, subtitles `srt vtt ass`
(`srt↔vtt`, `→txt` in-process; `ass→srt/vtt` via ffmpeg), `zip` (input only). New kinds
`subtitle`, `archive`.

### Wheel behaviour

- Triggers: Ctrl+drag files from Explorer/desktop (as v1); **Ctrl+Shift+drag opens the back
  face**; Explorer right-click "File Chameleon ile dönüştür" (opt-in setting, HKCU registry verb,
  `--wheel <file>`; one wheel for multi-selections by collecting paths for ~350 ms); a "Çarkı aç"
  button in Dönüştür. The last two open the wheel in **click mode** at the cursor.
- Look (Filee-like, original): light slices with even gaps, bold uppercase labels with an
  optional muted detail line ("70%", "16:9", "2×"), hovered slice filled with the wheel colour
  (default purple) and white text, white centre disc with a soft ring showing "1 JPG" (or
  "3 dosya") and "Esc ile kapat"; a flip hint when a back face exists. Label mode
  (auto / icon+text / text / icon), opacity, size and colour are settings.
- Flip front ↔ back with the mouse wheel or Tab (both via the global hooks while the wheel is
  open, since the overlay never has focus); slices turn over in a short wave.
- Groups: a group slot shows a chevron; resting on it (~200 ms) or moving outward fans its
  presets out in an outer ring; drop/click on a child runs it.
- A small file chip (kind icon + extension badge, image thumbnail when cheap) follows the
  cursor while dragging over the wheel.
- Esc closes (drag: the drag itself is cancelled by Windows; click mode: our keyboard hook).
  Click mode: a click outside the wheel closes it.
- Missing engines: the slot shows a download badge; dropping offers to install, then runs.

### Wheel editor ("Çark") and Presets ("Hazır ayarlar") pages

Filee-like three columns: (1) profiles with counts "ön·arka", search, "+ Yeni profil";
(2) Ön/Arka switch, a live donut preview (same component as the overlay) with selectable slots,
the ordered slot list (move ←/→, remove), profile options: name, "Karışık seçimlerde kullan",
extension chips, reset, delete (custom); (3) available presets with search, filter chips
(Tümü, Görsel, Belge, Video ve ses, Düzenle, Dosya), "+ Yeni hazır ayar", "+ Yeni grup",
"Klasöre kopyala" (quick preset with a chosen folder); presets that do not fit the profile are
dimmed with the reason on hover. No HTML5 drag-and-drop (Tauri's file-drop handler disables it
on Windows): clicks, buttons and keyboard (Alt+←/→) instead.

Presets page: list + editor (name, action, target format, quality, max width/height, edit ops
with parameters, copy/move folder, output folder), duplicate built-ins, delete user presets.

## 4. Other Filee/Nashi settings

- **Genel**: language, launch at login, close button → tray or quit, Explorer right-click entry,
  output folder, default image quality, download folder / mode / quality / cookies.
- **Kısayollar**: wheel trigger on/off, all apps vs Explorer only, never-trigger app list,
  flip with mouse wheel / Tab, Ctrl+Shift = back face; global hotkeys with a recorder:
  open clipboard shelf (`Ctrl+Alt+V`), open clipboard library (off), download the copied link
  (`Ctrl+Alt+D`, from Nashi). A hotkey that another app owns is reported.
- **İzlenen klasörler** (Filee watch folders): folder + preset + subfolders + on/off. New files
  are processed once they are complete (size and mtime unchanged for 2 s, not locked), results
  go to `<folder>\converted`, temp files (`.crdownload .part .tmp ~$*`) and our own outputs are
  ignored, files present when watching starts are not touched. Polling every 2 s on one thread
  (**call**: no new dependency, works on network drives).
- **Görünüm**: theme, accent, UI scale (80–150 %, webview zoom), reduce motion, wheel colour,
  wheel opacity, wheel labels, wheel size, show format detail.
- **Motorlar**, **Hakkında** (version, GitHub, licences, "inspired by" note): moved from v1.

## 5. Clipboard (Clipboardline feature set)

### Capture

Per-kind switches (text, links, images), ignore blank copies, skip password-manager/private
markers (v1), skip our own writes (v1), **ignored apps** (clipboard owner's exe name), dedupe
refreshes the existing item. Optional **watch image folders** (≤ 3, e.g. Screenshots) import new
images. **OCR** with the built-in Windows OCR (`Windows.Media.Ocr`, offline, uses the user's
installed languages) runs in the background for images; the text is searchable and copyable.
**Link titles** (opt-in, off by default): fetch the page title once in the background; local
addresses and URLs with secret-looking parameters are skipped.

### Store (`ClipItem` v2)

Adds `title, note, tags, folder, ocr, trashedAt, sourceApp, linkTitle, edited` (edited image
copy; original kept). Folders `{id, name, parent}`. An item is **saved** when pinned, in a folder,
tagged, titled or noted: saved items never expire. Retention per kind (1/3/7/15/30 days or
forever) applies to unsaved items, plus an overall cap. Delete moves to **Çöp kutusu** (restore,
undo, empty; trash auto-empties after 30 days). "Kaydedilmemişleri temizle" trashes all unsaved.

### Shelf (quick paste overlay) — `shelf.html`

Global hotkey opens a translucent panel that slides down at the top of the screen under the
cursor: three horizontal rows **Görseller / Metinler / Bağlantılar** (visible rows and order are
settings), a search field (typing anywhere starts a search across text, titles, notes, tags,
OCR and link titles), pinned-only filter. Keyboard: arrows, Home/End, 1–9 pick, Shift/Ctrl
multi-select, **Enter** copy and close, **Ctrl+Enter** paste into the previous app, Ctrl+C copy
and stay, P pin, Delete trash, Ctrl+Z undo, Space preview, Esc close. Click copies (or selects
in "select items" mode). Card size small/medium/large. Created on first use and then kept hidden
(**call**: fast reopen; ~0 cost until first use). Closes when it loses focus.

### Library — the "Pano" view

Sidebar filters (Tümü, Görseller, Metinler, Bağlantılar, Sabitlenenler, folders with add/rename/
delete, tags, Çöp kutusu), grid or list, search, multi-select, batch actions (copy, pin, move to
folder, tag, export to folder, trash), inspector (preview, title, note, tags, folder, editable
text, OCR text + copy, link title/open, source app, dates, size). Image **editor** (also used by
the wheel's "annotate" preset and the Convert view): crop, pen, marker, rectangle, ellipse,
arrow, line, text, undo/redo; clipboard images save as an edited copy, files save as
`<name> (düzenlendi).png` next to the source.

### Pano ayarları

Enable/pause, per-kind capture, suggestion toasts (v1), retention per kind, max items, ignored
apps, OCR on/off, link titles on/off, watched image folders, shelf rows / card size / click
behaviour, clear unsaved, empty trash, storage used.

## 6. Shortcut and release

- After reinstalling: delete `Desktop\File Chameleon.lnk` if the installer created it, and make
  sure `Desktop\Klasör\ZARLOF\File Chameleon.lnk` points to
  `%LOCALAPPDATA%\Programs\File Chameleon\file-chameleon.exe` (**call**: a local step, not in the
  public installer, because the folder is the user's own).
- Uninstall removes the right-click registry verb (NSIS hook).
- Version 0.2.0; README (TR/EN) updated; pushed through GitHub Desktop.

## Architecture changes

```
crates/fc-core/src/presets/{mod,builtin}.rs   WheelConfig, resolution, preset -> JobRequest
crates/fc-core/src/watch.rs                    generic polling folder watcher (settle, ignore)
crates/fc-core/src/engines/{image,ffmpeg,data,files,subtitle}.rs   new ops
crates/fc-core/src/clip/{history,enrich}.rs    ClipItem v2, folders, trash, retention, link title
src-tauri/src/platform/{hook,hotkeys,ocr,clipboard,contextmenu}.rs
src-tauri/src/{windows,shelf}.rs               overlay wheel v2, shelf window
src/lib/wheel/{Wheel.svelte,geometry.ts}       shared wheel (overlay + editor preview)
src/shelf/ + shelf.html                        shelf page (small bundle)
src/app/views/{settings/*,wheel,presets,shortcuts,watch,clipboard,editor}
```

RAM: the shelf webview exists only after first use; OCR runs one image at a time on a worker
thread; watchers are one polling thread total; keyboard LL hook does nothing unless the wheel is
open.

## Approaches considered

- Wheel flip input: (a) global LL hooks while the wheel is open — chosen, works during OLE drags
  where the overlay never has focus; (b) make the overlay focusable — breaks drags and steals
  focus from Explorer.
- Shelf: (a) separate on-demand webview — chosen; (b) reuse the overlay window — needs focus,
  would fight the wheel/toast state machine; (c) native Win32 UI — far more code.
- OCR: (a) Windows.Media.Ocr — chosen, offline, no download, Turkish when installed;
  (b) Tesseract — 30+ MB download; (c) none.
- Watch folders: (a) polling — chosen; (b) `ReadDirectoryChangesW` — more code, misses network
  shares; (c) `notify` crate — new dependency.

## Left to the builder

Exact visual metrics, icon details, ordering of default slots beyond the ids above, and the
Turkish wording (natural, sentence case).
