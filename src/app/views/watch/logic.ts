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

/** Error codes of the backend (`rule_problem`, `job_error`) that have their own text. A job error
 *  arrives as "<code>:<message>"; the message is kept as detail. Anything else (a folder read
 *  error from the OS) is shown as it is. */
const CODES = ['presetMissing', 'presetNotUnattended', 'selfTarget', 'missingTool', 'failed'];

export interface StatusLine {
  text: string;
  /** Shown as an error. */
  error: boolean;
  /** Raw message behind a coded error (tooltip). */
  detail?: string;
}

/** "Active · 12 files queued · last: x.png", "Off", or the error. */
export function statusLine(rule: WatchFolder, status: WatchStatus | undefined, t: Translate): StatusLine {
  if (status?.error) {
    const [code, ...rest] = status.error.split(':');
    if (CODES.includes(code)) {
      const detail = rest.join(':').trim();
      return { text: t(`watch.errors.${code}`), error: true, ...(detail ? { detail } : {}) };
    }
    return { text: status.error, error: true };
  }
  if (!rule.enabled) return { text: t('watch.status.off'), error: false };
  const parts = [t('watch.status.on')];
  if (status && status.processed > 0) parts.push(t('watch.status.queued', { count: status.processed }));
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
