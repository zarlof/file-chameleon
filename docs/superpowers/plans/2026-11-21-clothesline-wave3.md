# Clothesline wave 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ropes and pegs that look like the reference, a 4th "files" rope, removal of the old shelf, a rich İp settings page, and ~550 direct conversions.

**Architecture:**
- Task 6 (fc-core and contracts) runs first.
- Task 7 (Rust shell) and Task 8 (frontend) then run in parallel on disjoint paths: Task 7 owns `src-tauri/**`, Task 8 owns `src/**` and the HTML/vite files.

**Tech Stack:** Tauri 2.12.7, Rust (`fc-core`, `windows` 0.62, `image` 0.25), Svelte 5 runes, TS strict, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-21-clothesline-wave3-design.md`. The earlier specs (`…clothesline-design.md`, `…clothesline-wave2-design.md`) still bind.

## Global Constraints

The wave 1 and wave 2 constraints apply: `docs/superpowers/plans/2026-11-21-clothesline.md` and `…-wave2.md`. Plus:

### Settings

All settings are Rust snake_case with serde camelCase. Defaults and clamps:

| Setting | Default | Allowed values |
|---|---|---|
| `lineRopeStyle` | `"twisted"` | `twisted` \| `cord` \| `thin` |
| `lineSag` | 14 | 0–40 |
| `linePinStyle` | `"plastic"` | `plastic` \| `wood` \| `metal` |
| `linePinColor` | `"auto"` | `"auto"` or `#rrggbb` |
| `lineCardStyle` | `"glassDark"` | `glassDark` \| `glassLight` \| `paper` |
| `lineCardRadius` | 10 | 0–16 |
| `lineCardShadow` | true | |
| `lineShowLabels` | true | |
| `lineShowNumbers` | true | |
| `lineRopeOrder` | `["images","text","links","files"]` | sanitized to exactly these 4 keys, unique, missing keys appended |
| `lineRopeNames` | `{images:"",text:"",links:"",files:""}` | |
| `lineRopes.files` | true | |
| `lineFilesSources` | `{added:true,converted:true,downloaded:true}` | |
| `lineMaxPerRope` | 0 | 0, or 3–30 (values 1–2 become 3) |
| `lineEdge` | `"top"` | `top` \| `bottom` |
| `linePeekMs` | 2500 | 1000–6000 |
| `lineRevealDwellMs` | 350 | 150–1000 |
| `lineHideDelayMs` | 400 | 200–2000 |
| `lineClickAction` | `"copy"` | `copy` \| `copyClose` \| `paste` |
| `lineSwing` | 100 | 0–200 |
| `lineDamping` | 100 | 50–200 |
| `lineBreezeStrength` | 100 | 0–200 |
| `lineFall` | true | |
| `lineSoundVolume` | 60 | 0–100 |
| `lineLookVersion` | 0 | |

### Migrations on load

- When `lineLookVersion < 3`:
  - if `lineColor == "accent"`, set it to `"natural"`;
  - if `hotkeyShelf != ""` and `hotkeyLine == "Ctrl+Alt+L"`, then `hotkeyLine = hotkeyShelf`;
  - in all cases set `hotkeyShelf = ""` and `lineLookVersion = 3`.
- `lineColor` now also accepts `"natural"`, which is the new default.

### Layout

- Layout table as in wave 2. The window spans the full work-area width (side margin 0).
- `window_height(n, size)` holds for n = 1–4.
- `lineEdge = bottom` anchors the window to the bottom of the work area. The reveal zone is the bottom 2 px of the monitor (middle 60%).

### Files store and IPC

- `files.json` in the app local data folder: at most 300 entries, newest first, deduplicated by path (case-insensitive on Windows). Files that no longer exist are pruned on read.
- IPC (names exact):
  - `lineFiles(): Promise<LineFile[]>` → `line_files`
  - `lineFileCopy(paths: string[])` → `line_file_copy { paths }`
  - `lineFileOpen(path: string)` → `line_file_open { path }`
  - `lineFileReveal(path: string)` → `line_file_reveal { path }`
  - `lineFileRemove(paths: string[])` → `line_file_remove { paths }`
  - `lineFileDragOut(path: string)` → `line_file_drag_out { path }`
  - event `files://changed` → `api.events.filesChanged(cb)`
