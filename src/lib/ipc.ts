// Typed wrappers for every Tauri command and event. Views import only from here.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppInfo,
  ClipItem,
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
  WheelPayload,
  WheelTargets,
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

  clipList: (query?: string) => invoke<ClipItem[]>('clip_list', { query: query ?? null }),
  clipPin: (id: number, pinned: boolean) => invoke<boolean>('clip_pin', { id, pinned }),
  clipDelete: (id: number) => invoke<boolean>('clip_delete', { id }),
  clipClear: () => invoke<void>('clip_clear'),
  clipCopy: (id: number) => invoke<void>('clip_copy', { id }),
  clipSaveImage: (id: number, format: 'png' | 'jpg' | 'webp') => invoke<string>('clip_save_image', { id, format }),
  clipSetPaused: (paused: boolean) => invoke<void>('clip_set_paused', { paused }),
  clipGetPaused: () => invoke<boolean>('clip_get_paused'),

  revealPath: (path: string) => invoke<void>('reveal_path', { path }),
  openPath: (path: string) => invoke<void>('open_path', { path }),
  showMain: (view?: View, paths?: string[], url?: string) =>
    invoke<void>('show_main', { view: view ?? null, paths: paths ?? null, url: url ?? null }),
  takePendingNav: () => invoke<NavPayload | null>('take_pending_nav'),

  overlayReady: () => invoke<void>('overlay_ready'),
  overlayHide: () => invoke<void>('overlay_hide'),
  overlayToastAction: (kind: string, action: string, data: unknown) =>
    invoke<void>('overlay_toast_action', { kind, action, data }),
  wheelDrop: (paths: string[], target: string) => invoke<number[]>('wheel_drop', { paths, target }),
};

export const on = {
  jobUpdate: (cb: (s: JobSnapshot) => void): Promise<UnlistenFn> => listen<JobSnapshot>('job://update', (e) => cb(e.payload)),
  downloadUpdate: (cb: (s: DownloadSnapshot) => void) => listen<DownloadSnapshot>('download://update', (e) => cb(e.payload)),
  toolProgress: (cb: (p: ToolProgress) => void) => listen<ToolProgress>('tool://progress', (e) => cb(e.payload)),
  toolDone: (cb: (p: ToolDone) => void) => listen<ToolDone>('tool://done', (e) => cb(e.payload)),
  clipAdded: (cb: (c: ClipItem) => void) => listen<ClipItem>('clip://added', (e) => cb(e.payload)),
  clipPaused: (cb: (paused: boolean) => void) => listen<boolean>('clip://paused', (e) => cb(e.payload)),
  settingsChanged: (cb: (s: Settings) => void) => listen<Settings>('settings://changed', (e) => cb(e.payload)),
  navGoto: (cb: (n: NavPayload) => void) => listen<NavPayload>('nav://goto', (e) => cb(e.payload)),
  overlayWheel: (cb: (p: WheelPayload) => void) => listen<WheelPayload>('overlay://wheel', (e) => cb(e.payload)),
  overlayWheelTargets: (cb: (t: WheelTargets) => void) => listen<WheelTargets>('overlay://wheel-targets', (e) => cb(e.payload)),
  overlayToast: (cb: (t: ToastPayload) => void) => listen<ToastPayload>('overlay://toast', (e) => cb(e.payload)),
  overlayHide: (cb: () => void) => listen('overlay://hide', () => cb()),
};

/** Error from a rejected `invoke` (Rust `CmdError`), normalized. */
export function errorOf(e: unknown): { code: string; message: string } {
  if (e && typeof e === 'object' && 'code' in e) return e as { code: string; message: string };
  return { code: 'failed', message: String(e) };
}
