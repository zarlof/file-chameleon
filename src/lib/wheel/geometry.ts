// Pure wheel geometry: ring radii, hit-testing, slice paths and DPI conversion. No DOM access.
// Angles are radians, clockwise from the top; slot 0 is centred at the top. Coordinates are
// logical px in the wheel's square box (`size`), whose centre is the wheel centre.

const TAU = 2 * Math.PI;

/** Radii at scale 1: slot ring inner..outer, group fan ring fanInner..fanOuter, box size. */
const BASE = { inner: 64, outer: 168, fanInner: 176, fanOuter: 246, size: 540 } as const;

/** Box size at scale 1 (what `polar` and the path builders draw in by default). */
export const BASE_SIZE = BASE.size;

/** Each fan child gets at least this angle (30°). */
export const FAN_MIN_STEP = Math.PI / 6;

export interface RingRadii {
  inner: number;
  outer: number;
  fanInner: number;
  fanOuter: number;
  size: number;
}

/** Slot under the pointer, and the child of its open fan (-1 = none for both). */
export interface Hit {
  slot: number;
  child: number;
}

/** An open group fan: the group's slot index and its child count. */
export interface FanRef {
  index: number;
  children: number;
}

export function ringRadii(scale: number): RingRadii {
  const s = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return {
    inner: BASE.inner * s,
    outer: BASE.outer * s,
    fanInner: BASE.fanInner * s,
    fanOuter: BASE.fanOuter * s,
    size: BASE.size * s,
  };
}

/** Physical pixels (as reported by Tauri drag events) to logical CSS pixels. */
export function toLogical(physical: number, dpr: number): number {
  return physical / (dpr > 0 ? dpr : 1);
}

/** Centre angle of slice `i` of `count`. */
export function sliceCenterAngle(i: number, count: number): number {
  return (i * TAU) / count;
}

/** Point at angle `a` and radius `r` around the centre of a `size` box. */
export function polar(a: number, r: number, size: number = BASE_SIZE): { x: number; y: number } {
  return { x: size / 2 + r * Math.sin(a), y: size / 2 - r * Math.cos(a) };
}

/** Radius and angle (in [0, 2π)) of a point in the wheel box at `scale`. */
export function pointerPolar(x: number, y: number, scale: number): { r: number; a: number } {
  const c = ringRadii(scale).size / 2;
  const dx = x - c;
  const dy = y - c;
  let a = Math.atan2(dx, -dy);
  if (a < 0) a += TAU;
  return { r: Math.hypot(dx, dy), a };
}

/** Slot whose sector contains angle `a`, or -1 without slots. */
export function slotAtAngle(a: number, count: number): number {
  if (count <= 0) return -1;
  const step = TAU / count;
  return Math.floor((mod(a) + step / 2) / step) % count;
}

/** Child sectors of the fan of slot `index`: the group's angle widened to at least 30° per
 *  child, centred on the group, at most the full circle. `start` is the first child's edge. */
export function fanLayout(index: number, count: number, children: number): { start: number; step: number } {
  const n = Math.max(1, children);
  const span = Math.min(TAU, Math.max(TAU / Math.max(1, count), FAN_MIN_STEP * n));
  return { start: sliceCenterAngle(index, count) - span / 2, step: span / n };
}

/** Slot (and fan child) under (x, y). `fan` is the open group fan, if any. */
export function hitTest(x: number, y: number, count: number, fan: FanRef | null, scale: number): Hit {
  const none = { slot: -1, child: -1 };
  if (count <= 0) return none;
  const R = ringRadii(scale);
  const { r, a } = pointerPolar(x, y, scale);
  if (r < R.inner || r > R.fanOuter) return none;
  if (r <= R.outer) return { slot: slotAtAngle(a, count), child: -1 };
  if (!fan || fan.children <= 0 || fan.index < 0 || fan.index >= count) return none;
  const { start, step } = fanLayout(fan.index, count, fan.children);
  const off = mod(a - start);
  if (off > step * fan.children) return none;
  // Between the slot ring and the fan ring the group stays hovered (the fan stays open).
  if (r < R.fanInner) return { slot: fan.index, child: -1 };
  return { slot: fan.index, child: Math.min(fan.children - 1, Math.floor(off / step)) };
}

