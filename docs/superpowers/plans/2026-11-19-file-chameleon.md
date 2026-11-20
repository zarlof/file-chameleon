# File Chameleon — Implementation Plan

Spec: `docs/superpowers/specs/2026-11-19-file-chameleon-design.md` (approved design; Turkish).

The scaffold (workspace, contracts, Tauri shell, window manager, mouse hook, design tokens,
IPC surface) is already committed. Every task below fills in files that currently contain
`todo!("Task X")` stubs or placeholders. **Signatures in stub files are contracts: keep them.**
Read the doc comments on the stubs — they carry the exact behaviour required.

## Global Constraints

- Rust edition 2024, workspace at repo root. Core logic in `crates/fc-core` (no Tauri deps).
  Tauri shell in `src-tauri`. Frontend: Svelte 5 (runes only: `$state`, `$derived`, `$effect`,
  `$props`), TypeScript strict, Vite 8. No other UI framework, no CSS framework.
- **Do not add, remove or upgrade dependencies** in any `Cargo.toml` or `package.json`.
  Everything needed is already declared. If you are truly blocked by a missing crate feature,
  stop and report `NEEDS_CONTEXT`.
- **Only touch the files your task owns** (listed per task). Other tasks run in parallel in
  other worktrees; touching their files causes merge conflicts.
- Small binary, low RAM: no new background threads that poll; no busy loops; stream large
  files instead of reading them fully into memory where the API allows; never keep decoded
  images around after a job.
- Errors: core returns `fc_core::Error` (see `crates/fc-core/src/error.rs`); pick the most
  specific variant. Never `unwrap()`/`expect()` on user data or I/O in non-test code.
- Sources are never modified or overwritten. Partial outputs are deleted on failure/cancel.
- Child processes go through `fc_core::proc` (no console window, killed with the app).
- UI text: never hard-code user-visible strings in components; use the i18n `t()` function
  with keys in your namespace files under `src/locales/{tr,en}/<namespace>.json`. Turkish and
  English both required, natural wording (Turkish: sentence case, "Dönüştür", "İndir", "Pano").
- Design: macOS-inspired. Use only CSS variables from `src/lib/ui/theme.css` (colours, radii,
  shadows, fonts, motion). Components from `src/lib/ui/` once Task 4 lands; until then (parallel
  tasks) build against the documented component API in Task 4.
- Tests: Rust unit tests next to the code (`#[cfg(test)] mod tests`), integration tests in
  `crates/fc-core/tests/`. Tests that need ffmpeg must skip (return early with a printed note)
  when `fc_core::engine_mgr::which("ffmpeg.exe")` is `None`. Generate test inputs in code
  (tiny images via `image`, media via `ffmpeg -f lavfi`), don't commit binary fixtures larger
  than 20 KB.
- Verification before reporting DONE: `cargo test -p fc-core` (and `cargo check --workspace`
  if you touched `src-tauri`), `npm run check` and `npm run test` if you touched `src/`.
  `cargo clippy --workspace -- -D warnings` must be clean for your files.
- Commit your work on your branch with clear messages. Do not push.

## Task 1: Jobs — naming, queue, executor

**Owns:** `crates/fc-core/src/jobs/naming.rs`, `crates/fc-core/src/jobs/queue.rs`,
`crates/fc-core/src/engines/mod.rs` (only the `DefaultExecutor::run` body and private helpers
in that file; keep the public items), new `crates/fc-core/tests/jobs.rs`.

**Do:**
1. `naming.rs`: implement `output_path`, `temp_path`, `commit` exactly as documented.
   Reserved names: CON, PRN, AUX, NUL, COM1–COM9, LPT1–LPT9 (case-insensitive, stem only).
   `commit` must work when temp and final are on the same volume (rename) and fall back to
   copy+delete across volumes.
