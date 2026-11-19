// Pure wheel geometry: slice ordering, hit-testing and DPI conversion. No DOM access.
import type { TargetOption } from '../types';

export const WHEEL_SIZE = 440;
export const OUTER_R = 190;
export const INNER_R = 70;
export const MAX_SLICES = 8;
export const MORE_ID = 'more';

/** Physical pixels (as reported by Tauri drag events) to logical CSS pixels. */
export function toLogical(physical: number, dpr: number): number {
  return physical / (dpr > 0 ? dpr : 1);
}

/** `recent` ids first (only those present), then the rest; > 8 total -> 7 + "More…". */
export function orderSlices(targets: TargetOption[], recent: string[]): TargetOption[] {
  const byId = new Map(targets.map((t) => [t.id, t]));
  const seen = new Set<string>();
  const out: TargetOption[] = [];
  for (const id of recent) {
    const t = byId.get(id);
    if (t && !seen.has(id)) {
      seen.add(id);
      out.push(t);
    }
  }
  for (const t of targets) {
    if (!seen.has(t.id)) {
      seen.add(t.id);
      out.push(t);
    }
  }
  if (out.length <= MAX_SLICES) return out;
  return [...out.slice(0, MAX_SLICES - 1), { id: MORE_ID, label: '…', kind: 'data', needsTools: [] }];
}

/**
 * Slice index under (x, y) in logical window coordinates, or -1 (dead zone, outside the ring,
 * or no slices). Slice 0 is centred at the top; indices increase clockwise.
 */
export function sliceAt(
  x: number,
  y: number,
  count: number,
  size = WHEEL_SIZE,
  inner = INNER_R,
  outer = OUTER_R,
): number {
  if (count <= 0) return -1;
  const dx = x - size / 2;
  const dy = y - size / 2;
  const r = Math.hypot(dx, dy);
  if (r < inner || r > outer) return -1;
  const step = (2 * Math.PI) / count;
  let a = Math.atan2(dx, -dy); // clockwise from top
  if (a < 0) a += 2 * Math.PI;
  return Math.floor((a + step / 2) / step) % count;
}

/** Centre angle (radians, clockwise from top) of slice `i`. */
export function sliceCenterAngle(i: number, count: number): number {
  return (i * 2 * Math.PI) / count;
}

/** Point at clockwise-from-top angle `a` and radius `r` around the wheel centre. */
export function polar(a: number, r: number, size = WHEEL_SIZE): { x: number; y: number } {
  return { x: size / 2 + r * Math.sin(a), y: size / 2 - r * Math.cos(a) };
}

/** SVG path of an annular sector for slice `i` with an angular gap (radians) between slices. */
export function slicePath(i: number, count: number, gap = 0.035, inner = INNER_R + 4, outer = OUTER_R - 2): string {
  const half = count === 1 ? Math.PI - gap / 2 : Math.PI / count - gap / 2;
  const c = sliceCenterAngle(i, count);
  const a0 = c - half;
  const a1 = c + half;
  const o0 = polar(a0, outer), o1 = polar(a1, outer), i1 = polar(a1, inner), i0 = polar(a0, inner);
  const large = a1 - a0 > Math.PI ? 1 : 0;
  const f = (n: number) => n.toFixed(2);
  return `M${f(o0.x)} ${f(o0.y)}A${outer} ${outer} 0 ${large} 1 ${f(o1.x)} ${f(o1.y)}L${f(i1.x)} ${f(i1.y)}A${inner} ${inner} 0 ${large} 0 ${f(i0.x)} ${f(i0.y)}Z`;
}
