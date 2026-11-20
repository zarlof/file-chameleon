import { describe, expect, it } from 'vitest';
import {
  fanLayout,
  fanSlicePath,
  hitTest,
  pointerPolar,
  polar,
  ringRadii,
  sliceCenterAngle,
  slicePath,
  slotAtAngle,
  toLogical,
  waveSteps,
} from './geometry';

const NONE = { slot: -1, child: -1 };
const at = (a: number, r: number, scale = 1) => polar(a, r, ringRadii(scale).size);
const deg = (d: number) => (d * Math.PI) / 180;

/** Numbers of an SVG path, as [x, y] points of its M / L commands and arc end points. */
function points(d: string): Array<[number, number]> {
  const out: Array<[number, number]> = [];
  for (const m of d.matchAll(/([MLA])([^MLAZ]+)/g)) {
    const n = m[2].trim().split(/[\s,]+/).map(Number);
    out.push([n[n.length - 2], n[n.length - 1]]);
  }
  return out;
}

/** Perpendicular distance of (x, y) from the line through the wheel centre at angle `a`. */
function offLine(p: [number, number], a: number, c = 270): number {
  const dx = p[0] - c;
  const dy = p[1] - c;
  return Math.abs(dx * Math.cos(a) + dy * Math.sin(a));
}

describe('ringRadii', () => {
  it('uses the base radii at scale 1', () => {
    expect(ringRadii(1)).toEqual({ inner: 64, outer: 168, fanInner: 176, fanOuter: 246, size: 540 });
  });
  it('scales every radius and the box', () => {
    const r = ringRadii(1.2);
    expect(r.size).toBeCloseTo(648);
    expect(r.inner).toBeCloseTo(76.8);
    expect(r.outer).toBeCloseTo(201.6);
    expect(r.fanInner).toBeCloseTo(211.2);
    expect(r.fanOuter).toBeCloseTo(295.2);
  });
  it('treats a missing or broken scale as 1', () => {
    expect(ringRadii(0).size).toBe(540);
    expect(ringRadii(Number.NaN).size).toBe(540);
  });
});

describe('hitTest', () => {
  it('puts slot 0 at the top and goes clockwise', () => {
    expect(hitTest(270, 270 - 100, 8, null, 1)).toEqual({ slot: 0, child: -1 });
    expect(hitTest(270 + 100, 270, 8, null, 1)).toEqual({ slot: 2, child: -1 });
    expect(hitTest(270, 270 + 100, 8, null, 1)).toEqual({ slot: 4, child: -1 });
    expect(hitTest(270 - 100, 270, 8, null, 1)).toEqual({ slot: 6, child: -1 });
  });
  it('wraps around the top', () => {
    const justLeft = at(-0.05, 120);
    expect(hitTest(justLeft.x, justLeft.y, 8, null, 1)).toEqual({ slot: 0, child: -1 });
    const pastHalf = at(-deg(22.5) - 0.02, 120);
    expect(hitTest(pastHalf.x, pastHalf.y, 8, null, 1)).toEqual({ slot: 7, child: -1 });
  });
  it('has a dead centre and nothing outside the fan ring', () => {
    expect(hitTest(270, 270, 8, null, 1)).toEqual(NONE);
    expect(hitTest(270, 270 - 63, 8, null, 1)).toEqual(NONE);
    expect(hitTest(270, 270 - 65, 8, null, 1)).toEqual({ slot: 0, child: -1 });
    expect(hitTest(270, 270 - 247, 8, { index: 0, children: 3 }, 1)).toEqual(NONE);
    expect(hitTest(0, 0, 8, null, 1)).toEqual(NONE);
  });
  it('treats the fan ring as empty while no fan is open', () => {
    expect(hitTest(270, 270 - 210, 8, null, 1)).toEqual(NONE);
    expect(hitTest(270, 270 - 172, 8, null, 1)).toEqual(NONE);
  });
  it('finds the children of an open fan (at least 30° each, centred on the group)', () => {
    const fan = { index: 0, children: 3 };
    expect(hitTest(270, 270 - 210, 8, fan, 1)).toEqual({ slot: 0, child: 1 });
    const left = at(-deg(30), 210);
    expect(hitTest(left.x, left.y, 8, fan, 1)).toEqual({ slot: 0, child: 0 });
    const right = at(deg(30), 210);
    expect(hitTest(right.x, right.y, 8, fan, 1)).toEqual({ slot: 0, child: 2 });
    const beyond = at(deg(50), 210);
    expect(hitTest(beyond.x, beyond.y, 8, fan, 1)).toEqual(NONE);
  });
  it('keeps the group in the gap between the ring and its fan', () => {
    const p = at(0, 172);
    expect(hitTest(p.x, p.y, 8, { index: 0, children: 3 }, 1)).toEqual({ slot: 0, child: -1 });
  });
  it('uses the group angle when it is wider than 30° per child', () => {
    // 2 slots of 180°: the fan of slot 1 (centred at the bottom) spans 180°, 90° per child.
    const p = at(Math.PI - 0.3, 210);
    expect(hitTest(p.x, p.y, 2, { index: 1, children: 2 }, 1)).toEqual({ slot: 1, child: 0 });
    const q = at(Math.PI + 0.3, 210);
    expect(hitTest(q.x, q.y, 2, { index: 1, children: 2 }, 1)).toEqual({ slot: 1, child: 1 });
  });
  it('clamps a fan to the full circle', () => {
    // One slot (360°) with 6 children: 60° each, child 0 starts at the bottom.
    const fan = { index: 0, children: 6 };
    const top = at(0, 210);
    expect(hitTest(top.x, top.y, 1, fan, 1)).toEqual({ slot: 0, child: 3 });
    const bottomRight = at(Math.PI + 0.1, 210);
    expect(hitTest(bottomRight.x, bottomRight.y, 1, fan, 1)).toEqual({ slot: 0, child: 0 });
  });
  it('scales the radii and the centre', () => {
    const c = ringRadii(1.2).size / 2;
    expect(hitTest(c, c - 70, 8, null, 1.2)).toEqual(NONE);
    expect(hitTest(c, c - 80, 8, null, 1.2)).toEqual({ slot: 0, child: -1 });
    expect(hitTest(c, c - 200, 8, null, 1.2)).toEqual({ slot: 0, child: -1 });
    const fanPoint = at(0.1, 250, 1.2);
    expect(hitTest(fanPoint.x, fanPoint.y, 8, { index: 0, children: 2 }, 1.2)).toEqual({ slot: 0, child: 1 });
  });
  it('returns nothing without slots or for an out-of-range fan', () => {
    expect(hitTest(270, 170, 0, null, 1)).toEqual(NONE);
    expect(hitTest(270, 60, 8, { index: 9, children: 3 }, 1)).toEqual(NONE);
  });
});

