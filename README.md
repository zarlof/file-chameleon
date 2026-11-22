<p align="center">
  <img src="assets/icon-source.png" width="112" alt="File Chameleon icon">
</p>

<h1 align="center">File Chameleon</h1>

<p align="center">
  Convert files offline, download media from links and get smart clipboard suggestions,<br>
  all in one small, fast Windows app.
</p>

<p align="center">
  <a href="README.tr.md">Türkçe</a> ·
  <a href="#features">Features</a> ·
  <a href="#install">Install</a> ·
  <a href="#build-from-source">Build</a> ·
  <a href="#privacy">Privacy</a>
</p>

---

## Features

### Convert: offline, on your PC

Drop files on the window, or **hold Ctrl while dragging files** in File Explorer. A wheel opens under
your cursor: release on a slice and the job runs in the background. Your files never leave your
computer, and the originals are never changed or deleted.

- **A wheel per file type.** Images, video, audio, PDF, text, data, subtitles, archives, mixed
  selections and any other file each get their own wheel profile.
- **Front and back.** Each wheel has a front face (up to 8 slices) and a back face (up to 12).
  Flip it with the mouse wheel or Tab, or open the back directly with **Ctrl+Shift+drag**.
- **Groups.** A group slice opens an outer ring with up to 6 more actions.
- **Edit it all.** Settings → **Wheel** lets you arrange the slices of each profile, add your own
  profiles and file types, and create groups. Settings → **Presets** lets you make your own presets
  (format, quality, size, edits, target folder).
- **Right-click menu (optional).** "Convert with File Chameleon" in Explorer opens the same wheel at
  your cursor; click a slice, or press Esc to close.

558 conversions in total.

| Kind | Formats and actions |
|---|---|
| Images | PNG, JPG, WebP, BMP, GIF, TIFF, ICO, TGA, QOI, PNM; SVG → PNG/JPG/WebP/ICO; grayscale, rotate, mirror, trim transparent edges, pixelate, blur, resize, crop to 16:9 / 9:16 / 1:1 / 4:3, draw and write |
| Video | MP4, MKV, WebM, MOV, AVI, M4V, WMV, FLV, MPG, 3GP, TS (OGV is read only); video → GIF; GIF → MP4/WebM; extract audio; crop to an aspect ratio; 2x or half speed |
| Audio | MP3, M4A, Opus/OGG, FLAC, WAV, AAC, WMA, AIFF, AC3, MKA (AMR is read only); iPhone ringtone (M4R, up to 40 s) |
| PDF | merge, split, images → PDF, PDF → PNG/JPG |
| Text and data | Markdown → HTML, TXT → HTML, CSV ⇄ TSV ⇄ JSON ⇄ YAML ⇄ TOML |
| Subtitles | SRT ⇄ WebVTT, → plain text; ASS/SSA → SRT/WebVTT. Old Turkish (Windows-1254) and UTF-16 files are read correctly. |
| Files and folders | pack into one ZIP, extract a ZIP, copy or move to a folder |

Media tools: **compress** (by quality or target size), **trim**, **mute**, **speed** and **crop**.

### Watch folders

Pick a folder and a preset in Settings → **Watch folders**. New files that land there are converted
automatically into a `converted` subfolder, once they have finished copying. Files that were already
there are left alone.

### Download media from links

Paste a link from YouTube, Instagram (reels and posts), TikTok, X/Twitter, Facebook, Reddit, Vimeo,
Twitch clips, Pinterest, or a direct image or video URL. You can save it as video (MP4), audio (MP3),
GIF or the original file, at Best, 1080p or 720p. After it finishes, you can show it in its folder or
convert it in one click. **Ctrl+7** downloads the link you just copied.

### Clipboard

- **The clothesline:** up to four ropes (images, texts, links, files) of white spider
  silk across the top or bottom of the screen; little black spiders hang from them on their own
  threads and hold your clips. Every rope shows; the eye button by a rope's name folds it away. **Ctrl+1** opens it (**Ctrl+Alt+F**
  opens it with search), or move the mouse to the screen edge and keep it there for a moment (it
  can also peek when you copy something; off by default). Click a card to open it (or copy it, copy and
  close, or paste straight into the app you were in: your choice). Right-click for copy, edit,
  share (the Windows share sheet), pin and delete; the pin at a card's top left keeps it first. **Ctrl+Enter** pastes, 1-9 picks a card,
  typing searches, Delete moves to the trash (Ctrl+Z brings it back). Rest on a card for a moment
  to see it 2.5 times larger (the whole picture, all of a text, a link's full address, a file's
  name and folder). Drag a card along its rope to put it in another place (the place is kept);
  drag it out of the line to drop it into another app. The gear on a card gives it a colour of its
  own. Settings → **Line** changes the edge, size, rope and pin look, cards and their colour,
  swing, sounds, rope order and names, how many cards each rope holds, and more.
