import { describe, expect, it } from 'vitest';
import type { ClipItem, ClipKind } from '../lib/types';
import { cardText, fitCount, lineDefaults, matches, moveRope, overflow, ropeCards, ropeName, splitRopes } from './logic';

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
    // empty ropes are kept: the line shows every enabled rope
    expect(splitRopes([], ALL)).toHaveLength(3);
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
  });
});

describe('settings helpers', () => {
  it('rope_reorder', () => {
    expect(moveRope(['images', 'text', 'links', 'files'], 'files', -1)).toEqual(['images', 'text', 'files', 'links', 'shortcuts']);
    expect(moveRope(['images', 'text', 'links', 'files'], 'images', 1)).toEqual(['text', 'images', 'links', 'files', 'shortcuts']);
    expect(moveRope(['images', 'text', 'links', 'files'], 'images', -1)).toEqual(['images', 'text', 'links', 'files', 'shortcuts']);
    expect(moveRope(['links'], 'links', 1)).toEqual(['images', 'links', 'text', 'files', 'shortcuts']);
  });
  it('settings_reset_keeps_switch_and_hotkeys', () => {
    const d = { lineEnabled: true, lineSag: 14, lineColor: 'natural', lineLookVersion: 3, hotkeyLine: 'X', theme: 'dark' };
    expect(lineDefaults(d)).toEqual({ lineSag: 14, lineColor: 'natural' });
  });
});

describe('ropeCards', () => {
  it('colliding_file_ids_keep_every_card', () => {
    const f = (path: string) => ({ path, name: path, source: 'added' as const, at: 1, size: 1, kind: null, thumb: null });
    const rope = { key: 'files' as const, items: [], files: [f('a'), f('b'), f('c')], apps: [] };
    const cards = ropeCards(rope, () => -5);
    expect(cards).toHaveLength(3);
    expect(new Set(cards.map((c) => c.id)).size).toBe(3);
  });
});

describe('collapsed ropes', () => {
  it('toggles', async () => {
    const { toggleCollapsed } = await import('./logic');
    let keys = toggleCollapsed([], 'links');
    keys = toggleCollapsed(keys, 'images');
    expect(keys).toEqual(['links', 'images']);
    expect(toggleCollapsed(keys, 'links')).toEqual(['images']);
  });
});

describe('shortcuts rope', () => {
  it('hangs the apps in the user order with ids below every file id', async () => {
    const { APP_ID_BASE, splitRopes, ropeCards, ropeCount } = await import('./logic');
    const apps = [
      { name: 'Paint', path: 'C:\p.lnk', icon: null },
      { name: 'Notes', path: 'C:\n.exe', icon: 'C:\i.png' },
    ];
    const ropes = splitRopes([], { images: false, text: false, links: false, files: false, shortcuts: true }, undefined, [], apps);
    expect(ropes.map((r) => r.key)).toEqual(['shortcuts']);
    expect(ropeCount(ropes[0])).toBe(2);
    const cards = ropeCards(ropes[0], () => -1);
    expect(cards.map((c) => c.app?.name)).toEqual(['Paint', 'Notes']);
    expect(cards.map((c) => c.id)).toEqual([APP_ID_BASE, APP_ID_BASE - 1]);
    expect(cards.every((c) => c.id < -2147483647)).toBe(true);
  });
});

describe('card menu', () => {
  it('lists the entries a card gets', async () => {
    const { menuEntries, menuHeight } = await import('./menu');
    const noop = () => undefined;
    const a = { open: noop, copy: noop, paste: noop, edit: noop, pin: noop, share: noop, trash: noop, reveal: noop, copyPath: noop, convert: noop, float: noop, save: noop, removeShortcut: noop };
    const tt = (k: string) => k;
    const labels = (c: Parameters<typeof menuEntries>[0]) => menuEntries(c, a, tt).map((x) => (x === 'sep' ? '-' : x.label));
    const img = { id: 1, kind: 'image', pinned: false, image: 'C:/i.png', edited: null } as unknown as ClipItem;
    expect(labels({ id: 1, item: img, file: null })).toEqual(['menu.open', 'menu.copy', 'menu.paste', 'menu.edit', 'menu.pin', 'menu.share', 'menu.float', 'menu.save', '-', 'menu.delete']);
    const link = { id: 2, kind: 'link', pinned: true } as unknown as ClipItem;
    expect(labels({ id: 2, item: link, file: null })).toEqual(['menu.openLink', 'menu.copy', 'menu.paste', 'menu.unpin', 'menu.share', '-', 'menu.delete']);
    const app = { name: 'A', path: 'C:/a.lnk', icon: null };
    expect(labels({ id: -3, item: null, file: null, app })).toEqual(['menu.open', 'menu.reveal', 'menu.copyPath', '-', 'menu.removeShortcut']);
    expect(menuHeight(menuEntries({ id: -3, item: null, file: null, app }, a, tt))).toBe(26 + 8 + 4 * 30 + 9);
  });
});