/** Whole slice steps between slice `i` of `count` and angle `origin` (either way round). */
export function waveSteps(i: number, count: number, origin: number): number {
  if (count <= 0) return 0;
  const d = Math.abs(wrap(sliceCenterAngle(i, count) - origin));
  return Math.round(d / (TAU / count));
}

/** SVG path of slice `i` of `count` between radii `inner` and `outer`, with a constant
 *  `gapPx` wide gap to its neighbours (parallel edges, not a wedge). */
export function slicePath(i: number, count: number, inner: number, outer: number, gapPx: number, size: number = BASE_SIZE): string {
  if (count <= 1) return ringPath(inner, outer, size);
  const step = TAU / count;
  const c = sliceCenterAngle(i, count);
  return sectorPath(c - step / 2, c + step / 2, inner, outer, gapPx, size);
}

/** SVG path of child `child` of the fan of slot `index` (see `fanLayout`). */
export function fanSlicePath(
  index: number,
  count: number,
  child: number,
  children: number,
  inner: number,
  outer: number,
  gapPx: number,
  size: number = BASE_SIZE,
): string {
  const { start, step } = fanLayout(index, count, children);
  if (step >= TAU - 1e-9) return ringPath(inner, outer, size);
  return sectorPath(start + child * step, start + (child + 1) * step, inner, outer, gapPx, size);
}

function mod(a: number): number {
  return ((a % TAU) + TAU) % TAU;
}

/** Wraps an angle difference into (-π, π]. */
function wrap(d: number): number {
  const m = mod(d);
  return m > Math.PI ? m - TAU : m;
}

const f = (n: number) => n.toFixed(2);

/** Annular sector a0..a1 whose straight edges sit gapPx/2 inside the radial lines a0 and a1. */
function sectorPath(a0: number, a1: number, inner: number, outer: number, gapPx: number, size: number): string {
  const g = Math.max(0, gapPx / 2);
  const half = (a1 - a0) / 2;
  const mid = a0 + half;
  // Angular inset that puts a point at radius r exactly g px from the radial edge line.
  const inset = (r: number) => Math.min(half, Math.asin(Math.min(1, g / Math.max(r, 1e-6))));
  const dO = inset(outer);
  const dI = inset(inner);
  const o0 = polar(a0 + dO, outer, size);
  const o1 = polar(a1 - dO, outer, size);
  const i1 = polar(a1 - dI, inner, size);
  const i0 = polar(a0 + dI, inner, size);
  const largeO = a1 - a0 - 2 * dO > Math.PI ? 1 : 0;
  const largeI = a1 - a0 - 2 * dI > Math.PI ? 1 : 0;
  if (dI >= half) {
    // Too narrow at the inner radius: the edges meet in a point.
    const tip = polar(mid, g / Math.sin(half), size);
    return `M${f(o0.x)} ${f(o0.y)}A${f(outer)} ${f(outer)} 0 ${largeO} 1 ${f(o1.x)} ${f(o1.y)}L${f(tip.x)} ${f(tip.y)}Z`;
  }
  return (
    `M${f(o0.x)} ${f(o0.y)}A${f(outer)} ${f(outer)} 0 ${largeO} 1 ${f(o1.x)} ${f(o1.y)}` +
    `L${f(i1.x)} ${f(i1.y)}A${f(inner)} ${f(inner)} 0 ${largeI} 0 ${f(i0.x)} ${f(i0.y)}Z`
  );
}

/** Full ring (outer clockwise, inner counter-clockwise, so the hole stays with nonzero fill). */
function ringPath(inner: number, outer: number, size: number): string {
  const c = size / 2;
  const circle = (r: number, sweep: 0 | 1) =>
    `M${f(c)} ${f(c - r)}A${f(r)} ${f(r)} 0 1 ${sweep} ${f(c)} ${f(c + r)}A${f(r)} ${f(r)} 0 1 ${sweep} ${f(c)} ${f(c - r)}Z`;
  return circle(outer, 1) + circle(inner, 0);
}
