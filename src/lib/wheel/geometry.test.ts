import { describe, expect, it } from 'vitest';
import { INNER_R, OUTER_R, WHEEL_SIZE, orderSlices, polar, sliceAt, toLogical } from './geometry';
import type { TargetOption } from '../types';

const C = WHEEL_SIZE / 2;
const mk = (n: number): TargetOption[] =>
  Array.from({ length: n }, (_, i) => ({ id: `t${i}`, label: `T${i}`, kind: 'image', needsTools: [] }));

describe('sliceAt', () => {
  it('centres slice 0 at the top and goes clockwise', () => {
    const r = 130;
    expect(sliceAt(C, C - r, 4)).toBe(0);
    expect(sliceAt(C + r, C, 4)).toBe(1);
    expect(sliceAt(C, C + r, 4)).toBe(2);
    expect(sliceAt(C - r, C, 4)).toBe(3);
  });
  it('wraps around just left of top into the last slice', () => {
    const p = polar(-0.05, 130);
    expect(sliceAt(p.x, p.y, 8)).toBe(0);
    const q = polar(-Math.PI / 8 - 0.02, 130);
    expect(sliceAt(q.x, q.y, 8)).toBe(7);
    const s = polar(Math.PI / 8 + 0.02, 130);
    expect(sliceAt(s.x, s.y, 8)).toBe(1);
  });
  it('has a dead zone in the centre and nothing outside the ring', () => {
    expect(sliceAt(C, C, 5)).toBe(-1);
    expect(sliceAt(C, C - (INNER_R - 1), 5)).toBe(-1);
    expect(sliceAt(C, C - (INNER_R + 1), 5)).toBe(0);
    expect(sliceAt(C, C - (OUTER_R + 1), 5)).toBe(-1);
    expect(sliceAt(0, 0, 5)).toBe(-1);
  });
  it('returns -1 without slices', () => {
    expect(sliceAt(C, C - 100, 0)).toBe(-1);
  });
  it('handles a single slice', () => {
    expect(sliceAt(C + 100, C, 1)).toBe(0);
    expect(sliceAt(C - 100, C, 1)).toBe(0);
  });
});

describe('toLogical', () => {
  it('divides by the device pixel ratio', () => {
    expect(toLogical(330, 1.5)).toBe(220);
    expect(toLogical(100, 1)).toBe(100);
    expect(toLogical(100, 0)).toBe(100);
  });
});

describe('orderSlices', () => {
  it('puts recent first, ignoring unknown ids, then the rest in order', () => {
    const out = orderSlices(mk(5), ['t3', 'nope', 't1']);
    expect(out.map((t) => t.id)).toEqual(['t3', 't1', 't0', 't2', 't4']);
  });
  it('keeps exactly 8 as is', () => {
    const out = orderSlices(mk(8), []);
    expect(out).toHaveLength(8);
    expect(out.some((t) => t.id === 'more')).toBe(false);
  });
  it('cuts to 7 + More when above 8', () => {
    const out = orderSlices(mk(12), ['t9']);
    expect(out).toHaveLength(8);
    expect(out[7].id).toBe('more');
    expect(out.slice(0, 7).map((t) => t.id)).toEqual(['t9', 't0', 't1', 't2', 't3', 't4', 't5']);
  });
  it('returns empty for no targets', () => {
    expect(orderSlices([], ['x'])).toEqual([]);
  });
  it('dedupes repeated recent ids', () => {
    expect(orderSlices(mk(3), ['t1', 't1']).map((t) => t.id)).toEqual(['t1', 't0', 't2']);
  });
});
