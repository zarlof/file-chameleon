// Pure rope drawing recipes: which strokes make a rope of each style. Rope.svelte (and the settings
// preview) draw the same path once per layer.

export type RopeStyle = 'twisted' | 'cord' | 'thin';
export type RopeStroke = { stroke: string; width: number; dash?: string; opacity: number };

/** Light warm grey on dark, darker on light (the default rope colour). */
export function naturalRope(theme: 'dark' | 'light'): string {
  return theme === 'dark' ? '#d9d4cc' : '#8a8378';
}

/** Layers bottom to top. `thickness` is the setting (1..6). */
export function ropeStrokes(style: RopeStyle, color: string, thickness: number): RopeStroke[] {
  const w = Math.min(6, Math.max(1, thickness)) + 1;
  if (style === 'thin') return [{ stroke: color, width: 1, opacity: 0.9 }];
  if (style === 'cord') {
    return [
      { stroke: color, width: w, opacity: 0.95 },
      { stroke: '#ffffff', width: Math.max(0.6, w * 0.3), opacity: 0.35 },
    ];
  }
  const period = Math.max(3, w * 1.6);
  return [
    { stroke: color, width: w, opacity: 0.95 },
    { stroke: 'rgba(0,0,0,0.55)', width: Math.max(0.8, w * 0.7), dash: `${(period * 0.45).toFixed(1)} ${(period * 0.55).toFixed(1)}`, opacity: 0.45 },
    { stroke: '#ffffff', width: Math.max(0.6, w * 0.25), opacity: 0.4 },
  ];
}
