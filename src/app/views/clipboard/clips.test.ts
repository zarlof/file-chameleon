import { describe, expect, it } from 'vitest';
import type { ClipItem } from '../../../lib/types';
import { baseName, matchesFilter, ordered, relativeTime, ROW_H, rowWindow, scrollToReveal, withAdded } from './clips';

function item(id: number, kind: ClipItem['kind'], pinned = false): ClipItem {
  return { id, kind, text: null, image: null, thumb: null, width: null, height: null, site: null, hash: String(id), createdAt: id, pinned };
}

describe('clipboard helpers', () => {
  it('filters by kind', () => {
    const all = [item(1, 'text'), item(2, 'link'), item(3, 'mediaLink'), item(4, 'image')];
    const ids = (f: Parameters<typeof matchesFilter>[1]) => all.filter((i) => matchesFilter(i, f)).map((i) => i.id);
    expect(ids('all')).toEqual([1, 2, 3, 4]);
    expect(ids('links')).toEqual([2, 3]);
    expect(ids('text')).toEqual([1]);
    expect(ids('images')).toEqual([4]);
  });

  it('puts pinned items first and keeps order otherwise', () => {
    const list = [item(5, 'text'), item(4, 'text', true), item(3, 'text'), item(2, 'text', true)];
    expect(ordered(list, 'all').map((i) => i.id)).toEqual([4, 2, 5, 3]);
  });

  it('moves a re-added item to the top without duplicating', () => {
    const list = [item(2, 'text'), item(1, 'text')];
    expect(withAdded(list, item(1, 'text')).map((i) => i.id)).toEqual([1, 2]);
    expect(withAdded(list, item(3, 'text')).map((i) => i.id)).toEqual([3, 2, 1]);
  });

  it('takes the last path segment', () => {
    expect(baseName('C:\\Users\\a\\Downloads\\Clipboard 1.png')).toBe('Clipboard 1.png');
    expect(baseName('/x/y.png')).toBe('y.png');
  });

  it('formats relative time', () => {
    const now = 10_000_000;
    expect(relativeTime(now - 20_000, now, 'en', 'just now')).toBe('just now');
    expect(relativeTime(now - 5 * 60_000, now, 'en', 'x')).toBe('5 minutes ago');
    expect(relativeTime(now - 3 * 3_600_000, now, 'en', 'x')).toBe('3 hours ago');
    expect(relativeTime(now - 2 * 86_400_000, now, 'en', 'x')).toBe('2 days ago');
  });

  it('renders only a window of rows, with spacers for the rest', () => {
    const total = 10_000;
    const w = rowWindow(50_000, 600, total);
    expect(w.end - w.start).toBeLessThanOrEqual(60);
    expect(w.start).toBe(Math.floor(50_000 / ROW_H) - 10);
    expect(w.top + (w.end - w.start) * ROW_H + w.bottom).toBe(total * ROW_H);
    const top = rowWindow(0, 600, total);
    expect([top.start, top.top]).toEqual([0, 0]);
    const tail = rowWindow(total * ROW_H, 600, total);
    expect([tail.end, tail.bottom]).toEqual([total, 0]);
    expect(rowWindow(0, 600, 0)).toEqual({ start: 0, end: 0, top: 0, bottom: 0 });
    const few = rowWindow(0, 600, 5);
    expect([few.start, few.end]).toEqual([0, 5]);
  });

  it('scrolls the minimum distance to reveal a row', () => {
    expect(scrollToReveal(0, 400, 1)).toBe(0);
    expect(scrollToReveal(0, 400, 10)).toBe(10 * ROW_H + ROW_H - 400);
    expect(scrollToReveal(5000, 400, 3)).toBe(3 * ROW_H);
  });
});
