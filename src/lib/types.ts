// Mirrors of the Rust IPC types (crates/fc-core + src-tauri/src/{ipc,events}.rs).
// Keep in sync by hand: field names are camelCase on the wire.

export type Kind = 'image' | 'vector' | 'video' | 'audio' | 'pdf' | 'data' | 'subtitle' | 'archive';

export interface Settings {
  language: 'system' | 'tr' | 'en';
  theme: 'system' | 'light' | 'dark';
  accent: Accent;
  launchAtLogin: boolean;
  /** Closing the main window keeps the app in the tray (false = quit). */
  closeToTray: boolean;
  /** Explorer right-click verb (HKCU registry) that opens the wheel. */
  contextMenu: boolean;
  /** Main window zoom in percent, 80..150. */
  uiScale: number;
  reduceMotion: boolean;
  outputDir: string | null;
  imageQuality: number;
  downloadDir: string | null;
  downloadMode: DlMode;
  downloadQuality: DlQuality;
  cookiesBrowser: string | null;
  wheelEnabled: boolean;
  wheelAnyApp: boolean;
  recentTargets: string[];
  wheelFlipScroll: boolean;
  wheelFlipTab: boolean;
  wheelShiftBack: boolean;
  /** Lowercase exe names where Ctrl+drag never opens the wheel. */
  wheelExcludedApps: string[];
  wheelLabels: WheelLabels;
  /** 40..100 */
  wheelOpacity: number;
  /** 'accent' or an Accent name. */
  wheelColor: 'accent' | Accent;
  /** 80..130 */
  wheelScale: number;
  wheelShowDetail: boolean;
  /** Global hotkeys, "Ctrl+Alt+V" style; "" = off. */
  hotkeyShelf: string;
  hotkeyLibrary: string;
  hotkeyDownloadLink: string;
  hotkeyLine: string;
  /** Opens the line with the search field focused. */
  hotkeyLineSearch: string;
  /** Captures the monitor under the cursor into the clipboard history. */
  hotkeyCapture: string;
  /** Copies the newest image clip back to the clipboard. */
  hotkeyCopyLatestImage: string;
  /** The clothesline (clip rope). */
  lineEnabled: boolean;
  lineEdgeReveal: boolean;
  linePeekOnCopy: boolean;
  lineBreeze: boolean;
  lineRopes: { images: boolean; text: boolean; links: boolean };
  /** 'accent' or an Accent name. */
  lineColor: 'accent' | Accent;
  /** 1..6 px */
  lineThickness: number;
  /** 30..100 percent */
  lineOpacity: number;
  lineSize: 'small' | 'medium' | 'large';
  lineZoomOnHover: boolean;
  lineSound: boolean;
  watchFolders: WatchFolder[];
  clipboardEnabled: boolean;
  clipboardCaptureText: boolean;
  clipboardCaptureLinks: boolean;
  /** Capture images. */
  clipboardImages: boolean;
  clipboardSuggestLinks: boolean;
  clipboardSuggestImages: boolean;
  /** Cap on unsaved items. */
  clipboardMaxItems: number;
  /** Days; 0 = forever. Unsaved items only. */
  clipboardRetentionText: number;
  clipboardRetentionLinks: number;
  clipboardRetentionImages: number;
  clipboardIgnoreApps: string[];
  clipboardOcr: boolean;
  clipboardLinkTitles: boolean;
  /** Up to 3 folders whose new images are imported (e.g. Screenshots). */
  clipboardWatchFolders: string[];
  shelfRows: ClipShelf[];
  shelfCardSize: 'small' | 'medium' | 'large';
  /** true = click copies and closes; false = click selects. */
  shelfClickCopies: boolean;
  /** Changed only through `clipSetPaused` / the tray; `setSettings` keeps the stored value. */
  clipboardPaused: boolean;
  onboarded: boolean;
}

export type Accent = 'blue' | 'purple' | 'pink' | 'red' | 'orange' | 'yellow' | 'green' | 'graphite';

export type WheelLabels = 'auto' | 'iconText' | 'text' | 'icon';

