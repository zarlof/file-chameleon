// Mirrors of the Rust IPC types (crates/fc-core + src-tauri/src/{ipc,events}.rs).
// Keep in sync by hand: field names are camelCase on the wire.

export type Kind = 'image' | 'vector' | 'video' | 'audio' | 'pdf' | 'data';

export interface Settings {
  language: 'system' | 'tr' | 'en';
  theme: 'system' | 'light' | 'dark';
  accent: Accent;
  launchAtLogin: boolean;
  outputDir: string | null;
  imageQuality: number;
  downloadDir: string | null;
  downloadMode: DlMode;
  downloadQuality: DlQuality;
  cookiesBrowser: string | null;
  wheelEnabled: boolean;
  wheelAnyApp: boolean;
  recentTargets: string[];
  clipboardEnabled: boolean;
  clipboardImages: boolean;
  clipboardSuggestLinks: boolean;
  clipboardSuggestImages: boolean;
  clipboardMaxItems: number;
  clipboardRetentionDays: number;
  onboarded: boolean;
}

export type Accent = 'blue' | 'purple' | 'pink' | 'red' | 'orange' | 'yellow' | 'green' | 'graphite';

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

export type Operation =
  | { type: 'convert'; to: string }
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
}

// ---------------------------------------------------------------- navigation & overlay events

export type View = 'convert' | 'download' | 'clipboard' | 'recent' | 'settings';

export interface NavPayload {
  view: View;
  paths: string[];
  url: string | null;
}

export interface WheelPayload {
  scale: number;
}

export interface WheelTargets {
  paths: string[];
  /** Kind of the first dragged file (centre icon). */
  firstKind: Kind | null;
  targets: TargetOption[];
  recent: string[];
}

export type ToastKind =
  | 'jobDone'
  | 'jobFailed'
  | 'jobsBatch'
  | 'toolNeeded'
  | 'clipLink'
  | 'clipImage'
  | 'downloadDone';

export interface ToastPayload {
  id: number;
  kind: ToastKind;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  data: Record<string, any>;
  progress: number | null;
  timeoutMs: number | null;
}
