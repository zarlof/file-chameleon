// Tiny pendulum + rope maths for the line. Pure: no DOM, no timers.

export type Pend = { theta: number; omega: number };

const MAX_THETA = 0.5;
/** Below this the swing is invisible (< 1 px at a peg): snap to rest so the loop can stop. */
const SNAP_THETA = 0.012;
const SNAP_OMEGA = 0.05;

/** One semi-implicit Euler step of a damped pendulum (theta in radians, dt in seconds). */
export function step(p: Pend, dt: number, k: { w: number; c: number } = { w: 4.2, c: 2.2 }): Pend {
  const omega = p.omega + (-k.w * k.w * p.theta - k.c * p.omega) * dt;
  let theta = p.theta + omega * dt;
  let w = omega;
  if (theta > MAX_THETA || theta < -MAX_THETA) {
    theta = Math.sign(theta) * MAX_THETA;
    if (Math.sign(w) === Math.sign(theta)) w = 0;
  }
  if (Math.abs(theta) < SNAP_THETA && Math.abs(w) < SNAP_OMEGA) return { theta: 0, omega: 0 };
  return { theta, omega: w };
}

export function impulse(p: Pend, dOmega: number): Pend {
  return { theta: p.theta, omega: p.omega + dOmega };
}

export function atRest(p: Pend, eps = 1e-3): boolean {
  return Math.abs(p.theta) < eps && Math.abs(p.omega) < eps;
}

/** Height of the rope at x: a parabola sagging `sag` at the middle, plus a small dip under each pin. */
export function ropeY(width: number, x: number, pins: number[], sag = 18, dip = 4): number {
  const u = width > 0 ? (2 * x) / width - 1 : 0;
  let y = sag * (1 - u * u);
  for (const px of pins) {
    const d = (x - px) / 14;
    y += dip * Math.exp(-d * d);
  }
  return y;
}

/** SVG `d` of the rope across `width`: sags in the middle, dips a little under every pin. */
export function ropePath(width: number, pins: number[], sag = 18, dip = 4): string {
  const n = Math.max(8, Math.min(120, Math.round(width / 16)));
  let d = '';
  for (let i = 0; i <= n; i++) {
    const x = (width * i) / n;
    d += `${i === 0 ? 'M' : 'L'}${x.toFixed(1)} ${ropeY(width, x, pins, sag, dip).toFixed(2)} `;
  }
  return d.trim();
}

/** A card dropping onto the rope: 0 -> 1 with one small rebound. 0 at t<=0, exactly 1 at t>=1. */
export function fall(t: number): number {
  if (t <= 0) return 0;
  if (t >= 1) return 1;
  if (t < 0.7) return (t / 0.7) ** 2;
  return 1 - 0.07 * Math.sin((Math.PI * (t - 0.7)) / 0.3);
}
