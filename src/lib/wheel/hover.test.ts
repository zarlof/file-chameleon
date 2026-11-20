import { describe, expect, it } from 'vitest';
import type { WheelSlot } from '../types';
import { polar } from './geometry';
import { clickAction, dropTarget, otherFace, pointerAt } from './hover';

const slot = (id: string, children: WheelSlot[] = []): WheelSlot => ({
  id,
  name: '',
  format: null,
  detail: null,
  icon: children.length ? 'group' : 'image',
  kind: 'image',
  needsTools: [],
  children,
});

// Slot 0 is a group with three children (its fan: -45°..45°, 30° per child).
const slots: WheelSlot[] = [
  slot('group:files', [slot('zip'), slot('copy-to'), slot('move-to')]),
  ...['to-png', 'to-jpg', 'to-webp', 'to-gif', 'to-bmp', 'to-tiff', 'to-ico'].map((id) => slot(id)),
];
const deg = (d: number) => (d * Math.PI) / 180;
const move = (a: number, r: number, fan: number) => {
  const p = polar(a, r);
  return pointerAt(p.x, p.y, slots, fan, 1);
};

describe('pointerAt', () => {
  it('hovers a slot and arms the fan timer only over a group', () => {
    expect(move(0, 100, -1)).toEqual({ hit: { slot: 0, child: -1 }, fan: -1, arm: 0 });
    expect(move(deg(90), 100, -1)).toEqual({ hit: { slot: 2, child: -1 }, fan: -1, arm: -1 });
    expect(move(0, 20, -1)).toEqual({ hit: { slot: -1, child: -1 }, fan: -1, arm: -1 });
  });
  it('opens the fan at once when the pointer moves outward inside the group', () => {
    expect(move(0, 0.9 * 168, -1)).toEqual({ hit: { slot: 0, child: -1 }, fan: 0, arm: -1 });
    // Flicked straight into the outer ring before any event inside the slice.
    expect(move(0, 210, -1)).toEqual({ hit: { slot: 0, child: 1 }, fan: 0, arm: -1 });
    // Outward but outside the group: nothing opens.
    expect(move(deg(90), 210, -1)).toEqual({ hit: { slot: -1, child: -1 }, fan: -1, arm: -1 });
  });
  it('keeps the fan open over the group and its ring', () => {
    expect(move(deg(30), 210, 0)).toEqual({ hit: { slot: 0, child: 2 }, fan: 0, arm: -1 });
    expect(move(deg(10), 100, 0)).toEqual({ hit: { slot: 0, child: -1 }, fan: 0, arm: -1 });
  });
  it('closes the fan when the pointer leaves the group angle and the fan ring', () => {
    expect(move(deg(90), 100, 0)).toEqual({ hit: { slot: 2, child: -1 }, fan: -1, arm: -1 });
    expect(move(deg(60), 210, 0)).toEqual({ hit: { slot: -1, child: -1 }, fan: -1, arm: -1 });
    expect(move(0, 10, 0).fan).toBe(-1);
  });
  it('ignores a fan index that is not a group', () => {
    expect(move(deg(90), 100, 2)).toEqual({ hit: { slot: 2, child: -1 }, fan: -1, arm: -1 });
  });
});

describe('dropTarget', () => {
  it('returns the preset id under the pointer, the child for fans', () => {
    expect(dropTarget(slots, { slot: 3, child: -1 })).toBe('to-webp');
    expect(dropTarget(slots, { slot: 0, child: 1 })).toBe('copy-to');
  });
  it('returns null for a group slot itself, the centre and out of range', () => {
    expect(dropTarget(slots, { slot: 0, child: -1 })).toBeNull();
    expect(dropTarget(slots, { slot: -1, child: -1 })).toBeNull();
    expect(dropTarget(slots, { slot: 12, child: -1 })).toBeNull();
    expect(dropTarget(slots, { slot: 0, child: 7 })).toBeNull();
  });
});

describe('clickAction', () => {
  it('runs presets, opens groups and closes elsewhere', () => {
    expect(clickAction(slots, { slot: 1, child: -1 })).toEqual({ kind: 'run', id: 'to-png' });
    expect(clickAction(slots, { slot: 0, child: 2 })).toEqual({ kind: 'run', id: 'move-to' });
    expect(clickAction(slots, { slot: 0, child: -1 })).toEqual({ kind: 'fan', slot: 0 });
    expect(clickAction(slots, { slot: -1, child: -1 })).toEqual({ kind: 'close' });
    expect(clickAction([slot('group:empty')], { slot: 0, child: -1 })).toEqual({ kind: 'close' });
  });
});

describe('otherFace', () => {
  it('flips only when the other face has slots', () => {
    expect(otherFace('front', slots, slots)).toBe('back');
    expect(otherFace('back', slots, slots)).toBe('front');
    expect(otherFace('front', slots, [])).toBe('front');
    expect(otherFace('back', [], slots)).toBe('back');
  });
});
