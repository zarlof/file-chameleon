// Pure helpers for the clipboard library ("Pano") and its settings page.
import type { ClipFolder, ClipItem, ClipKind, ClipQuery, ClipShelf } from '../../../lib/types';

// ------------------------------------------------------------------ filters

/** What the left column shows. */
export type LibraryFilter =
  | { type: 'all' }
  | { type: 'shelf'; shelf: ClipShelf }
  | { type: 'pinned' }
  | { type: 'folder'; id: number }
  | { type: 'tag'; tag: string }
  | { type: 'trash' };

export function filterKey(f: LibraryFilter): string {
  switch (f.type) {
    case 'shelf':
      return `shelf:${f.shelf}`;
    case 'folder':
      return `folder:${f.id}`;
    case 'tag':
      return `tag:${f.tag}`;
    default:
      return f.type;
  }
}

/** Backend query for a filter plus the search box (the backend searches text, title, note, tags, OCR, link title). */
export function queryFor(f: LibraryFilter, search: string): ClipQuery {
  const text = search.trim() || null;
  switch (f.type) {
    case 'all':
      return { text };
    case 'shelf':
      return { text, shelf: f.shelf };
    case 'pinned':
      return { text, pinned: true };
    case 'folder':
      return { text, folder: f.id };
    case 'tag':
      return { text, tag: f.tag };
    case 'trash':
      return { text, trash: true };
  }
}

export function shelfOf(kind: ClipKind): ClipShelf {
  return kind === 'image' ? 'images' : kind === 'text' ? 'texts' : 'links';
}

/** Display order: the trash by trash time; elsewhere pinned first, newest first inside each group. */
export function arrange(items: ClipItem[], f: LibraryFilter): ClipItem[] {
  if (f.type === 'trash') return [...items].sort((a, b) => (b.trashedAt ?? 0) - (a.trashedAt ?? 0));
  return [...items.filter((i) => i.pinned), ...items.filter((i) => !i.pinned)];
}

/** A new copy can be inserted without a refetch only when it surely matches (no search, simple filter). */
export function acceptsAdded(f: LibraryFilter, search: string, item: ClipItem): boolean {
  if (search.trim()) return false;
  switch (f.type) {
    case 'all':
      return true;
    case 'shelf':
      return shelfOf(item.kind) === f.shelf;
    case 'pinned':
      return item.pinned;
    default:
      return false;
  }
}

/** Insert (or move to the top) a freshly recorded item. */
export function withAdded(items: ClipItem[], added: ClipItem): ClipItem[] {
  return [added, ...items.filter((i) => i.id !== added.id)];
}

// ------------------------------------------------------------------ selection

export interface Selection {
  /** Selected ids in display order. */
  ids: number[];
  /** Where Shift ranges start. */
  anchor: number | null;
}

export const EMPTY_SELECTION: Selection = { ids: [], anchor: null };

function range(order: number[], a: number, b: number): number[] {
  const i = order.indexOf(a);
  const j = order.indexOf(b);
  return order.slice(Math.min(i, j), Math.max(i, j) + 1);
}

/** Ids of `set`, in display order. */
function inOrder(order: number[], set: Set<number>): number[] {
  return order.filter((id) => set.has(id));
}

/** Click on `id`: plain selects it, Ctrl toggles it, Shift selects the range from the anchor (Ctrl+Shift adds it). */
export function selectionAfterClick(order: number[], current: Selection, id: number, mods: { shift?: boolean; ctrl?: boolean }): Selection {
  const anchor = current.anchor !== null && order.includes(current.anchor) ? current.anchor : null;
  if (mods.shift && anchor !== null) {
    const set = new Set(mods.ctrl ? current.ids : []);
    for (const x of range(order, anchor, id)) set.add(x);
    return { ids: inOrder(order, set), anchor };
  }
  if (mods.ctrl) {
    const set = new Set(current.ids);
    if (set.has(id)) set.delete(id);
    else set.add(id);
    return { ids: inOrder(order, set), anchor: id };
  }
  return { ids: [id], anchor: id };
}

