// Applies Settings.theme / Settings.accent to <html> via data-theme / data-accent.
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

/** Re-apply when the OS theme flips (only meaningful while theme === 'system'). Returns a disposer. */
export function watchSystemTheme(onChange: () => void): () => void {
  const q = darkQuery();
  if (!q) return () => {};
  q.addEventListener('change', onChange);
  return () => q.removeEventListener('change', onChange);
}