- `LineFile = { path: string; name: string; source: 'added'|'converted'|'downloaded'; at: number; size: number; kind: Kind | null; thumb: string | null }`

### Other

- **New formats:**
  - video `m4v wmv flv mpg 3gp ts ogv`
  - audio `aac wma aiff ac3 amr mka`
  - raster `tga qoi pnm`
  - `ass → txt`

  A format whose ffmpeg encoder or muxer is not in the shipped build is input-only.
- **No input injection** in any test or live check (keyboard or mouse). Live checks use CDP only.

## Review Focus

1. **This user's real settings** (shelf hotkey Ctrl+1, `lineColor` "accent", no files.json): Ctrl+1 opens the line, the rope is natural, and the files rope shows "nothing yet". Task 6 test `wave3_migration`; Task 7 handles a missing `files.json`.
2. **A file on the files rope that has since been deleted:** pruned on read, with no broken card and no crash on click. Task 6 test `prunes_missing`; Task 8 handles an error from `lineFileCopy` with a toast.
3. **Bottom edge with the taskbar at the bottom:** the window sits above the taskbar inside the work area, and the reveal still triggers from the bottom monitor row. Task 7 test `line_rect_bottom_edge`.
4. **All 4 ropes at Large size on a small screen:** the window is clamped to the work area, and the cards are not clipped beyond that clamp. Task 7 test `fit_height_clamps_4_large`.
5. **A new format that ffmpeg cannot encode** (e.g. `amr` without libopencore): it is never offered as a target. Task 6 test `input_only_formats_not_targets`.

---

### Task 6: Core and contracts (settings, migrations, files store, formats)

**Files:**
- Modify:
  - `crates/fc-core/src/settings.rs`
  - `crates/fc-core/src/formats/registry.rs`, `crates/fc-core/src/formats/mod.rs` (extension → kind)
  - the ffmpeg engine's extension/codec mapping in `crates/fc-core/src/engines/` (wherever output args per extension live)
  - the `image` crate *features* in the workspace/fc-core `Cargo.toml` (tga, qoi, pnm)
  - `src/lib/types.ts`, `src/lib/ipc.ts` (types and API entries only)
- Create: `crates/fc-core/src/files.rs` (+ `lib.rs` mod)

**Interfaces (Produces):**
- **Settings** fields per Global Constraints, with `clamped()` and the migration in `load` / `from_value`.
- **Files store** in `fc_core::files`:
  - `pub struct FileEntry { pub path: PathBuf, pub source: FileSource, pub at: u64 }`
  - `pub enum FileSource { Added, Converted, Downloaded }`, serde lowercase
  - `pub struct FileStore { .. }`, with:
    - `FileStore::open(dir: &Path) -> Result<FileStore>`
    - `record(&mut self, paths: &[PathBuf], source: FileSource, now: u64) -> Result<()>`
    - `list(&mut self, sources: &[FileSource]) -> Vec<FileEntry>`, which prunes missing files and saves if anything was pruned
    - `remove(&mut self, paths: &[PathBuf]) -> Result<()>`
  - It is a JSON file, written atomically like other stores.
- **Formats:**
  - New edges per Global Constraints.
  - `registry::edges()` count is asserted in tests. Use the real final number; the report states it.
  - `fc_core::formats::is_input_only(ext)`.
- **TS:**
  - all settings types and defaults;
  - `LineFile`, `FileSource`;
  - the 6 IPC entries;
  - the `filesChanged` event;
  - the `lineColor` doc for `"natural"`.

