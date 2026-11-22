import { describe, expect, it } from 'vitest';
import { naturalRope, ropeStrokes } from './ropeStyle';

describe('ropeStyle', () => {
  it('natural_colour_follows_the_theme', () => {
    expect(naturalRope('dark')).toBe('#d9d4cc');
    expect(naturalRope('light')).toBe('#8a8378');
  });
  it('every_layer_is_drawable', () => {
    for (const t of [1, 3, 6, 99]) {
      for (const l of ropeStrokes('red', t)) {
        expect(l.width).toBeGreaterThan(0);
        expect(l.opacity).toBeGreaterThan(0);
        expect(l.opacity).toBeLessThanOrEqual(1);
        expect(l.stroke).toBeTruthy();
      }
    }
  });
  it('silk_rope_with_an_edge', () => {
    const web = ropeStrokes('#ffffff', 2);
    expect(web).toHaveLength(5);
    expect(web[0].width).toBeGreaterThan(web[1].width); // the dark edge is under, and wider
    expect(web[1].stroke).toBe('#ffffff');
    expect(web.some((l) => l.dash)).toBe(true);
  });
});
