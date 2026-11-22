import { describe, expect, it } from 'vitest';
import { naturalRope, ropeStrokes, webExtras } from './ropeStyle';

describe('ropeStyle', () => {
  it('rope_strokes_layers', () => {
    const tw = ropeStrokes('twisted', '#d9d4cc', 3);
    expect(tw).toHaveLength(3);
    expect(tw[0].dash).toBeUndefined();
    expect(tw[1].dash).toBeTruthy();
    expect(tw[1].width).toBeLessThan(tw[0].width);
    expect(ropeStrokes('cord', '#d9d4cc', 3)).toHaveLength(2);
    const thin = ropeStrokes('thin', '#d9d4cc', 5);
    expect(thin).toHaveLength(1);
    expect(thin[0].width).toBe(1);
    expect(naturalRope('dark')).toBe('#d9d4cc');
    expect(naturalRope('light')).toBe('#8a8378');
  });
  it('every_layer_is_drawable', () => {
    for (const style of ['web', 'twisted', 'cord', 'thin'] as const) {
      for (const l of ropeStrokes(style, 'red', 2)) {
        expect(l.width).toBeGreaterThan(0);
        expect(l.opacity).toBeGreaterThan(0);
        expect(l.opacity).toBeLessThanOrEqual(1);
        expect(l.stroke).toBeTruthy();
      }
    }
  });
  it('web_is_one_fine_strand_with_extras', () => {
    const web = ropeStrokes('web', '#ffffff', 2);
    expect(web).toHaveLength(1);
    expect(web[0].stroke).toBe('#ffffff');
    expect(web[0].width).toBeLessThan(2);
    const x = webExtras(1000, 14, 22);
    expect(x.strand.startsWith('M0.0 0.00')).toBe(true);
    expect(x.rungs.split('M').length - 1).toBeGreaterThan(5);
    // the corner fans reach the top edge at both ends
    expect(x.anchors).toContain('L0.0 -22.0');
    expect(x.anchors).toContain('L1000.0 -22.0');
    expect(webExtras(0, 14, 22).rungs).toBe('');
  });
});
