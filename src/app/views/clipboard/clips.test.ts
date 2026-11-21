import { describe, expect, it } from 'vitest';
import type { ClipFolder, ClipItem } from '../../../lib/types';
import {
  acceptsAdded,
  addFolderPath,
  arrange,
  baseName,
  clipLabel,
  EMPTY_SELECTION,
  filterKey,
  fold,
  folderTree,
  gridColumns,
  linkHost,
  moveShelfRow,
  pruneSelection,
  pushUndo,
  queryFor,
  relativeTime,
  ROW_H,
  rowWindow,
  scrollToReveal,
  selectAll,
  selectionAfterClick,
  selectionAfterMove,
  shelfOf,
  shelfRowList,
  tagState,
  toggleShelfRow,
  UNDO_LIMIT,
  withAdded,
} from './clips';

function item(id: number, kind: ClipItem['kind'], extra: Partial<ClipItem> = {}): ClipItem {
  return { id, kind, text: null, image: null, thumb: null, width: null, height: null, site: null, hash: String(id), createdAt: id, pinned: false,
    title: null, note: null, tags: [], folder: null, ocr: null, trashedAt: null, sourceApp: null, linkTitle: null,
    edited: null, editedThumb: null, saved: false, size: 0, ...extra };
}

const ids = (list: ClipItem[]) => list.map((i) => i.id);

describe('filters and queries', () => {
  it('builds the backend query for each filter, trimming the search', () => {
    expect(queryFor({ type: 'all' }, '  cat ')).toEqual({ text: 'cat' });
    expect(queryFor({ type: 'shelf', shelf: 'images' }, '')).toEqual({ text: null, shelf: 'images' });
    expect(queryFor({ type: 'pinned' }, '')).toEqual({ text: null, pinned: true });
    expect(queryFor({ type: 'folder', id: 3 }, 'x')).toEqual({ text: 'x', folder: 3 });
    expect(queryFor({ type: 'tag', tag: 'iş' }, '')).toEqual({ text: null, tag: 'iş' });
    expect(queryFor({ type: 'trash' }, '')).toEqual({ text: null, trash: true });
  });

  it('gives every filter a stable key', () => {
    expect(filterKey({ type: 'folder', id: 3 })).toBe('folder:3');
    expect(filterKey({ type: 'tag', tag: 'A' })).toBe('tag:A');
    expect(filterKey({ type: 'shelf', shelf: 'links' })).toBe('shelf:links');
    expect(filterKey({ type: 'trash' })).toBe('trash');
  });

  it('maps kinds to shelf rows', () => {
    expect(shelfOf('image')).toBe('images');
    expect(shelfOf('text')).toBe('texts');
    expect(shelfOf('link')).toBe('links');
    expect(shelfOf('mediaLink')).toBe('links');
  });

  it('puts pinned items first, newest first inside each group', () => {
    const list = [item(5, 'text'), item(4, 'text', { pinned: true }), item(3, 'text'), item(2, 'text', { pinned: true })];
    expect(ids(arrange(list, { type: 'all' }))).toEqual([4, 2, 5, 3]);
  });

  it('orders the trash by when items were trashed', () => {
    const list = [item(1, 'text', { trashedAt: 10 }), item(2, 'text', { trashedAt: 30 }), item(3, 'text', { trashedAt: 20 })];
    expect(ids(arrange(list, { type: 'trash' }))).toEqual([2, 3, 1]);
  });

  it('shows a new copy in place only when it surely matches the filter', () => {
    const link = item(9, 'mediaLink');
    expect(acceptsAdded({ type: 'all' }, '', link)).toBe(true);
    expect(acceptsAdded({ type: 'shelf', shelf: 'links' }, '', link)).toBe(true);
    expect(acceptsAdded({ type: 'shelf', shelf: 'images' }, '', link)).toBe(false);
    expect(acceptsAdded({ type: 'all' }, 'abc', link)).toBe(false);
    expect(acceptsAdded({ type: 'trash' }, '', link)).toBe(false);
    expect(acceptsAdded({ type: 'pinned' }, '', link)).toBe(false);
    expect(acceptsAdded({ type: 'pinned' }, '', item(9, 'text', { pinned: true }))).toBe(true);
  });

  it('moves a re-added item to the top without duplicating', () => {
    const list = [item(2, 'text'), item(1, 'text')];
    expect(ids(withAdded(list, item(1, 'text')))).toEqual([1, 2]);
    expect(ids(withAdded(list, item(3, 'text')))).toEqual([3, 2, 1]);
  });
});

