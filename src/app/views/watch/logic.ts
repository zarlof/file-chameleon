// Pure helpers for the watch folders page: new rules, which presets work unattended, and the
// status line of a rule.
import type { Preset, WatchFolder, WatchStatus } from '../../../lib/types';
import { presetLabel, type Translate } from '../../../lib/wheel/labels';

/** Preset a new rule starts with, when it exists. */
export const DEFAULT_PRESET = 'to-webp';

/** Presets that can run without anyone watching: not the image editor (`annotate`) and not a
 *  copy/move that asks for its folder (`copyTo` without one). Mirrors `rule_problem` in Rust. */
export function usablePresets(presets: Preset[]): Preset[] {
  return presets.filter((p) => p.op.type !== 'annotate' && !(p.op.type === 'copyTo' && p.op.dir === null));
}

let counter = 0;

/** A new, enabled rule for `path` with a unique id. Falls back to the first usable preset. */
export function newRule(path: string, presets: Preset[], existing: Iterable<string> = []): WatchFolder {
  const usable = usablePresets(presets);
  const taken = new Set(existing);
  let id: string;
  do {
    id = `w-${Date.now().toString(36)}-${(counter++).toString(36)}`;
  } while (taken.has(id));
  const preset = usable.find((p) => p.id === DEFAULT_PRESET) ?? usable[0];
  return { id, path, preset: preset?.id ?? '', subfolders: false, enabled: true };
}

/** Error codes of the backend (`rule_problem`) that have their own text. */
const CODES = ['presetMissing', 'presetNotUnattended'];

export interface StatusLine {
  text: string;
  /** Shown as an error. */
  error: boolean;
}

/** "Active · 12 files processed · last: x.png", "Paused", or the error. */
export function statusLine(rule: WatchFolder, status: WatchStatus | undefined, t: Translate): StatusLine {
  if (status?.error) {
    return { text: CODES.includes(status.error) ? t(`watch.errors.${status.error}`) : status.error, error: true };
  }
  if (!rule.enabled) return { text: t('watch.status.off'), error: false };
  const parts = [t('watch.status.on')];
  if (status && status.processed > 0) parts.push(t('watch.status.processed', { count: status.processed }));
  if (status?.lastFile) parts.push(t('watch.status.last', { name: status.lastFile }));
  return { text: parts.join(' · '), error: false };
}

/** Select options for a rule: usable presets, plus the rule's own preset when it is gone or
 *  unusable (so the row does not silently show another one). */
export function presetOptions(rule: WatchFolder, presets: Preset[], t: Translate): { value: string; label: string }[] {
  const options = usablePresets(presets).map((p) => ({ value: p.id, label: presetLabel(p, t) }));
  if (rule.preset && !options.some((o) => o.value === rule.preset)) {
    const own = presets.find((p) => p.id === rule.preset);
    options.unshift({ value: rule.preset, label: own ? presetLabel(own, t) : rule.preset });
  }
  return options;
}
