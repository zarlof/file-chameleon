import { describe, expect, it } from 'vitest';
import {
  arrowHead,
  clientToImage,
  commit,
  cropToRect,
  dragHandle,
  fitScale,
  hitHandle,
  history,
  normRect,
  redo,
  smoothPath,
  undo,
} from './canvas';

describe('normRect', () => {
  it('turns two corners into a rect with positive size', () => {
    expect(normRect({ x: 10, y: 20 }, { x: 4, y: 2 })).toEqual({ x: 4, y: 2, w: 6, h: 18 });
    expect(normRect({ x: 1, y: 1 }, { x: 5, y: 9 })).toEqual({ x: 1, y: 1, w: 4, h: 8 });
    expect(normRect({ x: 3, y: 3 }, { x: 3, y: 3 })).toEqual({ x: 3, y: 3, w: 0, h: 0 });
  });
});

describe('arrowHead', () => {
  it('returns the two barb ends, 30° off the shaft, behind the tip', () => {
    const [p1, p2] = arrowHead({ x: 0, y: 0 }, { x: 10, y: 0 }, 4);
    const back = 10 - 4 * Math.cos(Math.PI / 6);
    expect(p1.x).toBeCloseTo(back);
    expect(p2.x).toBeCloseTo(back);
    expect([p1.y, p2.y].sort((a, b) => a - b)).toEqual([-2, 2].map((v) => expect.closeTo(v)));
  });

  it('follows the direction of the shaft', () => {
    const [p1, p2] = arrowHead({ x: 0, y: 0 }, { x: 0, y: -10 }, 4);
    // Pointing up: barbs are below the tip.
    expect(p1.y).toBeCloseTo(-10 + 4 * Math.cos(Math.PI / 6));
    expect(p2.y).toBeCloseTo(-10 + 4 * Math.cos(Math.PI / 6));
    expect(Math.abs(p1.x)).toBeCloseTo(2);
  });

  it('never yields NaN for a zero-length arrow', () => {
    const [p1, p2] = arrowHead({ x: 5, y: 5 }, { x: 5, y: 5 }, 4);
    for (const v of [p1.x, p1.y, p2.x, p2.y]) expect(Number.isFinite(v)).toBe(true);
  });
});

describe('cropToRect', () => {
  it('clamps to the image and rounds to whole pixels', () => {
    expect(cropToRect(100, 50, { x: -5, y: 10.4, w: 60, h: 100 })).toEqual({ x: 0, y: 10, w: 55, h: 40 });
    expect(cropToRect(100, 50, { x: 10.6, y: 0.2, w: 20.2, h: 10.1 })).toEqual({ x: 11, y: 0, w: 20, h: 10 });
  });

  it('accepts negative sizes (dragged up/left)', () => {
    expect(cropToRect(100, 50, { x: 50, y: 40, w: -20, h: -30 })).toEqual({ x: 30, y: 10, w: 20, h: 30 });
  });

  it('is empty when the rect lies outside the image', () => {
    const r = cropToRect(100, 50, { x: 200, y: 10, w: 20, h: 20 });
    expect(r.w).toBe(0);
  });
});

describe('hitHandle', () => {
  const r = { x: 10, y: 10, w: 100, h: 50 };
  it('finds corners, then edges, then the inside', () => {
    expect(hitHandle(r, { x: 10, y: 10 }, 4)).toBe('nw');
    expect(hitHandle(r, { x: 112, y: 58 }, 4)).toBe('se');
    expect(hitHandle(r, { x: 110, y: 10 }, 4)).toBe('ne');
    expect(hitHandle(r, { x: 10, y: 60 }, 4)).toBe('sw');
    expect(hitHandle(r, { x: 60, y: 9 }, 4)).toBe('n');
    expect(hitHandle(r, { x: 60, y: 62 }, 4)).toBe('s');
    expect(hitHandle(r, { x: 111, y: 35 }, 4)).toBe('e');
    expect(hitHandle(r, { x: 12, y: 35 }, 4)).toBe('w');
    expect(hitHandle(r, { x: 60, y: 35 }, 4)).toBe('move');
  });

  it('misses outside the tolerance', () => {
    expect(hitHandle(r, { x: 200, y: 200 }, 4)).toBeNull();
    expect(hitHandle(r, { x: 60, y: 3 }, 4)).toBeNull();
  });
});

