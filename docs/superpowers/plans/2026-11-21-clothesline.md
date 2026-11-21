# Clothesline ("İp") + file actions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clipboard items hang on three swinging ropes at the top of the screen (images, text, links). Add three wheel file actions: copy path, open terminal, float on screen.

**Architecture:**
- A new hidden-until-used transparent window `line` (Rust `src-tauri/src/line.rs`, page `line.html` → `src/line/`) reads the existing clip store over IPC and events.
- Reveal paths: a hotkey (activating), the top-edge hook zone (non-activating), and a peek on a new clip (non-activating).
- File actions are new `Operation` variants that `run_preset` handles directly (not jobs). Float opens `float.html` windows.

**Tech Stack:** Tauri 2.12, Rust (`fc-core`, `windows` 0.62), Svelte 5 runes, TS strict, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-21-file-chameleon-clothesline-design.md` (read the whole spec, especially "Builder-check answers", which overrides earlier lines).

## Global Constraints

- Original code only. Never copy Tendedero or Tangerine code, art or text.
- `crates/fc-core` has no Tauri. Frontend uses Svelte 5 runes only, TS strict, no UI/CSS frameworks, CSS variables from `src/lib/ui/theme.css` only.
- No new crates or npm packages. Extra `windows` crate features are allowed (OLE drag: `Win32_System_Ole`, `Win32_System_Com`, `Win32_System_SystemServices`, `Win32_System_Memory`, `Win32_UI_Shell` as needed).
- No `unwrap()`/`expect()` on I/O or user data outside tests. Commands return `CmdResult<T>`.
- Threading: never build a webview from a sync command or a WebView2 callback on the main thread; use `#[tauri::command(async)]` or spawn.
- LL hook callbacks only do atomics plus a channel send, always call `CallNextHookEx`, and never swallow input for the line.
- No polling loops faster than 1 s. The line's rAF loop runs only while something moves; zero work while hidden or at rest.
- With reduce motion (`data-reduce-motion` on root / `settings.reduceMotion`) there is no animation loop and no breeze.
- UI text only via i18n: `src/locales/{tr,en}/<ns>.json`, same keys, natural Turkish in sentence case. The existing locale-parity test covers all namespaces.
- Peek and edge reveal never take focus (`SW_SHOWNOACTIVATE` plus `WS_EX_NOACTIVATE`). The hotkey activates.
- Do not launch the app and do not write to the system clipboard in tests (the user's installed app is running).
- Verify: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `npm run check`, `npx vitest run`.
- Commits are conventional and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. In a shared tree, commit only your own paths (`git commit -- <paths>`).

## Review Focus

1. Typing in another app while a clip arrives: the peek must not steal focus. Task 2 has the test `peek_never_activates` (pure show-mode → flags).
2. A full-screen game or video plus a copy or the top edge: the line must not appear. Task 2 has the test `fullscreen_guard`.
3. A text-only or links-only history, or ropes turned off: no empty-rope crash, and the window shrinks. Task 3 has the test `splits_and_hides_empty_or_disabled_ropes`.
4. 200 clips with long text, huge images or missing thumbnail files: the line renders the fit count only, cards clamp text, and a missing thumb shows a placeholder. Task 3 has the tests `fit_count_and_overflow`, and `card_text_clamps` for the logic.
5. An old settings file without the line fields: it loads with defaults and the hotkey `Ctrl+Alt+L`. Task 1 has the test `old_settings_get_line_defaults`.

---

### Task 1: Contracts + core (settings, ops, built-ins, nav)

**Files:**
- Modify:
  - `crates/fc-core/src/settings.rs`: new fields and defaults.
  - `crates/fc-core/src/jobs/mod.rs`: `Operation::{CopyPath, OpenTerminal, Float}`, non-queueable like `Annotate`.
  - `crates/fc-core/src/presets/builtin.rs` and `mod.rs`: built-in presets `copy-path`, `open-terminal`, `float`.
  - `src/lib/types.ts`, `src/lib/ipc.ts`.
  - `src-tauri/src/events.rs`: `NavPayload.clip`, line events consts.
  - `src/locales/{tr,en}/recent.json`, `presets.json`: labels.

**Interfaces (Produces):**
- **Settings** (serde camelCase, defaults):

  | Field | Type | Default |
  |---|---|---|
  | `line_enabled` | bool | true |
  | `line_edge_reveal` | bool | true |
  | `line_peek_on_copy` | bool | true |
  | `line_breeze` | bool | true |
  | `line_ropes` | `LineRopes { images: bool, text: bool, links: bool }` | all true |
  | `hotkey_line` | String | `"Ctrl+Alt+L"` |

  TS mirrors: `lineEnabled`, `lineEdgeReveal`, `linePeekOnCopy`, `lineBreeze`, `lineRopes: { images: boolean; text: boolean; links: boolean }`, `hotkeyLine: string`.
- **Operation** (TS): `| { type: 'copyPath' } | { type: 'openTerminal' } | { type: 'float' }`. Rust `Operation::CopyPath | OpenTerminal | Float` (serde tag `type`, camelCase, matching the existing pattern). `queueable` returns `Err(invalid(..))` for all three.
- **Built-in presets** (ids stable, names via `presets.builtin.<id>`):
  - `copy-path` → CopyPath, `open-terminal` → OpenTerminal, `float` → Float.
  - All three apply to any file or folder; `float` applies to raster images only.
  - Add them to group `group:files` and to the `other` profile's slots after `move-to`.
- **NavPayload:** Rust `pub clip: Option<u64>` with `#[serde(default)]`; TS `clip?: number | null`.
- **events.rs consts:**
  - `LINE_SHOW = "line://show"`, with payload `LineShow { mode: String }` where mode is "hotkey" | "peek" | "reveal".
  - `LINE_HIDE = "line://hide"`.
- **ipc.ts** (Rust side done in Task 2):
  - `lineShow(mode: 'hotkey'): Promise<void>` → `line_show { mode }`
  - `lineHide(): Promise<void>` → `line_hide`
  - `lineActivate(): Promise<void>` → `line_activate`
  - `lineDragOut(id: number): Promise<void>` → `line_drag_out { id }`
  - `lineOpenItem(id: number): Promise<void>` → `line_open_item { id }`
  - `linePaste(ids: number[]): Promise<void>` → `line_paste { ids }`
  - `openUrl(url: string): Promise<void>` → `open_url { url }`
  - `floatOpen(paths: string[]): Promise<void>` → `float_open { paths }`
  - Events: `lineShow(cb: (p: { mode: 'hotkey'|'peek'|'reveal' }) => void)`, `lineHide(cb)`.

- [ ] **Step 1: Failing tests (fc-core)**
  - `old_settings_get_line_defaults`: a JSON without line fields → `line_enabled`, `line_edge_reveal`, `line_peek_on_copy`, `line_breeze` are true, `line_ropes` all true, `hotkey_line == "Ctrl+Alt+L"`.
  - `file_actions_are_not_queueable`: all three ops return Err from the queue check.
  - `file_actions_apply_to_files_folders_and_mixed`: `applicable` is true for `copy-path` and `open-terminal` on `[a.jpg, folder, b.docx]`. `float` is true only for a raster image list.
  - `builtin_config_still_validates`: existing validate passes with the new presets.
- [ ] **Step 2:** Run `cargo test -p fc-core`. Expect FAIL.
- [ ] **Step 3:** Implement per the Interfaces above. Update the TS types and ipc, `NavPayload`, and the events consts. Add Recent and preset labels in tr/en:
  - Yolu kopyala / Copy path
  - Terminalde aç / Open in terminal
  - Ekranda sabitle / Float on screen
- [ ] **Step 4:** Run `cargo test --workspace && npx vitest run && npm run check`. Expect PASS. The `Operation`-label test makes the labels compulsory. If any Rust `match` on `Operation` in src-tauri fails to compile, add an arm that returns `Err` / is unreachable via `queueable`. Task 2 fills the real handling.
- [ ] **Step 5:** Commit `feat(core): line settings, file-action ops and contracts`.

### Task 2: Shell — line window, reveal, hotkey, drag-out, file actions, float window (Rust)

**Files:**
- Create: `src-tauri/src/line.rs`, `src-tauri/src/float.rs`.
- Modify:
  - `src-tauri/src/lib.rs` (mods and commands)
  - `src-tauri/src/ipc/mod.rs` or a new `src-tauri/src/ipc/line.rs` (commands)
  - `src-tauri/src/ipc/presets.rs` (run_preset arms)
  - `src-tauri/src/platform/hook.rs` (`LINE_EDGE` zone)
  - `src-tauri/src/platform/hotkeys.rs` (4th hotkey `Line`)
  - `src-tauri/src/platform/win.rs` (no-activate show, fullscreen check, OLE drag)
  - `src-tauri/src/platform/clipboard.rs` or `mod.rs` (peek on new clip)
  - `src-tauri/Cargo.toml` (features)
  - `src-tauri/tauri.conf.json` capabilities if windows `line`/`float-*` need them
  - `src-tauri/capabilities/*.json`

**Interfaces:**
- **Consumes:** Task 1 settings, ops, `NavPayload.clip`, events consts, and the ipc command names and argument names exactly.
- **Produces:** the commands in Task 1's ipc list; the windows `line` (`line.html`) and `float-<n>` (`float.html?path=<urlencoded>`). The pages come from Task 3.

- [ ] **Step 1: Failing pure tests**
  - `line_rect_spans_work_area`: for work (0,0,1920,1040) at scale 1.0 with 3 ropes → (24, 0, 1872, 480). At scale 1.5 with 2 ropes → physical values scaled. Height = ropes × 150 + 30, logical.
  - `edge_zone`: point (960,0) on monitor (0,0,1920,1080) → in. (960,3) → out. (100,0) → out (outside the middle 60%). Second monitor (1920,0,3840,1080), point (2880,0) → in.
  - `fullscreen_guard`: fg rect == monitor rect and class not in [`Progman`,`WorkerW`,`Shell_TrayWnd`] → true. Maximized rect (work area) → false. Desktop class → false.
  - `peek_never_activates`: `show_flags("peek")` and `show_flags("reveal")` → no-activate. `show_flags("hotkey")` → activate.
  - `hotkey_line_parses`: `parse_hotkey("Ctrl+Alt+L")` is Some.
  - `copy_path_text`: `copy_path_text(&[a, b])` → `"C:\\x\\a.jpg\r\nC:\\x\\b"`, with paths joined by CRLF.
  - `terminal_dirs`: a file → its parent; a folder → itself; duplicates removed; capped at 5.
- [ ] **Step 2:** Run `cargo test -p file-chameleon` (src-tauri crate name per Cargo.toml). Expect FAIL.
- [ ] **Step 3:** Implement:
  - `line.rs` follows `shelf.rs`: create on first use, plus `line_rect`, `show(app, mode)`, `hide`, `toggle_hotkey`, prev-foreground memory, `paste` (reuse shelf's paste helper; extract a shared fn if needed without changing behaviour).
  - On show: position on the monitor per spec answer 4, emit `LINE_SHOW` to the `line` window, then show with flags.
  - Peek → after the clip store emits CLIP_ADDED/CLIP_UPDATED (edited), when `line_peek_on_copy && line_enabled` and the guard passes, call `show(app, "peek")`. The page hides itself after 2.5 s via `lineHide` if not hovered.
  - Hook: a `LINE_EDGE` AtomicBool set from settings. On `WM_MOUSEMOVE`, compute the zone with a cached monitor-rects snapshot in atomics/`RwLock` updated on settings and display change (never call monitor APIs inside the hook if avoidable; `MonitorFromPoint` is acceptable). Send `HookEvent::EdgeEnter(x,y)` / `EdgeLeave` on transitions. The action thread arms a 350 ms one-shot (thread sleep in a spawned thread, cancelled by a generation counter) then `show(app,"reveal")` if still in the zone and the guard passes. Start the hook when wheel OR line edge is enabled.
  - `line_activate`: clear `WS_EX_NOACTIVATE`, then `SetForegroundWindow`; remember the previous fg first.
  - `line_drag_out(id)`: load the item from the store. Build an `IDataObject` (image: `CF_HDROP` of `edited ?? image` path; text: `CF_UNICODETEXT`; link: `CF_UNICODETEXT`) and run `DoDragDrop` with `DROPEFFECT_COPY` via `app.run_on_main_thread` (await the result over a channel). Return Err on failure.
  - `line_open_item(id)`: `windows::show_main` with `NavPayload { view: "clipboard", paths: vec![], url: None, clip: Some(id) }`.
  - `open_url(url)`: only `http`/`https`, `ShellExecuteW` "open".
  - `run_preset` arms:
    - CopyPath → write the text to the clipboard (existing clipboard write helper).
    - OpenTerminal → spawn `wt.exe -d <dir>`, falling back to `powershell.exe -NoExit` with `current_dir`; no console flash for wt.
    - Float → `float::open(app, paths)`.
    - All return `Ok(vec![])`.
  - `float.rs`: `open(app, paths)` creates up to 6 windows labelled `float-<n>` (incrementing counter), frameless, always on top, skip taskbar, transparent false, sized to the image aspect fit in 40% of the work area (read dimensions with `image::ImageReader::into_dimensions`), URL `float.html?path=<urlencoded>`, built off the main thread. `float_open` command is async.
  - Hotkeys: add `Line` as id 4 with `hotkey_line`; the action calls `line::toggle_hotkey`. `status()` includes it (name `line`).
  - Settings change (`apply_settings`): update `LINE_EDGE`, hotkeys, and hide plus destroy the line window if `line_enabled` turns false.
- [ ] **Step 4:** Run the full verification list. Expect all green. Pages may not exist yet in this tree; `WebviewUrl::App` paths are strings, so they compile.
- [ ] **Step 5:** Commit `feat(shell): clothesline window, edge reveal, peek, drag-out, file actions, float`.

### Task 3: Frontend — line page, float page, settings page

**Files:**
- Create:
  - `line.html`, `src/line.ts`
  - `src/line/Line.svelte`, `Rope.svelte`, `Peg.svelte`, `physics.ts`, `physics.test.ts`, `logic.ts`, `logic.test.ts`, `i18n.ts`
  - `float.html`, `src/float.ts`, `src/float/Float.svelte`
  - `src/locales/{tr,en}/line.json`
  - `src/app/views/settings/Line.svelte`
- Modify:
  - `vite.config.ts` (inputs `line`, `float`)
  - Sidebar / settings nav: an "İp" page after the clipboard settings
  - `src/app/views/settings/Shortcuts.svelte`: the Line hotkey row
  - The clipboard library view: handle `nav.clip`, select and open the editor or text edit
  - `src/locales/{tr,en}/settings.json` (or the namespace the sidebar uses)

**Interfaces:**
- **Consumes:** Task 1 types and ipc (exact names above), `api.clipList`, `clipCopy`, `clipTrash`, `clipRestore`, `clipPin`, `clipSaveImage`, `api.events.clipAdded/clipUpdated/clipChanged`, `lineShow/lineHide` events, and the asset protocol (`convertFileSrc`) as used by the shelf.
- **Produces** (`logic.ts`):
  - `splitRopes(items: ClipItem[], ropes: {images:boolean;text:boolean;links:boolean}): { key:'images'|'text'|'links'; items: ClipItem[] }[]`
    - Only enabled ropes, in order images → text → links.
    - `link` and `mediaLink` go to links.
    - Each rope is sorted pinned first (newest first), then unpinned newest first.
    - Trashed items are excluded.
  - `fitCount(widthPx: number, card = 112, gap = 16, margin = 48): number`
  - `overflow(total: number, fit: number): number`
  - `cardText(item: ClipItem, maxLines = 3): { title: string; lines: string[] }`
- **Produces** (`physics.ts`):
  - `type Pend = { theta: number; omega: number }`
  - `step(p: Pend, dt: number, k?: {w: number; c: number}): Pend`, with defaults w=4.2, c=2.2, clamping |θ| ≤ 0.5
  - `impulse(p: Pend, dOmega: number): Pend`
  - `atRest(p: Pend, eps = 1e-3): boolean`
  - `ropePath(width: number, pins: number[], sag = 18, dip = 4): string` (SVG `d`)
  - `fall(t: number): number` (0→1 position with one small bounce, for 0 ≤ t ≤ 1)

- [ ] **Step 1: Failing tests**
  - `physics.test.ts`:
    - `settles_to_rest`: θ0=0.25 stepped at 1/60 for 3 s → `atRest`.
    - `damped_monotone_envelope`: the peak |θ| per half-swing decreases.
    - `impulse_clamps`: a huge impulse keeps |θ| ≤ 0.5 after a step.
    - `ropePath_has_sag`: the path's mid y is ≥ 18 for an empty pin list.
    - `fall_ends_at_1`: `fall(1) === 1` and `fall(0) === 0`.
  - `logic.test.ts`:
    - `splits_and_hides_empty_or_disabled_ropes`: mixed items, links off → 2 ropes; only text items with all ropes on → 3 ropes with images empty (the page decides to hide empty ropes visually: test that `splitRopes` keeps empty enabled ropes, and that a `visibleRopes` helper drops empty ones).
    - `pinned_first_newest_first`.
    - `mediaLink_goes_to_links`.
    - `fit_count_and_overflow`: width 1872 → `fitCount` = floor((1872−48+16)/(112+16)) = 14, and overflow(20,14) = 6.
    - `card_text_clamps`: a 10-line text with no title → the title is line 1, and 3 lines remain, the last ending with "…" when more lines exist.
- [ ] **Step 2:** Run `npx vitest run src/line`. Expect FAIL.
- [ ] **Step 3:** Implement the page:
  - `Line.svelte`:
    - Loads settings and items. On `lineShow` it refreshes when dirty and plays the reveal; while hidden, events only mark dirty.
    - On mouseleave it hides after 400 ms; on peek it hides after 2.5 s unless the pointer entered.
    - Esc hides. Del trashes the selection. Ctrl+Z restores the last trashed batch.
    - A pointerdown inside the window while not active calls `lineActivate()` first.
  - `Rope.svelte` draws the SVG rope (`ropePath`) and its pegs. The rAF loop is shared in `Line.svelte` and runs only while some pendulum or slide is not at rest. Breeze uses a `setTimeout` of 3–6 s while visible and `lineBreeze` is on (not reduce motion).
  - `Peg.svelte` handles the card kinds per spec plus our own clothespin SVG:
    - Click copies, plus a hop and a "Kopyalandı" tick.
    - Ctrl/Shift-click multi-selects.
    - Double-click: image/text → `lineOpenItem(id)`, then hide. Link → `openUrl`.
    - Drag (6 px) → `lineDragOut(id)`; on error, a small toast.
    - × on hover → trash.
    - Right-click opens our menu: Kopyala, Yapıştır (`linePaste`), Düzenle, Sabitle/Sabitlemeyi kaldır, Ekranda sabitle (image → `floatOpen([edited ?? image])`), Dosya olarak kaydet (`clipSaveImage(id,'png')`), Sil.
    - "+N" → `api.showShelf()`.
    - A "Duraklatıldı" label shows when the clipboard is paused.
  - `Float.svelte`:
    - Shows `?path` via the asset protocol, fit to the window.
    - Drag to move (`data-tauri-drag-region` / `startDragging`). Ctrl+wheel zooms 0.25–4×.
    - × on hover, and Esc, close the window (`getCurrentWindow().close()`).
  - Settings `Line.svelte`:
    - Toggles: enabled, edge reveal, peek on copy, breeze, and the three ropes.
    - A hotkey recorder (reuse the Shortcuts component), with conflict status from the existing hotkey status IPC.
    - A "Şimdi göster" button → `lineShow('hotkey')`.
    - A hint when `clipboardEnabled` is false.
  - Shortcuts page: add the Line row.
  - Clipboard library: on nav with `clip`, select that item and open its image editor or text edit (smallest change to the existing view).
  - All texts in `line.json` (tr/en) plus the existing namespaces for settings and sidebar labels.
- [ ] **Step 4:** Run `npx vitest run && npm run check`. Expect PASS. Then `npm run build` (vite) produces `dist/line.html` and `dist/float.html`.
- [ ] **Step 5:** Commit `feat(ui): clothesline page, float window, line settings`.

### Task 4: Integration check (controller)

- [ ] Full verification list on the merged tree. Run `npm run tauri build`, install silently, fix the shortcut per memory.
- [ ] Smoke check: the process runs and settings keep the old values plus the line defaults.
- [ ] Hand the user the manual checklist from the spec's Live section.
