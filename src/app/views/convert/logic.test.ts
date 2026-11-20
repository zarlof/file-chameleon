import { describe, expect, it } from 'vitest';
import type { InputInfo, ToolStatus } from '../../../lib/types';
import {
  buildOptions, buildRequest, buildToolOp, formatTime, groupTargets, missingTools, opNeeds, parseTime, pendingStatus, type ToolParams,
} from './logic';

describe('parseTime', () => {
  it('parses seconds, m:ss and h:mm:ss', () => {
    expect(parseTime('90')).toBe(90_000);
    expect(parseTime('1:30')).toBe(90_000);
    expect(parseTime(' 0:05.5 ')).toBe(5_500);
    expect(parseTime('1:02:03')).toBe(3_723_000);
  });
  it('rejects garbage', () => {
    for (const s of ['', 'abc', '1:75', '1:2:3:4', '-5', '1::2', '1.5:30', '0:60']) expect(parseTime(s), s).toBeNull();
  });
  it('formats back', () => {
    expect(formatTime(90_000)).toBe('1:30');
    expect(formatTime(3_723_000)).toBe('1:02:03');
    expect(formatTime(0)).toBe('0:00');
  });
});

const base: ToolParams = { level: 'balanced', start: '0:00', end: '', speed: 1.5, x: '0', y: '0', width: '', height: '' };

describe('buildToolOp', () => {
  it('requires end after start for trim', () => {
    expect(buildToolOp('trim', base)).toBeNull();
    expect(buildToolOp('trim', { ...base, end: '0:00' })).toBeNull();
    expect(buildToolOp('trim', { ...base, start: '0:05', end: '0:10' })).toEqual({ type: 'trim', startMs: 5000, endMs: 10000 });
  });
  it('validates crop numbers', () => {
    expect(buildToolOp('crop', base)).toBeNull();
    expect(buildToolOp('crop', { ...base, x: '10', y: '20', width: '300', height: '200' })).toEqual({
      type: 'crop', x: 10, y: 20, width: 300, height: 200,
    });
    expect(buildToolOp('crop', { ...base, width: '0', height: '5' })).toBeNull();
  });
  it('builds the simple ops', () => {
    expect(buildToolOp('compress', base)).toEqual({ type: 'compress', level: 'balanced' });
    expect(buildToolOp('speed', { ...base, speed: 2 })).toEqual({ type: 'speed', factor: 2 });
    expect(buildToolOp('mute', base)).toEqual({ type: 'mute' });
    expect(buildToolOp('mergePdf', base)).toEqual({ type: 'mergePdf' });
    expect(buildToolOp('splitPdf', base)).toEqual({ type: 'splitPdf' });
  });
});

describe('options and requests', () => {
  it('drops quality when unused and ignores bad dimensions', () => {
    expect(buildOptions({ useQuality: false, quality: 80, maxWidth: '1920', maxHeight: 'x', outputDir: null })).toEqual({
      quality: null, maxWidth: 1920, maxHeight: null, outputDir: null,
    });
    const o = buildOptions({ useQuality: true, quality: 80, maxWidth: '', maxHeight: '0', outputDir: 'C:\\out' });
    expect(o).toEqual({ quality: 80, maxWidth: null, maxHeight: null, outputDir: 'C:\\out' });
  });
  it('builds a request', () => {
    expect(buildRequest(['a.png'], { type: 'convert', to: 'jpg' }, {})).toEqual({
      inputs: ['a.png'], op: { type: 'convert', to: 'jpg' }, options: {},
    });
  });
});

describe('targets and tools', () => {
  it('groups by kind in order', () => {
    const g = groupTargets([
      { id: 'png', label: 'PNG', kind: 'image', needsTools: [] },
      { id: 'mp4', label: 'MP4', kind: 'video', needsTools: ['ffmpeg'] },
      { id: 'jpg', label: 'JPG', kind: 'image', needsTools: [] },
    ]);
    expect(g.map((x) => [x.kind, x.targets.map((t) => t.id)])).toEqual([['image', ['png', 'jpg']], ['video', ['mp4']]]);
  });
  it('finds missing tools', () => {
    const st = [{ id: 'ffmpeg', source: 'missing' }, { id: 'pdfium', source: 'managed' }] as ToolStatus[];
    expect(missingTools(['ffmpeg', 'pdfium'], st)).toEqual(['ffmpeg']);
    expect(missingTools(['ffmpeg'], [])).toEqual(['ffmpeg']);
    expect(missingTools([], st)).toEqual([]);
  });
  it('knows which ops need ffmpeg', () => {
    const img = [{ kind: 'image' }] as InputInfo[];
    const vid = [{ kind: 'video' }] as InputInfo[];
    expect(opNeeds('trim', img)).toEqual(['ffmpeg']);
    expect(opNeeds('compress', img)).toEqual([]);
    expect(opNeeds('compress', vid)).toEqual(['ffmpeg']);
    expect(opNeeds('mergePdf', img)).toEqual([]);
  });
});

describe('pendingStatus', () => {
  const mk = (installed: string[], installing: string[], errors: Record<string, string>) => ({
    installed: (id: string) => installed.includes(id),
    installing: (id: string) => installing.includes(id),
    error: (id: string) => errors[id],
  });
  it('waits while downloading and before the refresh lands', () => {
    expect(pendingStatus(['ffmpeg'], mk([], ['ffmpeg'], {}))).toEqual({ state: 'wait' });
    expect(pendingStatus(['ffmpeg'], mk([], [], {}))).toEqual({ state: 'wait' });
  });
  it('is ready once installed', () => {
    expect(pendingStatus(['ffmpeg'], mk(['ffmpeg'], [], {}))).toEqual({ state: 'ready' });
  });
  it('reports install errors', () => {
    expect(pendingStatus(['ffmpeg'], mk([], [], { ffmpeg: 'boom' }))).toEqual({ state: 'error', id: 'ffmpeg', message: 'boom' });
  });
});
