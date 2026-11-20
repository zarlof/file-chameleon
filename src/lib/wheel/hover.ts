// Pure wheel interaction: what the pointer hovers, when a group's fan opens or closes, and what
// a drop or a click does. Timers and events live in the overlay; this only decides.
import type { WheelFace, WheelSlot } from '../types';
import { hitTest, pointerPolar, ringRadii, slotAtAngle, type FanRef, type Hit } from './geometry';
import { isGroupSlot } from './labels';

/** Resting on a group this long opens its fan. */
export const FAN_DELAY_MS = 200;
/** Moving outward past this fraction of the outer radius inside a group opens its fan at once. */
export const FAN_PUSH = 0.85;

export interface PointerState {
  hit: Hit;
  /** Open fan (group slot index), -1 = none. */
  fan: number;
  /** Group slot whose FAN_DELAY_MS timer should run now, -1 = none. */
  arm: number;
}

function fanRef(slots: readonly WheelSlot[], i: number): FanRef | null {
  const s = i >= 0 ? slots[i] : undefined;
  return s && isGroupSlot(s) && s.children.length > 0 ? { index: i, children: s.children.length } : null;
}

/** Hover state for a pointer at (x, y) in the wheel box, given the fan that is open now. The
 *  fan stays open while the pointer is over its group's angle or its own ring. */
export function pointerAt(x: number, y: number, slots: readonly WheelSlot[], fan: number, scale: number): PointerState {
  const n = slots.length;
  let open = fanRef(slots, fan) ? fan : -1;
  let hit = hitTest(x, y, n, fanRef(slots, open), scale);
  if (open >= 0 && hit.slot !== open) {
    open = -1;
    hit = hitTest(x, y, n, null, scale);
  }
  if (open < 0) {
    const R = ringRadii(scale);
    const { r, a } = pointerPolar(x, y, scale);
    const g = r >= FAN_PUSH * R.outer && r <= R.fanOuter ? slotAtAngle(a, n) : -1;
    if (fanRef(slots, g)) {
      open = g;
      hit = hitTest(x, y, n, fanRef(slots, g), scale);
    }
  }
  const arm = open < 0 && hit.child < 0 && fanRef(slots, hit.slot) ? hit.slot : -1;
  return { hit, fan: open, arm };
}

/** Preset id a drop at `hit` runs (the child for an open fan), or null (centre, empty space or
 *  a group slot itself): the wheel just closes. */
export function dropTarget(slots: readonly WheelSlot[], hit: Hit): string | null {
  const s = hit.slot >= 0 ? slots[hit.slot] : undefined;
  if (!s) return null;
  if (hit.child >= 0) return s.children[hit.child]?.id ?? null;
  return isGroupSlot(s) ? null : s.id;
}

export type ClickAction = { kind: 'run'; id: string } | { kind: 'fan'; slot: number } | { kind: 'close' };

/** Click mode: run a preset, open a group's fan, or close the wheel. */
export function clickAction(slots: readonly WheelSlot[], hit: Hit): ClickAction {
  const id = dropTarget(slots, hit);
  if (id) return { kind: 'run', id };
  if (hit.child < 0 && fanRef(slots, hit.slot)) return { kind: 'fan', slot: hit.slot };
  return { kind: 'close' };
}

/** The face a flip shows: the other one, unless it has no slots. */
export function otherFace(face: WheelFace, front: readonly WheelSlot[], back: readonly WheelSlot[]): WheelFace {
  const next: WheelFace = face === 'front' ? 'back' : 'front';
  return (next === 'front' ? front : back).length > 0 ? next : face;
}
