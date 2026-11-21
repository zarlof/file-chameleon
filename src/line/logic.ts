// Pure helpers for the line: which clips hang on which rope, how many fit, what a card says.
import type { ClipItem } from '../lib/types';

export type RopeKey = 'images' | 'text' | 'links';
export type Rope = { key: RopeKey; items: ClipItem[] };

function ropeOf(item: ClipItem): RopeKey {
  if (item.kind === 'image') return 'images';
  if (item.kind === 'text') return 'text';
  return 'links'; // link + mediaLink
}

/** Pinned first, then newest first. */
function order(a: ClipItem, b: ClipItem): number {
  if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
  return b.createdAt - a.createdAt || b.id - a.id;
}

/** Enabled ropes in the order images, text, links (empty ones kept; see `visibleRopes`). Trash is left out. */
export function splitRopes(items: ClipItem[], ropes: { images: boolean; text: boolean; links: boolean }): Rope[] {
  const out: Rope[] = (['images', 'text', 'links'] as const).filter((k) => ropes[k]).map((key) => ({ key, items: [] }));
  for (const item of items) {
    if (item.trashedAt !== null) continue;
    out.find((r) => r.key === ropeOf(item))?.items.push(item);
  }
  for (const r of out) r.items.sort(order);
  return out;
}

/** The ropes worth drawing: an empty rope is not shown (the window shrinks). */
export function visibleRopes(ropes: Rope[]): Rope[] {
  return ropes.filter((r) => r.items.length > 0);
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