describe('pointerPolar / slotAtAngle', () => {
  it('measures from the wheel centre, clockwise from the top', () => {
    const p = pointerPolar(270 + 100, 270, 1);
    expect(p.r).toBeCloseTo(100);
    expect(p.a).toBeCloseTo(Math.PI / 2);
    expect(pointerPolar(270, 170, 1).a).toBeCloseTo(0);
    expect(slotAtAngle(Math.PI / 2, 8)).toBe(2);
    expect(slotAtAngle(2 * Math.PI - 0.01, 8)).toBe(0);
    expect(slotAtAngle(1, 0)).toBe(-1);
  });
});

describe('slicePath', () => {
  it('keeps a constant pixel gap on both edges', () => {
    const d = slicePath(0, 4, 60, 160, 10);
    const [o0, o1, i1, i0] = points(d);
    const a0 = -Math.PI / 4;
    const a1 = Math.PI / 4;
    expect(offLine(o0, a0)).toBeCloseTo(5, 1);
    expect(offLine(i0, a0)).toBeCloseTo(5, 1);
    expect(offLine(o1, a1)).toBeCloseTo(5, 1);
    expect(offLine(i1, a1)).toBeCloseTo(5, 1);
    expect(Math.hypot(o0[0] - 270, o0[1] - 270)).toBeCloseTo(160, 1);
    expect(Math.hypot(i0[0] - 270, i0[1] - 270)).toBeCloseTo(60, 1);
  });
  it('draws a single slot as a full ring', () => {
    const d = slicePath(0, 1, 60, 160, 10);
    expect(d).not.toContain('L');
    expect(d.match(/M/g)).toHaveLength(2);
  });
});

describe('fanLayout / fanSlicePath', () => {
  it('centres the children on the group', () => {
    const f = fanLayout(2, 8, 3);
    expect(f.step).toBeCloseTo(deg(30));
    expect(f.start + 1.5 * f.step).toBeCloseTo(sliceCenterAngle(2, 8));
    const [o0, o1] = points(fanSlicePath(0, 8, 1, 3, 176, 246, 6));
    expect(o0[0] + o1[0]).toBeCloseTo(540, 1); // symmetric around the top
    expect(o0[1]).toBeCloseTo(o1[1], 1);
  });
});

describe('waveSteps', () => {
  it('counts slices between a slice and the origin angle, both ways round', () => {
    expect(waveSteps(0, 8, 0)).toBe(0);
    expect(waveSteps(4, 8, 0)).toBe(4);
    expect(waveSteps(7, 8, 0)).toBe(1);
    expect(waveSteps(3, 12, sliceCenterAngle(2, 8))).toBe(0);
  });
});

describe('toLogical', () => {
  it('divides by the device pixel ratio', () => {
    expect(toLogical(330, 1.5)).toBe(220);
    expect(toLogical(100, 1)).toBe(100);
    expect(toLogical(100, 0)).toBe(100);
  });
});
