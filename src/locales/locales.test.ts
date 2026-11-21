import { describe, expect, it } from 'vitest';

type Tree = { [k: string]: string | Tree };
const modules = import.meta.glob('./*/*.json', { eager: true, import: 'default' }) as Record<string, Tree>;

function keys(tree: Tree, prefix = ''): string[] {
  return Object.entries(tree).flatMap(([k, v]) => (typeof v === 'string' ? [prefix + k] : keys(v, `${prefix}${k}.`)));
}

const namespaces = [...new Set(Object.keys(modules).map((p) => /\.\/[^/]+\/(.+)\.json$/.exec(p)?.[1] ?? ''))].filter(Boolean);

describe('locales', () => {
  it('finds every namespace', () => {
    expect(namespaces.length).toBeGreaterThanOrEqual(12);
  });
  for (const ns of namespaces) {
    it(`${ns}: tr and en define the same keys`, () => {
      const en = modules[`./en/${ns}.json`];
      const tr = modules[`./tr/${ns}.json`];
      expect(en, `en/${ns}.json missing`).toBeDefined();
      expect(tr, `tr/${ns}.json missing`).toBeDefined();
      expect(keys(tr).sort()).toEqual(keys(en).sort());
    });
  }
});
