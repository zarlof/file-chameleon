import { describe, expect, it } from 'vitest';
import { atRest, fall, impulse, ropePath, ropeY, step, type Pend } from './physics';

const DT = 1 / 60;

describe('pendulum', () => {
  it('settles_to_rest', () => {
    let p: Pend = { theta: 0.25, omega: 0 };
    for (let i = 0; i < 180; i++) p = step(p, DT);
    expect(atRest(p)).toBe(true);
  });

  it('damped_monotone_envelope', () => {
    let p: Pend = { theta: 0.3, omega: 0 };
    const peaks: number[] = [];
    let peak = 0;
    let sign = Math.sign(p.theta);
    for (let i = 0; i < 400; i++) {
      p = step(p, DT);
      if (Math.sign(p.theta) !== sign && p.theta !== 0) {
        peaks.push(peak);
        peak = 0;
        sign = Math.sign(p.theta);
      }
      peak = Math.max(peak, Math.abs(p.theta));
    }
    expect(peaks.length).toBeGreaterThan(2);
    for (let i = 1; i < peaks.length; i++) expect(peaks[i]).toBeLessThan(peaks[i - 1]);
  });

  it('impulse_clamps', () => {
    const p = step(impulse({ theta: 0, omega: 0 }, 1e6), DT);
    expect(Math.abs(p.theta)).toBeLessThanOrEqual(0.5);
    const q = step(impulse({ theta: 0, omega: 0 }, -1e6), DT);
    expect(Math.abs(q.theta)).toBeLessThanOrEqual(0.5);
  });
});

describe('rope', () => {
  it('ropePath_has_sag', () => {
    expect(ropeY(1000, 500, [], 18)).toBeGreaterThanOrEqual(18);
    const d = ropePath(1000, []);
    expect(d.startsWith('M')).toBe(true);
    const nums = (d.match(/-?\d+(\.\d+)?/g) ?? []).map(Number);
    const ys = nums.filter((_, i) => i % 2 === 1);
    expect(Math.max(...ys)).toBeGreaterThanOrEqual(18);
  });

  it('dips_under_pins', () => {
    expect(ropeY(1000, 300, [300], 18, 4)).toBeGreaterThan(ropeY(1000, 300, [], 18, 4));
  });
});

describe('fall', () => {
  it('fall_ends_at_1', () => {
    expect(fall(0)).toBe(0);
    expect(fall(1)).toBe(1);
    expect(fall(0.5)).toBeGreaterThan(0);
    expect(fall(0.5)).toBeLessThan(1);
  });
});
