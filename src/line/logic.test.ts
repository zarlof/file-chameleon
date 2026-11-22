import { describe, expect, it } from 'vitest';
import type { ClipItem, ClipKind } from '../lib/types';
import { cardText, fitCount, lineDefaults, matches, moveRope, overflow, ropeCards, ropeName, splitRopes, visibleRopes } from './logic';

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

describe('rope order and names', () => {
  it('rope_order_and_names', () => {
    const cfg = { images: true, text: true, links: false, files: true };
    const ropes = splitRopes([item('image'), item('text')], cfg, ['files', 'text', 'images', 'links']);
    expect(ropes.map((r) => r.key)).toEqual(['files', 'text', 'images']);
    expect(splitRopes([], cfg).map((r) => r.key)).toEqual(['images', 'text', 'files']);
    // missing keys in a stored order are appended
    expect(splitRopes([], { ...cfg, links: true }, ['links']).map((r) => r.key)).toEqual(['links', 'images', 'text', 'files']);
    const names = { images: 'Pics', text: '', links: '  ', files: '' };
    expect(ropeName('images', names, 'Images')).toBe('Pics');
    expect(ropeName('text', names, 'Texts')).toBe('Texts');
    expect(ropeName('links', names, 'Links')).toBe('Links');
  });
  it('files_rope_is_fed_from_files', () => {
    const f = { path: 'p', name: 'p', source: 'added' as const, at: 1, size: 1, kind: null, thumb: null };
    const ropes = splitRopes([], { images: false, text: false, links: false, files: true }, undefined, [f]);
    expect(ropes).toHaveLength(1);
    expect(ropes[0].files).toEqual([f]);
    expect(visibleRopes(ropes)).toHaveLength(1);
  });
});

describe('settings helpers', () => {
  it('rope_reorder', () => {
    expect(moveRope(['images', 'text', 'links', 'files'], 'files', -1)).toEqual(['images', 'text', 'files', 'links']);
    expect(moveRope(['images', 'text', 'links', 'files'], 'images', 1)).toEqual(['text', 'images', 'links', 'files']);
    expect(moveRope(['images', 'text', 'links', 'files'], 'images', -1)).toEqual(['images', 'text', 'links', 'files']);
    expect(moveRope(['links'], 'links', 1)).toEqual(['images', 'links', 'text', 'files']);
  });
  it('settings_reset_keeps_switch_and_hotkeys', () => {
    const d = { lineEnabled: true, lineSag: 14, lineColor: 'natural', lineLookVersion: 3, hotkeyLine: 'X', theme: 'dark' };
    expect(lineDefaults(d)).toEqual({ lineSag: 14, lineColor: 'natural' });
  });
});

describe('ropeCards', () => {
  it('colliding_file_ids_keep_every_card', () => {
    const f = (path: string) => ({ path, name: path, source: 'added' as const, at: 1, size: 1, kind: null, thumb: null });
    const rope = { key: 'files' as const, items: [], files: [f('a'), f('b'), f('c')] };
    const cards = ropeCards(rope, () => -5);
    expect(cards).toHaveLength(3);
    expect(new Set(cards.map((c) => c.id)).size).toBe(3);
  });
});

describe('collapsed ropes', () => {
  it('toggles, saves and loads safely', async () => {
    const { loadCollapsed, saveCollapsed, toggleCollapsed } = await import('./logic');
    const mem = new Map<string, string>();
    const store = { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v) };
    expect(loadCollapsed(store)).toEqual([]);
    let keys = toggleCollapsed([], 'links');
    keys = toggleCollapsed(keys, 'images');
    expect(keys).toEqual(['links', 'images']);
    saveCollapsed(store, keys);
    expect(loadCollapsed(store).sort()).toEqual(['images', 'links']);
    expect(toggleCollapsed(keys, 'links')).toEqual(['images']);
    mem.set('fc.line.collapsed', '{bad');
    expect(loadCollapsed(store)).toEqual([]);
    mem.set('fc.line.collapsed', '["nope","files"]');
    expect(loadCollapsed(store)).toEqual(['files']);
  });
});