2. `queue.rs`: implement `JobQueue` as documented.
   - Workers: plain `std::thread`s. Per-engine concurrency: Image/Svg/Data/Pdf jobs share a
     "cpu" pool of `max(1, available_parallelism/2)`; Ffmpeg/Pdfium jobs a pool of 1.
     Decide a job's pool from the first input's format and the operation (Convert → planner
     route's first engine; Compress/Trim/Mute/Speed/Crop on video/audio → ffmpeg; MergePdf/
     SplitPdf → cpu).
   - Threads exist only while there is work (spawn on demand, exit when the queue drains).
   - Every state change calls the listener with a full `JobSnapshot`; progress callbacks are
     throttled to ≥100 ms apart per job (always emit 1.0 / final state).
   - `cancel` on a queued job marks it Cancelled immediately; on a running job it triggers the
     job's `CancelToken`, and the job ends Cancelled when the executor returns `Error::Cancelled`.
   - Keep at most 100 finished jobs (drop oldest).
3. `engines/mod.rs` `DefaultExecutor::run`: implement the documented dispatch using the
   engine functions (their signatures are fixed; Task 2 and Task 3 implement them in parallel —
   call them as documented, don't implement them). Use `formats::planner::plan`,
   `jobs::naming`, `ToolLocator` (`"ffmpeg"`→`ffmpeg.exe`/`ffprobe.exe`, `"pdfium"`→`pdfium.dll`;
   missing → `Error::MissingTool(tool_id)`). Intermediate files of multi-step routes go in a
   fresh temp dir under `std::env::temp_dir()/file-chameleon/<job-uuid-ish>` removed afterwards.
   Wrap each engine call in `std::panic::catch_unwind(AssertUnwindSafe(..))`.
4. Tests: naming (collisions, reserved names, out_dir, suffix, commit race), queue (ordering,
   cancel queued, cancel running with a fake `Executor` that loops until cancelled, throttling
   emits final state, 100-job cap) using fake executors — no real engines needed.
   Executor routing tests with a fake `ToolLocator` returning `None` → `MissingTool("ffmpeg")`
   for mp4→webm (no ffmpeg call happens).

## Task 2: In-process engines — image, svg, data, pdf

**Owns:** `crates/fc-core/src/engines/{image,svg,data,pdf}.rs`, new
`crates/fc-core/tests/engines_inproc.rs`.

**Do:** implement every `todo!("Task B")` function exactly as its doc comment says.
- image: decode via `image::ImageReader::open(..)?.with_guessed_format()?`; check dimensions
  (`into_dimensions`) before full decode for the 200 MP limit. JPEG encoder quality from
  options (default 85). Lossy WebP via `webp::Encoder::from_image(..).encode(q)`; lossless via
  `encode_lossless()`. ICO: build multi-size with `image::codecs::ico::IcoEncoder` +
  `IcoFrame::as_png`. GIF output: single frame. TIFF/BMP: straightforward. Resize with
  `imageops::FilterType::Lanczos3` only when larger than the bounds.
- svg: `usvg::Tree::from_data` with `fontdb` loaded from system fonts once (cache in a
  `OnceLock`), render with `resvg::render` into a `tiny_skia::Pixmap`, convert to
  `image::RgbaImage` and call `image::encode`.
- data: as documented. HTML page shell shared by md/txt (small inline CSS, light+dark via
  `prefers-color-scheme`).
- pdf: lopdf for images_to_pdf/merge/split/compress/page_count (merge: renumber objects and
  combine page trees — follow lopdf's merge example approach). render_pages: `pdfium-render`
  `Pdfium::bind_to_library(pdfium_dll)` then `Pdfium::new`, render each page with
  `PdfRenderConfig` at 150 dpi capped as documented, `as_image()` → `image::encode`.
  If binding fails → `Error::MissingTool("pdfium")`.
- Tests (generate inputs in code): every raster pair converts and decodes back with the right
  dimensions; jpg has no alpha; webp q100 is lossless (pixel-equal roundtrip); ico contains
  multiple sizes; max_width downscales and never upscales; 200 MP guard (fake header via a
  tiny BMP/PNG header claiming huge dims — or test the dimension check function directly);
  svg → png size rule; md→html contains `<h1>` and `<title>`; csv↔json↔yaml↔toml roundtrips
  on a small table; to-csv error on nested non-array; images_to_pdf + page_count; merge two
  generated PDFs → page count sum; split → N files; compress output parses. pdfium render test
  skips unless `PDFIUM_DLL` env var points to a dll.

## Task 3: Process runner, engine manager, ffmpeg engine

**Owns:** `crates/fc-core/src/proc.rs`, `crates/fc-core/src/engine_mgr/mod.rs` (manifest.rs is
done; don't change its types), `crates/fc-core/src/engines/ffmpeg.rs`, new
`crates/fc-core/tests/{proc,engine_mgr,ffmpeg}.rs`.

**Do:**
- proc: `CREATE_NO_WINDOW` via `std::os::windows::process::CommandExt::creation_flags`
  (cfg(windows)); a process-wide Job Object (`CreateJobObjectW` +
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) created lazily in a `OnceLock`, each spawned child
  assigned with `AssignProcessToJobObject`. `run_streaming`: reader threads for stdout and
  stderr sending lines over channels; main loop `recv_timeout(100ms)` → callbacks + cancel
  check; on cancel `child.kill()`; cap captured text at 4 MB per stream. Non-Windows: same
  minus job object.
- engine_mgr: as documented in the module docs. HTTP via `ureq` 3 (native-tls feature is
  already enabled; build an agent with `ureq::Agent::config_builder().tls_config(...native...)`
  if needed); stream body to file with progress; enforce 400 MB cap; SHA-256 via `sha2`;
  zip via `zip` crate, tar.gz via `flate2`+`tar`; extract only mapped files (suffix match on
  normalized `/` paths). `status()` versions: managed → from `installed.json`; system → run
  `<tool> -version`/`--version` (ffmpeg: first line token 3; yt-dlp: whole line; deno: `deno
  --version` first line token 2) with a 3 s timeout via `proc::run` + a cancel token tripped
  by a timer thread; don't run anything for pdfium.
  `which`: split `PATH` on `;` (Windows) and check `is_file()`.
  `locate(tool, file)`: managed dir file if present, else if `file` is in the entry's `system`
  list, `which(file)` (yt-dlp system copy must satisfy `minVersion`).
- ffmpeg: as documented. Build argument vectors in small pure functions (unit-tested without
  ffmpeg). Progress: parse `out_time_us=`/`out_time_ms=` (both are microseconds in modern
  ffmpeg) and `progress=end`. Stream copy attempt: ffprobe codecs (`-show_entries
  stream=codec_type,codec_name -of json`); if copy fails, fall back to encode.
- Tests: proc (echo via `cmd /C echo`, cancel of `ping -n 30 127.0.0.1` returns Cancelled in
  < 2 s and the process is gone), engine_mgr with a local HTTP server? — no: unit-test
  verify/extract with in-memory zip/tar.gz written to a temp dir, and `which` with a temp
  PATH; network install is tested manually. ffmpeg integration (skip without ffmpeg):
  mp4→webm, mp4→gif, mp4→mp3, wav→flac, compress, trim duration check via `duration()`,
  mute has no audio stream, speed 2× halves duration (±10%), cancel deletes nothing extra.

## Task 4: Frontend shell, UI kit, i18n, stores

**Owns:** `src/main.ts`, `src/App.svelte`, `src/app/Shell.svelte`, `src/app/Sidebar.svelte`,
`src/app/TitleBar.svelte`, `src/app/views/*/index.svelte` **placeholders only** (one per view:
convert, download, clipboard, recent, settings — each just a heading via `t()`), everything in
`src/lib/ui/` except `theme.css` (you may append to theme.css only for component styles that
need globals — prefer component-scoped CSS), `src/lib/i18n.ts`, `src/lib/stores/*.ts`,
`src/locales/{tr,en}/common.json`, new `src/lib/*.test.ts` for your logic.

**Do:**
1. i18n (`src/lib/i18n.ts`): load all `src/locales/<lang>/*.json` with
   `import.meta.glob(..., { eager: true })`; key format `<namespace>.<path>` (file name =
   namespace, e.g. `convert.dropTitle`); `t(key, params?)` with `{name}` interpolation and a
   simple plural form: value may be `{ "one": "...", "other": "..." }` selected by
   `params.count`. Reactive language: a `$state` holder so components re-render on change.
   Language = settings.language, `system` → `appInfo.systemLanguage`. Fallback to `en`, then
   the key itself.
2. Stores (`src/lib/stores/`): `settings.svelte.ts` (load via `api.getSettings`, `update(patch)`
   → `api.setSettings`, follows `on.settingsChanged`), `jobs.svelte.ts` (list + live
   `on.jobUpdate` merge by id), `tools.svelte.ts` (status + install progress via
   `on.toolProgress/toolDone`), `nav.svelte.ts` (current view + pending payload; applies
   `api.takePendingNav()` on startup and `on.navGoto`), `theme.ts` (apply `data-theme` from
   setting or `prefers-color-scheme`, and `data-accent`).
3. UI kit (`src/lib/ui/`, Svelte 5 components, scoped CSS, theme variables only):
   `Button` (variants: primary | secondary | plain | destructive; sizes: sm | md | lg; icon
   slot/snippet; loading state), `IconButton`, `SegmentedControl` (options: {value,label,icon?}[],
   bindable value), `Toggle` (macOS switch, bindable checked), `Slider` (bindable value, min,
   max, step), `Select` (styled native select), `TextField` (bindable value, placeholder,
   leading icon, clear button), `Card`, `ListRow` (title, subtitle, leading/trailing snippets),
   `SectionHeader`, `ProgressBar` (determinate/indeterminate), `ProgressRing`, `Badge`,
   `EmptyState` (icon, title, message, action snippet), `Sheet` (modal panel), `Toast` (in-window
   transient message API `toast.show(msg, kind)`), `Spinner`, `FileChip` (name, size, kind icon,
   remove button), `Icon` (wraps `@lucide/svelte` icons by name for the few dynamic cases).
   Export all from `src/lib/ui/index.ts`. Document each component's props in a top comment.
4. Shell: frameless window — `TitleBar` (52px, `data-tauri-drag-region`, macOS traffic lights
   at left: close #ff5f57, minimize #febc2e, zoom #28c840 with hover glyphs, grey when window
   unfocused; calls `getCurrentWindow().close()/minimize()/toggleMaximize()`), translucent
   `Sidebar` (220px; app icon + name; items with Lucide icons: Convert `RefreshCw`, Download
   `Download`, Clipboard `Clipboard`, Recent `Clock`, Settings `Settings`; selected = accent
   soft pill; collapses to icons under 860px width), content area with a subtle inner card
   background. On mount: load settings + appInfo, apply theme, then
   `getCurrentWindow().show()` after first paint (requestAnimationFrame ×2).
   Keyboard: Ctrl+1..5 switch views, Ctrl+, opens Settings.
   Global file drop on the window: `getCurrentWebview().onDragDropEvent` → on drop, navigate to
   Convert with the paths (nav store) — Convert view (Task 8) consumes `nav.pending`.
5. Tests (vitest): i18n interpolation/plural/fallback; nav store payload handling (mock ipc).

## Task 5: Downloader (core + IPC + Download view)

**Owns:** `crates/fc-core/src/download/{classify,ytdlp,direct}.rs` (mod.rs types are fixed;
you may add functions to mod.rs below the types), `src-tauri/src/ipc/downloads.rs`,
`src/app/views/download/**`, `src/locales/{tr,en}/download.json`,
`crates/fc-core/tests/download.rs`, `crates/fc-core/tests/fixtures/ytdlp-*.json` (small).

**Do:**
- classify: as documented (pure, exhaustive unit tests: YouTube watch/shorts/youtu.be/music,
  Instagram reel/p/tv/stories, TikTok @user/video and vm.tiktok, X/Twitter status, Facebook
  watch/reel/fb.watch, Reddit comments, Vimeo id, Twitch clips, Pinterest pin, direct
  image/media by extension with query strings, bare `youtu.be/x`, text with spaces → None,
  profile pages → Other).
- ytdlp.rs: `probe(tools, url, cancel) -> Result<MediaInfo>` runs `yt-dlp -J` (+common args);
  `download(tools, req, progress, cancel) -> Result<PathBuf>`; pure builders
  `probe_args`, `download_args(req, tools, staging)` and parsers `parse_probe_json` (fixtures),
  `parse_progress` for a `--progress-template "download:FC %(progress.downloaded_bytes)s
  %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.speed)s
  %(progress.eta)s"` line format (values may be `NA`). Format selection:
  Original `bv*+ba/b`; Video `bv*[ext=mp4]+ba[ext=m4a]/bv*+ba/b` + `--merge-output-format mp4`
  + `--remux-video mp4`; quality caps add `[height<=1080]`/`[height<=720]`; Audio `-x
  --audio-format mp3 --audio-quality 0`; Gif: download Video at ≤720p then
  `engines::ffmpeg::Ffmpeg::to_gif(.., 480, 15, 60, ..)` and delete the intermediate.
  Output template `%(title).150B [%(id)s].%(ext)s` into `out_dir`; get the final path with
  `--print after_move:filepath`. Reject live (`is_live`) and DRM errors with a clear
  `Error::Unsupported`. JS runtime: if the URL is YouTube, pass `--js-runtimes node:<path>`
  when `which("node.exe")` exists, else `deno:<path>` from the `ToolLocator` (`"deno"`,
  `"deno.exe"`), else let yt-dlp try (it may fail → error code `missingTool` with "deno").
  Cookies: when `Settings.cookies_browser` is set, pass `--cookies-from-browser <name>`
  (the IPC layer passes it in; add a `cookies_browser: Option<String>` parameter to `download`
  and `probe`).
- direct.rs: `probe_direct(url) -> MediaInfo` (HEAD for size/type, file name from URL/
  Content-Disposition), `download_direct(url, out_dir, progress, cancel) -> PathBuf` streaming
  to `naming::temp_path` then `commit`; `og_image(url) -> Option<String>` fetches HTML (≤2 MB)
  and extracts `og:image` (Instagram photo posts fallback when yt-dlp says "no video").
  User-Agent: a generic desktop browser UA string. Timeouts: connect 15 s, read 30 s.
- ipc/downloads.rs: implement the four commands. `Downloads` holds a `Mutex<Vec<entry>>` with
  snapshot + `CancelToken`; downloads run on threads; emit `download://update`
  (`crate::events::DOWNLOAD_UPDATE`) with a `DownloadSnapshot` on every change (progress
  throttled 100 ms). Missing yt-dlp → before starting, the UI is told via error code
  `missingTool` + message `yt-dlp`; the Download view then offers one-click install (Tools
  store / `api.installTool('yt-dlp')`) and retries. Call
  `state.engines().update_ytdlp_if_stale` on a background thread before a yt-dlp download
  (ignore its errors). When finished and the main window is not focused, call
  `crate::windows::show_toast(app, "downloadDone", json!({downloadId, output, title}), Some(6000))`.
- Download view (`src/app/views/download/index.svelte` + subcomponents): URL field with Paste
  button (reads clipboard via `navigator.clipboard.readText()`), Enter = check; preview card
  (thumbnail via `<img src>` for https thumbnails, title, site badge, duration, resolution,
  size); `SegmentedControl` mode (Video / Audio / GIF / Original) and quality select (Best /
  1080p / 720p; hidden for Audio and images); save folder row with "Change…"
  (`@tauri-apps/plugin-dialog` `open({directory:true})`), default from settings or Downloads;
  Download button; active/recent downloads list with progress, speed, ETA, cancel, "Show in
  folder", "Convert" (navigates to Convert with the file). Prefill from `nav.pending.url`.
  Errors shown inline with friendly localized text per `errorCode`. Empty state explaining
  supported sites. All strings in `download.json`.

## Task 6: Smart clipboard (core history + Windows listener + IPC + Clipboard view)

**Owns:** `crates/fc-core/src/clip/history.rs`, `src-tauri/src/platform/clipboard.rs`,
`src-tauri/src/ipc/clipboard.rs`, `src/app/views/clipboard/**`,
`src/locales/{tr,en}/clipboard.json`, `crates/fc-core/tests/clip.rs`.

**Do:**
- history.rs: implement `ClipStore` exactly as documented (thumbnail generation with `image`,
  Lanczos3, longest side 256). Unit tests: add/dedupe/move-to-top keeps pin; oversized text
  skipped; search; pin; remove deletes files; clear_unpinned; prune by age and count with pins
  surviving; corrupt index recovery; persistence across reopen.
- platform/clipboard.rs (Windows): listener thread using `clipboard_win::Monitor`
  (`monitor` feature is enabled). On change: skip if `state.clip_paused`, if
  `state.clip_self_write` is set (then clear it), or if the clipboard has any of the formats
  `ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer Ignore`, or
  `CanIncludeInClipboardHistory`/`CanUploadToCloudClipboard` with DWORD 0 (register the format
  names with `clipboard_win::register_format`). Debounce ~120 ms, retry opening the clipboard
  up to 5× with backoff. Read: Unicode text (`formats::Unicode`), else image (registered "PNG"
  format bytes if present, else `CF_DIB`/`CF_DIBV5` → convert to PNG with `image`; skip if >
  25 MP) when `settings.clipboard_images`. Ignore file lists (CF_HDROP). Add to
  `state.clips()`, prune with settings limits, emit `clip://added` (`events::CLIP_ADDED`).
  Smart suggestion toasts via `crate::windows::show_toast`:
  media link (`ClipKind::MediaLink`) and `clipboard_suggest_links` →
  `"clipLink"` `{url, site}` timeout 8000; image and `clipboard_suggest_images` →
  `"clipImage"` `{clipId, width, height, thumb, image}` timeout 8000. No toast while the
  main window is focused. `set_enabled(false)` stops processing (thread may stay parked —
  no polling). Must not crash if another app holds the clipboard.
- ipc/clipboard.rs: implement the commands. `clip_copy` sets `state.clip_self_write` then
  writes text (or image as CF_DIB + PNG formats) with `clipboard_win`. `clip_save_image`
  converts the stored PNG with `fc_core::engines::image::encode` (decode PNG first) to the
  Downloads folder (`state.default_download_dir()`), unique name `Clipboard <yyyy-mm-dd
  HH.MM.SS>.<ext>` via `jobs::naming::output_path`.
- Clipboard view: search field, filter segmented (All / Links / Text / Images), virtualized-ish
  list (render at most ~60 rows at a time; simple windowing is fine) of `ClipItem`s: text
  preview (2 lines), link items with site badge and "Download" action (navigates to Download
  with url), image items with thumbnail (asset protocol `convertFileSrc`) and "Save as PNG /
  JPG / WebP", "Convert". Row actions: Copy (Enter/double-click), Pin, Delete. Header: pause
  toggle, "Clear unpinned" (confirm sheet). Live updates via `on.clipAdded`. Empty state with
  privacy note (passwords from password managers are never recorded).

## Task 7: Overlay — wheel and toasts

**Owns:** `src/overlay.ts`, `src/overlay/**` (new: `Overlay.svelte`, `Wheel.svelte`,
`ToastCard.svelte`), `src/lib/wheel/geometry.ts` + `geometry.test.ts`,
`src/locales/{tr,en}/overlay.json`. May import `src/lib/ipc.ts`, `src/lib/types.ts`,
`src/lib/ui/theme.css`, and `src/lib/i18n.ts` (Task 4 builds i18n in parallel — implement a
tiny local fallback `t` in `src/overlay/i18n.ts` that reads only `overlay.json` and the
language from `api.getSettings()` + `api.appInfo()`; do not import `src/app/**`).

**Behaviour (see `src-tauri/src/windows.rs` for the Rust side; don't change it):**
- Window is 440×440 logical for the wheel, 404×150 for toasts; transparent; never focused.
- Events: `overlay://wheel` → reset to "waiting" (subtle pulsing ring); `overlay://wheel-targets`
  → show slices; `overlay://toast` → toast mode; `overlay://hide` → clear.
- Wheel: ring centred in the window (outer radius 190 px, inner 70 px), max 8 slices:
  `recent` ids first (in order, only those present in targets), then remaining targets in
  their order, cut to 7 + a "More…" slice (id `more`) when there are more than 8 total.
  Each slice: format label (e.g. "WEBP") and a small Lucide icon by kind; tools needed shown as
  a tiny download glyph. Hover highlight with accent fill + scale spring; centre shows file
  count + first file's kind icon, or "Not supported" when targets are empty. Hit-testing in
  `geometry.ts` (pure, unit-tested): angle→slice with slice 0 centred at top, radius bounds,
  DPI conversion `physical / devicePixelRatio`.
- Drag events via `getCurrentWebview().onDragDropEvent`: enter/over → hover; drop → if on a
  slice: `api.wheelDrop(paths, id)`; else `api.overlayHide()`; leave → clear hover.
- Look: frosted dark/light glass disc (no `backdrop-filter` — use translucent fill
  `color-mix(in srgb, var(--elevated-bg) 92%, transparent)`, hairline border, `--shadow-lg`,
  `--glass-edge`), appear animation scale 0.85→1 + fade in 180 ms spring.
- Toasts (macOS notification banner look, bottom-right of the work area, so they never cover
  the caption buttons of maximized windows): 380×~110 card, radius 18, app icon
  (`/src-tauri/icons/64x64.png` copied into `public/`? — no: use an inline SVG chameleon mark
  or the Lucide `Sparkles` icon), title + body localized per `kind` (see `ToastPayload` docs in
  `src-tauri/src/events.rs` for kinds, data and actions), up to 2 action buttons + close (×) on
  hover, optional thumbnail (`convertFileSrc(data.thumb)`), progress bar when `progress` set.
  Slide in from right 220 ms. Actions call `api.overlayToastAction(kind, action, data)`;
  close calls `api.overlayHide()`. Respect `timeoutMs` visually (Rust also hides).
- Keep the overlay bundle tiny: no UI-kit imports; only `@lucide/svelte` icons used directly.
- Tests: geometry (slice index for angles incl. wrap-around, centre dead zone, outside ring,
  DPI), slice ordering (recent first, More… rule).

## Task 8: Convert, Recent and Settings views

**Owns:** `src/app/views/convert/**`, `src/app/views/recent/**`, `src/app/views/settings/**`,
`src/locales/{tr,en}/{convert,recent,settings}.json`. Uses Task 4's UI kit and stores
(merged before this task starts).

**Do:**
- Convert: large drop card ("Drop files here or click to choose", dialog `open({multiple:true})`)
  that animates on drag-over; consumes `nav.pending.paths`; file chips (name, size, kind icon,
  remove); calls `api.listTargets(paths)`; format grid grouped by kind (pill buttons, selected
  = accent); tools row for `response.tools` (Compress with level segmented; Trim with start/end
  mm:ss inputs; Speed select 0.5×/1.5×/2×; Mute; Merge PDF; Split PDF; Crop: numeric x/y/w/h);
  options disclosure: quality slider (images/video), max width/height, output folder (same as
  source / choose). If a target `needsTools` a tool whose status is `missing`, the Convert
  button becomes "Download FFmpeg (≈110 MB) and convert" → installs with progress then submits.
  Queue list below: running/queued jobs with progress, cancel; finished with Reveal/Open.
  Unknown formats chip flagged "Not supported".
- Recent: all jobs from the jobs store (newest first) grouped Today / Earlier with status icons,
  outputs, errors (localized from `errorCode`), Reveal/Open/Retry (resubmit same request),
  "Clear finished".
- Settings (sidebar-style sections inside the view, macOS System Settings look):
  General (language, theme, accent swatches, launch at login), Wheel (enable, any app,
  explanation with a small illustration of Ctrl+drag), Clipboard (enable, images, suggestions,
  max items, retention days, cookies browser select for downloads: None/Edge/Chrome/Firefox/
  Brave with a privacy note), Output (convert output folder, download folder, default image
  quality), Engines (each tool: name, purpose, size, license, source badge Managed/System/
  Missing, version, Install/Remove buttons with progress; note that FFmpeg from PATH is used
  automatically), About (version, links text: GitHub repo placeholder, licenses summary,
  "Inspired by Filee, Nashi and ClipBoardLine — not affiliated").
- All settings write through the settings store immediately (no Save button).
