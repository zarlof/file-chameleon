import { describe, expect, it } from 'vitest';
import { firstSelectable, moveSelection, numberPick, toggleSelect } from './logic';

const grid = [[1, 2, 3, 4, 5], [], [6, 7]];

describe('moveSelection', () => {
  it('moves within a row and clamps at the edges', () => {
    expect(moveSelection(grid, { row: 0, col: 1 }, 'ArrowRight')).toEqual({ row: 0, col: 2 });
    expect(moveSelection(grid, { row: 0, col: 0 }, 'ArrowLeft')).toEqual({ row: 0, col: 0 });
    expect(moveSelection(grid, { row: 2, col: 1 }, 'ArrowRight')).toEqual({ row: 2, col: 1 });
  });
  it('jumps to Home / End', () => {
    expect(moveSelection(grid, { row: 0, col: 2 }, 'End')).toEqual({ row: 0, col: 4 });
    expect(moveSelection(grid, { row: 0, col: 2 }, 'Home')).toEqual({ row: 0, col: 0 });
  });
  it('moves between rows of different lengths, skipping empty ones, clamping the column', () => {
    expect(moveSelection(grid, { row: 0, col: 4 }, 'ArrowDown')).toEqual({ row: 2, col: 1 });
    expect(moveSelection(grid, { row: 2, col: 1 }, 'ArrowUp')).toEqual({ row: 0, col: 1 });
  });
  it('stays put when there is no row in that direction', () => {
    expect(moveSelection(grid, { row: 0, col: 2 }, 'ArrowUp')).toEqual({ row: 0, col: 2 });
    expect(moveSelection(grid, { row: 2, col: 0 }, 'ArrowDown')).toEqual({ row: 2, col: 0 });
  });
});

describe('numberPick', () => {
  it('picks the n-th card, 1-based, up to 9', () => {
    expect(numberPick([10, 20, 30], 2)).toBe(20);
    expect(numberPick([10, 20, 30], 4)).toBeNull();
    expect(numberPick([10], 0)).toBeNull();
    expect(numberPick(Array.from({ length: 12 }, (_, i) => i), 10)).toBeNull();
  });
});

describe('toggleSelect', () => {
  const order = [1, 2, 3, 4, 5];
  it('plain click selects only that id', () => {
    expect(toggleSelect([1, 2], 4, {}, order)).toEqual([4]);
  });
  it('ctrl toggles', () => {
    expect(toggleSelect([1], 3, { ctrl: true }, order)).toEqual([1, 3]);
    expect(toggleSelect([1, 3], 1, { ctrl: true }, order)).toEqual([3]);
  });
  it('shift selects the range from the last selected id, either direction', () => {
    expect(toggleSelect([2], 4, { shift: true }, order)).toEqual([2, 3, 4]);
    expect(toggleSelect([4], 2, { shift: true }, order)).toEqual([2, 3, 4]);
  });
  it('repeated shift steps from a fixed anchor grow, not slide', () => {
    let sel = toggleSelect([], 2, { shift: true }, order, 2);
    sel = toggleSelect(sel, 3, { shift: true }, order, 2);
    sel = toggleSelect(sel, 4, { shift: true }, order, 2);
    expect(sel).toEqual([2, 3, 4]);
    sel = toggleSelect(sel, 5, { shift: true }, order, 2);
    expect(sel).toEqual([2, 3, 4, 5]);
  });
  it('shift-left steps from the same anchor, and can cross it', () => {
    let sel = toggleSelect([], 4, { shift: true }, order, 4);
    sel = toggleSelect(sel, 3, { shift: true }, order, 4);
    sel = toggleSelect(sel, 2, { shift: true }, order, 4);
    expect(sel).toEqual([2, 3, 4]);
    expect(toggleSelect(sel, 5, { shift: true }, order, 4)).toEqual([4, 5]);
  });
  it('shift without a selection behaves like a plain click', () => {
    expect(toggleSelect([], 3, { shift: true }, order)).toEqual([3]);
  });
});

describe('firstSelectable', () => {
  it('skips empty rows', () => {
    expect(firstSelectable([[], [], [9]])).toEqual({ row: 2, col: 0 });
    expect(firstSelectable([[1], [2]])).toEqual({ row: 0, col: 0 });
  });
  it('is null when everything is empty', () => {
    expect(firstSelectable([[], []])).toBeNull();
    expect(firstSelectable([])).toBeNull();
  });
});
