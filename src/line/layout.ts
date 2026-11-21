// Pure layout numbers for the line. The same table lives in src-tauri/src/line.rs (band_height); keep both in step.

export type LineSize = 'small' | 'medium' | 'large';

export const SIZES: Record<LineSize, { scale: number; card: number; band: number }> = {
  small: { scale: 0.8, card: 90, band: 144 },
  medium: { scale: 1, card: 112, band: 178 },
  large: { scale: 1.25, card: 140, band: 222 },
};

/** Space between two bands. */
export const GAP = 12;
/** Top and bottom margin of the window (half a gap). */
export const MARGIN = 6;

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

/** Cards keep at least 90 % opacity so they stay readable on a faint backdrop. */
export function cardOpacity(lineOpacity: number): number {
  return Math.max(0.9, lineOpacity / 100);
}

const HEX = /^#[0-9a-f]{6}$/i;
const NAMES = ['blue', 'purple', 'pink', 'red', 'orange', 'yellow', 'green', 'graphite'];

/** A CSS colour for the rope: "accent" -> `accent`, a #rrggbb hex as is, an accent name -> its variable. */
export function ropeColor(setting: string, accent: string): string {
  if (HEX.test(setting)) return setting;
  if (NAMES.includes(setting)) return `var(--${setting})`;
  return accent;
}
