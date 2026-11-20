# File Chameleon v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Windows caption buttons, a new icon, Filee-style settings pages with a wheel (donut) editor and presets, a Filee-like wheel, watch folders, and Clipboardline's clipboard feature set.

**Architecture:** Pure logic in `crates/fc-core` (presets, new operations and engines, clipboard store v2, folder watcher); Windows integration in `src-tauri` (hooks, hotkeys, OCR, shelf window, registry verb); Svelte 5 UI in `src/` with three entries (main, overlay, shelf). Contracts (settings fields, TS types, IPC wrappers, module stubs, locale `presets.json`) were committed first (Task 0), so tasks can run in parallel.

**Tech Stack:** Rust 2024, Tauri 2.12, Svelte 5 (runes), TypeScript strict, Vite 8, vitest, `windows` 0.62.

**Spec:** `docs/superpowers/specs/2026-11-20-file-chameleon-v2-design.md` (read it first). v1 spec/plan: `docs/superpowers/specs/2026-11-19-file-chameleon-design.md`, `docs/superpowers/plans/2026-11-19-file-chameleon.md`.

## Global Constraints

- Original code only. Filee / Nashi / Clipboardline are feature references; never copy their code, icons or text.
- Core logic in `crates/fc-core` (no Tauri). Shell in `src-tauri`. Frontend Svelte 5 runes only (`$state`, `$derived`, `$effect`, `$props`), TS strict, no UI/CSS frameworks.
- Dependencies: no new crates/npm packages except where a task says so (`base64` in Task 3; extra `windows` crate *features* in Tasks 6 and 9).
- Only touch the files your task owns. Shared files (`src-tauri/src/lib.rs`, `src-tauri/src/ipc/mod.rs`, `src-tauri/src/events.rs`, `src-tauri/src/platform/mod.rs`, `src-tauri/Cargo.toml`): append in your own block, do not reformat others' code.
- Contracts from Task 0 are fixed: `src/lib/types.ts`, `src/lib/ipc.ts`, `crates/fc-core/src/settings.rs`. Rust command names/argument names must match `src/lib/ipc.ts` exactly (Tauri maps camelCase JS args to snake_case Rust params). If a contract is wrong, report it; do not silently change it.
- Errors: core returns `fc_core::Error`; never `unwrap()`/`expect()` on user data or I/O outside tests. Commands return `CmdResult<T>` (`src-tauri/src/ipc/mod.rs`).
- Threading rule (v1, `src-tauri/src/ipc/mod.rs` docs): never build a webview from a sync command or a WebView2 callback on the main thread; use `#[tauri::command(async)]` or spawn.
- Sources are never modified or overwritten (except "move to folder", which moves). Outputs never overwrite: use `fc_core::jobs::naming` (" (1)" suffix). Partial outputs are deleted on failure/cancel.
- RAM/size: no polling loops faster than 1 s; no decoded images kept after a job; LL hook callbacks only do atomics + channel sends and always call `CallNextHookEx`, and never swallow input unless the wheel is open.
- UI text only through i18n: `src/locales/{tr,en}/<namespace>.json`, both languages, same keys, natural Turkish (sentence case). Each task owns its namespace files (listed per task). Only CSS variables from `src/lib/ui/theme.css`; reuse `src/lib/ui` components.
- Reduce motion: when `settings.reduceMotion` is on, the root has `data-reduce-motion` and animations are instant.
- Verification commands (run what your task touches): `cargo test -p fc-core`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `npm run check`, `npx vitest run`. In a shared tree other tasks may leave temporary errors in *their* files: only your files must be clean.
- Commit with a conventional message ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Mixed / unknown drops**: dragging `a.jpg + b.docx + c.mp4` must open the wheel with the mixed profile (or `other`), never an empty or crashed wheel → Task 3 test `resolves_mixed_and_unknown_selections`.
2. **Broken or old `wheel.json`** (deleted preset still referenced by a group/profile/watch folder, file from an older version, garbage): loads with bad references dropped and new built-ins merged → Task 3 test `load_drops_dangling_refs_and_merges_builtins`; Task 10 shows a watch folder whose preset is gone as an error row.
3. **Watch folder feeding itself**: a file still being copied (growing) is not processed until stable; outputs written under the watched folder (`converted\`) or a temp name never trigger a new job → Task 4 tests `waits_until_file_is_stable`, `ignores_temp_names_and_excluded_dirs`, `ignored_paths_never_fire`.
4. **Clipboard junk**: blank-only text, >1 MB text, copies from an ignored app, password-manager markers, the same text copied twice → nothing stored or one refreshed item → Task 4 tests `blank_and_oversized_text_are_skipped`, `duplicate_refreshes_and_restores_from_trash`; Task 9 checks ignore-apps before reading data.
5. **Global input**: Tab / mouse wheel / Esc while the wheel is *not* open must pass through untouched; a hotkey already owned by another app is reported (`ok: false`) and the app keeps running → Task 6 pure test `flip_keys_only_when_wheel_open`; Task 9 test `parse_hotkey` + status path.

---

## Task 0: Contracts (DONE, committed)

What exists now (do not redo):
- `crates/fc-core/src/settings.rs`: all v2 fields + `WatchFolder { id, path, preset, subfolders, enabled }`. `clipboard_retention_days` was replaced by `clipboard_retention_{text,links,images}` (0 = forever); call sites temporarily pass `clipboard_retention_images` to the v1 `ClipStore::prune(now, days, max)`.
- `crates/fc-core/src/presets/mod.rs`, `crates/fc-core/src/watch.rs`: empty stubs, declared in `lib.rs`.
- `src/lib/types.ts`: `Kind` (+ `subtitle`, `archive`), `Settings` v2, `ImageOp`, `Operation` v2, `Preset`, `PresetGroup`, `Profile`, `WheelConfig`, `WheelFace`, `SlotIcon`, `WheelSlot`, `WheelStyle`, `WheelSlots`, `WheelThumb`, `WheelPayload.click`, `ClipShelf`, `ClipItem` v2, `ClipQuery`, `ClipFolder`, `ClipTag`, `ClipField`, `ClipStats`, `HotkeyStatus`, `WatchStatus`, `View` v2.
- `src/lib/ipc.ts`: every new command/event wrapper (see the comments there for semantics).
- `src/locales/{tr,en}/presets.json`: labels for built-in preset ids (`builtin.<id>`), profile ids (`profile.<id>`), filter chips, "group", "newPreset", "newGroup", "newProfile".
- `shelf.html`, `src/shelf.ts`, `src/shelf/Shelf.svelte` placeholder; vite entry `shelf`; capability window `shelf`.
- Nav: the v1 `settings` view is now `general` (still renders the v1 settings page until Task 1).

---

## Task 1: Window chrome, sidebar and settings pages (frontend)

**Files:**
- Modify: `src/app/TitleBar.svelte`, `src/app/Sidebar.svelte`, `src/app/Shell.svelte`, `src/lib/stores/nav.svelte.ts`, `src/lib/nav.test.ts`, `src/lib/stores/theme.ts`, `src/app/views/convert/index.svelte` (home hint + recent list only)
- Create: `src/app/views/settings/{General,Appearance,Engines,About,ClipboardSettings,Shortcuts}.svelte`, `src/app/views/settings/Placeholder.svelte`
- Delete: `src/app/views/settings/index.svelte` (content split into the files above)
- Locales: `src/locales/{tr,en}/common.json`, `settings.json`, `convert.json`

**Interfaces:**
- Consumes: Task 0 `Settings`, `View`, `api.*`.
- Produces: `NAV_MAIN: View[] = ['convert','download','clipboard','recent']`, `NAV_SETTINGS: View[] = ['general','wheel','presets','shortcuts','watch','clipboardSettings','appearance','engines','about']` exported from `nav.svelte.ts`; `Shell.svelte` renders each view from a `{#if}` chain — later tasks replace `Placeholder` for `wheel`, `presets` (Task 7), `watch` (Task 10), `editor` (Task 8) and the `clipboard` view (Task 8) by editing only their branch line. `Shortcuts.svelte` holds the wheel-trigger section; Task 8 appends a "Global shortcuts" section to it. `ClipboardSettings.svelte` holds the v1 clipboard settings; Task 8 extends it.

Requirements:
- TitleBar per spec §1: drag region full width; caption buttons top-right (46×32, Segoe Fluent Icons / Segoe MDL2 Assets glyphs E921/E922/E923/E8BB, hover/pressed/close-red, dim when unfocused); maximize glyph tracks `getCurrentWindow().isMaximized()` via `onResized`. Remove traffic lights entirely. Keep `aria-label`s (`common.window.minimize|maximize|restore|close`).
- Sidebar per spec §1: logo + name at the top (no titlebar padding), main group, "AYARLAR"/"SETTINGS" caption, 9 settings items with Lucide icons (General: `Settings2`, Wheel: `CircleDot`, Presets: `SlidersHorizontal`, Shortcuts: `Keyboard`, Watch: `FolderSync`, Clipboard settings: `ClipboardList`, Appearance: `Palette`, Engines: `Cpu`, About: `Info`), scrollable list, bottom: GitHub button (`api.openPath('https://github.com/zarlof/file-chameleon')`; the Rust side uses the opener plugin, which hands URLs to the shell) and version. Collapsed mode: icons + separator.
- Shortcuts in `Shell.svelte`: Ctrl+1..4 → `NAV_MAIN`, Ctrl+, → `general`.
- Settings pages (each a scrolling page with the v1 look: `SectionHeader` + `ListRow`s):
  - General: language, launch at login, close to tray (`closeToTray`), Explorer right-click (`contextMenu`), output folder, default image quality, download folder/mode/quality/cookies (moved from v1).
  - Appearance: theme, accent, UI scale (`uiScale` 80–150 step 10 → `getCurrentWebview().setZoom(uiScale/100)`; add capability permission `core:webview:allow-set-webview-zoom` in `src-tauri/capabilities/default.json`), reduce motion (`reduceMotion` → `document.documentElement.dataset.reduceMotion`, and make `theme.css`'s existing reduced-motion block also apply to `:root[data-reduce-motion]`), wheel colour (`wheelColor`: accent + 8 accents as swatches), wheel opacity (40–100), wheel labels (auto/icon+text/text/icon), wheel size (80–130), show detail line.
  - Shortcuts: wheel on/off, any app vs Explorer only (v1 fields), never-trigger apps (`wheelExcludedApps`, chip list + text field, lowercase, `.exe` appended when missing), flip with mouse wheel, flip with Tab, Ctrl+Shift = back face.
  - ClipboardSettings: the v1 clipboard rows (retention now `clipboardRetentionImages`), unchanged otherwise.
  - Engines, About: moved from v1. About adds the GitHub URL, licences link to THIRD_PARTY_NOTICES text if v1 had it, and the "inspired by Filee, Nashi, Clipboardline — not affiliated" line.
  - `Placeholder.svelte`: title + "Bu sayfa hazırlanıyor" (used until later tasks land).
- Convert home (Filee "Home"): hint line under the drop card (`convert.home.hint`: "Ya da Explorer'da Ctrl'ye basılı tutarak sürükleyin; çark imlecin yanında açılır.") and "Son dönüşümler" list: last 6 `jobs` with state `done`, each with output name, open, show in folder; a "Temizle" button calls `api.clearJobs()`. Hidden when empty or when files are loaded.

- [ ] **Step 1: Write failing tests** in `src/lib/nav.test.ts`: `it('lists main and settings views')` → `expect(NAV_MAIN).toEqual(['convert','download','clipboard','recent'])`, `expect(NAV_SETTINGS).toHaveLength(9)`, `expect(new Set([...NAV_MAIN, ...NAV_SETTINGS]).size).toBe(13)`. Extend `src/locales/locales.test.ts`'s namespace list with `common` (tr/en same keys).
- [ ] **Step 2:** `npx vitest run src/lib/nav.test.ts` → FAIL (NAV_MAIN undefined).
- [ ] **Step 3:** Implement everything above.
- [ ] **Step 4:** `npx vitest run` → PASS; `npm run check` → 0 errors in your files.
- [ ] **Step 5:** Visual check: `npm run tauri dev`, screenshot the window (light + dark), caption buttons hover states, a settings page. Fix what looks off.
- [ ] **Step 6:** Commit `feat(ui): Windows caption buttons, sectioned sidebar and split settings pages`.

---

## Task 2: App icon (orchestrator does this)

**Files:** Create `assets/icon.svg`, `crates/fc-core/examples/svg_to_png.rs`; regenerate `assets/icon-source.png`, `src-tauri/icons/*`; delete `assets/make_icon.py`.

- [ ] Draw the folder + green chameleon SVG (spec §2); render 1024² with `cargo run -p fc-core --example svg_to_png -- assets/icon.svg assets/icon-source.png 1024`; inspect at 1024/64/32/16 px; iterate until legible.
- [ ] `npx tauri icon assets/icon-source.png -o src-tauri/icons`; commit `feat: new app icon (green chameleon on a brown folder)`.

---

## Task 3: Core — presets, new operations, formats and engines (`crates/fc-core`)

**Files:**
- Create: `crates/fc-core/src/presets/mod.rs` (replace stub), `crates/fc-core/src/presets/builtin.rs`, `crates/fc-core/src/engines/files.rs`, `crates/fc-core/src/engines/subtitle.rs`, `crates/fc-core/src/b64.rs`, `crates/fc-core/tests/presets.rs`, `crates/fc-core/tests/engines_v2.rs`
- Modify: `crates/fc-core/src/jobs/mod.rs` (Operation/ImageOp), `crates/fc-core/src/jobs/queue.rs` (only if new ops need queue-level handling, e.g. one job for Zip), `crates/fc-core/src/engines/{mod,image,ffmpeg,data}.rs`, `crates/fc-core/src/formats/{mod,registry,planner}.rs`, `crates/fc-core/src/lib.rs` (`pub mod b64;`), `crates/fc-core/Cargo.toml` (add `base64 = "0.22"`)
- Also update any `src-tauri` match on `fc_core::formats::Kind` / `Operation` that stops compiling (exhaustive matches) with the minimal arm.

**Interfaces:**
- Produces (Rust):
  - `jobs::Operation` new variants, serde `tag="type"`, camelCase, matching TS: `Edit { ops: Vec<ImageOp>, to: Option<String> }`, `CropAspect { w: u32, h: u32 }`, `CopyTo { dir: Option<PathBuf>, move_files: bool }`, `Zip`, `Extract`, `Annotate`.
  - `jobs::ImageOp` (serde `tag="type"`, camelCase variants and fields): `Grayscale, RotateLeft, RotateRight, Rotate180, FlipH, FlipV, Trim, Pixelate { block: u32 }, Blur { sigma: f32 }, Scale { percent: u32 }`.
  - `formats::Kind` + `Subtitle`, `Archive` (serde lowercase). New formats: `m4r` (Audio, `["m4r"]`, label "M4R"), `tsv` (Data), `srt` (Subtitle), `vtt` (Subtitle, `["vtt"]`, label "WebVTT"), `ass` (Subtitle, `["ass","ssa"]`), `zip` (Archive). Edges: audio→m4r, video→m4r (ffmpeg, ≤ 40 s, AAC in ipod muxer); `tsv`↔csv/json/yaml/toml (Data); `srt`↔`vtt`, `srt`/`vtt`→`txt` (new `EngineId::Subtitle`, in-process); `ass`→`srt`/`vtt` (ffmpeg).
  - `b64::{encode(&[u8]) -> String, decode(&str) -> Result<Vec<u8>>}`; `decode` accepts an optional `data:*;base64,` prefix.
  - `presets::{WheelConfig, Preset, Group, Profile, Face, WheelSlot}` (serde camelCase, field names exactly as TS `WheelConfig`/`Preset`/`PresetGroup`/`Profile`/`WheelSlot`; `WheelSlot.kind: Option<formats::Kind>`, `icon: String` with the `SlotIcon` keys):
    - `WheelConfig::builtin() -> Self`; `WheelConfig::load(path: &Path) -> Self` (missing/corrupt → builtin; merges built-ins missing from the file — presets by id, profiles by id — without changing existing arrangements; drops dangling slot ids, group members and over-limit entries); `WheelConfig::save(&self, path: &Path) -> Result<()>` (atomic); `WheelConfig::validate(self) -> Result<Self>` (`Error::Unsupported("invalid: ...")` → give it its own variant `Error::Invalid(String)` with code `"invalid"`; checks: unique ids, front ≤ 8, back ≤ 12, group ≤ 6, refs exist, user preset ops valid e.g. quality 1..=100, scale 1..=400, pixelate block 2..=256, blur sigma 0.5..=50);
    - `fn reset_profile(&mut self, id: &str) -> bool` (built-in → its default arrangement/options; custom → false);
    - `fn preset(&self, id: &str) -> Option<&Preset>`;
    - `fn profile_for(&self, paths: &[PathBuf]) -> &Profile` (resolution rules in spec §3);
    - `fn slots(&self, profile: &Profile, face: Face, paths: &[PathBuf]) -> Vec<WheelSlot>` (applicable only; groups keep only applicable children and are dropped when empty);
    - `fn applicable_ids(&self, extensions: &[String]) -> Vec<String>` (preset applicable to a file with at least one of these extensions);
    - `fn requests(&self, preset_id: &str, paths: &[PathBuf]) -> Result<Vec<JobRequest>>` (convert/edit/crop/compress/mute/speed: one request with all paths — the queue already splits per input; merge/zip: one request; `CopyTo { dir: None }` and `Annotate` → `Error::Invalid` (the shell resolves them first)).
    - `pub fn applicable(preset: &Preset, paths: &[PathBuf]) -> bool`.
  - Built-in ids, arrangements: spec §3; default fronts: images `to-png to-webp to-pdf to-jpg-70 to-ico to-bmp to-tiff to-gif`, back `rotate-left rotate-right flip-h grayscale trim pixelate blur resize-50 fit-1920 annotate merge-pdf group:files`; built-in group `files` (name `""`, the UI shows `presets.groupName.files`) holding `zip copy-to move-to`. Video front `to-mp4 to-webm to-gif to-mp3 compress mute to-mkv to-mov`, back `crop-16-9 crop-9-16 crop-1-1 crop-4-3 speed-2x speed-half to-m4a to-wav to-m4r zip copy-to move-to`. Audio front `to-mp3 to-m4a to-wav to-flac to-ogg to-opus to-m4r compress`, back `speed-2x speed-half zip copy-to move-to`. PDF front `to-png to-jpg merge-pdf split-pdf compress zip`, back `copy-to move-to`. Vector front `to-png to-jpg to-webp to-ico`, back `zip copy-to move-to`. Text (`md markdown txt html htm`) front `to-html to-txt`… only routes that exist; Data front `to-csv to-tsv to-json to-yaml to-toml`; Subtitles front `to-srt to-vtt to-txt`; Archives (`zip`) front `extract copy-to move-to`; Mixed front `to-pdf merge-pdf zip copy-to move-to`; Other front `zip copy-to move-to`. (Slots that do not fit a drop are hidden, so shared fronts are fine.)
  - `WheelSlot` details: convert → `format` = target label, `detail` = `"70%"` when quality set, `"1920 px"` when max size set; edit → icon per first op, detail for scale (`"50%"`), pixelate/blur none; crop → `"16:9"`; speed → `"2×"`/`"0.5×"`; group → `icon "group"`, `children`.
- Engines: image edits in `engines/image.rs` (trim = bounding box of alpha > 0, no-op when nothing to trim; pixelate = downscale by block with nearest then upscale nearest; blur = `imageops::blur`); `CropAspect` images in-process, video ffmpeg `crop=` centre; `files.rs`: `CopyTo` (copy or move each input; move falls back to copy+delete across volumes; same folder + move → skip with no error), `Zip` (deflate, entry names = file names, duplicates get " (1)"), `Extract` (into `<stem>` folder, unique; reject `..`/absolute entry paths — zip-slip); `subtitle.rs`: SRT↔VTT (timestamps `,`↔`.`, `WEBVTT` header, drop cue settings/position tags `{\an8}` and `<...>` tags for txt), →txt (text lines only, blank line between cues).

- [ ] **Step 1: Write failing tests** `crates/fc-core/tests/presets.rs`:
  - `builtin_is_valid` → `WheelConfig::builtin().validate().is_ok()`; every profile front ≤ 8, back ≤ 12; every slot id resolves.
  - `resolves_profiles` → `["a.JPG","b.png"]` → profile `images`; `["x.mp4"]` → `video`; `["a.jpg","b.mp4"]` → `mixed`; `["a.docx"]` → `other`; `["a.jpg","b.docx"]` → `mixed`.
  - `resolves_mixed_and_unknown_selections` → for `["a.jpg","b.docx","c.mp4"]` the mixed front slots are non-empty and contain `zip`, and do not contain `to-png` (not applicable to docx).
  - `slots_hide_inapplicable_and_carry_details` → images front for `["a.png"]` contains `to-jpg-70` with `format == Some("JPG")`, `detail == Some("70%")`; for `["a.gif"]` (animated or not) `to-gif` is absent (same format); PDF front without pdfium still lists `to-png` with `needs_tools == ["pdfium"]`.
  - `load_drops_dangling_refs_and_merges_builtins` → write a JSON with profile `images` front `["to-png","ghost"]`, a group referencing `ghost`, no `video` profile → load → front `["to-png"]`, group without `ghost`, `video` profile present.
  - `validate_rejects_limits` → front of 9 → `Err(Error::Invalid(_))`; duplicate preset id → `Err`; quality 0 → `Err`.
  - `requests_build_jobs` → `requests("to-jpg-70", [a.png, b.png])` → one request, `op == Convert{to:"jpg"}`, `options.quality == Some(70)`; `requests("copy-to", ..)` → `Err(Error::Invalid(_))`; `requests("zip", [a,b])` → one request `op == Zip` with 2 inputs.
  - `applicable_ids_for_extensions` → `["mp4"]` contains `crop-16-9`, `to-gif`, not `grayscale`.
- [ ] **Step 2:** `cargo test -p fc-core --test presets` → FAIL to compile.
- [ ] **Step 3:** Implement `presets`, `b64`, new `Operation`/`ImageOp`, formats/edges.
- [ ] **Step 4: Write failing engine tests** `crates/fc-core/tests/engines_v2.rs` (in-process only; ffmpeg ones `#[ignore]` unless ffmpeg on PATH, like v1 `tests/ffmpeg.rs`):
  - `edit_ops_change_pixels` → 4×2 RGBA PNG; `RotateLeft` → output 2×4; `FlipH` mirrors pixel (0,0)↔(3,0); `Grayscale` → r==g==b; `Scale{50}` → 2×1.
  - `trim_removes_transparent_border` → 10×10 transparent with opaque 2×3 block at (4,5) → output 2×3; fully opaque → unchanged size.
  - `crop_aspect_image` → 400×200 + `CropAspect{1,1}` → 200×200.
  - `zip_and_extract_roundtrip` → zip two files with the same name from different folders → entries `a.txt`, `a (1).txt`; extract → identical bytes in `<stem>/`.
  - `extract_rejects_zip_slip` → archive entry `../evil.txt` → `Err`, nothing written outside.
  - `copy_and_move` → copy into a folder that already has the name → `name (1).ext`; move within the same folder → source still exists, no error.
  - `subtitles_convert` → SRT `1\n00:00:01,000 --> 00:00:02,500\n{\an8}<i>Hi</i>\n` → VTT starts `WEBVTT`, has `00:00:01.000 --> 00:00:02.500`; → TXT equals `Hi\n`; VTT → SRT round-trips the timestamp.
  - `tsv_roundtrip` → TSV → CSV → TSV equal.
  - `m4r_limits_duration` (`#[ignore]`/ffmpeg) → 60 s sine → m4r duration ≤ 40.5 s.
  - `b64_roundtrip` → `decode(&format!("data:image/png;base64,{}", encode(b"\x00\xffhi")))` == bytes.
- [ ] **Step 5:** Run → FAIL; implement engines; run → PASS.
- [ ] **Step 6:** `cargo test -p fc-core`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check -p file-chameleon` → all green.
- [ ] **Step 7:** Commit `feat(core): presets, wheel profiles, image edits, crop, zip/extract, copy/move, subtitles, m4r, tsv`.

---

## Task 4: Core — clipboard store v2, link titles, folder watcher (`crates/fc-core`)

**Files:**
- Modify: `crates/fc-core/src/clip/{mod,history}.rs`, `crates/fc-core/tests/clip.rs`
- Create: `crates/fc-core/src/clip/enrich.rs`, `crates/fc-core/src/watch.rs` (replace stub), `crates/fc-core/tests/watch.rs`
- Modify call sites of `ClipStore::prune` / removed methods in `src-tauri/src/{state.rs,platform/mod.rs,platform/clipboard.rs,ipc/clipboard.rs}` with the minimal change that compiles (Task 9 rewrites the shell side).

**Interfaces:**
- Produces (Rust, `fc_core::clip`):
  - `ClipItem` v2: all v1 fields + `title: Option<String>`, `note: Option<String>`, `tags: Vec<String>`, `folder: Option<u64>`, `ocr: Option<String>`, `trashed_at: Option<u64>`, `source_app: Option<String>`, `link_title: Option<String>`, `edited: Option<PathBuf>`, `edited_thumb: Option<PathBuf>`, `size: u64`; every new field `#[serde(default)]` so v1 `history.json` loads; serialized `saved: bool` (computed: pinned || folder || tags || title || note) — serialize via a getter (`#[serde(serialize_with)]` or a wire struct), ignored on deserialize. `fn saved(&self) -> bool`, `fn shelf(&self) -> Shelf`.
  - `enum Shelf { Images, Texts, Links }` (serde lowercase: `images|texts|links`).
  - `ClipQuery { text: Option<String>, shelf: Option<Shelf>, pinned: bool, folder: Option<u64>, tag: Option<String>, trash: bool, saved: bool, limit: Option<usize> }` (serde camelCase, all `default`).
  - `Retention { text_days: u32, links_days: u32, images_days: u32, max_items: usize }` (0 days = forever) + `Retention::from_settings(&Settings) -> Self`.
  - `Folder { id: u64, name: String, parent: Option<u64> }`; `FolderInfo { id, name, parent, count: usize }` (serde camelCase = TS `ClipFolder`); `TagInfo { tag: String, count: usize }`; `ClipStats { items: usize, trash: usize, bytes: u64 }`; `enum Field { Title, Note, Text }` (serde lowercase).
  - `ClipStore` methods (all persist atomically; ids unknown → `Error::NotFound`-like `Error::Failed("no such item")` is fine):
    `open(dir)` (migrates v1), `items()`, `get(id) -> Option<&ClipItem>`, `add_text(text, now) -> Result<Option<ClipItem>>` (skips blank/whitespace-only and > `MAX_TEXT_BYTES`; dedupe by hash → bump to top, and if the match was trashed, restore it), `add_image_png(..)` (v1 signature, same dedupe/restore), `set_source_app(id, Option<String>)`, `query(&ClipQuery) -> Vec<ClipItem>` (newest first; pinned first only when `pinned` filter is off? → no: plain newest first; text search case-insensitive over text, title, note, tags, ocr, link_title, site; Turkish dotted/dotless i folded: `İ/I/ı/i` all match), `set_pinned(ids, bool)`, `set_field(id, Field, &str) -> Result<ClipItem>` (empty → None for title/note; `Text` only for text kinds, re-hashes), `trash(ids, now)`, `restore(ids)`, `delete_forever(ids)` (removes image files), `empty_trash()`, `clear_unsaved(now) -> Result<usize>`, `folders() -> Vec<FolderInfo>`, `folder_create(name, parent) -> Result<Folder>`, `folder_rename(id, name)`, `folder_delete(id)` (items → `None`, children → parent), `move_to(ids, Option<u64>)`, `tags() -> Vec<TagInfo>` (sorted by count desc then name), `tag(ids, add, remove)` (tags trimmed, case-preserving, deduped case-insensitively), `set_ocr(id, String)`, `set_link_title(id, String)`, `save_edited(id, png: &[u8]) -> Result<ClipItem>` (writes `img/<id>.edit.png` + thumb), `revert_edit(id) -> Result<ClipItem>`, `needs_ocr() -> Vec<(u64, PathBuf)>` (live images without OCR, newest first), `prune(now, &Retention) -> Result<usize>` (unsaved live items older than their kind's days; then the oldest unsaved beyond `max_items`; trashed items older than 30 days deleted for good; image files removed), `stats() -> ClipStats`, `export(ids, dir) -> Result<usize>` (images as `<title or "Görsel <stamp>">.png` (edited version if any), texts/links as `.txt`, names unique via `jobs::naming`).
  - `clip::enrich::{fn fetch_link_title(url: &str) -> Option<String>, fn is_private_or_secret(url: &str) -> bool}`: skip `localhost`, `*.local`, private/loopback/link-local IPs, non-http(s), and query keys like `token, key, sig, signature, password, pass, secret, auth, code, session` (case-insensitive); GET with 5 s timeout, read at most 256 KB, take `og:title` else `<title>`, decode basic HTML entities, trim to 200 chars.
  - `fc_core::watch`:
    `pub struct WatchSpec { pub id: String, pub dir: PathBuf, pub recursive: bool, pub exclude: Vec<PathBuf>, pub extensions: Option<Vec<String>> }`;
    `pub struct Scanner` (pure, testable): `Scanner::new(settle: Duration)`, `fn set_specs(&mut self, specs: Vec<WatchSpec>, now: Instant)` (new specs take a baseline: files already there never fire), `fn ignore(&mut self, path: PathBuf)`, `fn tick(&mut self, now: Instant) -> Vec<(String, PathBuf)>` (scans; returns files whose size and mtime have not changed for `settle` and that can be opened for reading; each path fires once per content change), `fn errors(&self) -> Vec<(String, String)>` (spec id, message — e.g. folder missing);
    `pub struct Watcher` (thread around `Scanner`): `Watcher::start(interval: Duration, settle: Duration, on_file: impl Fn(String, PathBuf) + Send + 'static) -> Watcher`, `fn set_specs(&self, Vec<WatchSpec>)`, `fn ignore(&self, PathBuf)`, `fn errors(&self) -> Vec<(String, String)>`; dropping it stops the thread. `pub fn is_temp_name(name: &str) -> bool` (`.crdownload .part .partial .tmp .download`, prefix `~$`, `.~lock`).
- Consumes: `fc_core::settings::Settings`, `jobs::naming`, `download::classify_url` (v1).

- [ ] **Step 1: Write failing tests** `crates/fc-core/tests/clip.rs` (add): `v1_index_migrates` (write a v1 JSON line from the v1 test fixture style → open → `tags.is_empty()`, `!saved`); `blank_and_oversized_text_are_skipped` (`"  \n\t"` → `None`, 1 MB + 1 → `None`); `duplicate_refreshes_and_restores_from_trash` (add "a", trash it, add "a" → same id, `trashed_at == None`, top of list); `query_filters_and_search` (shelf links returns link + mediaLink; `text:"istanbul"` matches title "İSTANBUL"; tag filter; folder filter; trash only with `trash: true`); `saved_items_survive_prune` (pinned, tagged, foldered old items kept; unsaved old text removed after 30 d with `text_days: 30`; `0` = forever keeps; `max_items` drops oldest unsaved); `trash_restore_and_empty` (trash → hidden from default query → restore → visible; empty_trash removes files on disk); `folders_and_tags` (create nested, delete parent → child parent is None... (moves up), items folder None; tags dedupe "Work"/"work"); `edit_and_revert_keep_original`; `export_writes_unique_files`.
- [ ] **Step 2:** `cargo test -p fc-core --test clip` → FAIL.
- [ ] **Step 3:** Implement store v2; `enrich` with a pure test `private_and_secret_urls_are_skipped` (`http://192.168.1.4/x`, `http://localhost:3000`, `https://a.com/?token=1` → true; `https://youtube.com/watch?v=1` → false) and `title_extraction` on an HTML string (expose `pub fn extract_title(html: &str) -> Option<String>`; `og:title` wins over `<title>`; `&amp;` decoded).
- [ ] **Step 4: Write failing watcher tests** `crates/fc-core/tests/watch.rs` using `Scanner` with explicit `Instant`s and a temp dir: `existing_files_never_fire`; `waits_until_file_is_stable` (write → tick at t0 → none; append → tick at t0+3s → none (changed); tick at t0+6s → fires once; next tick → none); `ignores_temp_names_and_excluded_dirs` (`x.crdownload`, `~$doc.docx`, file under `converted\` excluded → never fire); `ignored_paths_never_fire`; `recursive_flag`; `extension_filter`; `missing_folder_reports_error`.
- [ ] **Step 5:** Implement `watch`; run → PASS.
- [ ] **Step 6:** `cargo test -p fc-core`, clippy, fmt, `cargo check -p file-chameleon` → green.
- [ ] **Step 7:** Commit `feat(core): clipboard store v2 (folders, tags, trash, retention, edits), link titles, folder watcher`.

---

## Task 5: Overlay wheel v2 (frontend: `src/lib/wheel`, `src/overlay`)

**Files:**
- Create: `src/lib/wheel/Wheel.svelte` (moved from `src/overlay/Wheel.svelte`, rewritten), `src/lib/wheel/labels.ts`, `src/lib/wheel/icons.ts`, `src/lib/wheel/labels.test.ts`
- Modify: `src/lib/wheel/geometry.ts`, `src/lib/wheel/geometry.test.ts`, `src/overlay/Overlay.svelte`, `src/overlay/i18n.ts`, `src/overlay.ts`, `src/lib/types.ts` (remove `WheelTargets`), `src/lib/ipc.ts` (remove `overlayWheelTargets`), `src/locales/{tr,en}/overlay.json`
- Delete: `src/overlay/Wheel.svelte`

**Interfaces:**
- Consumes: Task 0 `WheelSlots`, `WheelSlot`, `WheelStyle`, `WheelThumb`, `on.overlayWheel/overlayWheelSlots/overlayWheelFlip/overlayWheelThumb/overlayHide`, `api.wheelDrop/overlayHide`.
- Produces (used by Task 7's editor preview):
  - `Wheel.svelte` props: `{ front: WheelSlot[]; back: WheelSlot[]; face: WheelFace; hovered: { slot: number; child: number }; fan: number /* group slot index whose ring is open, -1 */; style: WheelStyle; center: { title: string; subtitle: string }; missingTools: string[]; size?: number /* px, default from geometry */; selected?: number /* editor highlight, -1 */; onslotclick?: (i: number) => void /* editor */ }`.
  - `geometry.ts`: `ringRadii(scale: number) -> { inner, outer, fanInner, fanOuter, size }` (scale 1: inner 64, outer 168, fanInner 176, fanOuter 246, size 540); `hitTest(x, y, count, fan: { index: number; children: number } | null, scale) -> { slot: number; child: number }` (-1s for none; child ring spans the group's angle widened to at least 30° per child, centred on it, clamped to the ring); `slicePath(i, count, inner, outer, gapPx)` (constant pixel gap); `fanSlicePath(...)`; keep `toLogical`, `polar`, `sliceCenterAngle`; remove `orderSlices`/`MAX_SLICES`/`MORE_ID`/`needsDownload` users.
  - `labels.ts`: `slotLabel(slot: WheelSlot, t: (k: string) => string): string` (name || format || `t('presets.builtin.'+id)`; groups: name || `t('presets.groupName.'+groupId)` || `t('presets.group')`), `presetLabel(p: Preset, t): string`, `profileLabel(p: { id; name }, t): string`, `centerTitle(count: number, firstExt: string | null, t): string` ("1 JPG", "3 dosya" when extensions differ).
  - `icons.ts`: `slotIcon(key: SlotIcon): Component` (Lucide; group → `Layers`).
  - Overlay i18n reads `overlay.json` and `presets.json`.

Requirements: spec §3 "Wheel behaviour" look and interaction. Drag mode: hover from drag `over` (physical → logical with `devicePixelRatio`), drop → `api.wheelDrop(paths, presetId)` (child preset id for fans; dropping on a group slot itself or the centre → `api.overlayHide()`). Click mode: pointer events for hover, click → `wheelDrop`, DOM `wheel` event flips, click in the transparent margin → `overlayHide()`. `overlay://wheel-flip` flips. Group fan opens after 200 ms hover or when the pointer passes 0.85×outer inside the group's angle, closes when the pointer leaves group angle + fan ring. Flip animation: each slice `rotateY` 90° out/in with delay `40 ms × angular distance from hovered`; instant with `style.reduceMotion`. Colours: slice fill light `#F3F2EF` / dark `#2C2C30` (define as CSS vars in the component scoped to the theme), hovered = `style.color` with white text, opacity = `style.opacity/100` on the disc. Centre: white (dark: `--elevated-bg`) disc r 52 with an 8 px soft ring, `center.title` bold, `center.subtitle` muted ("Esc ile kapat"), small flip hint (`RefreshCw` 12 px + "Tab") when `back.length > 0`. Labels by `style.labels` (`auto` = icon+text when ≤ 6 slots, else text). File chip: 44×52 card following the pointer (+16, +18) with the kind icon, the extension badge (`firstExt` upper-case) and the `WheelThumb` image when it arrives. Download badge on slots whose `needsTools` intersect `missingTools`. Empty faces: centre shows `overlay.notSupported`.

- [ ] **Step 1: Failing tests** `geometry.test.ts`: `ringRadii(1).size === 540`; `hitTest(270, 270-100, 8, null, 1)` → `{slot:0, child:-1}` (top); right side → slot 2; centre → `{-1,-1}`; outside fanOuter → `{-1,-1}`; with `fan {index:0, children:3}` a point at radius 210, angle 0 → `{slot:0, child:1}`; scale 1.2 scales radii. `labels.test.ts`: `slotLabel` precedence; `centerTitle(1,'jpg')` → `'1 JPG'`; `centerTitle(3, null)` uses the count key.
- [ ] **Step 2:** `npx vitest run src/lib/wheel` → FAIL.
- [ ] **Step 3:** Implement; remove the deprecated `WheelTargets` type/wrapper.
- [ ] **Step 4:** `npx vitest run` → PASS; `npm run check` clean in your files.
- [ ] **Step 5:** No live check here (the overlay needs Task 6's events); Task 12 verifies it live. Re-read the component against spec §3 instead.
- [ ] **Step 6:** Commit `feat(wheel): Filee-style donut with front/back faces, groups, click mode and file chip`.

---

## Task 6: Shell — wheel v2, presets IPC, Explorer verb, close behaviour, editor backend (`src-tauri`)

**Files:**
- Modify: `src-tauri/src/platform/hook.rs`, `src-tauri/src/platform/win.rs`, `src-tauri/src/platform/mod.rs`, `src-tauri/src/windows.rs`, `src-tauri/src/events.rs`, `src-tauri/src/state.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/ipc/mod.rs`, `src-tauri/src/tray.rs` (only if close/quit needs it), `src-tauri/windows/hooks.nsh`, `src-tauri/Cargo.toml` (`windows` feature `Win32_System_Registry`)
- Create: `src-tauri/src/ipc/presets.rs`, `src-tauri/src/platform/contextmenu.rs`

**Interfaces:**
- Consumes: Task 3 `presets::*`, `b64`, `Operation` v2; Task 0 settings fields.
- Produces: commands `get_wheel_config`, `set_wheel_config(config)`, `reset_wheel_profile(id)`, `reset_wheel_config`, `presets_for_extensions(extensions)`, `run_preset(paths, preset_id)`, `wheel_open(paths)`, `wheel_drop(paths, target)` (target = preset id), `read_image_preview(path, max_side) -> String` (PNG data URL, longest side ≤ max_side, ≤ 4096), `save_annotated(path, data_url) -> String`; events `overlay://wheel {scale, click}`, `overlay://wheel-slots` (`WheelSlots`), `overlay://wheel-flip`, `overlay://wheel-thumb`; `AppState::wheel_config() -> MutexGuard<WheelConfig>` (lazy load from `<config>/wheel.json`) and `AppState::save_wheel_config()`; `pub fn run_preset(app: &AppHandle, paths: Vec<PathBuf>, preset_id: &str, interactive: bool, output_dir: Option<PathBuf>) -> CmdResult<Vec<JobId>>` in `ipc/presets.rs` (Task 10's watcher calls it with `interactive = false`: no dialogs, no editor, no toasts for missing tools — returns `Err` instead; `output_dir` overrides the preset's).

Requirements:
- Hook (`hook.rs`): `DragStart` gains `shift: bool`; a `WHEEL_OPEN: AtomicBool` and `CLICK_MODE: AtomicBool` plus overlay rect atomics set by `windows.rs` (`pub fn set_wheel_open(open: bool, click: bool, rect: (i32,i32,i32,i32))`); while open: `WM_MOUSEWHEEL` → `HookEvent::Flip` (if `wheel_flip_scroll`; debounce 250 ms), in click mode `WM_LBUTTONDOWN`/`WM_RBUTTONDOWN` outside the rect → `HookEvent::Close`. Add a `WH_KEYBOARD_LL` hook on the same thread: while open, Tab keydown → `Flip` (if `wheel_flip_tab`; swallow it only in click mode), Esc keydown → `Close` (swallow in click mode). Not open → pure pass-through. Put the decision in a pure fn `fn key_action(open: bool, click: bool, vk: u32, flip_tab: bool) -> (Option<HookEvent>, bool /*swallow*/)` with a unit test `flip_keys_only_when_wheel_open`. Excluded apps: `source_allowed` also rejects exe names in `wheel_excluded_apps`.
- `windows.rs`: wheel window size from `ringRadii` (540 logical × `wheel_scale/100`); on drag `Enter` with paths: resolve profile/slots via `WheelConfig`, emit `overlay://wheel-slots` (style: colour resolved to hex from accent names: blue `#007AFF`, purple `#7C6CF0`, pink `#FF2D55`, red `#FF3B30`, orange `#FF9500`, yellow `#FFCC00`, green `#34C759`, graphite `#8E8E93`; "accent" → the current accent; face back when the drag started with Shift and `wheel_shift_back`); spawn a thread for the first image ≤ 30 MB → 96 px thumbnail → `overlay://wheel-thumb`. Flip/Close events → emit `overlay://wheel-flip` / hide. Click mode (`wheel_open`): show at cursor, make the overlay accept clicks (it never activates: keep `WS_EX_NOACTIVATE`), emit wheel + slots with `click: true`, no auto-hide timer, close on Esc/outside click/after a drop.
- `wheel_drop`/`run_preset`: preset lookup; `Annotate` → `show_main(NavPayload{view:"editor", paths})` (first image only); `CopyTo{dir: None}` → `tauri_plugin_dialog` blocking folder picker (interactive only; cancelled → `Ok(vec![])`); missing tool → v1 `toolNeeded` toast flow with `target` = preset id (extend `install_then_convert` to run the preset); else submit `requests()`; record the preset in `recent_targets` only for convert presets.
- Explorer verb (`contextmenu.rs`): `pub fn set_enabled(enabled: bool, lang: &str) -> std::io::Result<()>` writes/deletes `HKCU\Software\Classes\*\shell\FileChameleon` (`MUIVerb` = "File Chameleon ile dönüştür" / "Convert with File Chameleon", `Icon` = `"<exe>",0`, `command` = `"<exe>" --wheel "%1"`); applied in `apply_settings` when `context_menu` or `language` changes and once at startup if enabled (fixes a moved exe). `hooks.nsh` `NSIS_HOOK_POSTUNINSTALL`: `DeleteRegKey HKCU "Software\Classes\*\shell\FileChameleon"`.
- `--wheel` args: in the single-instance callback and at first start, collect paths after `--wheel` for 350 ms (several Explorer invocations) then `wheel_open`; do not show the main window. At first start wait for `overlay_ready`.
- `close_to_tray == false`: closing the main window exits the app (`app.exit(0)`) — handle in `on_window_event` for `MAIN` `CloseRequested`.
- Thumbnails/preview/annotated save use `image` + `b64`; `save_annotated` writes `<stem> (düzenlendi).png` (EN UI: `(edited)`) unique next to the source and returns it.
- [ ] **Step 1: Failing tests:** `hook.rs` `#[cfg(test)]` `flip_keys_only_when_wheel_open` (closed: Tab/Esc → `(None,false)`; open drag: Tab → `(Some(Flip), false)`, Esc → `(Some(Close), false)`; open click: Tab → swallow true; `flip_tab=false` → Tab ignored). `contextmenu.rs` pure `fn command_line(exe: &Path) -> String` test (quotes). `ipc/presets.rs` test `wheel_slots_payload_for_images` building the payload from `WheelConfig::builtin()` for `["a.png"]` (front non-empty, `count == 1`, `first_ext == "png"`).
- [ ] **Step 2:** `cargo test -p file-chameleon` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests, `cargo clippy --workspace --all-targets -- -D warnings`, fmt → green.
- [ ] **Step 5: Live check** (Task 5 merged first if available): `npm run tauri dev`; Ctrl+drag a PNG from Explorer → wheel shows image profile; mouse wheel flips; drop on WEBP → file created; Ctrl+Shift+drag opens back; enable the right-click setting → Explorer "Show more options" has the verb → click → wheel in click mode → Esc closes, click outside closes, click on PNG converts. Use computer-use if available; otherwise describe what you could not verify.
- [ ] **Step 6:** Commit `feat(shell): wheel v2 (profiles, flip, groups, click mode), presets IPC, Explorer verb, editor backend`.

---

## Task 7: Wheel editor and presets pages (frontend)

**Files:**
- Create: `src/app/views/wheel/{index.svelte,ProfileList.svelte,SlotEditor.svelte,PresetPicker.svelte,GroupSheet.svelte,logic.ts,logic.test.ts}`, `src/app/views/presets/{index.svelte,PresetEditor.svelte,logic.ts,logic.test.ts}`, `src/lib/stores/wheelConfig.svelte.ts`
- Modify: `src/app/Shell.svelte` (the `wheel` and `presets` branch lines only)
- Locales: `src/locales/{tr,en}/wheel.json`, `presetsPage.json` (namespace `presetsPage`), may add keys to `presets.json`

**Interfaces:**
- Consumes: Task 5 `Wheel.svelte`, `labels.ts`, `icons.ts`; Task 0 `api.getWheelConfig/setWheelConfig/resetWheelProfile/resetWheelConfig/presetsForExtensions`.
- Produces: `wheelConfig` store (`value`, `load()`, `save(next)` debounced 300 ms with rollback + error toast on `invalid`).

Requirements: spec §3 "Wheel editor" and "Presets page". Pure helpers in `logic.ts` (tested): `slotCounts(profile) -> string` ("8·12"), `addSlot(profile, face, slotId, at?) -> Profile` (respects limits 8/12, no duplicates), `moveSlot(profile, face, from, to)`, `removeSlot`, `filterPresets(presets, groups, query, filter: 'all'|'image'|'document'|'media'|'edit'|'files', t)`, `presetCategory(p: Preset): filter key`, `newPresetId(existing) -> "u-<n>"`, `normalizeExt(s) -> string` (lowercase, no dot, `[a-z0-9]{1,10}` else null). Presets page helpers: `describeOp(op, t) -> string`, `withOp(preset, kind)` default op per action kind. Editor preview uses `Wheel.svelte` with `selected` + `onslotclick`, `hovered` = selected. Built-ins are read-only in the Presets page ("Çoğalt" creates an editable `u-*` copy). "Klasöre kopyala" button: folder picker (`@tauri-apps/plugin-dialog` `open({directory:true})`) → creates a `copyTo` preset named after the folder and adds it to the current face.

- [ ] **Step 1:** Failing tests for every `logic.ts` helper above (limits: adding a 9th front slot returns the profile unchanged; duplicates refused; `normalizeExt('.JPG') === 'jpg'`, `normalizeExt('a b') === null`; `filterPresets` with filter `edit` returns `grayscale` and not `to-png`; `describeOp({type:'cropAspect',w:16,h:9})` contains `16:9`).
- [ ] **Step 2:** `npx vitest run src/app/views/wheel src/app/views/presets` → FAIL.
- [ ] **Step 3:** Implement pages; wire into `Shell.svelte`.
- [ ] **Step 4:** Tests + `npm run check` → green.
- [ ] **Step 5:** Visual check in `npm run tauri dev` (after Tasks 3/5/6 are merged): edit a profile, flip faces, add/remove/move slots, create a preset and a group, reload → persisted. Screenshot.
- [ ] **Step 6:** Commit `feat(ui): wheel editor and presets pages`.

---

## Task 8: Clipboard library, image editor, clipboard settings, global shortcuts section (frontend)

**Files:**
- Rewrite: `src/app/views/clipboard/*` (library; keep/extend `clips.ts` helpers and tests)
- Create: `src/app/views/editor/{index.svelte,canvas.ts,canvas.test.ts}`, `src/lib/ui/HotkeyRecorder.svelte`, `src/lib/hotkeys.ts`, `src/lib/hotkeys.test.ts`
- Modify: `src/app/views/settings/ClipboardSettings.svelte`, `src/app/views/settings/Shortcuts.svelte` (append a section), `src/app/Shell.svelte` (`clipboard` and `editor` branch lines only), `src/lib/ui/index.ts` (export `HotkeyRecorder`)
- Locales: `src/locales/{tr,en}/clipboard.json`, `editor.json`, `settings.json` (only keys under `settings.clipboard.*` and `settings.hotkeys.*`)

**Interfaces:**
- Consumes: Task 0 clip/shelf/hotkey wrappers and events (`clipList(query)`, `clipFolders`, `clipTags`, `clipSetField`, `clipPin`, `clipTrash/Restore/DeleteForever/EmptyTrash/ClearUnsaved`, `clipMove`, `clipTag`, `clipCopy`, `clipCopyOcr`, `clipExport`, `clipSaveEdited`, `clipRevertEdit`, `clipStats`, `hotkeyStatus`, `on.clipAdded/clipUpdated/clipChanged`), `api.readImagePreview`, `api.saveAnnotated` (Task 6), `convertFileSrc` for clipboard images (asset scope already covers `clipboard*`).
- Produces: `hotkeys.ts`: `fromKeyboardEvent(e: KeyboardEvent): string | null` ("Ctrl+Alt+V"; needs at least one of Ctrl/Alt/Win plus a non-modifier key; Shift alone + key is not allowed; F1–F24 allowed alone), `formatHotkey(s: string, lang): string` (Turkish shows "Ctrl + Alt + V"); `HotkeyRecorder.svelte` props `{ value: string; onchange: (v: string) => void; status?: boolean /* false = conflict */ }` (Esc cancels, Backspace/Delete clears = off).

Requirements: spec §5 Library, Editor, Pano ayarları; Shortcuts page "Global shortcuts" section (shelf, library, download link rows with `HotkeyRecorder`, conflict text when `hotkeyStatus()` says `ok:false`). Library layout: left filter column (Tümü / Görseller / Metinler / Bağlantılar, Sabitlenenler, Klasörler with + / rename / delete, Etiketler, Çöp kutusu), toolbar (search, grid/list toggle, batch actions when selection), content (virtualized list as v1 `rowWindow`; grid of cards), right inspector (collapsible) for one selected item. Keyboard: arrows, Ctrl+A, Shift/Ctrl+click, Enter copy, Delete trash, Ctrl+Z restore last trashed batch (keep a stack of 32), Ctrl+F search, Space preview sheet. Editor (`editor` view): loads `clip:<id>` (via `ClipItem.image`/`edited` + `convertFileSrc`) or a path (`readImagePreview(path, 4096)`); tools crop, pen, marker (40 % alpha, wide), rectangle, ellipse, arrow, line, text; colour swatches + stroke width; undo/redo (Ctrl+Z / Ctrl+Y); save → clip: `clipSaveEdited(id, canvas.toDataURL('image/png'))`, file: `saveAnnotated(path, dataUrl)` then toast with "Klasörde göster"; cancel → back to the previous view. Pure geometry in `canvas.ts` (tested): `normRect(a,b)`, `arrowHead(from,to,size) -> [p1,p2]`, `cropToRect(w,h,rect) -> clamped integer rect`, `hitHandle(rect, point, tol)`. Pano ayarları rows: enable, pause, capture text/links/images, suggestion toasts, retention per kind (1/3/7/15/30/forever), max items (100/300/500/1000/2000), ignored apps (chips), OCR, link titles (with a privacy note), watched image folders (≤ 3, add/remove via folder picker), shelf rows (toggle + order up/down), card size, click behaviour, "Kaydedilmemişleri temizle", "Çöp kutusunu boşalt", storage used (`clipStats`).

- [ ] **Step 1:** Failing tests: `hotkeys.test.ts` (`fromKeyboardEvent({ctrlKey:true, altKey:true, key:'v', code:'KeyV'})` → `'Ctrl+Alt+V'`; Shift+`a` alone → null; `F5` alone → `'F5'`; modifier-only → null); `canvas.test.ts` for the helpers; `clips.test.ts` new helpers you add (e.g. `selectionAfterClick(ids, current, id, {shift, ctrl})`).
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests + `npm run check` → green.
- [ ] **Step 5:** Visual check after Task 9 is merged: copy text/link/image → appear; folders/tags/trash/undo; edit an image; settings rows persist. Screenshot.
- [ ] **Step 6:** Commit `feat(ui): clipboard library, image editor, clipboard settings and global shortcuts`.

---

## Task 9: Shell — clipboard v2, OCR, link titles, image folders, hotkeys, shelf window (`src-tauri`)

**Files:**
- Modify: `src-tauri/src/platform/clipboard.rs`, `src-tauri/src/ipc/clipboard.rs`, `src-tauri/src/platform/mod.rs`, `src-tauri/src/platform/win.rs` (helpers), `src-tauri/src/state.rs`, `src-tauri/src/events.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/tray.rs` (menu item "Panoyu aç" → shelf), `src-tauri/Cargo.toml` (`windows` features: `Media_Ocr`, `Graphics_Imaging`, `Storage_Streams`, `Foundation`, `Foundation_Collections`, `Globalization`, `Win32_System_WinRT`, `Win32_System_Com`)
- Create: `src-tauri/src/platform/hotkeys.rs`, `src-tauri/src/platform/ocr.rs`, `src-tauri/src/shelf.rs`

**Interfaces:**
- Consumes: Task 4 store v2, `Retention`, `enrich`, `watch::Watcher`; Task 0 settings and `src/lib/ipc.ts` clip/shelf/hotkey wrappers.
- Produces: every clip/shelf/hotkey command in `src/lib/ipc.ts` (`clip_list(query)`, `clip_get`, `clip_set_field`, `clip_pin(ids,pinned)`, `clip_trash`, `clip_restore`, `clip_delete_forever`, `clip_empty_trash`, `clip_clear_unsaved`, `clip_folders`, `clip_folder_create`, `clip_folder_rename`, `clip_folder_delete`, `clip_move`, `clip_tags`, `clip_tag`, `clip_copy(ids)`, `clip_copy_ocr`, `clip_export`, `clip_save_image`, `clip_save_edited(id, data_url)`, `clip_revert_edit`, `clip_stats`, `clip_set_paused`, `clip_get_paused`, `show_shelf`, `shelf_hide`, `shelf_paste(ids)`, `hotkey_status`); events `clip://added`, `clip://updated`, `clip://changed`, `shelf://open`; `pub fn show_shelf(app: &AppHandle)` in `shelf.rs`.

Requirements:
- Capture: per-kind switches; ignore apps (clipboard owner → pid → exe name via `win.rs`; check before reading data); blank text skipped by the store; record `source_app`; after an image is added and `clipboard_ocr` is on → OCR queue; after a link is added and `clipboard_link_titles` is on and not `is_private_or_secret` → fetch title in a worker thread → `set_link_title` → `clip://updated`. Retention via `Retention::from_settings` on start (delayed 10 s, v1) and hourly while running (one sleeping thread is fine) and on settings change.
- OCR (`ocr.rs`): one worker thread with a channel, `RoInitialize(RO_INIT_MULTITHREADED)`; `OcrEngine::TryCreateFromUserProfileLanguages()` (None → OCR unavailable, no error spam); load PNG with the `image` crate, downscale so the longest side ≤ `OcrEngine::MaxImageDimension()`, build `SoftwareBitmap` (Bgra8, premultiplied) from bytes, `RecognizeAsync()?.get()?` → join lines with `\n`; store via `set_ocr`; emit `clip://updated`. Backfill: when OCR is turned on (or at start), queue `needs_ocr()` items, at most 50 per run.
- Image folders: a `watch::Watcher` (only when `clipboard_watch_folders` non-empty, ≤ 3, extensions png/jpg/jpeg/webp/bmp/gif) imports new images via `add_image_png` (convert non-PNG with `image`).
- Hotkeys (`hotkeys.rs`): one thread with a message loop; `RegisterHotKey(None, id, MOD_* | MOD_NOREPEAT, vk)` for shelf (1), library (2), download link (3); re-register on `PostThreadMessageW(WM_APP)` after settings change; `pub fn parse_hotkey(s: &str) -> Option<(u32 /*mods*/, u32 /*vk*/)>` (Ctrl, Alt, Shift, Win; A–Z, 0–9, F1–F24, Space, Insert, Home, …) with test `parse_hotkey` (`"Ctrl+Alt+V"` → `(MOD_CONTROL|MOD_ALT, 0x56)`, `"Ctrl+Shift+F5"`, `""` → None, `"Ctrl+"` → None, `"V"` → None (no modifier, not F-key)); `pub fn status() -> Vec<HotkeyStatus>`. Actions: shelf → `show_shelf`; library → `show_main(view clipboard)`; download link → read clipboard text; if `download::classify_url` says media → `show_main(view download, url)` (the Download view already auto-probes a passed url), else a short toast "Panoda bağlantı yok" via a new toast kind `noLink` (add to `events.rs` docs and the overlay `toasts.ts` + `overlay.json` keys `toast.noLink.title`).
- Shelf (`shelf.rs`): window label `shelf`, `shelf.html`, created on first use with `decorations(false)`, `transparent(true)`, `always_on_top(true)`, `skip_taskbar(true)`, `resizable(false)`, `shadow(false)`, same `data_directory` and browser args as v1; positioned top-centre of the work area under the cursor, width `min(1180, workarea − 48)`, height by rows × card size (small 112, medium 148, large 196 per row + 96 chrome); before showing remember `GetForegroundWindow()`; show + focus; emit `shelf://open`; `shelf_hide` hides (keep the window). `shelf_paste(ids)`: copy items (same logic as `clip_copy`), hide, `SetForegroundWindow(prev)` (if it still exists), sleep 60 ms, `SendInput` Ctrl+V; if `prev` is gone → just copy. `clip_copy(ids)`: one text → text; one image → image (edited if present); several all text-like → joined with `\n`; otherwise → CF_HDROP with the image files (edited if present) — mark our write as self-write (v1 `clip_self_write`).
- `clip_save_edited(id, data_url)` uses `fc_core::b64::decode` (Task 3).
- [ ] **Step 1:** Failing tests: `parse_hotkey`; `ipc/clipboard.rs` pure `fn copy_plan(items: &[ClipItem]) -> CopyPlan { Text(String) | Image(PathBuf) | Files(Vec<PathBuf>) | Nothing }` test `copy_plan_cases`; `shelf.rs` pure `fn shelf_rect(work: (i32,i32,i32,i32), scale: f64, rows: usize, card: &str) -> (i32,i32,i32,i32)` test (centred, clamped).
- [ ] **Step 2:** `cargo test -p file-chameleon` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests, clippy, fmt → green.
- [ ] **Step 5: Live check:** `npm run tauri dev`: copy text in Notepad → Ctrl+Alt+V opens the (placeholder) shelf window; copy an image with text (e.g. screenshot of this plan) → OCR text appears in `clip_list` (check via devtools `__TAURI__` or a log line); register conflict: set the shelf hotkey to one already used (e.g. `Ctrl+Alt+Delete` is invalid) → `hotkey_status` ok false.
- [ ] **Step 6:** Commit `feat(shell): clipboard v2 capture, OCR, link titles, image folders, global hotkeys, shelf window`.

---

## Task 10: Watch folders (full stack)

**Files:**
- Create: `src-tauri/src/watchfolders.rs`, `src/app/views/watch/{index.svelte,logic.ts,logic.test.ts}`
- Modify: `src-tauri/src/lib.rs` (start + commands), `src-tauri/src/platform/mod.rs` (`apply_settings`: reconfigure on `watch_folders` change), `src-tauri/src/state.rs` (holder), `src/app/Shell.svelte` (`watch` branch line only)
- Locales: `src/locales/{tr,en}/watch.json`

**Interfaces:**
- Consumes: Task 4 `watch::Watcher`, Task 6 `ipc::presets::run_preset(app, paths, preset_id, false, Some(<dir>\converted))`, Task 0 `WatchFolder`, `WatchStatus`, `api.watchStatus`, `on.watchStatus`.
- Produces: command `watch_status() -> Vec<WatchStatus>`, event `watch://status`.

Requirements: spec §4 "İzlenen klasörler". Specs: enabled rules only; `exclude` = `<dir>\converted`; extensions = those the preset applies to (from `WheelConfig::applicable_ids` inverse: pass `None` and check `presets::applicable` per file instead). On file: run the preset non-interactively with `ConvertOptions.output_dir = <dir>\converted` (create it); presets that need a folder picker (`copyTo` without dir) or `annotate` are rejected in the UI and skipped in Rust with an error status; outputs land in the excluded `converted` folder, so they never fire; emit `watch://status` after each file. UI: list of rules (folder path, preset select limited to presets usable here, subfolders toggle, enabled toggle, status line: "Etkin · 12 dosya işlendi · son: x.png" or the error), add (folder picker), remove; an empty state explaining the feature.

- [ ] **Step 1:** Failing tests: `logic.test.ts` `newRule(path, presets)` (id unique, default preset `to-webp` if present, enabled true), `usablePresets(presets)` excludes `annotate` and `copyTo{dir:null}`; Rust `watchfolders.rs` test `specs_from_settings` (disabled rules dropped, exclude path is `<dir>\converted`).
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Tests, clippy, fmt, `npm run check` → green.
- [ ] **Step 5: Live check:** add a temp folder with preset `to-webp`; copy a PNG in → `converted\x.webp` appears within ~5 s; copy a big file slowly (or write in chunks with PowerShell) → converted only after it settles; existing files untouched.
- [ ] **Step 6:** Commit `feat: watch folders`.

---

## Task 11: Clipboard shelf UI (`src/shelf`)

**Files:**
- Create/modify: `src/shelf.ts`, `src/shelf/{Shelf.svelte,Row.svelte,Card.svelte,Preview.svelte,i18n.ts,logic.ts,logic.test.ts}`
- Locales: `src/locales/{tr,en}/shelf.json`

**Interfaces:**
- Consumes: Task 0 wrappers `clipList`, `clipCopy`, `shelfPaste`, `shelfHide`, `clipPin`, `clipTrash`, `clipRestore`, `on.shelfOpen/clipAdded/clipUpdated/clipChanged/settingsChanged`, `api.getSettings`, `convertFileSrc`.
- Produces: nothing for other tasks.

Requirements: spec §5 Shelf. Bundle stays small: import only from `src/lib` (not `src/app`), its own tiny i18n like `src/overlay/i18n.ts` reading `shelf.json`. On `shelf://open`: reset search, select the first card of the first visible non-empty row, refresh lists (`clipList({shelf, limit: 60})` per visible row, `pinned` filter when on), slide-down animation (none with reduce motion). Window `blur` → `shelfHide()`. Keys exactly as spec §5 (arrows move within/between rows, Home/End, 1–9, Shift/Ctrl multi-select, Enter copy+close (`clipCopy(ids)` then `shelfHide()`), Ctrl+Enter `shelfPaste(ids)`, Ctrl+C copy and stay, P pin toggle, Delete trash + undo stack (Ctrl+Z → `clipRestore(lastBatch)`), Space preview (large image / full text / link with title), F pinned-only, Esc closes preview → clears search → hides). Typing a printable key focuses search with that character. Cards: image thumbnail (edited thumb if present), text (first lines), link (title or host + path), pin badge, number badge 1–9 on the first nine cards of the active row. Rows from `settings.shelfRows`, card size from `settings.shelfCardSize`, click behaviour from `settings.shelfClickCopies`. Empty state per row.

- [ ] **Step 1:** Failing tests `logic.test.ts`: `moveSelection(grid, pos, key)` for arrows across rows of different lengths (column clamped), Home/End; `numberPick(row, n)`; `toggleSelect(sel, id, {shift, ctrl}, order)`; `firstSelectable(rows)` skips empty rows.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** Tests + `npm run check` → green.
- [ ] **Step 5: Live check** after Task 9 merged: Ctrl+Alt+V opens the shelf; arrows/Enter copy; Ctrl+Enter pastes into Notepad; Esc closes. Screenshot.
- [ ] **Step 6:** Commit `feat(ui): clipboard shelf`.

---

## Task 12: Integration and release (orchestrator)

- [ ] Merge all branches; resolve shared-file conflicts; full verification: `cargo test --workspace`, clippy, fmt, `npm run check`, `npx vitest run`.
- [ ] Live end-to-end in the dev app (computer-use): caption buttons, sidebar, every settings page, wheel drag/flip/group/click mode/right-click verb, presets editing, watch folder, clipboard capture/OCR/shelf/paste/library/editor, hotkeys.
- [ ] Bump version to 0.2.0 (`Cargo.toml` workspace, `package.json`, `tauri.conf.json`); README.md / README.tr.md feature lists; THIRD_PARTY_NOTICES if needed.
- [ ] `npm run tauri build`; measure installer/exe size and idle RAM; reinstall; delete `Desktop\File Chameleon.lnk` if created; ensure `Desktop\Klasör\ZARLOF\File Chameleon.lnk` targets `%LOCALAPPDATA%\Programs\File Chameleon\file-chameleon.exe`.
- [ ] Final whole-branch review (fresh reviewer), fix wave, commit, push via GitHub Desktop.

## Execution order

- Wave 1 (parallel): Task 1 (main tree), Task 3 (worktree), Task 4 (worktree); Task 2 by the orchestrator.
- Wave 2 (parallel): Task 5 (main tree), Task 6 (worktree), Task 9 (worktree).
- Wave 3 (parallel): Task 7, Task 8, Task 10, Task 11 (main tree; disjoint files).
- Wave 4: Task 12.
