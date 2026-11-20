import { describe, expect, it } from 'vitest';

type Tree = { [k: string]: string | Tree };
const modules = import.meta.glob('./*/{common,convert,recent,settings}.json', { eager: true, import: 'default' }) as Record<string, Tree>;

function keys(tree: Tree, prefix = ''): string[] {
  return Object.entries(tree).flatMap(([k, v]) => (typeof v === 'string' ? [prefix + k] : keys(v, `${prefix}${k}.`)));
}

describe('common / convert / recent / settings locales', () => {
  for (const ns of ['common', 'convert', 'recent', 'settings']) {
    it(`${ns}: tr and en define the same keys`, () => {
      const en = keys(modules[`./en/${ns}.json`]).sort();
      const tr = keys(modules[`./tr/${ns}.json`]).sort();
      expect(en.length).toBeGreaterThan(5);
      expect(tr).toEqual(en);
    });
  }
});