export interface WatchFolder {
  id: string;
  path: string;
  /** Preset id from the wheel config. */
  preset: string;
  subfolders: boolean;
  enabled: boolean;
}

export interface AppInfo {
  version: string;
  os: string;
  systemLanguage: 'tr' | 'en';
}

export interface CmdError {
  code: string;
  message: string;
}

// ---------------------------------------------------------------- convert

export interface InputInfo {
  path: string;
  name: string;
  format: string | null;
  kind: Kind | null;
  size: number;
}

export interface TargetOption {
  id: string;
  label: string;
  kind: Kind;
  /** External tool ids needed ("ffmpeg", "pdfium"). */
  needsTools: string[];
}

export type ToolOp = 'compress' | 'trim' | 'mute' | 'speed' | 'crop' | 'mergePdf' | 'splitPdf';

export interface TargetsResponse {
  inputs: InputInfo[];
  targets: TargetOption[];
  tools: ToolOp[];
}

export type CompressLevel = 'light' | 'balanced' | 'strong';

/** Image edit steps (`Operation.edit`), applied in order. */
export type ImageOp =
  | { type: 'grayscale' }
  | { type: 'rotateLeft' }
  | { type: 'rotateRight' }
  | { type: 'rotate180' }
  | { type: 'flipH' }
  | { type: 'flipV' }
  /** Crop away fully transparent borders. */
  | { type: 'trim' }
  | { type: 'pixelate'; block: number }
  | { type: 'blur'; sigma: number }
  | { type: 'scale'; percent: number };

export type Operation =
  | { type: 'convert'; to: string }
  /** Image edits; `to` = output format id, null = keep the input format. */
  | { type: 'edit'; ops: ImageOp[]; to: string | null }
  /** Centre crop to aspect w:h (images in-process, video via ffmpeg). */
  | { type: 'cropAspect'; w: number; h: number }
  /** dir null = ask with a folder picker when run. */
  | { type: 'copyTo'; dir: string | null; moveFiles: boolean }
  /** All inputs -> one .zip next to the first input. */
  | { type: 'zip' }
  /** .zip -> folder next to it. */
  | { type: 'extract' }
  /** Opens the image editor (never queued). */
  | { type: 'annotate' }
  /** Copies the paths as text (shell, never queued). */
  | { type: 'copyPath' }
  /** Opens a terminal in the folder (shell, never queued). */
  | { type: 'openTerminal' }
  /** Pins the image(s) on screen (shell, never queued). */
  | { type: 'float' }
  | { type: 'mergePdf' }
  | { type: 'splitPdf' }
  | { type: 'compress'; level: CompressLevel }
  | { type: 'trim'; startMs: number; endMs: number }
  | { type: 'mute' }
  | { type: 'speed'; factor: number }
  | { type: 'crop'; x: number; y: number; width: number; height: number };

export interface ConvertOptions {
  quality?: number | null;
  maxWidth?: number | null;
  maxHeight?: number | null;
  outputDir?: string | null;
  outputName?: string | null;
}

// ---------------------------------------------------------------- presets & wheel

export interface Preset {
  id: string;
  /** "" for built-ins: the UI shows `presetLabel()` (src/lib/wheel/labels.ts). */
  name: string;
  builtin: boolean;
  op: Operation;
  options: ConvertOptions;
}

export interface PresetGroup {
  id: string;
  name: string;
  /** Preset ids, at most 6. */
  presets: string[];
}

export interface Profile {
  id: string;
  /** "" for built-ins: the UI shows t(`presets.profile.<id>`). */
  name: string;
  builtin: boolean;
  /** Lowercase, without dot. Empty for the "other" fallback profile. */
  extensions: string[];
  /** Used when a selection spans several profiles. */
  mixed: boolean;
  /** Slot ids (preset id or "group:<group id>"): front at most 8, back at most 12. */
  front: string[];
  back: string[];
}

export interface WheelConfig {
  version: number;
  presets: Preset[];
  groups: PresetGroup[];
  profiles: Profile[];
}

