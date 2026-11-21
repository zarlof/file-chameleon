import { describe, expect, it } from 'vitest';
import { translate } from '../../../lib/i18n';
import enPage from '../../../locales/en/presetsPage.json';
import trPage from '../../../locales/tr/presetsPage.json';
import enWheel from '../../../locales/en/wheel.json';
import trWheel from '../../../locales/tr/wheel.json';
import type { Preset, WheelConfig } from '../../../lib/types';
import { ACTION_KINDS, describeOp, describePreset, duplicatePreset, parseNumber, presetUsage, withName, withOp } from './logic';

const en = (key: string, params?: Record<string, string | number>) => translate('en', key, params);
const tr = (key: string, params?: Record<string, string | number>) => translate('tr', key, params);

const mine = (op: Preset['op'], options: Preset['options'] = {}): Preset => ({ id: 'u-1', name: 'Mine', builtin: false, op, options });

describe('describeOp', () => {
  it('describes crops, conversions and edits', () => {
    expect(describeOp({ type: 'cropAspect', w: 16, h: 9 }, en)).toContain('16:9');
    expect(describeOp({ type: 'cropAspect', w: 16, h: 9 }, tr)).toContain('16:9');
    expect(describeOp({ type: 'convert', to: 'jpg' }, en)).toContain('JPG');
    expect(describeOp({ type: 'convert', to: 'webp' }, tr)).toContain('WebP');
    const edit = describeOp({ type: 'edit', ops: [{ type: 'grayscale' }, { type: 'pixelate', block: 12 }], to: null }, en);
    expect(edit).toContain('Grayscale');
    expect(edit).toContain('12');
    expect(edit).toContain('Same format');
    expect(describeOp({ type: 'edit', ops: [{ type: 'scale', percent: 50 }], to: 'png' }, en)).toContain('PNG');
  });

  it('names the folder of copy and move presets', () => {
    expect(describeOp({ type: 'copyTo', dir: 'C:\\Users\\me\\Photos', moveFiles: false }, en)).toContain('Photos');
    expect(describeOp({ type: 'copyTo', dir: 'D:\\Archive', moveFiles: true }, en)).toContain('Archive');
    expect(describeOp({ type: 'copyTo', dir: null, moveFiles: false }, en)).not.toBe(describeOp({ type: 'copyTo', dir: null, moveFiles: true }, en));
  });

  it('has text for every operation in both languages', () => {
    const ops: Preset['op'][] = [
      { type: 'zip' }, { type: 'extract' }, { type: 'annotate' }, { type: 'mergePdf' }, { type: 'splitPdf' },
      { type: 'compress', level: 'strong' }, { type: 'mute' }, { type: 'speed', factor: 0.5 },
      { type: 'trim', startMs: 0, endMs: 1000 }, { type: 'crop', x: 0, y: 0, width: 10, height: 20 },
      { type: 'edit', ops: [{ type: 'rotateLeft' }, { type: 'rotateRight' }, { type: 'rotate180' }, { type: 'flipH' }, { type: 'flipV' }, { type: 'trim' }, { type: 'blur', sigma: 6 }], to: null },
      { type: 'edit', ops: [], to: null },
    ];
    for (const t of [en, tr]) {
      for (const op of ops) {
        const s = describeOp(op, t);
        expect(s, op.type).not.toMatch(/presetsPage\.|\{/);
        expect(s.length, op.type).toBeGreaterThan(0);
      }
    }
    expect(describeOp({ type: 'speed', factor: 0.5 }, en)).toContain('0.5');
  });
});

describe('describePreset', () => {
  it('adds quality and max size to the operation', () => {
    const jpg70: Preset = { id: 'to-jpg-70', name: '', builtin: true, op: { type: 'convert', to: 'jpg' }, options: { quality: 70 } };
    expect(describePreset(jpg70, en)).toBe('Convert to JPG · quality 70');
    const fit = mine({ type: 'edit', ops: [], to: null }, { maxWidth: 1920, maxHeight: 1920 });
    expect(describePreset(fit, en)).toContain('1920 px');
    expect(describePreset(mine({ type: 'convert', to: 'png' }, { maxWidth: 1280, maxHeight: 720 }), tr)).toContain('1280×720');
    // Options an action does not use are not mentioned.
    expect(describePreset(mine({ type: 'zip' }, { quality: 50 }), en)).toBe(describeOp({ type: 'zip' }, en));
  });
});

describe('withOp', () => {
  it('switches to the default operation of each action kind', () => {
    const p = mine({ type: 'convert', to: 'png' });
    expect(withOp(p, 'edit').op).toEqual({ type: 'edit', ops: [{ type: 'grayscale' }], to: null });
    expect(withOp(p, 'cropAspect').op).toEqual({ type: 'cropAspect', w: 16, h: 9 });
    expect(withOp(p, 'copyTo').op).toEqual({ type: 'copyTo', dir: null, moveFiles: false });
    expect(withOp(p, 'compress').op).toEqual({ type: 'compress', level: 'balanced' });
    expect(withOp(p, 'speed').op).toEqual({ type: 'speed', factor: 2 });
    expect(withOp(p, 'zip').op).toEqual({ type: 'zip' });
    for (const kind of ACTION_KINDS) expect(withOp(p, kind).op.type).toBe(kind);
  });

  it('keeps the preset when the kind does not change', () => {
    const p = mine({ type: 'convert', to: 'webp' }, { quality: 80 });
    expect(withOp(p, 'convert')).toBe(p);
  });

  it('drops options the new kind cannot use', () => {
    const p = mine({ type: 'convert', to: 'jpg' }, { quality: 70, maxWidth: 800, outputDir: 'C:\\out' });
    const edit = withOp(p, 'edit');
    expect(edit.options).toMatchObject({ quality: 70, maxWidth: 800, outputDir: 'C:\\out' });
    const zip = withOp(p, 'zip');
    expect(zip.options).toMatchObject({ quality: null, maxWidth: null, maxHeight: null, outputDir: 'C:\\out' });
    expect(withOp(p, 'copyTo').options.outputDir).toBeNull();
    expect(zip.id).toBe('u-1');
    expect(zip.name).toBe('Mine');
  });

  it('renames a preset whose name was the automatic one', () => {
    const auto = { ...mine({ type: 'convert', to: 'png' }), name: describeOp({ type: 'convert', to: 'png' }, en) };
    expect(withName(withOp(auto, 'zip'), auto, en).name).toBe(describeOp({ type: 'zip' }, en));
    const named = mine({ type: 'convert', to: 'png' });
    expect(withName(withOp(named, 'zip'), named, en).name).toBe('Mine');
  });
});

describe('duplicates, usage and numbers', () => {
  const builtin: Preset = { id: 'to-jpg-70', name: '', builtin: true, op: { type: 'convert', to: 'jpg' }, options: { quality: 70 } };

  it('duplicates a built-in into an editable u-<n> copy', () => {
    const copy = duplicatePreset(builtin, ['to-jpg-70', 'u-2'], en);
    expect(copy).toMatchObject({ id: 'u-3', builtin: false, op: { type: 'convert', to: 'jpg' }, options: { quality: 70 } });
    expect(copy.name).toContain('JPG 70%');
    copy.options.quality = 10;
    expect(builtin.options.quality).toBe(70);
  });

  it('counts where a preset is used', () => {
    const config: WheelConfig = {
      version: 1,
      presets: [builtin],
      groups: [{ id: 'g', name: '', presets: ['to-jpg-70'] }],
      profiles: [
        { id: 'a', name: '', builtin: true, extensions: [], mixed: false, front: ['to-jpg-70'], back: ['to-jpg-70'] },
        { id: 'b', name: '', builtin: true, extensions: [], mixed: false, front: [], back: [] },
      ],
    };
    expect(presetUsage(config, 'to-jpg-70')).toEqual({ profiles: 1, groups: 1 });
    expect(presetUsage(config, 'zip')).toEqual({ profiles: 0, groups: 0 });
  });

  it('has the same locale keys in both languages', () => {
    const keys = (o: object, prefix = ''): string[] =>
      Object.entries(o).flatMap(([k, v]) => (typeof v === 'object' ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`])).sort();
    expect(keys(trPage)).toEqual(keys(enPage));
    expect(keys(trWheel)).toEqual(keys(enWheel));
  });

  it('parses numbers within a range', () => {
    expect(parseNumber('12', 2, 256)).toBe(12);
    expect(parseNumber(' 0,5 ', 0.5, 50)).toBe(0.5);
    expect(parseNumber('1', 2, 256)).toBeNull();
    expect(parseNumber('', 1, 10)).toBeNull();
    expect(parseNumber('3.5', 1, 10, true)).toBeNull();
    expect(parseNumber('abc', 1, 10)).toBeNull();
  });
});
