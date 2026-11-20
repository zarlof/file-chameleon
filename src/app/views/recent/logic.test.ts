import { describe, expect, it } from 'vitest';
import type { JobSnapshot } from '../../../lib/types';
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
});
