# Clothesline wave 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add rope look settings (colour, thickness, opacity, size) with symmetric, live re-layout. Add the remaining Tendedero and Clipboardline extras: line keyboard, search, preview, zoom, markdown, "İpe as" verb, capture hotkey, copy-latest-image hotkey, YouTube thumbnails, hang sound.

**Architecture:** Task 4 (Rust and contracts) adds the settings, hotkeys, the `search` show mode, hang import, capture and thumbnails, plus the band-height table on the Rust side. Task 5 (frontend) adds the layout module, the look and settings UI, and the page features. Task 4 runs first, then Task 5.

**Tech Stack:** Tauri 2.12, Rust (`fc-core`, `windows` 0.62), Svelte 5 runes, TS strict, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-21-clothesline-wave2-design.md`. The base spec is `docs/superpowers/specs/2026-11-21-file-chameleon-clothesline-design.md`.

## Global Constraints

The constraints in `docs/superpowers/plans/2026-11-21-clothesline.md` all apply, and so do these:

- **Layout table (the same numbers in TS `src/line/layout.ts` and Rust `src-tauri/src/line.rs`, logical px):**

  | size | scale | card width | band height |
  |---|---|---|---|
  | small | 0.8 | 90 | 144 |
  | medium | 1.0 | 112 | 178 |
  | large | 1.25 | 140 | 222 |

  - Gap between bands: 12.
  - Top and bottom margin: 6 each.
  - Window height = `n × (band + 12)` for n visible ropes (n ≥ 1).
- New settings and their defaults:
  - `lineColor` = `"accent"`; `lineThickness` = 2, clamped 1–6; `lineOpacity` = 100, clamped 30–100; `lineSize` = `"medium"`.
  - `lineZoomOnHover` = false; `lineSound` = false.
  - `hotkeyLineSearch` = `"Ctrl+Alt+F"`; `hotkeyCapture` = `""`; `hotkeyCopyLatestImage` = `""`.
- Show modes are `hotkey | peek | reveal | search`. `search` activates, like `hotkey`.
- Markdown rendering never passes raw HTML through. Escape first, then apply our own small subset.

## Review Focus

1. **A size change while the line is shown:** the window and the bands resize together, with no clipped cards and no dead area. Covered by Task 4 `band_table_matches` and Task 5 `layout_table_and_symmetry`.
2. **Opacity at 30%:** cards stay readable, because cards are kept at ≥ 90% opacity. Covered by Task 5 `card_opacity_floor`.
3. **"İpe as" on a 2 GB file, a binary `.txt`, or a folder:** rejected with no crash and no import. Covered by Task 4 `hang_import_rejects`.
4. **Typing in the search field:** Del, P, E, digits and Space must type text, not trigger card commands. Covered by Task 5 `keys_in_search_are_text`.
5. **A markdown text containing `<script>` or `<img onerror>`:** shown as text. Covered by Task 5 `markdown_escapes_html`.

---

### Task 4: Contracts and shell (settings, hotkeys, search mode, hang verb, capture, thumbnails)

**Files:**
- Modify:
  - `crates/fc-core/src/settings.rs`
  - `crates/fc-core/src/clip/enrich.rs` (YouTube id and thumbnail)
  - `crates/fc-core/src/clip/history.rs`, if a helper is needed to set `thumb` on a link item
  - `src/lib/types.ts`, `src/lib/ipc.ts`
  - `src-tauri/src/line.rs` (layout table, `search` mode)
  - `src-tauri/src/platform/hotkeys.rs` (ids 5–7)
  - `src-tauri/src/platform/contextmenu.rs` (second verb "İpe as" / "Hang on the line", `--hang`)
  - `src-tauri/src/lib.rs` (single-instance and startup args `--hang`)
  - `src-tauri/src/platform/win.rs` (monitor capture)
  - `src-tauri/src/ipc/clipboard.rs` or a new `src-tauri/src/hang.rs` (import)
  - `src-tauri/Cargo.toml` (GDI features only)
  - `src/locales/{tr,en}/` (only the namespaces holding verb or hotkey labels if Rust reads them; otherwise none)

**Interfaces (Produces):**
- **Settings.**
  - Rust: `line_color: String`, `line_thickness: u8`, `line_opacity: u8`, `line_size: String` (`"small" | "medium" | "large"`; anything else is treated as medium), `line_zoom_on_hover: bool`, `line_sound: bool`, `hotkey_line_search: String`, `hotkey_capture: String`, `hotkey_copy_latest_image: String`. Defaults as in Global Constraints. Clamp on load.
  - TS mirrors: `lineColor`, `lineThickness`, `lineOpacity`, `lineSize: 'small'|'medium'|'large'`, `lineZoomOnHover`, `lineSound`, `hotkeyLineSearch`, `hotkeyCapture`, `hotkeyCopyLatestImage`.
- **TS line show mode type:** `'hotkey'|'peek'|'reveal'|'search'`. `lineShow(mode: 'hotkey'|'search')`. The `line_state` mode includes `'search'`.
- **`HotkeyStatus.name`** adds `'lineSearch' | 'capture' | 'copyLatestImage'`. The Rust names are the same strings, with ids 5, 6 and 7.
- **Rust functions:**
  - `line::band_height(size: &str) -> f64` and `line::window_height(ropes: usize, size: &str) -> f64`, both logical.
  - `hang::paths_from_args(args: &[String]) -> Vec<String>`, which returns the args after `--hang`.
  - `hang::classify(path: &Path, len: u64, head: &[u8]) -> HangKind`, where `enum HangKind { Image, Text, Reject }`.
- **`enrich`:** `youtube_id(url: &str) -> Option<String>` handles watch?v=, youtu.be/ and shorts/, and returns an 11-character id.

- [ ] **Step 1: Failing tests**
  - **`band_table_matches`:** `band_height("small")` = 144, `"medium"` = 178, `"large"` = 222, `"weird"` = 178. `window_height(3, "medium")` = 570. `window_height(1, "small")` = 156.
  - **`old_settings_get_wave2_defaults`:** a JSON without the new fields loads with the defaults above. `lineThickness: 99` loads as 6, and `lineOpacity: 5` loads as 30.
  - **`hang_args`:**
    - `["exe", "--hang", "C:\\a.png"]` gives `["C:\\a.png"]`.
    - With no switch, the result is empty.
  - **`hang_import_rejects`:**
    - A `.txt` larger than 1 MB is Reject.
    - A `.txt` with invalid UTF-8 is Reject.
    - A `.png` larger than 50 MB is Reject.
    - A `.exe` is Reject.
    - A `.md` under 1 MB with valid UTF-8 is Text.
    - A `.jpg` under 50 MB is Image.
  - **`youtube_ids`:**
    - `https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1` gives `dQw4w9WgXcQ`.
    - `https://youtu.be/dQw4w9WgXcQ` gives the same id.
    - `https://youtube.com/shorts/dQw4w9WgXcQ` gives the same id.
    - `https://example.com/?v=dQw4w9WgXcQ` gives None.
  - **`wave2_hotkeys_parse`:** `parse_hotkey("Ctrl+Alt+F")` is Some.