describe('dragHandle', () => {
  const r = { x: 10, y: 10, w: 100, h: 50 };
  it('moves one edge or corner', () => {
    expect(dragHandle(r, 'se', 5, -10)).toEqual({ x: 10, y: 10, w: 105, h: 40 });
    expect(dragHandle(r, 'n', 99, 5)).toEqual({ x: 10, y: 15, w: 100, h: 45 });
    expect(dragHandle(r, 'w', -10, 99)).toEqual({ x: 0, y: 10, w: 110, h: 50 });
  });

  it('moves the whole rect', () => {
    expect(dragHandle(r, 'move', 7, -3)).toEqual({ x: 17, y: 7, w: 100, h: 50 });
  });

  it('stays normalized when dragged past the opposite edge', () => {
    expect(dragHandle(r, 'nw', 150, 0)).toEqual({ x: 110, y: 10, w: 50, h: 50 });
  });
});

describe('view mapping', () => {
  it('maps a client point into image coordinates of the shown (cropped) area', () => {
    const box = { left: 100, top: 50, width: 200, height: 100 };
    const view = { x: 10, y: 20, w: 400, h: 200 };
    expect(clientToImage({ x: 150, y: 100 }, box, view)).toEqual({ x: 110, y: 120 });
    expect(clientToImage({ x: 100, y: 50 }, box, view)).toEqual({ x: 10, y: 20 });
  });

  it('fits without upscaling', () => {
    expect(fitScale(4000, 2000, 800, 600)).toBeCloseTo(0.2);
    expect(fitScale(100, 100, 800, 600)).toBe(1);
    expect(fitScale(0, 0, 800, 600)).toBe(1);
  });
});

describe('smoothPath', () => {
  it('keeps the end points and skips points closer than the step', () => {
    const pts = [{ x: 0, y: 0 }, { x: 0.2, y: 0 }, { x: 5, y: 0 }, { x: 5.1, y: 0 }, { x: 10, y: 0 }];
    expect(smoothPath(pts, 1)).toEqual([{ x: 0, y: 0 }, { x: 5, y: 0 }, { x: 10, y: 0 }]);
    expect(smoothPath([{ x: 1, y: 1 }], 1)).toEqual([{ x: 1, y: 1 }]);
  });
});

describe('history', () => {
  it('commits, undoes and redoes; a new commit drops the redo branch', () => {
    let h = history(0);
    h = commit(h, 1);
    h = commit(h, 2);
    expect(h.stack[h.index]).toBe(2);
    h = undo(h);
    expect(h.stack[h.index]).toBe(1);
    h = redo(h);
    expect(h.stack[h.index]).toBe(2);
    h = undo(undo(h));
    expect(h.stack[h.index]).toBe(0);
    expect(undo(h)).toBe(h);
    h = commit(h, 9);
    expect(h.stack).toEqual([0, 9]);
    expect(redo(h)).toBe(h);
  });

  it('keeps at most `limit` states', () => {
    let h = history(0);
    for (let i = 1; i <= 10; i++) h = commit(h, i, 4);
    expect(h.stack).toEqual([7, 8, 9, 10]);
    expect(h.index).toBe(3);
  });
});

describe('editor locale', () => {
  type Tree = { [k: string]: string | Tree };
  const files = import.meta.glob('../../../locales/*/editor.json', { eager: true, import: 'default' }) as Record<string, Tree>;
  const keys = (tree: Tree, prefix = ''): string[] =>
    Object.entries(tree).flatMap(([k, v]) => (typeof v === 'string' ? [prefix + k] : keys(v, `${prefix}${k}.`)));
  it('tr and en define the same keys', () => {
    const en = keys(files['../../../locales/en/editor.json']).sort();
    expect(en.length).toBeGreaterThan(20);
    expect(keys(files['../../../locales/tr/editor.json']).sort()).toEqual(en);
  });
});
