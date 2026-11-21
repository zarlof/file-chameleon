import { describe, expect, it } from 'vitest';
import { translate } from '../../../lib/i18n';
import type { Preset, WatchFolder, WatchStatus } from '../../../lib/types';
import enWatch from '../../../locales/en/watch.json';
import trWatch from '../../../locales/tr/watch.json';
import { newRule, presetOptions, statusLine, usablePresets } from './logic';

const en = (key: string, params?: Record<string, string | number>) => translate('en', key, params);
const tr = (key: string, params?: Record<string, string | number>) => translate('tr', key, params);

const preset = (id: string, op: Preset['op']): Preset => ({ id, name: id, builtin: false, op, options: {} });
const PRESETS: Preset[] = [
  preset('to-png', { type: 'convert', to: 'png' }),
  preset('to-webp', { type: 'convert', to: 'webp' }),
  preset('annotate', { type: 'annotate' }),
  preset('pick', { type: 'copyTo', dir: null, moveFiles: false }),
  preset('fixed', { type: 'copyTo', dir: 'D:\\x', moveFiles: false }),
];

const rule = (over: Partial<WatchFolder> = {}): WatchFolder => ({ id: 'w', path: 'C:\\in', preset: 'to-webp', subfolders: false, enabled: true, ...over });
const status = (over: Partial<WatchStatus> = {}): WatchStatus => ({ id: 'w', active: true, error: null, processed: 0, lastFile: null, ...over });

describe('usablePresets', () => {
  it('drops annotate and a copy without a folder', () => {
    expect(usablePresets(PRESETS).map((p) => p.id)).toEqual(['to-png', 'to-webp', 'fixed']);
  });
});

describe('newRule', () => {
  it('is enabled, uses to-webp when present and has a unique id', () => {
    const a = newRule('C:\\in', PRESETS);
    const b = newRule('C:\\in', PRESETS);
    expect(a).toMatchObject({ path: 'C:\\in', preset: 'to-webp', enabled: true, subfolders: false });
    expect(a.id).not.toBe(b.id);
    expect(newRule('C:\\in', PRESETS, [a.id, b.id]).id).not.toBe(a.id);
  });

  it('falls back to the first usable preset, or none', () => {
    expect(newRule('C:\\in', PRESETS.filter((p) => p.id !== 'to-webp')).preset).toBe('to-png');
    expect(newRule('C:\\in', [preset('annotate', { type: 'annotate' })]).preset).toBe('');
  });
});

describe('statusLine', () => {
  it('summarizes an active rule', () => {
    expect(statusLine(rule(), status({ processed: 12, lastFile: 'x.png' }), en)).toEqual({
      text: 'Active · 12 files processed · last: x.png',
      error: false,
    });
    expect(statusLine(rule(), undefined, tr).text).toBe('Etkin');
  });

  it('shows pause and errors', () => {
    expect(statusLine(rule({ enabled: false }), status(), en).text).toBe('Off');
    const gone = statusLine(rule(), status({ active: false, error: 'presetMissing' }), en);
    expect(gone.error).toBe(true);
    expect(gone.text).toBe(enWatch.errors.presetMissing);
    expect(statusLine(rule(), status({ error: 'Access is denied.' }), en)).toEqual({ text: 'Access is denied.', error: true });
  });
});

describe('presetOptions', () => {
  it('keeps the rule\'s own missing preset visible', () => {
    const ids = (r: WatchFolder) => presetOptions(r, PRESETS, en).map((o) => o.value);
    expect(ids(rule())).toEqual(['to-png', 'to-webp', 'fixed']);
    expect(ids(rule({ preset: 'gone' }))).toEqual(['gone', 'to-png', 'to-webp', 'fixed']);
    expect(ids(rule({ preset: 'annotate' }))[0]).toBe('annotate');
  });
});

describe('locales', () => {
  it('have the same keys in both languages', () => {
    const keys = (o: object, p = ''): string[] =>
      Object.entries(o).flatMap(([k, v]) => (typeof v === 'object' ? keys(v, `${p}${k}.`) : [`${p}${k}`]));
    expect(keys(trWatch).sort()).toEqual(keys(enWatch).sort());
  });
});