- [ ] **Step 2:** Run `cargo test --workspace` and expect FAIL.
- [ ] **Step 3: Implement.**
  - **Line window size:** `line.rs` sizes the window from `window_height(visible_ropes, line_size)` using the existing visible-rope count. `apply_settings` resizes the window if it is shown.
  - **`search` mode:** behaves like `hotkey`: it activates and remembers the previous foreground window. Its hotkey toggles it.
  - **"İpe as" verb:**
    - Registered under `HKCU\Software\Classes\*\shell\FileChameleonHang`, with `MUIVerb` set to the localized label and command `"<exe>" --hang "%1"`. It is shown for all files; the import rejects others.
    - It is installed and removed together with the existing verb, using the same setting.
  - **Hang import, on first start and in single-instance:** for each `--hang` path, classify it and add it through the store's `add_image_png` (decode then re-encode as PNG, using the existing helpers) or `add_text`. Emit `CLIP_ADDED`, then peek. Each file runs on a worker thread.
  - **Capture:** a GDI `BitBlt` of the monitor under the cursor into a PNG, added as an image item with `sourceApp` = "File Chameleon", then peek. It runs on a worker thread.
  - **Copy latest image:** writes the newest non-trashed image (edited copy if present) to the clipboard with the existing clipboard image writer.
  - **YouTube thumbnail:** done in the existing link-title enrich path (only when link titles are on). For a `mediaLink` item with a YouTube id, GET `https://i.ytimg.com/vi/<id>/mqdefault.jpg` with the existing HTTP client, a timeout, and a 200 KB cap. Save it as that item's thumb file and emit `CLIP_UPDATED`.
