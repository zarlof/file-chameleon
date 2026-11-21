// Pure model of a card on the files rope.
import type { LineFile } from '../lib/types';

export type FileCard = { name: string; ext: string; sizeText: string; sourceIcon: LineFile['source']; isImage: boolean };

/** "500 B", "980 KB", "1,2 MB" (decimal comma for Turkish). */
export function sizeText(bytes: number, lang: 'tr' | 'en'): string {
  const n = Math.max(0, bytes);
  if (n < 1024) return `${n} B`;
  const fmt = (v: number) => {
    const s = (Math.round(v * 10) / 10).toString();
    return lang === 'tr' ? s.replace('.', ',') : s;
  };
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  if (n < 1024 ** 3) return `${fmt(n / 1024 ** 2)} MB`;
  return `${fmt(n / 1024 ** 3)} GB`;
}

export function fileCard(f: LineFile, lang: 'tr' | 'en' = 'en'): FileCard {
  const dot = f.name.lastIndexOf('.');
  const ext = dot > 0 && dot < f.name.length - 1 ? f.name.slice(dot + 1).toLowerCase() : '';
  return { name: f.name, ext, sizeText: sizeText(f.size, lang), sourceIcon: f.source, isImage: f.kind === 'image' };
}

/** A stable negative number for a path (case-insensitive), so a file card can share the id-keyed
 *  animation maps with the clip cards (clip ids are positive). */
export function fileId(path: string): number {
  let h = 2166136261;
  for (const ch of path.toLowerCase()) {
    h ^= ch.codePointAt(0)!;
    h = Math.imul(h, 16777619);
  }
  return -((h >>> 0) % 2147483647) - 1;
}