- **The files rope:** the fourth rope holds files you added, converted or downloaded. Drag them out,
  open them, show them in Explorer, or put them on the clipboard. Right-click a file for more.
  The Explorer right-click verb **Hang on the line** ("İpe as" in Turkish) puts files there.
- **Library:** the **Clipboard** page has folders, tags, a pinned filter, a trash, a grid or list
  view, and an inspector for titles, notes and editing text. Export items to a folder.
- **Text in images:** the built-in Windows OCR reads text in copied images, offline. You can search
  it and copy it.
- **Image editor:** crop, pen, marker, rectangle, ellipse, arrow, line and text, with undo. The
  original is kept.
- **Your rules:** turn capture on or off for texts, links and images; keep each kind 1–30 days or
  forever; never record chosen apps. Password-manager content is never recorded. Pinned, foldered,
  tagged or titled items never expire. Link titles are fetched only if you turn that on.
- Copy a media link and File Chameleon offers to download it; copy an image and it offers to save or
  convert it.

### Light by design

- **Small installer.** The installer is about 4 MB. Heavy engines (FFmpeg, yt-dlp, PDFium) are
  downloaded only the first time a feature needs them, and each download is SHA-256 verified. An
  FFmpeg already on your `PATH` is used as is.
- **Low memory.** The background work (tray, hooks, hotkeys, clipboard watcher, job queue) runs in
  Rust. The main window is destroyed when you close it and rebuilt when you open it. The line
  window is only created the first time you open it.
- **Native feel.** Windows 11 Mica and caption buttons, the Inter font and Lucide icons. It follows
  light and dark mode, has an interface size setting, and the interface is in Turkish and English.

## Install

Download the latest `File Chameleon_x.y.z_x64-setup.exe` from
[Releases](../../releases) and run it. It installs for your user only, so no admin rights are needed.
Each installer has a matching `.sha256` file you can use to check the download.

Requirements: Windows 10 (1809+) or Windows 11, 64-bit. WebView2 is installed automatically if it
is missing.

## Usage tips

- **Ctrl + drag** files or folders from File Explorer or the desktop to open the wheel. Press
  **Esc** or release outside the wheel to cancel. **Ctrl+Shift + drag** opens the back face.
- **Ctrl+1** opens the clothesline, **Ctrl+2** the clipboard library, **Ctrl+3** hangs a screenshot
  on it and **Ctrl+7** downloads the copied link. Change or turn off these shortcuts in Settings →
  **Shortcuts**.
- Closing the window keeps File Chameleon in the tray (you can change that in Settings → **General**).
  Use **Quit** in the tray menu to exit.
- Settings → **Engines** shows which engines are installed and lets you update or remove them.

## Build from source

You need [Rust](https://rustup.rs) (stable, 1.88 or newer), [Node.js](https://nodejs.org) 22 or
newer, and the WebView2 runtime.

```powershell
npm ci
npm run tauri dev      # run in development
npm run tauri build    # build the NSIS installer into target/release/bundle/nsis
```

Checks:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npm run check
npm run test
```

Project layout:

```
crates/fc-core/   core logic: formats, conversion engines, job queue, engine manager,
                  downloader, clipboard history (no UI code)
src-tauri/        desktop shell: windows, tray, Ctrl+drag hook, clipboard watcher, IPC
src/              Svelte 5 interface: main window (index.html) and wheel/toast overlay (overlay.html)
```

## Privacy

- Conversions run entirely on your computer.
- The app connects to the internet only to download media you ask for, and to download or update
  an engine (from the official release pages listed in `engines.manifest.json`).
- Copied text is matched against a built-in list of sites. Nothing is sent anywhere when you copy.
- There is no telemetry and no account.

## Responsible use

Download only content you have the right to save. Respect the terms of the sites you use and the
rights of creators. You are responsible for how you use the downloader.

## Credits

Inspired by [Filee](https://github.com/KnifeLemon/Filee),
[Nashi](https://github.com/orkunisitmak/nashi-app) and
[Clipboardline](https://github.com/orkunisitmak/clipboardline). File Chameleon is an independent
project, is not affiliated with them, and contains none of their code. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for the open-source software it uses.

## License

[MIT](LICENSE)
