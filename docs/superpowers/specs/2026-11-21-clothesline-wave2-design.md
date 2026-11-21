# Clothesline wave 2: look settings, even spacing, Tendedero + Clipboardline extras

Date: 2026-11-21. This builds on `2026-11-21-file-chameleon-clothesline-design.md` (all its constraints bind) and branch `feat/line` at 6ad6df9. Decisions marked *call* are the controller's; the user prefers that sensible calls are made and stated.

## The user's asks (verbatim intent)

0. No kinks in the rope. This is done in 6ad6df9: each rope is one smooth sagging curve.
1. Settings for the rope's **colour**, **size** and **opacity**.
2. The ropes sit at **equal distances** from each other, and the layout **re-adjusts when the size changes**.
3. Add the features of the linked apps (Tendedero, Tangerine) and **Clipboardline** that we still lack.

## 1–2. Look settings and layout

The new settings are fc-core `settings.rs` fields with serde defaults, mirrored in `types.ts`:

| Setting | Type | Default | Meaning |
|---|---|---|---|
| `lineColor` | string | `"accent"` | Rope colour. `"accent"` follows the app accent; otherwise a `#rrggbb` hex. The UI offers 8 swatches (*call*: natural, charcoal, white, accent, red, green, blue, pink) plus a custom colour input. |
| `lineThickness` | number | `2` | Rope stroke width in logical px, clamped 1–6. |
| `lineOpacity` | number | `100` | Opacity in percent, clamped 30–100. It applies to the rope bands and backdrop, while cards keep at least 90% so they stay readable (*call*). |
| `lineSize` | `'small' \| 'medium' \| 'large'` | `'medium'` | The size of cards and clothespins. Medium is today's look. *Call*: small = 0.8×, large = 1.25× card size. |

The layout must be **symmetric**:

- Every visible rope gets the same band height for the chosen size, and the gap between bands is equal.
- The top and bottom margins equal half a gap, so the ropes are spaced evenly in the window.
- The band height comes from one formula per size. It is pure, defined once in TS (`src/line/layout.ts`) and once in Rust (`line.rs`), with a test on each side asserting the same numbers.
- A unit test in each language pins the table for 1–3 ropes × 3 sizes.

Settings changes take effect live:

- When a look setting changes while the line is shown, the page re-lays out at once and Rust resizes the window. Rust applies size on `apply_settings` and on every show.
- Card x-slots are recomputed (`fitCount` uses the card width for the size), and existing cards ease to their new slots.

The **settings page "İp"** gains an "Görünüm / Look" group:

- colour swatches plus a custom colour;
- a thickness slider 1–6;
- an opacity slider 30–100%;
- a size segmented control (Küçük / Orta / Büyük);
- a small live preview strip (one rope with 3 sample cards, drawn with the same components) that updates as the sliders move.

## 3. Extras from Tendedero and Clipboardline

What we already have (no work): three rows, pins, folders, tags, trash with undo, per-kind retention, OCR plus copy OCR text, link titles, editor (markup and crop), dedupe, ignored apps, private markers, watched image folders, Ctrl+Enter paste into the previous app, export, and the shelf keyboard.

New in this wave (*call*, chosen for value per cost):

1. **Keyboard on the line**, matching our shelf:
   - arrows move focus across cards and ropes; Home/End;
   - 1–9 pick a card on the focused rope;
   - Enter copies and closes; Ctrl+Enter pastes into the previous app (existing `linePaste`); Ctrl+C copies and stays;
   - P pins; Del trashes; Ctrl+Z undoes;
   - Space opens a preview; E edits text (opens the library item); Ctrl+Shift+C copies an image's OCR text;
   - Esc clears the selection first, then hides.

   The keys work when the line is active (the hotkey activates it). A peek becomes active on the first click.
2. **Search on the line.** A new global hotkey, `hotkeyLineSearch`, defaulting to `"Ctrl+Alt+F"`, opens the line in mode `search`. This is an activating show with a search field at the top-right of the strip.
   - Typing filters all ropes live, matching text, title, note, tags, OCR and link title, case- and diacritic-insensitive. The match logic is pure in `logic.ts` and tested.
   - Enter copies the newest match. Esc clears the query, then hides.
   - Typing any printable character while the line is active (no modifier) also starts a search, as the shelf does.
3. **Preview (Space).** A large in-window preview over the strip:
   - an image fitted to the window height;
   - text rendered with light Markdown;
   - a link showing its title, domain and URL.

   Left/Right move to the next or previous item on the rope; Space or Esc closes the preview.
4. **Zoom on hover.** Setting `lineZoomOnHover`, default `false`: after the pointer rests 400 ms on an image card, a 2× enlarged image appears below the card.
5. **Light Markdown on text cards and previews.** Headings (`#`), bold and italic, inline code, bullet lists and links shown as text. It is rendered by our own tiny escaped renderer (pure, tested, no HTML pass-through).
6. **"İpe as / Hang on the line" Explorer verb** for image and text files. Registry is the same as the existing verb mechanism; the args are `--hang "<file>"`.
   - Single-instance args import each file into the clip store:
     - an image (PNG/JPG/WebP/BMP/GIF first frame, ≤ 50 MB) becomes an image item;
     - a `.txt`/`.md`/`.csv`/`.json`/`.log` file of ≤ 1 MB, valid UTF-8, becomes a text item.
   - Then the line peeks. It is toggled with the existing "Explorer menu" setting (one switch covers both verbs).
7. **Screen capture hotkey.** `hotkeyCapture`, default `""` (off; *call*, because Ctrl+Alt+A is commonly taken; the settings page suggests it). It captures the monitor under the cursor (GDI) to PNG, adds it as an image item, and peeks the line.
8. **Copy latest image hotkey.** `hotkeyCopyLatestImage`, default `""` (off). It copies the newest image item to the clipboard.
9. **YouTube thumbnails for links.** When link titles are on, `mediaLink` items whose URL is a YouTube video get the thumbnail `https://i.ytimg.com/vi/<id>/mqdefault.jpg`. It is fetched once, stored as the item's `thumb`, and capped at 200 KB. The link card shows it.
10. **Hang sound.** Setting `lineSound`, default `false`: a soft synthesized "clip" tick (WebAudio, no audio files) when a card lands.

Not in this wave (*call*): screen recordings or video items, CLIP visual search, name-and-pin batch workflow, Ctrl+K actions palette, cloud sync, share sheet.

## Testing

- **TS:**
  - `layout.ts` table and spacing symmetry;
  - search match (diacritics, OCR, tags);
  - markdown renderer (escaping, headings, lists, no raw HTML);
  - keyboard focus model (arrows across ropes, 1–9).
- **Rust:**
  - band-height table equal to TS;
  - `--hang` arg parsing;
  - hang import (image vs text vs rejected: size, encoding);
  - YouTube id extraction;
  - hotkey ids for search, capture and latest.
- **Live (controller):** hotkey show and screenshot with each size; a colour and opacity change; the search hotkey.
