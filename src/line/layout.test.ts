import { describe, expect, it } from 'vitest';
import { SIZES, bandTops, cardOpacity, maxRopes, cardVars, ropeColor, windowHeight } from './layout';

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

  it('layout_four_ropes', () => {
    expect(windowHeight(4, 'medium')).toBe(760);
    expect(windowHeight(4, 'small')).toBe(624);
  });

  it('card_opacity_floor', () => {
    expect(cardOpacity(30)).toBe(0.9);
    expect(cardOpacity(100)).toBe(1);
    expect(cardOpacity(95)).toBe(0.95);
  });

  it('rope_color', () => {
    expect(ropeColor('natural', 'X', 'dark')).toBe('#d9d4cc');
    expect(ropeColor('natural', 'X', 'light')).toBe('#8a8378');
    expect(ropeColor('accent', 'X')).toBe('X');
    expect(ropeColor('#aabbcc', 'X')).toBe('#aabbcc');
    expect(ropeColor('red', 'X')).toBe('var(--red)');
    expect(ropeColor('javascript:1', 'X')).toBe('X');
  });
});

describe('card look', () => {
  it('card_vars', () => {
    expect(cardVars('glassDark', 10, true)['--c-bg']).toBe('rgba(20,20,24,0.78)');
    expect(cardVars('glassDark', 99, false)).toMatchObject({ '--c-radius': '16px', '--c-shadow': 'none' });
    expect(cardVars('glassLight', 4, true)['--c-fg']).toBe('#16161a');
    expect(cardVars('paper', 4, true)['--c-bg']).toBe('var(--elevated-bg)');
    expect(cardVars('bogus', 4, true)['--c-bg']).toBe('rgba(20,20,24,0.78)');
  });
});

describe('maxRopes', () => {
  it('fits whole ropes only, same numbers as the Rust side', () => {
    expect(maxRopes(688, 'medium')).toBe(3);
    expect(maxRopes(760, 'medium')).toBe(4);
    expect(maxRopes(100, 'large')).toBe(1);
    expect(maxRopes(900, 'large')).toBe(3);
    expect(maxRopes(0, 'small')).toBe(1);
  });
});

describe('folded ropes', () => {
  it('take a thin band and still fit whole', async () => {
    const { FOLD_H, fitFolded, foldedHeight, foldedTops, windowHeight } = await import('./layout');
    expect(foldedHeight([false, false], 'medium')).toBe(windowHeight(2, 'medium'));
    expect(foldedHeight([false, true, true], 'medium')).toBe(190 + 2 * FOLD_H);
    expect(foldedTops([true, false, true], 'medium')).toEqual([6, 6 + FOLD_H, 6 + FOLD_H + 190]);
    expect(fitFolded([false, true, false], 250, 'medium')).toBe(2);
    expect(fitFolded([false, true, false], 1000, 'medium')).toBe(3);
    expect(fitFolded([true], 10, 'medium')).toBe(1);
  });
});

describe('fitSize', () => {
  it('steps down until every rope fits', async () => {
    const { fitSize } = await import('./layout');
    const five = [false, false, false, false, false];
    expect(fitSize(five, 950, 'large')).toBe('medium');
    expect(fitSize([false, false, false, false, true], 966, 'large')).toBe('large');
    expect(fitSize(five, 700, 'medium')).toBe('small');
    expect(fitSize(five, 100, 'small')).toBe('small');
    expect(fitSize([false], 1000, 'odd')).toBe('medium');
  });
});