- [ ] **Step 1: Failing tests**
  - **`wave3_defaults_and_clamps`:** an old JSON without the new fields gets the defaults. Out-of-range values clamp: `lineSag: 99 → 40`, `lineMaxPerRope: 1 → 3`, `linePeekMs: 10 → 1000`. `lineRopeOrder: ["files","files","text"]` becomes `["files","text","images","links"]`.
  - **`wave3_migration`:** `{hotkeyShelf:"Ctrl+1", hotkeyLine:"Ctrl+Alt+L", lineColor:"accent"}` gives `hotkeyLine "Ctrl+1"`, `hotkeyShelf ""`, `lineColor "natural"`, `lineLookVersion 3`. A second load leaves a user-chosen `lineColor "accent"` alone (version is now 3).
  - **`files_store_dedupe_and_cap`:**
    - recording the same path twice keeps one entry with the newer `at`;
    - recording 310 files keeps 300;
    - `C:\A\x.png` and `c:\a\X.PNG` are the same entry.
  - **`prunes_missing`:** an entry whose file was deleted is gone from `list` and from the saved file.
  - **`files_filter_by_source`.**
  - **`new_formats_edges`:**
    - `mp4 → wmv`, `flv → mp3`, `aac → flac`, `tga → png`, `png → qoi` and `ass → txt` exist;
    - `wmv` is in the video kind, `aiff` in audio, `tga` in image.
  - **`input_only_formats_not_targets`:** for each input-only format X, no edge has `to == X`.
- [ ] **Step 2:** Run `cargo test -p fc-core` and expect FAIL.
- [ ] **Step 3:** Implement. For the ffmpeg mapping, follow the existing table that maps an output extension to muxer and codec args. Decide which formats are input-only using the shipped ffmpeg's `-muxers` / `-encoders` if a managed ffmpeg is present under the engines dir; otherwise use ffmpeg's documented default build, and list the reasoning in the report.
- [ ] **Step 4:** Run `cargo test --workspace`, clippy, fmt, `npm run check` and `npx vitest run`, and expect them green. TS callers only need type compatibility; Tasks 7 and 8 use the new APIs.
- [ ] **Step 5:** Commit `feat(core): wave-3 settings and migration, files store, more formats`.

### Task 7: Shell (remove shelf, files rope backend, edge, behaviour settings)

**Files:**
- Delete: `src-tauri/src/shelf.rs`
- Modify:
  - `src-tauri/src/lib.rs`, `src-tauri/src/line.rs`, `src-tauri/src/tray.rs`
  - `src-tauri/src/platform/hotkeys.rs` (drop the shelf hotkey id; keep the numbering stable or remap carefully, with tests)
  - `src-tauri/src/platform/hook.rs` (bottom zone)
  - `src-tauri/src/ipc/*` (remove the shelf commands; add `line_file_*` / `line_files`)
  - the job and download completion paths (record converted and downloaded)
  - the Convert, wheel and verb input paths (record added)
  - `src-tauri/src/state.rs` (files store; `allow_directory` for `files-thumbs`)
  - `src-tauri/src/platform/win.rs` (`CF_HDROP` file copy; reuse drag-out with a file data object)
  - `src-tauri/capabilities/default.json` (remove `shelf`)
  - `src-tauri/windows/hooks.nsh` only if needed

**Interfaces:**
- **Consumes:** the Task 6 settings, files store and contracts.
- **Produces:** the IPC commands exactly as in Global Constraints. `files://changed` is emitted after record and remove. Files thumbnails are 256 px PNGs in `<localdata>/files-thumbs/<hash>.png`, made lazily in `line_files` (25 MP cap, skipped above it), with `thumb` = that path.

- [ ] **Step 1: Failing tests**
  - **`line_rect_bottom_edge`:** work (0,0,1920,1040) at scale 1, 3 ropes, medium gives the window at y = 1040 − 570 = 470, height 570, x 0, width 1920.
  - **`fit_height_clamps_4_large`:** 4 ropes at large is 936 logical. In a 900 px-high work area it is clamped to 900.
  - **`edge_zone_bottom`:** monitor (0,0,1920,1080), point (960,1079) is inside the zone when the edge is bottom.
  - **`files_thumb_name_stable`:** the hash of the same path is the same, and differs for different paths.
  - **`shelf_hotkey_not_registered`:** the wanted hotkeys exclude the shelf.
- [ ] **Step 2:** Run the shell crate tests and expect FAIL.
- [ ] **Step 3:** Implement.
  - Behaviour settings Rust uses: `lineRevealDwellMs` for the dwell, `linePeekMs`, which the page uses but which is also passed in the `LINE_SHOW` payload if convenient, and `lineEdge`. Everything else is page-side.
  - The paste helper moves to `line.rs` (or a small `paste.rs`).
  - The tray item opens the line (`show("hotkey")`).
  - When a job finishes, it records converted outputs; when a download finishes, it records downloaded outputs. `convert` / `queue_jobs` / `wheel` / verb input paths record added inputs.
  - Recording happens off the main thread, and failures are only logged.
