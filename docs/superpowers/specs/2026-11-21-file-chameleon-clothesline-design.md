# File Chameleon — Clothesline ("İp") + file actions — design

Date: 2026-11-21. Builds on v2 (0.2.0, `main` f07f632). Spec for v2: `2026-11-20-file-chameleon-v2-design.md` — its Global Constraints still bind (original code only, fc-core has no Tauri, Svelte 5 runes, i18n tr/en, threading rule, RAM rules, LL hook rules).

## Intent

The user saw Tendedero (macOS, MIT) and wants its "clothesline" mechanic in File Chameleon. Clipboard items hang on ropes, with clothespins, at the top of the screen. They hang there when something is copied or edited. They swing gently and can be clicked, dragged out and deleted.

The user also asked which features of Tangerine (a macOS file-converter wheel) are worth adding. They delegated the choices ("sen karar ver"). Decisions marked *call* are the controller's and can be corrected.

Why: the user wants the app to feel alive and playful like those apps, and to be faster to reach than the Ctrl+Alt+V shelf. Clips should be visible at a glance.

## Goals

1. **Three ropes, stacked top to bottom** (*call*):
   - Rope 1, **Resimler / Images**: `image` items.
   - Rope 2, **Metinler / Text**: `text` items.
   - Rope 3, **Linkler / Links**: `link` and `mediaLink` items.
2. **New item behaviour:** a new or edited clipboard item drops onto its rope with a small bounce and swings. When it arrives while the line is hidden and "Peek on copy" is on, the line peeks for ~2.5 s. A peek never takes focus.
3. **Opening the line:** a global hotkey, or by resting the pointer on the top edge of the screen.
4. **Closing the line:** it hides when the pointer leaves, on Esc, or on a click elsewhere.
5. **Interactions:**
   - Click: copy the item.
   - Double-click: open or edit it.
   - Drag out to another app or Explorer: drop it there. The item stays on the rope.
   - × on hover, or Del: delete (to the trash).
   - Right-click menu.
   - Ctrl+Z: undo the last delete.
   - Pinned items stay at the left end of their rope.
6. **Physics:**
   - The rope sags.
   - Each card hangs from a clothespin and swings like a damped pendulum.
   - Neighbours get a nudge when an item arrives or is removed.
   - An optional light breeze.
   - Everything is instant with reduce-motion on.
7. **Settings page** "İp / Line" in the sidebar, next to the clipboard pages.
8. **Tangerine file actions** added as wheel presets:
   - **Yolu kopyala / Copy path**
   - **Terminalde aç / Open in terminal**
   - **Ekranda sabitle / Float on screen**

## Non-goals

- Replacing the Ctrl+Alt+V shelf. The shelf stays the place for search, folders, tags and bulk work. The rope is the quick, glanceable view.
- Search on the rope (the shelf has it), screenshot takeover, screen recordings, AirDrop or Share sheet.
- Tangerine's keyboard-driven wheel and per-modifier wheels. These are possible later; v2 already has Ctrl+Shift back face.
- New data storage. The rope only shows the existing clip store.

## Anti-goals (failure even if it works)

- The line steals focus or keystrokes from the app the user is typing in (peek, reveal).
- It uses CPU or RAM while hidden or idle: no animation loop while nothing moves, and no polling faster than 1 s.
- It pops up during games or full-screen video.
- It covers the screen in a way that blocks clicks after it should be gone.
- It shows items the store marked private or ignored.
- It looks like Tendedero: its code, art and text are not copied. Our clothespin, cards and rope are drawn by us.

## Approaches considered

- **A. New separate "line" window (picked):** a transparent, always-on-top strip at the top of the monitor, its own Vite entry `line.html`, created on first use and then only hidden and shown (same pattern as `shelf.rs`). It reads the clip store through the existing IPC and events.
  - Pros: it is independent of the shelf and keeps the shelf intact. Reveal and peek can be tuned without touching shelf code.
  - Cons: one more webview, about 10–20 MB while alive. It is created only on first use, and only if the line is enabled.
- **B. Turn the shelf into ropes:** loses the shelf's search, grid and folders, and the user did not ask to remove them.
- **C. A "rope mode" toggle inside the shelf window:** keeps one webview, but the shelf window's geometry (top-centre panel, activates on open, paste-into-previous) conflicts with a non-activating, edge-revealed strip. Two behaviours would be tangled in one 540-line component.