/** Arrow keys: move the focus by `delta` (clamped); Shift extends the range from the anchor. */
export function selectionAfterMove(order: number[], current: Selection, focus: number | null, delta: number, shift: boolean): { selection: Selection; focus: number | null } {
  if (order.length === 0) return { selection: EMPTY_SELECTION, focus: null };
  const at = focus === null ? -1 : order.indexOf(focus);
  const next = order[at < 0 ? 0 : Math.min(order.length - 1, Math.max(0, at + delta))];
  const anchor = current.anchor !== null && order.includes(current.anchor) ? current.anchor : null;
  if (shift && anchor !== null) return { selection: { ids: range(order, anchor, next), anchor }, focus: next };
  return { selection: { ids: [next], anchor: next }, focus: next };
}

export function selectAll(order: number[]): Selection {
  return { ids: [...order], anchor: order[0] ?? null };
}

/** Keep only ids that are still shown (after a refetch). */
export function pruneSelection(order: number[], current: Selection): Selection {
  const shown = new Set(order);
  const ids = inOrder(order, new Set(current.ids.filter((id) => shown.has(id))));
  const anchor = current.anchor !== null && shown.has(current.anchor) ? current.anchor : null;
  return { ids, anchor };
}

/** Hover buttons on one row or card. */
export type QuickAction = 'copy' | 'pin' | 'trash' | 'restore';

/** Toolbar actions on the selection. */
export type BatchAction =
  | { type: 'copy' }
  | { type: 'pin'; pinned: boolean }
  | { type: 'move'; folder: number | null }
  | { type: 'moveNew'; name: string }
  | { type: 'tag'; add: string[]; remove: string[] }
  | { type: 'export' }
  | { type: 'trash' }
  | { type: 'restore' }
  | { type: 'delete' }
  | { type: 'deselect' };

// ------------------------------------------------------------------ undo

/** Ctrl+Z restores the last trashed batches, up to this many. */
export const UNDO_LIMIT = 32;

export function pushUndo(stack: number[][], batch: number[], limit = UNDO_LIMIT): number[][] {
  if (batch.length === 0) return stack;
  return [...stack, batch].slice(-limit);
}

// ------------------------------------------------------------------ layout

/** Every list row slot has this height (px), so the visible slice is plain arithmetic. */
export const ROW_H = 64;
/** Grid: minimum card width, gap, and row height (card + gap). */
export const CARD_MIN_W = 168;
export const GRID_GAP = 12;
export const GRID_ROW_H = 184;
export const OVERSCAN = 10;

export interface RowWindow {
  /** First rendered index. */
  start: number;
  /** One past the last rendered index. */
  end: number;
  /** Spacer heights (px) standing in for the rows that are not rendered. */
  top: number;
  bottom: number;
}

/** Slice of `total` fixed-height rows to render for a viewport at `scrollTop`, plus `overscan` rows each side. */
export function rowWindow(scrollTop: number, viewport: number, total: number, rowH = ROW_H, overscan = OVERSCAN): RowWindow {
  const first = Math.floor(Math.max(0, scrollTop) / rowH);
  const last = Math.ceil((Math.max(0, scrollTop) + Math.max(0, viewport)) / rowH);
  const start = Math.min(Math.max(0, first - overscan), total);
  const end = Math.min(total, Math.max(start, last + overscan));
  return { start, end, top: start * rowH, bottom: (total - end) * rowH };
}

/** New scrollTop that brings row `index` fully into view (unchanged if it already is). */
export function scrollToReveal(scrollTop: number, viewport: number, index: number, rowH = ROW_H): number {
  const top = index * rowH;
  if (top < scrollTop) return top;
  if (top + rowH > scrollTop + viewport) return top + rowH - viewport;
  return scrollTop;
}

/** Card columns that fit in `width`. */
export function gridColumns(width: number, minCard = CARD_MIN_W, gap = GRID_GAP): number {
  return Math.max(1, Math.floor((width + gap) / (minCard + gap)));
}

// ------------------------------------------------------------------ folders, tags, labels

/** Case-insensitive comparison key, Turkish İ/I/ı/i folded together (as the backend does). */
export function fold(s: string): string {
  return s.replace(/[İIı]/g, 'i').toLowerCase();
}

export interface FolderNode {
  folder: ClipFolder;
  depth: number;
}

