import type { ClipItem } from '../../../lib/types';

export type ClipFilter = 'all' | 'links' | 'text' | 'images';

/** Rows rendered at once; more are revealed on scroll / "show more". */
export const PAGE = 60;

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