- [ ] **Step 4:** Run the full verification list: cargo test, clippy, fmt, `npm run check`, vitest. Expect green. Frontend callers that need updating for the widened TS types are in scope only for type compatibility.
- [ ] **Step 5:** Commit `feat(line): wave-2 contracts, size-aware window, search mode, hang verb, capture, thumbnails`.

### Task 5: Frontend (layout, look settings, keyboard, search, preview, zoom, markdown, sound)

**Files:**
- Create:
  - `src/line/layout.ts` and `layout.test.ts`
  - `src/line/markdown.ts` and `markdown.test.ts`
  - `src/line/keys.ts` and `keys.test.ts`
  - `src/line/Preview.svelte`, `src/line/sound.ts`
- Modify:
  - `src/line/Line.svelte`, `Rope.svelte`, `Peg.svelte`, `logic.ts` (search match plus tests)
  - `src/app/views/settings/Line.svelte`
  - `src/app/views/settings/Shortcuts.svelte` (3 new rows)
  - `src/locales/{tr,en}/line.json`, plus the settings and shortcuts namespaces

**Interfaces:**
- **Consumes:** Task 4's settings, mode type, hotkey names and `lineShow('search')`, plus the existing APIs (`clipCopyOcr`, `linePaste`, `lineOpenItem`, `clipPin`, `clipTrash`, `clipRestore`).
- **Produces:**
  - `layout.ts`:
    - `SIZES = { small: { scale: 0.8, card: 90, band: 144 }, medium: { scale: 1, card: 112, band: 178 }, large: { scale: 1.25, card: 140, band: 222 } }`
    - `GAP = 12`, `MARGIN = 6`
    - `bandTops(n: number, size): number[]` returns `MARGIN + i × (band + GAP)`.
    - `windowHeight(n, size) = n × (band + GAP)`
    - `ropeColor(setting: string, accent: string): string`
  - `markdown.ts`: `renderMd(src: string): string` returns HTML-safe markup for headings 1–3, `**b**`, `*i*`, `` `code` ``, `-`/`*` lists and line breaks. Everything is escaped first.
  - `keys.ts`: `nextFocus(ropes: number[] /*counts*/, cur: {r:number;i:number}|null, key: 'ArrowLeft'|'ArrowRight'|'ArrowUp'|'ArrowDown'|'Home'|'End'): {r:number;i:number}|null`. Up and Down keep the index clamped. Empty ropes are skipped.
  - `logic.ts`: `matches(item: ClipItem, query: string): boolean`. It is case- and diacritic-insensitive (`İ/ı/ş/ğ/ü/ö/ç` folded) over `text`, `title`, `note`, `tags`, `ocr` and `linkTitle`.