export type WheelFace = 'front' | 'back';

/** Icon keys a slot can carry (mapped to Lucide icons by the UI). */
export type SlotIcon =
  | Kind
  | 'grayscale' | 'rotateLeft' | 'rotateRight' | 'rotate180' | 'flipH' | 'flipV' | 'trim' | 'pixelate' | 'blur'
  | 'resize' | 'annotate' | 'compress' | 'mute' | 'speed' | 'crop' | 'merge' | 'split' | 'zip' | 'extract'
  | 'copy' | 'move' | 'ringtone' | 'group' | 'copyPath' | 'openTerminal' | 'float';

/** One resolved wheel slot (only slots that fit every dragged file are sent). */
export interface WheelSlot {
  /** Preset id, or "group:<id>". */
  id: string;
  /** User-given name ("" for built-ins). */
  name: string;
  /** Output format label for convert presets ("JPG"), else null. */
  format: string | null;
  /** Detail line: "70%", "16:9", "2x", "1920 px"; null = none. */
  detail: string | null;
  icon: SlotIcon;
  /** Output kind, null for file operations. */
  kind: Kind | null;
  /** Tool ids this preset needs ("ffmpeg", "pdfium"). */
  needsTools: string[];
  /** Group members (non-empty only for groups). */
  children: WheelSlot[];
}

export interface WheelStyle {
  labels: WheelLabels;
  opacity: number;
  /** Resolved CSS colour of the hovered slice ("#7c6cf0"). */
  color: string;
  /** 0.8..1.3 */
  scale: number;
  showDetail: boolean;
  reduceMotion: boolean;
}

/** `overlay://wheel-slots`: what the wheel shows for the dragged / chosen files. */
export interface WheelSlots {
  paths: string[];
  count: number;
  /** Extension of the first file, lowercase ("jpg"). */
  firstExt: string | null;
  firstKind: Kind | null;
  profileId: string;
  /** "" for built-in profiles. */
  profileName: string;
  front: WheelSlot[];
  back: WheelSlot[];
  /** Face to show first (back for Ctrl+Shift+drag). */
  face: WheelFace;
  /** Click mode (opened from the context menu or the app): hover/click with the mouse. */
  click: boolean;
  missingTools: string[];
  style: WheelStyle;
}

/** `overlay://wheel-thumb`: small thumbnail of the first dragged image. */
export interface WheelThumb {
  path: string;
  dataUrl: string;
}

export interface JobRequest {
  inputs: string[];
  op: Operation;
  options?: ConvertOptions;
}

export type JobState = 'queued' | 'running' | 'done' | 'failed' | 'cancelled';

export interface JobSnapshot {
  id: number;
  inputs: string[];
  op: Operation;
  state: JobState;
  progress: number;
  outputs: string[];
  errorCode: string | null;
  error: string | null;
  createdAt: number;
  finishedAt: number | null;
}

// ---------------------------------------------------------------- tools

export type ToolSource = 'managed' | 'system' | 'missing';

export interface ToolStatus {
  id: string;
  name: string;
  purpose: string;
  license: string;
  source: ToolSource;
  path: string | null;
  version: string | null;
  size: number;
}

export interface ToolProgress {
  id: string;
  done: number;
  total: number;
}

export interface ToolDone {
  id: string;
  ok: boolean;
  errorCode: string | null;
  error: string | null;
}

// ---------------------------------------------------------------- downloads

export type DlMode = 'original' | 'video' | 'audio' | 'gif';
export type DlQuality = 'best' | 'p1080' | 'p720';

export interface MediaInfo {
  url: string;
  title: string;
  site: string;
  kind: 'video' | 'audio' | 'image';
  durationSecs: number | null;
  width: number | null;
  height: number | null;
  sizeBytes: number | null;
  thumbnail: string | null;
  uploader: string | null;
  direct: boolean;
  imageUrls: string[];
}

export interface DownloadRequest {
  url: string;
  mode: DlMode;
  quality: DlQuality;
  outDir: string;
  /** Title from the preview, shown while downloading. */
  title?: string;
}

