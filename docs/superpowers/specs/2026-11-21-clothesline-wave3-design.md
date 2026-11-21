# Clothesline wave 3: real rope and pegs, files rope, no shelf, rich settings, more formats

Date: 2026-11-21. This builds on the wave 1 and wave 2 specs, whose constraints all bind. Branch `feat/line` after e7b9f70. The user prefers that sensible calls are made and stated; calls are marked *call*.

## The user's asks (from their message plus a reference screenshot)

1. **Rope and pegs look like the reference.**
   - Thin, natural ropes run across the **whole screen** edge to edge. There is **no white translucent band** behind a rope; that band made the rope hard to see. The rope is not a flat single colour.
   - Small realistic **clothespins** (light grey clip, a hint of a spring) hold each card at its top centre.
   - The cards are dark glass:
     - **Images:** rounded thumbnails with a thin border.
     - **Text cards:** dark translucent, white text, first lines.
     - **Link cards:** a preview image or favicon on top, then the title, then the domain, with a small number badge (1–9) in a corner.
   - Small rope labels ("Görseller", "Metinler", "Linkler") are pills at the left end of each rope.
2. **A 4th rope: files.** It shows the files the user added, downloaded and converted in the app.
3. **Remove the Ctrl+1 window**, which is the old shelf. The ropes replace it.
4. **A much richer settings page**: "baya bir şeyi değiştirebilelim".
5. Filee has ~270 conversion options. The user asked how many we have.
   - Direct conversions today: **187** over **32 formats**, and ~206 with the 2-step routes.
   - *Call*: expand with formats our existing engines already handle (ffmpeg, image crate codecs) to pass Filee's count, without adding heavy engines.

## 1. Look

### Rope (`Rope.svelte`)

- No band background. Only the rope, its knots and its label are drawn.
- **The rope spans the full window width.** The window is the full work-area width, with 0 side margin for the rope. Knots sit 6 px inside each edge.
- **Rope styles** (setting `lineRopeStyle`):
  - **`twisted`** (default): a braided cord. It is drawn as a base stroke in the rope colour plus a darker, thinner `stroke-dasharray` overlay offset by half a period, which gives a twisted look. A 1 px lighter highlight sits on top, and a soft drop shadow (`filter: drop-shadow`) below.
  - **`cord`**: a smooth single stroke with highlight and shadow.
  - **`thin`**: a 1 px line, like the reference.
- **Default rope colour** (`lineColor`): `"natural"`, a light warm grey `#d9d4cc` on dark, `#8a8378` on light. Swatches stay as they are, and `"natural"` is added as the default.
- **Sag** is the setting `lineSag`, 0–40 logical px, default 14. The curve stays one smooth parabola.

### Pegs (`Peg.svelte` / new `Pin.svelte`)

- Our own SVG clothespin, about 10×22 px at medium size:
  - two long jaws, with a slight bevel gradient;
  - a small metal spring coil hint in the middle (3 short strokes);
  - the jaws close over the rope.
- **Styles** (setting `linePinStyle`):
  - `plastic` (default, light grey like the reference);
  - `wood` (warm beige with grain lines);
  - `metal` (cool grey with a bright edge).
- The colour can be overridden with `linePinColor` (`"auto"` or a hex value).
- Pinned items get an accent-coloured peg.

### Cards

- **Card styles** (setting `lineCardStyle`):
  - `glassDark` (default): `rgba(20,20,24,.78)` with backdrop blur, a 1 px white border at 10% opacity, and white text;
  - `glassLight`;
  - `paper`: the previous look.
- **Corner radius** is `lineCardRadius`, 0–16, default 10. The **shadow** is toggled by `lineCardShadow`, default on.
- **Number badges**: when `lineShowNumbers` is on (default), the first 9 cards of the focused or hovered rope show a small 1–9 badge in the bottom-left corner. The digits match the keyboard pick.
- **Rope labels** are shown when `lineShowLabels` is on (default). They are pills at the left end of each rope, and their text can be set with `lineRopeNames`.

## 2. Files rope

- **Storage.** A new fc-core module `files` keeps a small persistent list `files.json` in the app local data folder: `[{ path, source, at }]`, where `source` is `"added" | "converted" | "downloaded"`.
  - It holds at most 300 entries, newest first.
  - A repeated path is deduplicated, and the repeat refreshes `at`.
  - Missing files are dropped on read.
- **Recording:**
  - **converted:** each output of a finished job;
  - **downloaded:** each finished download output;
  - **added:** files that the user drops on or picks in the Convert view, or that arrive through the wheel or the "Convert with" verb.
- **Settings:**
  - Which sources show: `lineFilesSources { added, converted, downloaded }`, all default true.
  - The rope is on or off through `lineRopes.files`, default true.
- **Card:**
  - The card shows the file name, the extension as a chip, the size, and a source icon (↓ downloaded, ⟳ converted, + added).
  - An image shows its thumbnail. Thumbnails are made by Rust into `files-thumbs/`, cached, with a 25 MP decode cap. They are allowed via `allow_directory`.
  - Other files show a generic type glyph by kind.
- **Interactions:**
  - **Click** copies the file to the clipboard as a file (`CF_HDROP`), so it pastes in Explorer or chat apps.
  - **Double-click** opens the file.
  - **Dragging out** drags the file.
  - **Right-click menu:** Aç, Klasörde göster, Dönüştür… (opens the wheel on it), Yolu kopyala, Listeden kaldır.
  - **Del** removes the entry from the list; the file itself stays.