- [ ] **Step 1: Failing tests**
  - **`layout_table_and_symmetry`:**
    - `bandTops(3,'medium')` is `[6, 196, 386]`.
    - `windowHeight(3,'medium')` is 570.
    - `windowHeight(1,'small')` is 156.
    - Consecutive tops differ by `band + 12` for every size.
  - **`card_opacity_floor`:** the exported `cardOpacity(lineOpacity)` is `max(0.9, lineOpacity/100)`.
  - **`markdown_escapes_html`:**
    - `renderMd('<script>x</script>')` contains `&lt;script&gt;` and no `<script`.
    - `renderMd('<img src=x onerror=1>')` contains no `<img`.
    - `renderMd('# Başlık\n**kalın** ve *eğik*\n- a\n- b')` contains `<h1>`, `<strong>`, `<em>`, `<ul>` and two `<li>`.
  - **`keys_focus`:**
    - With rope counts `[3,0,2]`, ArrowDown from `{r:0,i:2}` gives `{r:2,i:1}`.
    - ArrowRight at the end of a rope stays put.
    - Home gives `i:0`.
  - **`keys_in_search_are_text`:** the exported `isCommandKey(e, searchFocused)` is false for `Delete`, `p`, `e`, `1` and `' '` when `searchFocused` is true, and true when it is false (with the line active).
  - **`search_matches`:**
    - `matches(item{text:'Şirket raporu'}, 'sirket')` is true.
    - `matches(item{ocr:'INVOICE 42'}, 'invoice')` is true.
    - `matches(item{tags:['iş']}, 'is')` is true.
    - An empty query is true.
- [ ] **Step 2:** Run `npx vitest run src/line` and expect FAIL.
- [ ] **Step 3: Implement.**
  - **Rope look:**
    - `Rope.svelte` uses the band height and card width from the size. The stroke is `ropeColor`, with width `lineThickness`.
    - The rope band and backdrop opacity is `lineOpacity/100`, and cards use `cardOpacity`.
    - Bands are absolutely positioned at `bandTops`, so spacing is exactly symmetric.
    - Cards scale with `scale`, and so do the clothespins and fonts. On a size or setting change, slots recompute and cards ease to them.
    - Settings changes arrive through the existing settings event and apply live.
  - **Keyboard per spec §3.1:** commands only fire when `isCommandKey` is true. Esc clears the selection, then the search, then hides.
  - **Search mode (spec §3.2):**
    - The field sits top-right inside the strip, focused on `search` show.
    - A printable key while active (and not in the field) starts a search.
    - Ropes filter live with `matches`; Enter copies the newest match.
  - **Preview (Space):** `Preview.svelte` is an overlay inside the window, with an image fitted to the height, Markdown text via `renderMd` (`{@html}` only with `renderMd` output) and link details. Left/Right move between items; Space or Esc closes.
  - **Zoom on hover:** when the setting is on, a 400 ms hover over an image card shows a 2× enlarged image below the card, clamped inside the window.
  - **Text cards** render `renderMd` for the first lines. Link cards show `thumb` when present (YouTube).
  - **Hang sound:** `sound.ts` `tick()` synthesizes a short soft click with WebAudio, played on card landing when `lineSound` is on (and not under reduce motion).
  - **Settings page "İp":**
    - The "Görünüm / Look" group has 8 swatches plus a custom colour (`<input type=color>`), a thickness slider 1–6, an opacity slider 30–100, and a size segmented control. A live preview strip shows one rope with 3 sample cards using `Rope.svelte`/`Peg.svelte`, or a static copy if importing across entries is not allowed.
    - Also: toggles for zoom on hover and sound.
    - Hotkey rows for search, capture (with the hint "Öneri: Ctrl+Alt+A") and copy latest image.
  - **Shortcuts page:** the 3 new rows with conflict status.
  - **i18n:** all strings in tr and en.
- [ ] **Step 4:** Run `npx vitest run`, `npm run check` and `npm run build`, and expect all PASS.
- [ ] **Step 5:** Commit `feat(line): look settings, symmetric layout, keyboard, search, preview, zoom, markdown, sound`.
