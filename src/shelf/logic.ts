// Pure keyboard / selection logic of the shelf (no DOM, no IPC).

/** Card ids per visible row, in display order. Rows may be empty. */
export type Grid = number[][];
export interface Pos {
  row: number;
  col: number;
}
export type NavKey = 'ArrowLeft' | 'ArrowRight' | 'ArrowUp' | 'ArrowDown' | 'Home' | 'End';

/** First card of the first non-empty row, or null when every row is empty. */
export function firstSelectable(rows: Grid): Pos | null {
  const row = rows.findIndex((r) => r.length > 0);
  return row < 0 ? null : { row, col: 0 };
}

function clamp(n: number, lo: number, hi: number) {
  return Math.max(lo, Math.min(hi, n));
}

/** Moves the cursor. Left/Right/Home/End stay in the row; Up/Down jump to the nearest non-empty
 *  row (skipping empty ones) and clamp the column to that row's length. Edges do not wrap. */
export function moveSelection(grid: Grid, pos: Pos, key: NavKey): Pos {
  const len = grid[pos.row]?.length ?? 0;
  switch (key) {
    case 'ArrowLeft':
      return len ? { row: pos.row, col: clamp(pos.col - 1, 0, len - 1) } : pos;
    case 'ArrowRight':
      return len ? { row: pos.row, col: clamp(pos.col + 1, 0, len - 1) } : pos;
    case 'Home':
      return len ? { row: pos.row, col: 0 } : pos;
    case 'End':
      return len ? { row: pos.row, col: len - 1 } : pos;
    case 'ArrowUp':
    case 'ArrowDown': {
      const step = key === 'ArrowUp' ? -1 : 1;
      for (let r = pos.row + step; r >= 0 && r < grid.length; r += step) {
        if (grid[r].length > 0) return { row: r, col: clamp(pos.col, 0, grid[r].length - 1) };
      }
      return pos;
    }
  }
}

/** Id of the n-th card (1-9) of a row, or null. */
export function numberPick(row: number[], n: number): number | null {
  if (!Number.isInteger(n) || n < 1 || n > 9) return null;
  return row[n - 1] ?? null;
}

/** Selection after a click or Shift+arrow. Shift = range between `anchor` (default: the last selected id)
 *  and `id`, ascending in `order`; Ctrl = toggle; plain = just it. `order` is the display order of the
 *  ids the range runs over (usually one row). Keep `anchor` fixed while extending a range. */
export function toggleSelect(
  sel: number[],
  id: number,
  mods: { shift?: boolean; ctrl?: boolean },
  order: number[],
  anchor?: number | null,
): number[] {
  if (mods.shift) {
    const from = order.indexOf(anchor ?? sel[sel.length - 1] ?? NaN);
    const to = order.indexOf(id);
    if (from >= 0 && to >= 0) {
      const [a, b] = from <= to ? [from, to] : [to, from];
      return order.slice(a, b + 1);
    }
  }
  if (mods.ctrl) return sel.includes(id) ? sel.filter((x) => x !== id) : [...sel, id];
  return [id];
}
