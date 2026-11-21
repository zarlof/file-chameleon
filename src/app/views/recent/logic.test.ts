import { describe, expect, it } from 'vitest';
import { translate } from '../../../lib/i18n';
import type { JobSnapshot, Kind, Operation } from '../../../lib/types';
import { baseName, errorKey, groupJobs, jobTitle, opLabel } from './logic';

const job = (id: number, createdAt: number): JobSnapshot => ({
  id, inputs: [], op: { type: 'mute' }, state: 'done', progress: 1, outputs: [], errorCode: null, error: null, createdAt, finishedAt: null,
});

describe('recent logic', () => {
  it('groups Today / Earlier newest first', () => {
    const now = new Date(2026, 10, 20, 15, 0).getTime();
    const today = new Date(2026, 10, 20, 0, 5).getTime();
    const yesterday = new Date(2026, 10, 19, 23, 59).getTime();
    const g = groupJobs([job(1, yesterday), job(2, today), job(3, now - 1000)], now);
    expect(g.today.map((j) => j.id)).toEqual([3, 2]);
    expect(g.earlier.map((j) => j.id)).toEqual([1]);
  });
  it('maps error codes with a generic fallback', () => {
    expect(errorKey('missingTool')).toBe('recent.errors.missingTool');
    expect(errorKey('weird')).toBe('recent.errors.failed');
    expect(errorKey(null)).toBe('recent.errors.failed');
  });
  it('titles jobs', () => {
    expect(baseName('C:\\a\\b\\pic.png')).toBe('pic.png');
    expect(baseName('/x/y.mp4')).toBe('y.mp4');
    expect(jobTitle(['C:\\a\\1.png'])).toBe('1.png');
    expect(jobTitle(['a/1.png', 'a/2.png', 'a/3.png'])).toBe('1.png +2');
  });
  it('labels operations', () => {
    expect(opLabel({ type: 'convert', to: 'png' })).toEqual({ key: 'recent.op.convert', params: { to: 'PNG' } });
    expect(opLabel({ type: 'trim', startMs: 0, endMs: 1 }).key).toBe('recent.op.trim');
  });
  it('has a translated label for every operation type, in both languages', () => {
    const ops: Record<Operation['type'], Operation> = {
      convert: { type: 'convert', to: 'png' },
      edit: { type: 'edit', ops: [], to: null },
      cropAspect: { type: 'cropAspect', w: 1, h: 1 },
      copyTo: { type: 'copyTo', dir: null, moveFiles: false },
      zip: { type: 'zip' },
      extract: { type: 'extract' },
      annotate: { type: 'annotate' },
      copyPath: { type: 'copyPath' },
      openTerminal: { type: 'openTerminal' },
      float: { type: 'float' },
      mergePdf: { type: 'mergePdf' },
      splitPdf: { type: 'splitPdf' },
      compress: { type: 'compress', level: 'light' },
      trim: { type: 'trim', startMs: 0, endMs: 1 },
      mute: { type: 'mute' },
      speed: { type: 'speed', factor: 2 },
      crop: { type: 'crop', x: 0, y: 0, width: 1, height: 1 },
    };
    const all = [...Object.values(ops), { type: 'copyTo', dir: 'D:/x', moveFiles: true } as Operation];
    for (const lang of ['tr', 'en'] as const) {
      for (const op of all) {
        const { key, params } = opLabel(op);
        expect(translate(lang, key, params), `${lang} ${key}`).not.toBe(key);
      }
    }
    expect(opLabel({ type: 'copyTo', dir: null, moveFiles: true }).key).toBe('recent.op.move');
    expect(opLabel({ type: 'copyTo', dir: null, moveFiles: false }).key).toBe('recent.op.copy');
  });
  it('has a convert.kind text for every Kind, in both languages', () => {
    const kinds: Record<Kind, true> = { image: true, vector: true, video: true, audio: true, pdf: true, data: true, subtitle: true, archive: true };
    for (const lang of ['tr', 'en'] as const) {
      for (const k of Object.keys(kinds)) {
        expect(translate(lang, `convert.kind.${k}`), `${lang} ${k}`).not.toBe(`convert.kind.${k}`);
      }
    }
  });
  it('localizes the invalid error code', () => {
    expect(errorKey('invalid')).toBe('recent.errors.invalid');
  });
});
