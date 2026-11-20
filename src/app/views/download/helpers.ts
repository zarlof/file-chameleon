// Pure helpers of the Download view (kept out of the components so they can be unit-tested).
import type { DlState, DownloadSnapshot } from '../../../lib/types';

/** Tools the downloader can ask the user to install (manifest ids). */
export const DOWNLOAD_TOOLS = ['yt-dlp', 'deno', 'ffmpeg'] as const;

/** The tool an error asks for: a `missingTool` error carries the manifest id as its message. */
export function missingToolOf(code: string | null | undefined, message: string | null | undefined): string | null {
  if (code !== 'missingTool' || !message) return null;
  const id = message.trim();
  return (DOWNLOAD_TOOLS as readonly string[]).includes(id) ? id : null;
}

/** 75 -> "1:15", 3725 -> "1:02:05". Empty for unknown values. */
export function formatDuration(secs: number | null | undefined): string {
  if (secs === null || secs === undefined || !Number.isFinite(secs) || secs < 0) return '';
  const total = Math.round(secs);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** Image/video size as "1920×1080", empty when unknown. */
export function resolutionLabel(width: number | null, height: number | null): string {
  return width && height ? `${width}×${height}` : '';
}

export function isActive(state: DlState): boolean {
  return state === 'probing' || state === 'downloading' || state === 'processing';
}

/** Newest first. */
export function sortedDownloads(list: DownloadSnapshot[]): DownloadSnapshot[] {
  return [...list].sort((a, b) => b.createdAt - a.createdAt || b.id - a.id);
}

/** Insert or replace by id. */
export function upsert(list: DownloadSnapshot[], snap: DownloadSnapshot): DownloadSnapshot[] {
  const i = list.findIndex((d) => d.id === snap.id);
  if (i < 0) return [...list, snap];
  const next = list.slice();
  next[i] = snap;
  return next;
}

/** Only https images may be shown as a thumbnail (the webview must not load plain http). */
export function safeThumbnail(url: string | null | undefined): string | null {
  return url && /^https:\/\//i.test(url) ? url : null;
}

/** Host of a URL for display, falling back to the text itself. */
export function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, '');
  } catch {
    return url;
  }
}