/** Folders depth-first (input order kept among siblings); unknown parents and cycles become roots. */
export function folderTree(folders: ClipFolder[]): FolderNode[] {
  const ids = new Set(folders.map((f) => f.id));
  const out: FolderNode[] = [];
  const seen = new Set<number>();
  const walk = (parent: number | null, depth: number) => {
    for (const f of folders) {
      const p = f.parent !== null && ids.has(f.parent) ? f.parent : null;
      if (p !== parent || seen.has(f.id)) continue;
      seen.add(f.id);
      out.push({ folder: f, depth });
      walk(f.id, depth + 1);
    }
  };
  walk(null, 0);
  // Folders only reachable through a cycle.
  for (const f of folders) {
    if (seen.has(f.id)) continue;
    seen.add(f.id);
    out.push({ folder: f, depth: 0 });
    walk(f.id, 1);
  }
  return out;
}

/** Whether all, some or none of `items` carry `tag` (case-insensitive). */
export function tagState(items: ClipItem[], tag: string): 'all' | 'some' | 'none' {
  const key = fold(tag);
  const n = items.filter((i) => i.tags.some((t) => fold(t) === key)).length;
  return n === 0 ? 'none' : n === items.length ? 'all' : 'some';
}

/** Short label: title, then the link title (links), then the first non-empty text line. */
export function clipLabel(item: ClipItem): string | null {
  if (item.title) return item.title;
  if ((item.kind === 'link' || item.kind === 'mediaLink') && item.linkTitle) return item.linkTitle;
  const line = item.text?.split(/\r?\n/).map((l) => l.trim()).find(Boolean);
  return line ?? null;
}

/** "youtube.com" for "https://www.youtube.com/watch?v=1"; null when not a URL. */
export function linkHost(url: string): string | null {
  try {
    const host = new URL(url.trim()).hostname;
    return host ? host.replace(/^www\./, '') : null;
  } catch {
    return null;
  }
}

/** Last path segment (Windows or POSIX separators). */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** "5 minutes ago" in `lang`; `nowLabel` is used below one minute. */
export function relativeTime(then: number, now: number, lang: string, nowLabel: string): string {
  const seconds = Math.round((then - now) / 1000);
  if (Math.abs(seconds) < 60) return nowLabel;
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: 'auto' });
  const minutes = Math.round(seconds / 60);
  if (Math.abs(minutes) < 60) return rtf.format(minutes, 'minute');
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return rtf.format(hours, 'hour');
  return rtf.format(Math.round(hours / 24), 'day');
}

// ------------------------------------------------------------------ settings page

export const SHELF_ROWS: ClipShelf[] = ['images', 'texts', 'links'];

/** Visible rows in their order, then the hidden ones. */
export function shelfRowList(rows: ClipShelf[]): { row: ClipShelf; visible: boolean }[] {
  const visible = rows.filter((r, i) => SHELF_ROWS.includes(r) && rows.indexOf(r) === i);
  return [...visible.map((row) => ({ row, visible: true })), ...SHELF_ROWS.filter((r) => !visible.includes(r)).map((row) => ({ row, visible: false }))];
}

/** Show (appended) or hide a row; the last visible row cannot be hidden. Same array when nothing changes. */
export function toggleShelfRow(rows: ClipShelf[], row: ClipShelf, on: boolean): ClipShelf[] {
  const has = rows.includes(row);
  if (on === has) return rows;
  if (!on) return rows.length > 1 ? rows.filter((r) => r !== row) : rows;
  return [...rows, row];
}

/** Move a visible row by `delta`; same array when it cannot move. */
export function moveShelfRow(rows: ClipShelf[], row: ClipShelf, delta: number): ClipShelf[] {
  const i = rows.indexOf(row);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= rows.length) return rows;
  const next = [...rows];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

/** Watched image folders: at most `max`, no duplicates (case and trailing separator ignored). */
export function addFolderPath(list: string[], raw: string, max = 3): string[] {
  const trimmed = raw.trim();
  // A drive root keeps its separator ("C:" alone means the current folder of drive C).
  const path = /^[a-z]:[\\/]*$/i.test(trimmed) ? `${trimmed.slice(0, 2)}\\` : trimmed.replace(/[\\/]+$/, '');
  if (!path || list.length >= max) return list;
  const key = (p: string) => p.replace(/[\\/]+$/, '').toLowerCase();
  return list.some((p) => key(p) === key(path)) ? list : [...list, path];
}
