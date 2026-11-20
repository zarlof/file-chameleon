// Typed wrappers for every Tauri command and event. Views import only from here.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppInfo,
  ClipField,
  ClipFolder,
  ClipItem,
  ClipQuery,
  ClipStats,
  ClipTag,
  HotkeyStatus,
  DownloadRequest,
  DownloadSnapshot,
  JobRequest,
  JobSnapshot,
  MediaInfo,
  NavPayload,
  Settings,
  TargetsResponse,
  ToastPayload,
  ToolDone,
  ToolProgress,
  ToolStatus,
  View,
  WheelConfig,
  WheelPayload,
  WheelSlots,
  WheelThumb,
  WatchStatus,
} from './types';

export const api = {
  appInfo: () => invoke<AppInfo>('app_info'),
  getSettings: () => invoke<Settings>('get_settings'),
  setSettings: (settings: Settings) => invoke<Settings>('set_settings', { settings }),

  listTargets: (paths: string[]) => invoke<TargetsResponse>('list_targets', { paths }),
  submitJob: (req: JobRequest) => invoke<number[]>('submit_job', { req }),
  cancelJob: (id: number) => invoke<boolean>('cancel_job', { id }),
  listJobs: () => invoke<JobSnapshot[]>('list_jobs'),
  clearJobs: () => invoke<void>('clear_jobs'),

  toolsStatus: () => invoke<ToolStatus[]>('tools_status'),
  installTool: (id: string) => invoke<void>('install_tool', { id }),
  /** Install the missing tools, then submit (runs in Rust: survives closing the window). Error code "installFailed". */
  installAndSubmit: (tools: string[], req: JobRequest) => invoke<number[]>('install_and_submit', { tools, req }),
  /** Check for a newer yt-dlp now; true when it was updated. */
  updateYtdlp: () => invoke<boolean>('update_ytdlp'),
  uninstallTool: (id: string) => invoke<void>('uninstall_tool', { id }),

  probeUrl: (url: string) => invoke<MediaInfo>('probe_url', { url }),
  startDownload: (req: DownloadRequest) => invoke<number>('start_download', { req }),
  cancelDownload: (id: number) => invoke<boolean>('cancel_download', { id }),
  listDownloads: () => invoke<DownloadSnapshot[]>('list_downloads'),

  // ---- clipboard (Task 9 implements the Rust side)
  /** Newest first. Without `trash`, trashed items are excluded. */
  clipList: (query: ClipQuery = {}) => invoke<ClipItem[]>('clip_list', { query }),
  clipGet: (id: number) => invoke<ClipItem>('clip_get', { id }),
  /** Empty value clears title/note. `text` edits text items only. */
  clipSetField: (id: number, field: ClipField, value: string) => invoke<ClipItem>('clip_set_field', { id, field, value }),
  clipPin: (ids: number[], pinned: boolean) => invoke<void>('clip_pin', { ids, pinned }),
  /** Move to the trash (restorable). */
  clipTrash: (ids: number[]) => invoke<void>('clip_trash', { ids }),
  clipRestore: (ids: number[]) => invoke<void>('clip_restore', { ids }),
  clipDeleteForever: (ids: number[]) => invoke<void>('clip_delete_forever', { ids }),
  clipEmptyTrash: () => invoke<void>('clip_empty_trash'),
  /** Trash every unsaved live item; returns how many. */
  clipClearUnsaved: () => invoke<number>('clip_clear_unsaved'),
  clipFolders: () => invoke<ClipFolder[]>('clip_folders'),
  clipFolderCreate: (name: string, parent: number | null = null) => invoke<ClipFolder>('clip_folder_create', { name, parent }),
  clipFolderRename: (id: number, name: string) => invoke<void>('clip_folder_rename', { id, name }),
  /** Items in it move to "no folder"; sub-folders move up. */
  clipFolderDelete: (id: number) => invoke<void>('clip_folder_delete', { id }),
  clipMove: (ids: number[], folder: number | null) => invoke<void>('clip_move', { ids, folder }),
  clipTags: () => invoke<ClipTag[]>('clip_tags'),
  clipTag: (ids: number[], add: string[], remove: string[]) => invoke<void>('clip_tag', { ids, add, remove }),
  /** One text/image -> that; several texts -> joined with newlines; images -> a file list (CF_HDROP). */
  clipCopy: (ids: number[]) => invoke<void>('clip_copy', { ids }),
  /** Copy the OCR text of an image item. */
  clipCopyOcr: (id: number) => invoke<void>('clip_copy_ocr', { id }),
  /** Write the items as files into `dir`; returns how many files. */
  clipExport: (ids: number[], dir: string) => invoke<number>('clip_export', { ids, dir }),
  clipSaveImage: (id: number, format: 'png' | 'jpg' | 'webp') => invoke<string>('clip_save_image', { id, format }),
  /** Store an edited copy (PNG data URL) of an image item; the original is kept. */
  clipSaveEdited: (id: number, dataUrl: string) => invoke<ClipItem>('clip_save_edited', { id, dataUrl }),
  clipRevertEdit: (id: number) => invoke<ClipItem>('clip_revert_edit', { id }),
  clipStats: () => invoke<ClipStats>('clip_stats'),
  clipSetPaused: (paused: boolean) => invoke<void>('clip_set_paused', { paused }),
  clipGetPaused: () => invoke<boolean>('clip_get_paused'),

  // ---- shelf window (Task 9)
  showShelf: () => invoke<void>('show_shelf'),
  shelfHide: () => invoke<void>('shelf_hide'),
  /** Copy the items, hide the shelf and paste (Ctrl+V) into the app that was active before. */
  shelfPaste: (ids: number[]) => invoke<void>('shelf_paste', { ids }),
  hotkeyStatus: () => invoke<HotkeyStatus[]>('hotkey_status'),

  // ---- presets & wheel (Task 6 implements the Rust side)
  getWheelConfig: () => invoke<WheelConfig>('get_wheel_config'),
  /** Validates (limits, ids, references) and saves; returns the stored config. Error code "invalid". */
  setWheelConfig: (config: WheelConfig) => invoke<WheelConfig>('set_wheel_config', { config }),
  resetWheelProfile: (id: string) => invoke<WheelConfig>('reset_wheel_profile', { id }),
  resetWheelConfig: () => invoke<WheelConfig>('reset_wheel_config'),
  /** Preset ids usable for files with at least one of these extensions. */
  presetsForExtensions: (extensions: string[]) => invoke<string[]>('presets_for_extensions', { extensions }),
  /** Run a preset on files (copy/move without a folder asks first; annotate opens the editor). */
  runPreset: (paths: string[], presetId: string) => invoke<number[]>('run_preset', { paths, presetId }),
  /** Open the wheel in click mode at the cursor for these files. */
  wheelOpen: (paths: string[]) => invoke<void>('wheel_open', { paths }),
  /** Downscaled image as a PNG data URL (editor). */
  readImagePreview: (path: string, maxSide: number) => invoke<string>('read_image_preview', { path, maxSide }),
  /** Save an edited image (PNG data URL) as "<name> (düzenlendi).png" next to `path`; returns the new path. */
  saveAnnotated: (path: string, dataUrl: string) => invoke<string>('save_annotated', { path, dataUrl }),

  // ---- watch folders (Task 10)
  watchStatus: () => invoke<WatchStatus[]>('watch_status'),

  revealPath: (path: string) => invoke<void>('reveal_path', { path }),
  openPath: (path: string) => invoke<void>('open_path', { path }),
  showMain: (view?: View, paths?: string[], url?: string) =>
    invoke<void>('show_main', { view: view ?? null, paths: paths ?? null, url: url ?? null }),
  takePendingNav: () => invoke<NavPayload | null>('take_pending_nav'),

  overlayReady: () => invoke<void>('overlay_ready'),
  overlayHide: () => invoke<void>('overlay_hide'),
  overlayToastAction: (kind: string, action: string, data: unknown) =>
    invoke<void>('overlay_toast_action', { kind, action, data }),
  /** Files dropped on (or clicked in) a wheel slot; `target` is a preset id. */
  wheelDrop: (paths: string[], target: string) => invoke<number[]>('wheel_drop', { paths, target }),
};

