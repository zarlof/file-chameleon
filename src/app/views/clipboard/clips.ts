import type { ClipItem } from '../../../lib/types';

export type ClipFilter = 'all' | 'links' | 'text' | 'images';

/** Every row slot has this height (px), so the visible slice is plain arithmetic. */
export const ROW_H = 92;
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

export function matchesFilter(item: ClipItem, filter: ClipFilter): boolean {
  switch (filter) {
    case 'all':
      return true;
    case 'links':
      return item.kind === 'link' || item.kind === 'mediaLink';
    case 'text':
      return item.kind === 'text';
    case 'images':
      return item.kind === 'image';
  }
}

/** Filtered, pinned items first (stable: newest first inside each group). */
export function ordered(items: ClipItem[], filter: ClipFilter): ClipItem[] {
  const shown = items.filter((i) => matchesFilter(i, filter));
  return [...shown.filter((i) => i.pinned), ...shown.filter((i) => !i.pinned)];
}

/** Insert (or move to the top) a freshly recorded item. */
export function withAdded(items: ClipItem[], added: ClipItem): ClipItem[] {
  return [added, ...items.filter((i) => i.id !== added.id)];
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
