// Pure helpers for the settings pages.

/**
 * Normalize what the user typed into an exe name for `wheelExcludedApps`: file name only,
 * trimmed, lowercase, ".exe" appended when missing. Null when nothing usable is left.
 */
export function exeName(raw: string): string | null {
  const parts = raw.trim().split(/[\\/]/);
  const name = (parts[parts.length - 1] ?? '').trim().toLowerCase();
  if (!name || name === '.exe') return null;
  return name.endsWith('.exe') ? name : `${name}.exe`;
}

/** `list` plus the normalized name (once); the same array when the input is unusable or already listed. */
export function addExe(list: string[], raw: string): string[] {
  const name = exeName(raw);
  return name && !list.includes(name) ? [...list, name] : list;
}