export const on = {
  jobUpdate: (cb: (s: JobSnapshot) => void): Promise<UnlistenFn> => listen<JobSnapshot>('job://update', (e) => cb(e.payload)),
  downloadUpdate: (cb: (s: DownloadSnapshot) => void) => listen<DownloadSnapshot>('download://update', (e) => cb(e.payload)),
  toolProgress: (cb: (p: ToolProgress) => void) => listen<ToolProgress>('tool://progress', (e) => cb(e.payload)),
  toolDone: (cb: (p: ToolDone) => void) => listen<ToolDone>('tool://done', (e) => cb(e.payload)),
  clipAdded: (cb: (c: ClipItem) => void) => listen<ClipItem>('clip://added', (e) => cb(e.payload)),
  /** One item changed (OCR finished, edited, pinned elsewhere). */
  clipUpdated: (cb: (c: ClipItem) => void) => listen<ClipItem>('clip://updated', (e) => cb(e.payload)),
  /** Bulk change (trash, restore, folders, prune): refetch. */
  clipChanged: (cb: () => void) => listen('clip://changed', () => cb()),
  /** Shelf window: it was just shown. */
  shelfOpen: (cb: () => void) => listen('shelf://open', () => cb()),
  watchStatus: (cb: (s: WatchStatus) => void) => listen<WatchStatus>('watch://status', (e) => cb(e.payload)),
  clipPaused: (cb: (paused: boolean) => void) => listen<boolean>('clip://paused', (e) => cb(e.payload)),
  settingsChanged: (cb: (s: Settings) => void) => listen<Settings>('settings://changed', (e) => cb(e.payload)),
  navGoto: (cb: (n: NavPayload) => void) => listen<NavPayload>('nav://goto', (e) => cb(e.payload)),
  overlayWheel: (cb: (p: WheelPayload) => void) => listen<WheelPayload>('overlay://wheel', (e) => cb(e.payload)),
  overlayWheelSlots: (cb: (s: WheelSlots) => void) => listen<WheelSlots>('overlay://wheel-slots', (e) => cb(e.payload)),
  /** Flip front <-> back (mouse wheel / Tab caught by the global hooks). */
  overlayWheelFlip: (cb: () => void) => listen('overlay://wheel-flip', () => cb()),
  overlayWheelThumb: (cb: (t: WheelThumb) => void) => listen<WheelThumb>('overlay://wheel-thumb', (e) => cb(e.payload)),
  overlayToast: (cb: (t: ToastPayload) => void) => listen<ToastPayload>('overlay://toast', (e) => cb(e.payload)),
  overlayHide: (cb: () => void) => listen('overlay://hide', () => cb()),
};

/** Error from a rejected `invoke` (Rust `CmdError`), normalized. */
export function errorOf(e: unknown): { code: string; message: string } {
  if (e && typeof e === 'object' && 'code' in e) return e as { code: string; message: string };
  return { code: 'failed', message: String(e) };
}
