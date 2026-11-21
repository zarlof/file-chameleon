import { describe, expect, it } from 'vitest';
import type { ClipItem, ClipKind } from '../lib/types';
import { cardText, fitCount, matches, overflow, splitRopes, visibleRopes } from './logic';

let n = 0;
function item(kind: ClipKind, over: Partial<ClipItem> = {}): ClipItem {
  n++;
  return {
    id: n, kind, text: kind === 'image' ? null : `t${n}`, image: null, thumb: null, width: null, height: null, site: null,
    hash: `h${n}`, createdAt: n, pinned: false, title: null, note: null, tags: [], folder: null, ocr: null, trashedAt: null,
    sourceApp: null, linkTitle: null, edited: null, editedThumb: null, saved: false, size: 0, ...over,
  };
}
const ALL = { images: true, text: true, links: true };

describe('splitRopes', () => {
  it('splits_and_hides_empty_or_disabled_ropes', () => {
    const items = [item('image'), item('text'), item('link')];
    const two = splitRopes(items, { ...ALL, links: false });
    expect(two.map((r) => r.key)).toEqual(['images', 'text']);
    const textOnly = splitRopes([item('text'), item('text')], ALL);
    expect(textOnly.map((r) => r.key)).toEqual(['images', 'text', 'links']);
    expect(textOnly[0].items).toEqual([]);
    expect(visibleRopes(textOnly).map((r) => r.key)).toEqual(['text']);
    expect(visibleRopes(splitRopes([], ALL))).toEqual([]);
  });

  it('pinned_first_newest_first', () => {
    const a = item('text', { createdAt: 10 });
    const b = item('text', { createdAt: 30 });
    const c = item('text', { createdAt: 20, pinned: true });
    const d = item('text', { createdAt: 5, pinned: true });
    const [, text] = splitRopes([a, b, c, d], ALL);
    expect(text.items.map((i) => i.id)).toEqual([c.id, d.id, b.id, a.id]);
  });

  it('mediaLink_goes_to_links', () => {
    const m = item('mediaLink');
    const l = item('link');
    const [, , links] = splitRopes([m, l], ALL);
    expect(links.items.map((i) => i.id).sort()).toEqual([m.id, l.id].sort());
  });

  it('excludes_trashed', () => {
    const [, text] = splitRopes([item('text', { trashedAt: 5 }), item('text')], ALL);
    expect(text.items).toHaveLength(1);
  });
});

describe('fit', () => {
  it('fit_count_and_overflow', () => {
    expect(fitCount(1872)).toBe(14);
    expect(fitCount(10)).toBe(1);
    expect(overflow(20, 14)).toBe(6);
    expect(overflow(3, 14)).toBe(0);
  });
});

describe('cardText', () => {
  it('card_text_clamps', () => {
    const text = Array.from({ length: 10 }, (_, i) => `line ${i + 1}`).join('\n');
    const c = cardText(item('text', { text }));
    expect(c.title).toBe('line 1');
    expect(c.lines).toEqual(['line 2', 'line 3', 'line 4…']);
  });

  it('uses_title_and_keeps_short_text_whole', () => {
    const c = cardText(item('text', { title: 'Heading', text: 'a\nb' }));
    expect(c.title).toBe('Heading');
    expect(c.lines).toEqual(['a', 'b']);
  });

  it('link_shows_host', () => {
    const c = cardText(item('link', { text: 'https://www.example.com/a/b?x=1', linkTitle: 'Example' }));
    expect(c.title).toBe('Example');
    expect(c.lines).toEqual(['example.com/a/b?x=1']);
  });
});

describe('matches', () => {
  it('search_matches', () => {
    expect(matches(item('text', { text: 'Şirket raporu' }), 'sirket')).toBe(true);
    expect(matches(item('image', { ocr: 'INVOICE 42' }), 'invoice')).toBe(true);
    expect(matches(item('text', { tags: ['iş'] }), 'is')).toBe(true);
    expect(matches(item('text', { text: 'abc' }), '')).toBe(true);
    expect(matches(item('text', { text: 'abc' }), 'xyz')).toBe(false);
    expect(matches(item('link', { linkTitle: 'İstanbul Rehberi' }), 'istanbul')).toBe(true);
    expect(matches(item('text', { text: 'ILIK' }), 'ılık')).toBe(true);
  });
});