## 3. Remove the shelf

- Delete `shelf.html`, `src/shelf.ts`, `src/shelf/`, `src-tauri/src/shelf.rs` and the shelf IPC commands. The paste helper is now `line`-owned.
- Settings migration (*call*): if `hotkeyShelf` is non-empty and `hotkeyLine` is still the default `Ctrl+Alt+L`, `hotkeyLine` takes the shelf hotkey. So this user's **Ctrl+1 now opens the line**. Then `hotkeyShelf` becomes `""`. The field stays in the struct for old files.
- The tray "Pano rafı" item becomes "İpi göster".
- The "+N" overflow on a rope opens the main library ("Pano" view) instead of the shelf.
- Pano settings that only configured the shelf move into the line page or are removed: shelf rows and order become rope order; shelf card size becomes `lineSize`.

## 4. Settings page "İp" (rich)

The page is organised in groups:

- **Görünüm (Look):**
  - Rope style: `lineRopeStyle`.
  - Rope colour: `lineColor`, with swatches, "natural" and custom.
  - Rope thickness: `lineThickness`, 1–6.
  - Sag: `lineSag`, 0–40.
  - Peg style: `linePinStyle`.
  - Peg colour: `linePinColor`.
  - Card style: `lineCardStyle`.
  - Card corner radius: `lineCardRadius`, 0–16.
  - Card shadow: `lineCardShadow`.
  - Opacity: `lineOpacity`.
  - Size: `lineSize`.
  - Labels: `lineShowLabels`.
  - Number badges: `lineShowNumbers`.
  - A **live preview** that uses the real components, as before.
- **İpler (Ropes):** a reorderable list of 4 ropes with up/down buttons (`lineRopeOrder`). Each rope has an on/off toggle (`lineRopes`) and a custom name field (`lineRopeNames`). The Files rope also has its 3 source toggles. There is a maximum number of cards per rope, `lineMaxPerRope`: 0 = auto, otherwise 3–30.
- **Konum (Position):** screen edge `lineEdge`, `top` | `bottom`. With `bottom`, the ropes hang from the bottom of the work area and the reveal zone is the bottom edge.
- **Davranış (Behaviour):**
  - Show on copy: `linePeekOnCopy`.
  - Peek duration: `linePeekMs`, 1000–6000, default 2500.
  - Reveal at the edge: `lineEdgeReveal`.
  - Edge dwell: `lineRevealDwellMs`, 150–1000, default 350.
  - Hide delay: `lineHideDelayMs`, 200–2000, default 400.
  - Click action: `lineClickAction`, `copy` | `copyClose` | `paste`, default `copy`.
  - Zoom on hover: `lineZoomOnHover`.
- **Hareket ve ses (Motion and sound):**
  - Swing strength: `lineSwing`, 0–200%.
  - Damping: `lineDamping`, 50–200%.
  - Breeze: `lineBreeze`, plus breeze strength `lineBreezeStrength`, 0–200%.
  - Drop animation: `lineFall`.
  - Sound: `lineSound`, plus volume `lineSoundVolume`, 0–100.
  - Reduce motion still overrides everything.
- **Kısayollar (Shortcuts):** the line, search, capture and copy-latest hotkeys, as now.
- A **"Varsayılanlara dön"** button resets only the line settings.

All of these are fc-core `settings.rs` fields with serde defaults and clamps, mirrored in TS.

## 5. More formats

The engines stay the same. The new formats are:

- **Video** (ffmpeg): `m4v`, `wmv`, `flv`, `mpg`, `3gp`, `ts`, `ogv`, for 12 video formats in total.
- **Audio** (ffmpeg): `aac`, `wma`, `aiff`, `ac3`, `amr`, `mka`, for 13 in total.
  - Each new format must map to an ffmpeg muxer and codec that the bundled ffmpeg build supports.
  - Formats ffmpeg can't encode with the shipped build are input-only. The implementer checks this with `ffmpeg -muxers` / `-encoders` on the managed ffmpeg if present, and otherwise by documentation, listing in the report what was verified.
- **Images** (image crate codecs, enabled as *features of the existing crate*): `tga`, `qoi`, `pnm` (`ppm`/`pgm`), for 10 raster formats in total.
- **Subtitles:** `ass` → `txt` through ffmpeg.

The format list in the Convert view and wheel labels (`OUTPUT_FORMATS` and the locales) are updated. Edge-count tests are updated. The expected direct conversion count is about 550, which the report states.

## Testing

- **Rust:**
  - the files store (dedupe, cap, missing-file pruning, sources);
  - the settings defaults, clamps and the shelf→line hotkey migration;
  - registry edge counts and the new ffmpeg extension maps;
  - `window_height` with 4 ropes;
  - bottom-edge geometry.
- **TS:**
  - rope SVG path generation per style (pure);
  - the rope order and names logic;
  - the files card model (`fileCard(entry)`, which gives icon, chip and size text);
  - layout with 4 ropes;
  - the settings reset.
- **Live (controller, CDP only, no input injection):**
  - the page renders four ropes;
  - asset loads work for the files thumbnails;
  - screenshots only when the user opens the line themselves.
