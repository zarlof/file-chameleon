// Pure rope drawing recipes: which strokes make a rope of each style. Rope.svelte (and the settings
// preview) draw the same path once per layer.

export type RopeStyle = 'web' | 'twisted' | 'cord' | 'thin';
export type RopeStroke = { stroke: string; width: number; dash?: string; opacity: number };

/** Light warm grey on dark, darker on light (the default rope colour). */
export function naturalRope(theme: 'dark' | 'light'): string {
  return theme === 'dark' ? '#d9d4cc' : '#8a8378';
}

/** Layers bottom to top. `thickness` is the setting (1..6). */
export function ropeStrokes(style: RopeStyle, color: string, thickness: number): RopeStroke[] {
  const w = Math.min(6, Math.max(1, thickness)) + 1;
  // Spider silk: one fine strand (the second strand, the rungs and the corner webs come from webExtras).
  if (style === 'web') return [{ stroke: color, width: 0.6 + w * 0.35, opacity: 0.95 }];
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

/** The parts of a spider-web rope besides the main strand, as SVG path data in the rope's own
 *  coordinates (y = 0 at the ends, `top` px of room above): a second, deeper strand, short silk
 *  rungs between the two, and a small fan of threads anchoring each end to the top edge. */
export function webExtras(width: number, sag: number, top: number): { strand: string; rungs: string; anchors: string } {
  const deep = sag + 7;
  const y = (x: number, s: number) => {
    const u = width > 0 ? (2 * x) / width - 1 : 0;
    return s * (1 - u * u);
  };
  const n = Math.max(8, Math.min(120, Math.round(width / 16)));
  let strand = '';
  for (let i = 0; i <= n; i++) {
    const x = (width * i) / n;
    strand += `${i === 0 ? 'M' : 'L'}${x.toFixed(1)} ${y(x, deep).toFixed(2)} `;
  }
  let rungs = '';
  for (let x = 40; x + 18 < width - 30; x += 74) {
    rungs += `M${x.toFixed(1)} ${y(x, sag).toFixed(2)} L${(x + 18).toFixed(1)} ${y(x + 18, deep).toFixed(2)} `;
  }
  const fan = (x0: number, dir: 1 | -1) => {
    const h = Math.max(6, top);
    const ends = [
      [0, -h],
      [h * 0.75, -h],
      [h * 1.7, -h * 0.55],
    ];
    let d = '';
    const pts = ends.map(([dx, dy]) => [x0 + dir * dx, dy]);
    for (const [px, py] of pts) d += `M${x0} 0 L${px.toFixed(1)} ${py.toFixed(1)} `;
    // two rings across the spokes
    for (const f of [0.45, 0.8]) {
      d += pts.map(([px, py], i) => `${i === 0 ? 'M' : 'L'}${(x0 + (px - x0) * f).toFixed(1)} ${(py * f).toFixed(1)}`).join(' ') + ' ';
    }
    return d;
  };
  return { strand: strand.trim(), rungs: rungs.trim(), anchors: (fan(0, 1) + fan(width, -1)).trim() };
}