## Design

### Window (`src-tauri/src/line.rs`, label `line`)

- Built with `WebviewWindowBuilder` like the shelf: no decorations, transparent, always on top, skip taskbar, no shadow, created hidden on first use, same data dir and browser args.
- It is never created on the main thread inside a WebView2 callback (threading rule).
- **Geometry.** Full width of the work area of the monitor under the cursor, minus 24 logical px each side. Top at the work-area top. Height = 3 ropes × 150 logical + 30. A pure `line_rect(work, scale)` function gets a unit test, like `shelf_rect`.
- **Show without activation.** Use `ShowWindow(SW_SHOWNOACTIVATE)` and `WS_EX_NOACTIVATE` through the existing `windows` crate, so peeks and edge reveals never take focus.
  - Clicks on cards still work. A no-activate window gets mouse input.
  - Keyboard (Esc, Del, Ctrl+Z) works after the user clicks inside the line. Then the window activates on purpose: on a click, turn `WS_EX_NOACTIVATE` off.
- **Transparent gaps between cards** should let clicks through to the app below where nothing is drawn.
  - *Call:* not required for v1. The strip is short-lived and hides when the pointer leaves.
  - If cheap, the builder may use `set_ignore_cursor_events` toggled from the page by hit-testing.
- **Hiding:**
  - The pointer leaves the window rect for 400 ms (tracked by the page with `mouseleave` plus a timer).
  - Esc, or a click outside (focus lost, after activation).
  - The hotkey again (toggle).
  - The peek timer ends (2.5 s) while the pointer has not entered.
- **Full-screen guard.** Reveal and peek are suppressed when the foreground window covers its whole monitor and is not the desktop or shell.

### Reveal by top edge

- Uses the existing LL mouse hook (`platform/hook.rs`). On `WM_MOUSEMOVE` the hook only does atomics and a channel send, per the LL hook rules.
- The trigger: y is within 2 physical px of the monitor's top, x is within the middle 60% of that monitor, and the line is enabled with edge reveal on. On entering that zone the hook sends `EdgeEnter` once. Leaving the zone sends `EdgeLeave`.
- The action thread arms a 350 ms one-shot timer on `EdgeEnter`. If no `EdgeLeave` arrived, it shows the line. No polling loop.
- Remember that the hook is only installed while the wheel or any hook user is enabled. It must also install when the line's edge reveal is on.

### Hotkey

- New setting `hotkeyLine`, default `"Ctrl+Alt+L"`.
- Registered and reported through the existing hotkeys module (conflict → `ok:false`, shown on the Shortcuts page), and listed on the Shortcuts page.

### Page (`src/line/`: `Line.svelte`, `Rope.svelte`, `Peg.svelte` card, `physics.ts`, `logic.ts`)

- **Data.**
  - On open, and on `clip://added` / `clip://updated` / `clip://changed`, call `clipList` with `{ trashed: false }`, newest first.
  - Split by kind into the three ropes. Ropes turned off in settings are not drawn and the window height shrinks.
  - While hidden, mark dirty only and refresh on show (same lesson as the shelf).
- **Order on a rope.** Pinned items come first, from the left knot, newest pinned first. Then unpinned, newest first. As many as fit (card 112 logical wide + 16 gap). Overflow shows a "+N" tag at the right end; clicking it opens the shelf.
- **Cards** (original drawings):
  - **Image:** a photo print with a white border, using `editedThumb ?? thumb` (asset protocol). A small badge shows when edited.
  - **Text:** a note sheet. The title is `title` or the first line, plus up to 3 lines and an ellipsis.
  - **Link:** a tag-like card with `linkTitle ?? text`, the domain from `site`, and a play glyph for `mediaLink`.
