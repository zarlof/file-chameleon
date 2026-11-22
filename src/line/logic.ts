// Pure helpers for the line: which clips hang on which rope, how many fit, what a card says.
import type { ClipItem, LineFile, LineRopeKey } from '../lib/types';

export type RopeKey = LineRopeKey;
export type Rope = { key: RopeKey; items: ClipItem[]; files: LineFile[] };

const DEFAULT_ORDER: RopeKey[] = ['images', 'text', 'links', 'files'];

function ropeOf(item: ClipItem): RopeKey {
  if (item.kind === 'image') return 'images';
  if (item.kind === 'text') return 'text';
  return 'links'; // link + mediaLink
}

/** Where a clip sits on its rope: the place it was dragged to, else its time (newest first). */
export const clipRank = (i: ClipItem): number => i.rank ?? i.createdAt;
/** The same for a file of the files rope. */
export const fileRank = (f: LineFile): number => f.rank ?? f.at;

/** Pinned first, then by rank (higher first): a dragged place, else newest first. */
function order(a: ClipItem, b: ClipItem): number {
  if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
  return clipRank(b) - clipRank(a) || b.id - a.id;
}

function fileOrder(a: LineFile, b: LineFile): number {
  if (!!a.pinned !== !!b.pinned) return a.pinned ? -1 : 1;
  return fileRank(b) - fileRank(a);
}

/** `o` made safe: unknown and repeated keys dropped, missing ones appended in the default order. */
export function sanitizeOrder(o: readonly string[] | undefined): RopeKey[] {
  const out: RopeKey[] = [];
  for (const k of o ?? []) if ((DEFAULT_ORDER as string[]).includes(k) && !out.includes(k as RopeKey)) out.push(k as RopeKey);
  for (const k of DEFAULT_ORDER) if (!out.includes(k)) out.push(k);
  return out;
}

/** Enabled ropes in `ropeOrder` (empty ones kept: every enabled rope shows). A rope missing from `ropes` is off. Trash is left out. */
export function splitRopes(
  items: ClipItem[],
  ropes: { images: boolean; text: boolean; links: boolean; files?: boolean },
  ropeOrder?: readonly string[],
  files: LineFile[] = [],
): Rope[] {
  const out: Rope[] = sanitizeOrder(ropeOrder).filter((k) => ropes[k] === true).map((key) => ({ key, items: [], files: [] }));
  for (const item of items) {
    if (item.trashedAt !== null) continue;
    out.find((r) => r.key === ropeOf(item))?.items.push(item);
  }
  for (const r of out) {
    r.items.sort(order);
    if (r.key === 'files') r.files = [...files].sort(fileOrder);
  }
  return out;
}

/** A rope's label: the custom name when it has one, else the localized default. */
export function ropeName(key: RopeKey, names: Partial<Record<RopeKey, string>> | undefined, fallback: string): string {
  const n = names?.[key]?.trim();
  return n ? n : fallback;
}

/** How many cards fit in a rope `widthPx` wide. At least 1. */
export function fitCount(widthPx: number, card = 112, gap = 16, margin = 48): number {
  return Math.max(1, Math.floor((widthPx - margin + gap) / (card + gap)));
}

export function overflow(total: number, fit: number): number {
  return Math.max(0, total - fit);
}

function hostOf(raw: string): string {
  try {
    const u = new URL(raw);
    return u.host.replace(/^www\./, '') + (u.pathname === '/' ? '' : u.pathname) + u.search;
  } catch {
    return raw;
  }
}

/** The words on a card: a title and at most `maxLines` more lines, the last one ending with "…" if text was cut. */
export function cardText(item: ClipItem, maxLines = 3): { title: string; lines: string[] } {
  if (item.kind === 'image') return { title: item.title ?? '', lines: [] };
  const raw = (item.text ?? '').trim();
  if (item.kind !== 'text') {
    return { title: item.linkTitle ?? item.title ?? '', lines: raw ? [hostOf(raw)] : [] };
  }
  const all = raw.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
  let title = item.title ?? '';
  let rest = all;
  if (!title) {
    title = all[0] ?? '';
    rest = all.slice(1);
  }
  const lines = rest.slice(0, maxLines);
  if (rest.length > maxLines && lines.length > 0) lines[lines.length - 1] += '…';
  return { title, lines };
}

