import { describe, expect, it } from 'vitest';
import { SIZES, bandTops, cardOpacity, ropeColor, windowHeight } from './layout';

describe('layout', () => {
  it('layout_table_and_symmetry', () => {
    expect(bandTops(3, 'medium')).toEqual([6, 196, 386]);
    expect(windowHeight(3, 'medium')).toBe(570);
    expect(windowHeight(1, 'small')).toBe(156);
    expect(windowHeight(1, 'large')).toBe(234);
    for (const size of ['small', 'medium', 'large'] as const) {
      const tops = bandTops(3, size);
      for (let i = 1; i < tops.length; i++) expect(tops[i] - tops[i - 1]).toBe(SIZES[size].band + 12);
      // top margin = bottom margin = half a gap
      expect(windowHeight(3, size) - (tops[2] + SIZES[size].band)).toBe(tops[0]);
    }
  });

  it('card_opacity_floor', () => {
    expect(cardOpacity(30)).toBe(0.9);
    expect(cardOpacity(100)).toBe(1);
    expect(cardOpacity(95)).toBe(0.95);
  });

  it('rope_color', () => {
    expect(ropeColor('accent', 'X')).toBe('X');
    expect(ropeColor('#aabbcc', 'X')).toBe('#aabbcc');
    expect(ropeColor('red', 'X')).toBe('var(--red)');
    expect(ropeColor('javascript:1', 'X')).toBe('X');
  });
});