- [ ] **Step 4:** Run the full verification and expect green. Commit only `-- src-tauri` (Task 8 is running in parallel). Run `npm run check` but ignore errors in Task 8's files.
- [ ] **Step 5:** Commit `feat(shell): remove shelf, files rope backend, bottom edge, behaviour settings`.

### Task 8: Frontend (look, files rope, settings page, remove shelf UI, formats list)

**Files:**
- Delete: `shelf.html`, `src/shelf.ts`, `src/shelf/` (move anything the line or library still imports first)
- Create: `src/line/Pin.svelte`, `src/line/ropeStyle.ts` (+ test), `src/line/files.ts` (+ test)
- Modify:
  - `vite.config.ts` (drop the shelf input)
  - `src/line/Line.svelte`, `Rope.svelte`, `Peg.svelte`, `layout.ts`, `logic.ts`, `physics.ts` (swing and damping scale)
  - `src/app/views/settings/Line.svelte` (rich page), `Shortcuts.svelte` (remove the shelf row), the clipboard settings page (remove shelf-only options)
  - the tray and other labels in the locales
  - the Convert view and wheel format lists (`OUTPUT_FORMATS` and labels) for the new formats
  - `src/locales/{tr,en}/*.json`

**Interfaces:**
- **Consumes:** Task 6's settings, types and IPC.
- **Produces:**
  - `ropeStyle.ts`:
    - `ropeStrokes(style: 'twisted'|'cord'|'thin', color: string, thickness: number): { stroke: string; width: number; dash?: string; opacity: number }[]`, with 3 layers for twisted (base, dashed twist, highlight), 2 for cord and 1 for thin;
    - `naturalRope(theme: 'dark'|'light'): string`, which gives `#d9d4cc` / `#8a8378`.
  - `files.ts`: `fileCard(f: LineFile): { name: string; ext: string; sizeText: string; sourceIcon: 'added'|'converted'|'downloaded'; isImage: boolean }`, with `sizeText` like "1,2 MB" / "980 KB" in Turkish number format when the language is tr.
  - The rope keys union is `'images'|'text'|'links'|'files'`. `splitRopes` / `visibleRopes` follow `lineRopeOrder` and `lineRopes`, with the files rope fed from `lineFiles()`.

- [ ] **Step 1: Failing tests**
  - **`rope_strokes_layers`:** twisted gives 3 layers with a dash on the 2nd; cord gives 2; thin gives 1 layer of width 1. `naturalRope('dark') === '#d9d4cc'`.
  - **`file_card_model`:**
    - `fileCard({name:'a.PNG', size: 1258291, ...})` gives `ext 'png'`, `isImage true`, `sizeText` "1,2 MB" (tr);
    - a 500-byte file gives "500 B".
  - **`rope_order_and_names`:** order `['files','text','images','links']` with links off gives 3 ropes in that order. A custom name overrides the default label, and an empty name falls back to the localized default.
  - **`layout_four_ropes`:** `windowHeight(4,'medium') === 760`.
- [ ] **Step 2:** Run `npx vitest run src/line` and expect FAIL.
- [ ] **Step 3:** Implement the look per spec §1:
  - no band background;
  - full-width rope;
  - rope styles;
  - realistic `Pin.svelte` (original SVG);
  - card styles with dark glass as default;
  - number badges;
  - label pills;
  - the reference-like link card (image/favicon on top, title, domain).

  Then the files rope per spec §2, and the rich settings page per spec §4 (a live preview with the real components, reset button, rope reorder). Remove the shelf UI and every reference to it. Add the new formats to the Convert/wheel lists.
- [ ] **Step 4:** Run `npx vitest run`, `npm run check` and `npm run build`, and expect green. Commit only `-- src shelf.html vite.config.ts` (deleted files included via `git rm`).
- [ ] **Step 5:** Commit `feat(ui): real rope and pegs, files rope, rich İp settings, shelf removed, new formats`.