describe('selection', () => {
  const order = [10, 11, 12, 13, 14];

  it('selects one item on a plain click', () => {
    expect(selectionAfterClick(order, { ids: [10, 11], anchor: 10 }, 13, {})).toEqual({ ids: [13], anchor: 13 });
  });

  it('toggles with Ctrl and moves the anchor', () => {
    const s = selectionAfterClick(order, { ids: [10], anchor: 10 }, 12, { ctrl: true });
    expect(s).toEqual({ ids: [10, 12], anchor: 12 });
    expect(selectionAfterClick(order, s, 10, { ctrl: true })).toEqual({ ids: [12], anchor: 10 });
  });

  it('selects a range from the anchor with Shift, in display order', () => {
    expect(selectionAfterClick(order, { ids: [11], anchor: 11 }, 13, { shift: true })).toEqual({ ids: [11, 12, 13], anchor: 11 });
    expect(selectionAfterClick(order, { ids: [13], anchor: 13 }, 10, { shift: true })).toEqual({ ids: [10, 11, 12, 13], anchor: 13 });
  });

  it('adds the range to the selection with Ctrl+Shift', () => {
    expect(selectionAfterClick(order, { ids: [10, 14], anchor: 12 }, 13, { shift: true, ctrl: true })).toEqual({ ids: [10, 12, 13, 14], anchor: 12 });
  });

  it('treats Shift without a usable anchor as a plain click', () => {
    expect(selectionAfterClick(order, EMPTY_SELECTION, 12, { shift: true })).toEqual({ ids: [12], anchor: 12 });
    expect(selectionAfterClick(order, { ids: [99], anchor: 99 }, 12, { shift: true })).toEqual({ ids: [12], anchor: 12 });
  });

  it('moves the focus with the arrows, clamped, extending with Shift', () => {
    expect(selectionAfterMove(order, { ids: [11], anchor: 11 }, 11, 1, false)).toEqual({ selection: { ids: [12], anchor: 12 }, focus: 12 });
    expect(selectionAfterMove(order, { ids: [14], anchor: 14 }, 14, 1, false).focus).toBe(14);
    expect(selectionAfterMove(order, { ids: [10], anchor: 10 }, 10, -3, false).focus).toBe(10);
    expect(selectionAfterMove(order, { ids: [11], anchor: 11 }, 11, 2, true)).toEqual({ selection: { ids: [11, 12, 13], anchor: 11 }, focus: 13 });
    expect(selectionAfterMove(order, EMPTY_SELECTION, null, 1, false)).toEqual({ selection: { ids: [10], anchor: 10 }, focus: 10 });
    expect(selectionAfterMove([], EMPTY_SELECTION, null, 1, false)).toEqual({ selection: EMPTY_SELECTION, focus: null });
  });

  it('selects all and drops ids that are no longer shown', () => {
    expect(selectAll(order)).toEqual({ ids: order, anchor: 10 });
    expect(pruneSelection([12, 10], { ids: [10, 11, 12], anchor: 11 })).toEqual({ ids: [12, 10], anchor: null });
    expect(pruneSelection([12, 10], { ids: [10], anchor: 10 })).toEqual({ ids: [10], anchor: 10 });
  });
});

describe('undo stack', () => {
  it(`keeps the last ${UNDO_LIMIT} trashed batches`, () => {
    let stack: number[][] = [];
    for (let i = 1; i <= 40; i++) stack = pushUndo(stack, [i]);
    expect(stack).toHaveLength(UNDO_LIMIT);
    expect(stack[0]).toEqual([9]);
    expect(stack[stack.length - 1]).toEqual([40]);
  });

  it('ignores empty batches', () => {
    const stack = [[1]];
    expect(pushUndo(stack, [])).toBe(stack);
  });
});