- **Clothespin.** A small SVG on top of each card, clamped on the rope. Pinned items get the app accent colour on their clothespin.
- **Rope.** An SVG path between two knots at the window edges. Base sag is 18 logical px at the middle, plus a local dip of 4 px under each hanging card. Recomputed when items change and eased so it does not jump.
- **Physics (`physics.ts`, pure, unit-tested).** Each card is a damped pendulum hanging from its pin point:
  - θ'' = −(g/L)·sinθ − c·θ' + impulse, with angle θ (radians).
  - Constants (*call*, tunable): ω ≈ 4.2 rad/s, damping c ≈ 2.2/s, |θ| clamped to 0.5.
  - Integrate with semi-implicit Euler at a fixed 1/60 s step, accumulated from rAF time.
  - **Arrival:** the card falls from −60 px with a spring to rest (one small bounce), then θ₀ = ±0.25. Neighbours within two slots get an impulse of ±0.08 that falls off with distance.
  - **Removal:** neighbours slide to new slots (eased x) and get a ±0.06 nudge.
  - **Hover:** an impulse of 0.05 in the pointer's direction.
  - **Reveal:** the whole line slides down 120 ms with a stagger of θ₀ = 0.1.
  - **Breeze** (setting, default on): while visible, every 3–6 s (random) a small impulse of ≤0.03 hits a random 1–3 cards.
  - The rAF loop runs only while some card has |θ| or |θ'| above 1e-3, or a slide or fall is in progress. Otherwise the loop stops: zero CPU when idle.
  - With reduce motion: no loop at all; cards are placed at rest instantly, and there is no breeze.
- **Interactions:**
  - **Click** → `clipCopy([id])`, plus a small "hop" impulse and a "Kopyalandı" tick on the card.
  - **Ctrl-click / Shift-click** → multi-select. Click copies all selected; Esc clears.
  - **Double-click:**
    - Image → open the main window's clipboard library on that item in its image editor.
    - Text → the library item in text-edit mode.
    - Link → open in the default browser.
    - Reuse existing nav/IPC. If the library cannot be deep-linked to an item yet, add a nav target `{ view: 'clipboard', focus: id, edit: true }`.
  - **Drag out** (to Explorer, chat apps, editors) through a new command `line_drag_out(id)`. It starts a native OLE `DoDragDrop` from Rust, on the window's thread, with:
    - Image: a file (`CF_HDROP` of `edited ?? image`), plus a bitmap/PNG.
    - Text: `CF_UNICODETEXT`.
    - Link: `CF_UNICODETEXT` plus a URL format.

    The drag is copy-only and the item stays on the rope. This needs extra `windows` crate features (`Win32_System_Ole`, `Win32_System_Com`, `Win32_System_SystemServices` as needed), which are allowed as feature flags only.
  - **× on hover / Del** → `clipTrash`, and Ctrl+Z → `clipRestore` of the last trashed batch (one undo level).
  - **Right-click menu** (in-page, our own):
    - Kopyala
    - Yapıştır: reuse `shelfPaste` semantics, pasting into the window that was in front before the line was activated. The line must remember the previous foreground like the shelf does.
    - Düzenle
    - Sabitle / Sabitlemeyi kaldır
    - Ekranda sabitle (Float, images only)
    - Dosya olarak kaydet (images, `clipSaveImage`)
    - Sil
- **Privacy:** only what the store returns. Paused clipboard → the ropes still show existing items, and a small "Duraklatıldı" label shows at the right of the top rope.

### Settings (fc-core `settings.rs`, new fields with defaults; `src/lib/types.ts`)

| Field | Default |
|---|---|
| `lineEnabled` | `true` |
| `lineEdgeReveal` | `true` |
| `linePeekOnCopy` | `true` |
| `lineBreeze` | `true` |
| `lineRopes` | `{ images: true, text: true, links: true }` |
| `hotkeyLine` | `"Ctrl+Alt+L"` |

- Old settings files load with these defaults (serde defaults; add a v2-file test).
- **Settings page** "İp / Line": toggles for each of the above, the hotkey (recorder, as on the Shortcuts page), and a "Şimdi göster" button.
- If `clipboardEnabled` is off, show a hint that the line needs the clipboard.

### Tangerine file actions (wheel presets)

New preset op types in fc-core `presets` (built-ins in the actions/"other" group, applicable to any file or folder):

- **`copyPath`:** puts the full paths, newline-separated, on the clipboard as text.
  - The clipboard watcher stores it as a normal text clip, which is fine.
  - No job and no output file; it counts as done instantly in Recent with op label "Yol kopyalandı".