export type DlState = 'probing' | 'downloading' | 'processing' | 'done' | 'failed' | 'cancelled';

export interface DownloadSnapshot {
  id: number;
  url: string;
  title: string;
  state: DlState;
  progress: number | null;
  speedBps: number | null;
  etaSecs: number | null;
  output: string | null;
  errorCode: string | null;
  error: string | null;
  createdAt: number;
}

// ---------------------------------------------------------------- clipboard

export type ClipKind = 'text' | 'link' | 'mediaLink' | 'image';
/** Shelf rows: images = image; texts = text; links = link + mediaLink. */
export type ClipShelf = 'images' | 'texts' | 'links';

export interface ClipItem {
  id: number;
  kind: ClipKind;
  text: string | null;
  image: string | null;
  thumb: string | null;
  width: number | null;
  height: number | null;
  site: string | null;
  hash: string;
  createdAt: number;
  pinned: boolean;
  title: string | null;
  note: string | null;
  tags: string[];
  folder: number | null;
  /** Text recognized in the image (Windows OCR), null = not yet / none. */
  ocr: string | null;
  /** Set while the item is in the trash. */
  trashedAt: number | null;
  /** Exe name of the app that copied it. */
  sourceApp: string | null;
  linkTitle: string | null;
  /** Edited copy of an image (the original `image` is kept). */
  edited: string | null;
  editedThumb: string | null;
  /** Pinned, in a folder, tagged, titled or noted: never expires. */
  saved: boolean;
  /** Bytes of the text or the image file. */
  size: number;
}

export interface ClipQuery {
  /** Searches text, title, note, tags, OCR text and link title (case-insensitive). */
  text?: string | null;
  shelf?: ClipShelf | null;
  pinned?: boolean;
  folder?: number | null;
  tag?: string | null;
  /** true = only trashed items; false/absent = only live items. */
  trash?: boolean;
  /** Only saved items. */
  saved?: boolean;
  limit?: number | null;
}

export interface ClipFolder {
  id: number;
  name: string;
  parent: number | null;
  /** Live items directly in this folder. */
  count: number;
}

export interface ClipTag {
  tag: string;
  count: number;
}

export type ClipField = 'title' | 'note' | 'text';

export interface ClipStats {
  items: number;
  trash: number;
  bytes: number;
}

export interface HotkeyStatus {
  name: 'shelf' | 'library' | 'downloadLink' | 'line' | 'lineSearch' | 'capture' | 'copyLatestImage';
  keys: string;
  /** false = could not register (another app owns it, or invalid). */
  ok: boolean;
}

export interface WatchStatus {
  id: string;
  active: boolean;
  error: string | null;
  processed: number;
  lastFile: string | null;
}

// ---------------------------------------------------------------- navigation & overlay events

export type View =
  | 'convert'
  | 'download'
  | 'clipboard'
  | 'recent'
  | 'general'
  | 'wheel'
  | 'presets'
  | 'shortcuts'
  | 'watch'
  | 'clipboardSettings'
  | 'line'
  | 'appearance'
  | 'engines'
  | 'about'
  /** Image editor (not in the sidebar); NavPayload.paths[0] is the image path, or `clip:<id>`. */
  | 'editor';

export interface NavPayload {
  view: View;
  paths: string[];
  url: string | null;
  /** Clipboard item to open (clipboard view). */
  clip?: number | null;
}

export interface WheelPayload {
  scale: number;
  /** Click mode (no drag in progress). */
  click: boolean;
}

export type ToastKind =
  | 'jobDone'
  | 'jobFailed'
  | 'jobsBatch'
  | 'toolNeeded'
  | 'toolInstalling'
  | 'toolFailed'
  | 'clipLink'
  | 'clipImage'
  | 'imageSaved'
  | 'downloadDone'
  /** The "download the copied link" hotkey found no link on the clipboard. */
  | 'noLink';

export interface ToastPayload {
  id: number;
  kind: ToastKind;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: Record<string, any>;
  progress: number | null;
  timeoutMs: number | null;
}