describe('layout', () => {
  it('fits as many card columns as the width allows', () => {
    expect(gridColumns(800, 168, 12)).toBe(4);
    expect(gridColumns(100, 168, 12)).toBe(1);
    expect(gridColumns(0, 168, 12)).toBe(1);
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

describe('folders and tags', () => {
  const f = (id: number, name: string, parent: number | null = null): ClipFolder => ({ id, name, parent, count: 0 });

  it('flattens folders depth-first, children under their parent', () => {
    const tree = folderTree([f(1, 'A'), f(2, 'B'), f(3, 'A1', 1), f(4, 'A1a', 3), f(5, 'Orphan', 99)]);
    expect(tree.map((n) => [n.folder.id, n.depth])).toEqual([[1, 0], [3, 1], [4, 2], [2, 0], [5, 0]]);
  });

  it('survives a parent cycle', () => {
    const tree = folderTree([f(1, 'A', 2), f(2, 'B', 1)]);
    expect(tree.map((n) => n.folder.id).sort()).toEqual([1, 2]);
  });

  it('folds Turkish i like the backend', () => {
    expect(fold('İSTANBUL ıspanak')).toBe('istanbul ispanak');
  });

  it('tells whether all, some or none of the items carry a tag', () => {
    const list = [item(1, 'text', { tags: ['İş'] }), item(2, 'text', { tags: ['iş', 'x'] })];
    expect(tagState(list, 'iş')).toBe('all');
    expect(tagState(list, 'x')).toBe('some');
    expect(tagState(list, 'y')).toBe('none');
    expect(tagState([], 'x')).toBe('none');
  });
});

describe('labels', () => {
  it('prefers the title, then the link title, then the first text line', () => {
    expect(clipLabel(item(1, 'text', { title: 'T', text: 'body' }))).toBe('T');
    expect(clipLabel(item(1, 'link', { linkTitle: 'Page', text: 'https://a.com' }))).toBe('Page');
    expect(clipLabel(item(1, 'text', { linkTitle: 'ignored', text: '\n  first line \nsecond' }))).toBe('first line');
    expect(clipLabel(item(1, 'image'))).toBeNull();
  });

  it('extracts a short host from a link', () => {
    expect(linkHost('https://www.youtube.com/watch?v=1')).toBe('youtube.com');
    expect(linkHost('http://example.org:8080/a')).toBe('example.org');
    expect(linkHost('not a url')).toBeNull();
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
});

describe('settings helpers', () => {
  it('lists visible shelf rows in order, then the hidden ones', () => {
    expect(shelfRowList(['links', 'images'])).toEqual([
      { row: 'links', visible: true },
      { row: 'images', visible: true },
      { row: 'texts', visible: false },
    ]);
  });

  it('shows and hides shelf rows, never hiding the last one', () => {
    expect(toggleShelfRow(['images', 'texts'], 'links', true)).toEqual(['images', 'texts', 'links']);
    expect(toggleShelfRow(['images', 'texts'], 'images', false)).toEqual(['texts']);
    const one: ('images' | 'texts' | 'links')[] = ['texts'];
    expect(toggleShelfRow(one, 'texts', false)).toBe(one);
    expect(toggleShelfRow(one, 'texts', true)).toBe(one);
  });

  it('moves a visible shelf row up or down', () => {
    expect(moveShelfRow(['images', 'texts', 'links'], 'links', -1)).toEqual(['images', 'links', 'texts']);
    expect(moveShelfRow(['images', 'texts', 'links'], 'images', 1)).toEqual(['texts', 'images', 'links']);
    const rows: ('images' | 'texts' | 'links')[] = ['images', 'texts'];
    expect(moveShelfRow(rows, 'images', -1)).toBe(rows);
    expect(moveShelfRow(rows, 'links', 1)).toBe(rows);
  });

  it('adds up to three watched folders, ignoring duplicates', () => {
    expect(addFolderPath([], '  C:\\Shots\\ ')).toEqual(['C:\\Shots']);
    const one = ['C:\\Shots'];
    expect(addFolderPath(one, 'c:\\shots\\')).toBe(one);
    const three = ['a', 'b', 'c'];
    expect(addFolderPath(three, 'd')).toBe(three);
    expect(addFolderPath(one, '  ')).toBe(one);
  });
});

describe('clipboard locale', () => {
  type Tree = { [k: string]: string | Tree };
  const files = import.meta.glob('../../../locales/*/clipboard.json', { eager: true, import: 'default' }) as Record<string, Tree>;
  const keys = (tree: Tree, prefix = ''): string[] =>
    Object.entries(tree).flatMap(([k, v]) => (typeof v === 'string' ? [prefix + k] : keys(v, `${prefix}${k}.`)));
  it('tr and en define the same keys', () => {
    const en = keys(files['../../../locales/en/clipboard.json']).sort();
    expect(en.length).toBeGreaterThan(50);
    expect(keys(files['../../../locales/tr/clipboard.json']).sort()).toEqual(en);
  });
});
