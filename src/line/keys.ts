// Keyboard model of the line: where the focus moves, and which keys are card commands.

export type Focus = { r: number; i: number };
export type NavKey = 'ArrowLeft' | 'ArrowRight' | 'ArrowUp' | 'ArrowDown' | 'Home' | 'End';

/** Moves the focus over ropes holding `ropes[r]` cards. Up/Down keep the index (clamped); empty ropes are skipped. */
export function nextFocus(ropes: number[], cur: Focus | null, key: NavKey): Focus | null {
  const filled = ropes.map((n, r) => (n > 0 ? r : -1)).filter((r) => r >= 0);
  if (filled.length === 0) return null;
  if (!cur || ropes[cur.r] === undefined || ropes[cur.r] === 0) return { r: filled[0], i: 0 };
  const last = ropes[cur.r] - 1;
  const i = Math.min(Math.max(cur.i, 0), last);
  switch (key) {
    case 'ArrowLeft':
      return { r: cur.r, i: Math.max(0, i - 1) };
    case 'ArrowRight':
      return { r: cur.r, i: Math.min(last, i + 1) };
    case 'Home':
      return { r: cur.r, i: 0 };
    case 'End':
      return { r: cur.r, i: last };
    case 'ArrowUp':
    case 'ArrowDown': {
      const at = filled.indexOf(cur.r);
      const to = filled[at + (key === 'ArrowDown' ? 1 : -1)];
      return to === undefined ? { r: cur.r, i } : { r: to, i: Math.min(i, ropes[to] - 1) };
    }
  }
}

/** A key that runs a card command. While the search field has focus every key is text (Esc and Enter are handled apart). */
export function isCommandKey(e: KeyboardEvent, searchFocused: boolean): boolean {
  if (searchFocused) return false;
  const ctrl = e.ctrlKey || e.metaKey;
  if (ctrl) return e.key === 'Enter' || ['c', 'z'].includes(e.key.toLowerCase());
  if (e.altKey) return false;
  return (
    e.key === 'Delete' || e.key === ' ' || e.key === 'Enter' || e.key === 'Home' || e.key === 'End' || e.key.startsWith('Arrow') ||
    /^[1-9]$/.test(e.key) || e.key === 'p' || e.key === 'P' || e.key === 'e' || e.key === 'E'
  );
}