/** Lower-case, diacritics removed, Turkish dotted/dotless i folded together. */
function fold(s: string): string {
  return s
    .replace(/İ/g, 'i')
    .replace(/I/g, 'i')
    .replace(/ı/g, 'i')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/ß/g, 'ss');
}

/** Search: every word of `query` must appear in the text, title, note, tags, OCR text or link title. Empty = everything. */
export function matches(item: ClipItem, query: string): boolean {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const hay = fold([item.text, item.title, item.note, ...item.tags, item.ocr, item.linkTitle].filter(Boolean).join('\n'));
  return words.every((w) => hay.includes(w));
}

/** One card on a rope: a clip or a file, with a number that is unique across both (files are negative). */
export type LineCard = { id: number; item: ClipItem | null; file: LineFile | null };

/** The cards of a rope in display order. */
export function ropeCards(r: Rope, fileIdOf: (path: string) => number): LineCard[] {
  if (r.key === 'files') {
    const used = new Set<number>();
    return r.files.map((file) => {
      let id = fileIdOf(file.path);
      while (used.has(id)) id -= 1; // a hash collision never drops a card
      used.add(id);
      return { id, item: null, file };
    });
  }
  return r.items.map((item) => ({ id: item.id, item, file: null }));
}

const cardPinned = (c: LineCard) => (c.item ? c.item.pinned : !!c.file?.pinned);
const cardRank = (c: LineCard) => (c.item ? clipRank(c.item) : c.file ? fileRank(c.file) : 0);

/** The slot a dragged card may take: pinned cards stay among the pinned ones at the start, the
 *  others after them. `cards` is the rope without the dragged card, `to` the wanted slot in it. */
export function allowedSlot(cards: readonly LineCard[], pinned: boolean, to: number): number {
  const pinnedCount = cards.filter(cardPinned).length;
  const t = Math.max(0, Math.min(cards.length, to));
  return pinned ? Math.min(t, pinnedCount) : Math.max(t, pinnedCount);
}

/** The rank that puts the card at `from` into slot `to` of the others (both indexes in the rope's
 *  current order; `to` counts without the dragged card). Null when the card stays where it is. */
export function dropRank(cards: readonly LineCard[], from: number, to: number): number | null {
  const moved = cards[from];
  if (!moved) return null;
  const rest = cards.filter((_, i) => i !== from);
  const slot = allowedSlot(rest, cardPinned(moved), to);
  if (slot === from) return null;
  // Ranks only order cards within their group (pinned or not): a neighbour of the other group
  // does not count.
  const same = (c: LineCard | undefined) => (c && cardPinned(c) === cardPinned(moved) ? c : undefined);
  const left = same(rest[slot - 1]);
  const right = same(rest[slot]);
  if (left && right) return (cardRank(left) + cardRank(right)) / 2;
  if (right) return cardRank(right) + 1;
  if (left) return cardRank(left) - 1;
  return null;
}

/** Move rope `key` by `delta` places in `order` (up = -1, down = 1). Same array when it cannot move. */
export function moveRope(ropeOrder: readonly RopeKey[], key: RopeKey, delta: number): RopeKey[] {
  const list = sanitizeOrder(ropeOrder);
  const i = list.indexOf(key);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= list.length) return list;
  [list[i], list[j]] = [list[j], list[i]];
  return list;
}

/** The line look and behaviour settings of `defaults`, for the "reset" button. The on/off switch,
 *  the hotkeys and the migration marker are not part of it. */
export function lineDefaults<T extends Record<string, unknown>>(defaults: T): Partial<T> {
  const keep = (k: string) => k.startsWith('line') && !['lineEnabled', 'lineLookVersion'].includes(k);
  return Object.fromEntries(Object.entries(defaults).filter(([k]) => keep(k))) as Partial<T>;
}

/** `keys` with `key` folded when it was open, opened when it was folded. */
export function toggleCollapsed(keys: readonly RopeKey[], key: RopeKey): RopeKey[] {
  return keys.includes(key) ? keys.filter((k) => k !== key) : [...keys, key];
}
