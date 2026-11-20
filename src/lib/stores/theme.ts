// Applies Settings.theme / Settings.accent to <html> via data-theme / data-accent, Settings.reduceMotion
// via data-reduce-motion, and Settings.uiScale as the webview zoom.
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { Accent, Settings } from '../types';

export type ResolvedTheme = 'light' | 'dark';

export function resolveTheme(setting: Settings['theme'], prefersDark: boolean): ResolvedTheme {
  return setting === 'system' ? (prefersDark ? 'dark' : 'light') : setting;
}

function darkQuery(): MediaQueryList | null {
  return typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null;
}

export function applyTheme(theme: Settings['theme'], accent: Accent): ResolvedTheme {
  const resolved = resolveTheme(theme, darkQuery()?.matches ?? false);
  const root = document.documentElement;
  root.dataset.theme = resolved;
  root.dataset.accent = accent;
  root.style.colorScheme = resolved;
  return resolved;
}

/** `data-reduce-motion` on <html>: theme.css then zeroes the motion durations. */
export function applyMotion(reduce: boolean): void {
  const root = document.documentElement;
  if (reduce) root.dataset.reduceMotion = '';
  else delete root.dataset.reduceMotion;
}

/** Zoom the main webview to `uiScale` percent (clamped to 80..150). */
export function applyZoom(uiScale: number): void {
  const pct = Number.isFinite(uiScale) ? Math.min(150, Math.max(80, uiScale)) : 100;
  try {
    getCurrentWebview()
      .setZoom(pct / 100)
      .catch(() => {});
  } catch {
    /* not running inside Tauri */
  }
}

/** Re-apply when the OS theme flips (only meaningful while theme === 'system'). Returns a disposer. */
export function watchSystemTheme(onChange: () => void): () => void {
  const q = darkQuery();
  if (!q) return () => {};
  q.addEventListener('change', onChange);
  return () => q.removeEventListener('change', onChange);
}
