// Pure rope drawing recipe: the strokes of the spider-silk rope. Rope.svelte (and the settings
// preview) draw the same path once per layer.

export type RopeStroke = { stroke: string; width: number; dash?: string; opacity: number };

/** Light warm grey on dark, darker on light (the "natural" rope colour). */
export function naturalRope(theme: 'dark' | 'light'): string {
  return theme === 'dark' ? '#d9d4cc' : '#8a8378';
}

/** Layers bottom to top. `thickness` is the setting (1..6). Spider silk twisted into a rope: a dark
 *  edge (so it shows on white screens), the base colour, cream and grey twists, a white sheen. */
export function ropeStrokes(color: string, thickness: number): RopeStroke[] {
  const w = Math.min(6, Math.max(1, thickness)) + 1;
  const period = Math.max(4, w * 2.2);
  const dash = `${(period * 0.5).toFixed(1)} ${(period * 0.5).toFixed(1)}`;
  return [
    { stroke: 'rgba(40,36,32,0.55)', width: w + 1.6, opacity: 1 },
    { stroke: color, width: w, opacity: 1 },
    { stroke: '#ddd3c0', width: Math.max(0.8, w * 0.75), dash, opacity: 0.95 },
    { stroke: '#a49e95', width: Math.max(0.6, w * 0.4), dash: `0 ${(period * 0.5).toFixed(1)} ${(period * 0.25).toFixed(1)} ${(period * 0.25).toFixed(1)}`, opacity: 0.8 },
    { stroke: '#ffffff', width: Math.max(0.5, w * 0.22), opacity: 0.85 },
  ];
}
