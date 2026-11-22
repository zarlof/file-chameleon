import { naturalRope } from './ropeStyle';
import table from './sizes.json';

// Pure layout numbers for the line. The numbers live in sizes.json, which the Rust side
// (src-tauri/src/line/geometry.rs) checks in a test, so the window and the page agree.

export type LineSize = 'small' | 'medium' | 'large';

export const SIZES: Record<LineSize, { scale: number; card: number; band: number }> = table.sizes;

/** Space between two bands. */
export const GAP = table.gap;
/** Top and bottom margin of the window (half a gap). */
export const MARGIN = table.margin;

function sizeOf(size: string): (typeof SIZES)[LineSize] {
  return Object.hasOwn(SIZES, size) ? SIZES[size as LineSize] : SIZES.medium;
}

export function sizeInfo(size: string) {
  return sizeOf(size);
}

/** Top of each of `n` bands inside the window. */
export function bandTops(n: number, size: string): number[] {
  const { band } = sizeOf(size);
  return Array.from({ length: Math.max(0, n) }, (_, i) => MARGIN + i * (band + GAP));
}

/** Logical window height for `n` visible ropes (at least one). */
export function windowHeight(n: number, size: string): number {
  return Math.max(1, n) * (sizeOf(size).band + GAP);
}

/** How many whole ropes fit a window `height` logical px high (at least one). Same rule as `max_ropes` in Rust. */
export function maxRopes(height: number, size: string): number {
  const n = Math.floor(height / (sizeOf(size).band + GAP) + 0.01);
  return Number.isFinite(n) && n >= 1 ? n : 1;
}

/** Cards keep at least 90 % opacity so they stay readable on a faint backdrop. */
export function cardOpacity(lineOpacity: number): number {
  return Math.max(0.9, lineOpacity / 100);
}

const HEX = /^#[0-9a-f]{6}$/i;
const NAMES = ['blue', 'purple', 'pink', 'red', 'orange', 'yellow', 'green', 'graphite'];

/** A CSS colour for the rope: "natural" -> the hemp grey for the theme, "accent" -> `accent`, a #rrggbb hex as is, an accent name -> its variable. */
export function ropeColor(setting: string, accent: string, theme: 'dark' | 'light' = 'dark'): string {
  if (setting === 'natural') return naturalRope(theme);
  if (HEX.test(setting)) return setting;
  if (NAMES.includes(setting)) return `var(--${setting})`;
  return accent;
}

/** CSS custom properties for the card look (set on the strip and on the settings preview). */
export function cardVars(style: string, radius: number, shadow: boolean): Record<string, string> {
  const r = Math.min(16, Math.max(0, Math.round(radius)));
  const base: Record<string, string> = {
    '--c-radius': `${r}px`,
    '--c-shadow': shadow ? '0 4px 12px rgba(0,0,0,0.35)' : 'none',
  };
  if (style === 'glassLight') {
    return { ...base, '--c-bg': 'rgba(250,250,252,0.8)', '--c-fg': '#16161a', '--c-fg2': 'rgba(22,22,26,0.62)', '--c-edge': 'rgba(0,0,0,0.12)', '--c-blur': 'blur(10px)' };
  }
  if (style === 'paper') {
    return { ...base, '--c-bg': 'var(--elevated-bg)', '--c-fg': 'var(--text)', '--c-fg2': 'var(--text-secondary)', '--c-edge': 'var(--glass-edge)', '--c-blur': 'none' };
  }
  return { ...base, '--c-bg': 'rgba(20,20,24,0.78)', '--c-fg': '#ffffff', '--c-fg2': 'rgba(255,255,255,0.66)', '--c-edge': 'rgba(255,255,255,0.1)', '--c-blur': 'blur(10px)' };
}

/** `cardVars` as an inline style string. */
export function cardStyleAttr(style: string, radius: number, shadow: boolean): string {
  return Object.entries(cardVars(style, radius, shadow)).map(([k, v]) => `${k}:${v}`).join(';');
}

/** Height of a folded rope's band (its label and the gap below it). */
export const FOLD_H = table.foldH;

/** Top of each band when some ropes are folded (`folded[i]` = rope i is folded). */
export function foldedTops(folded: boolean[], size: string): number[] {
  const { band } = sizeOf(size);
  const out: number[] = [];
  let y = MARGIN;
  for (const f of folded) {
    out.push(y);
    y += f ? FOLD_H : band + GAP;
  }
  return out;
}

/** Logical window height for these ropes (`windowHeight` when nothing is folded). */
export function foldedHeight(folded: boolean[], size: string): number {
  if (!folded.some(Boolean)) return windowHeight(folded.length, size);
  const { band } = sizeOf(size);
  return folded.reduce((h, f) => h + (f ? FOLD_H : band + GAP), 0);
}

/** How many of the ropes (in order) fit a window `height` px high: whole bands only, at least one. */
export function fitFolded(folded: boolean[], height: number, size: string): number {
  if (!folded.some(Boolean)) return Math.min(folded.length, maxRopes(height, size));
  const { band } = sizeOf(size);
  let h = 0;
  let n = 0;
  for (const f of folded) {
    h += f ? FOLD_H : band + GAP;
    if (h > height + 1) break;
    n++;
  }
  return Math.max(1, n);
}

/** The size to draw: `size`, or the next smaller one(s) when the ropes do not all fit `height`
 *  (same rule as `fit_size` in src-tauri/src/line/geometry.rs). */
export function fitSize(folded: boolean[], height: number, size: string): LineSize {
  const order: LineSize[] = ['large', 'medium', 'small'];
  const i = order.indexOf(size as LineSize);
  for (const s of order.slice(i < 0 ? 1 : i)) if (foldedHeight(folded, s) <= height + 1) return s;
  return 'small';
}