- **`openTerminal`:** opens a terminal in the folder (for a file, its parent folder).
  - Windows Terminal (`wt.exe -d <dir>`) if on PATH; otherwise `powershell.exe` with the working dir set.
  - One terminal per distinct folder, capped at 5.
- **`float`:** for raster images, opens each (max 6) in a small frameless always-on-top window (`float.html`, label `float-<n>`).
  - Size is fit to 40% of the work area. The window is draggable, closes with × or Esc, and Ctrl+wheel zooms.
  - Non-images are ignored, with a toast if none qualify.
  - The same window opens from the line's "Ekranda sabitle".

The editor, Recent, i18n and locale-parity tables get the new op types (as in the final fix wave: every `Operation['type']` has a label).

## Testing

- **fc-core:** settings defaults and old-file load, preset validation of the new ops, `applicable_ids` includes the actions for folders, files and mixed selections.
- **Rust shell:** `line_rect` geometry; hook zone function (pure: point, monitor rect → in zone) and "no input swallowed" (the line never swallows input); full-screen guard (pure: fg rect vs monitor rect).
- **TS:**
  - `physics.ts`: settles to rest, idle detection stops the loop, impulses decay, and reduce motion gives the rest pose.
  - `logic.ts`: kind → rope split, pinned-first order, fit count and "+N".
  - Locale parity (existing glob test).
- **Live:** the user's manual checklist, since GUI driving was denied.
  - Copy an image, text and a link: each drops onto its rope.
  - Peek does not steal typing.
  - Top-edge reveal works; nothing happens in a full-screen game or video.
  - Drag a photo into Explorer and into a chat app.
  - Float a screenshot.
  - Copy path, open terminal.

## Builder-check answers (controller calls, 2026-11-21; the user is away and said "hallet")

1. **Show modes.** There are three:
   - **Hotkey:** ACTIVATES the line. The previous foreground window is remembered as in `shelf.rs`, so Esc, Del, Ctrl+Z and Yapıştır work at once.
   - **Peek and edge reveal:** never activate. A click inside then activates.
   - **Hiding on focus loss** is armed only while the line is active.
2. **File actions are not jobs.**
   - `copyPath`, `openTerminal` and `float` are new `Operation` variants. `run_preset` handles them before queueing, like `Annotate`, and returns an empty job list.
   - They create no Recent entry. Recent's `opLabel` still gets labels so the parity test holds; the "counts in Recent" line above is dropped.
3. **Full-screen guard.**
   - It applies to peek and edge reveal only. The hotkey always works.
   - A window counts as full screen when its rect equals its monitor rect and it is not the desktop or shell (`Progman`, `WorkerW`, `Shell_TrayWnd`). There is no style check.
4. **Monitor choice.**
   - Peek → the monitor of the foreground window.
   - Edge reveal → the monitor whose top edge was hit.
   - Hotkey → the monitor under the cursor.
5. **Drag out.**
   - The page detects pointer-down plus 6 px of movement, then invokes `line_drag_out(id)`.
   - Rust runs `DoDragDrop` on the main/UI thread with `run_on_main_thread`, which is OLE-initialised by Tauri. `DoDragDrop` blocks that call until the drop ends; this is acceptable and the only blocking main-thread call.
   - On failure the command returns an error, the page shows a small toast, and click-to-copy still works.
6. **Mouse hook flags.** The hook's wheel flag stays as it is. A separate `LINE_EDGE` atomic gates the edge-zone check. The hook thread starts when the wheel OR line edge reveal is on, and `mouse_proc` does atomics plus a send only.
7. **Undo after a hidden-time change.** On show the line simply refreshes.
8. **Double-click deep link.** A new command, `line_open_item(id)`, calls `windows::show_main` with `NavPayload { view: "clipboard", clip: Some(id), .. }`.
   - `NavPayload` and TS `NavPayload` gain an optional `clip: number | null`.
   - The clipboard library view selects that item and opens its editor (image) or text edit (text).
   - Links open in the browser from the page through the existing open-url IPC.

## Left to the builder

- Exact card visuals and colours, using theme tokens only.
- Exact physics constants within the ranges above.
- Animation durations.
- Whether click-through of transparent gaps is done now.
- File split inside `src/line/`.
- How the main window deep-links to a clip item, using the smallest change to the existing nav.
