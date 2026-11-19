// Tiny i18n: src/locales/<lang>/<namespace>.json, keys are "<namespace>.<path>".
// Supported value shapes: nested objects, flat dotted keys inside a namespace file
// ("toast.jobDone.title"), {name} placeholders, { one, other } plural objects, and
// suffix plurals ("files_one" / "files_other"), all chosen by params.count.
import { langState, type Lang } from './lang.svelte';

export type { Lang };
export type Params = Record<string, string | number>;
export type Tree = { [k: string]: string | Tree };
export type Dict = Record<string, Tree>;

const modules = import.meta.glob('../locales/*/*.json', { eager: true, import: 'default' }) as Record<string, Tree>;

const dicts: Dict = {};
for (const [path, tree] of Object.entries(modules)) {
  const m = /locales\/([^/]+)\/([^/]+)\.json$/.exec(path);
  if (!m) continue;
  (dicts[m[1]] ??= {})[m[2]] = tree;
}

function walk(root: Tree | undefined, path: string): string | Tree | undefined {
  let node: string | Tree | undefined = root;
  for (const part of path.split('.')) {
    if (node === undefined || typeof node === 'string') return undefined;
    node = node[part];
  }
  return node;
}

/** Nested walk first, then a flat dotted key inside the namespace. */
function find(dict: Dict, lang: string, key: string): string | Tree | undefined {
  const dot = key.indexOf('.');
  if (dot < 0) return undefined;
  const ns = dict[lang]?.[key.slice(0, dot)];
  if (!ns || typeof ns === 'string') return undefined;
  const rest = key.slice(dot + 1);
  return walk(ns, rest) ?? ns[rest];
}

function pluralForm(lang: string, count: number): 'one' | 'other' {
  if (count === 1) return 'one';
  try {
    return new Intl.PluralRules(lang).select(count) === 'one' ? 'one' : 'other';
  } catch {
    return 'other';
  }
}

function resolve(dict: Dict, lang: string, key: string, params?: Params): string | undefined {
  const value = find(dict, lang, key);
  if (typeof value === 'string') return value;
  const form = pluralForm(lang, Number(params?.count));
  if (value) {
    const chosen = value[form] ?? value.other;
    return typeof chosen === 'string' ? chosen : undefined;
  }
  if (params?.count === undefined) return undefined;
  const suffixed = find(dict, lang, `${key}_${form}`) ?? find(dict, lang, `${key}_other`);
  return typeof suffixed === 'string' ? suffixed : undefined;
}

function interpolate(s: string, params?: Params): string {
  if (!params) return s;
  return s.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** Translate against an explicit dictionary (exported for tests). */
export function translateIn(dict: Dict, lang: string, key: string, params?: Params): string {
  const raw = resolve(dict, lang, key, params) ?? resolve(dict, 'en', key, params);
  return raw === undefined ? key : interpolate(raw, params);
}

/** Translate `key` in `lang` (pure; falls back to en, then the key itself). */
export function translate(lang: string, key: string, params?: Params): string {
  return translateIn(dicts, lang, key, params);
}

/** Reactive translate: reads the language state, so templates/derived re-run on language change. */
export function t(key: string, params?: Params): string {
  return translate(langState.current, key, params);
}

/** Resolve the settings value (`system` follows the OS language) and make it active. */
export function setLanguage(setting: 'system' | Lang, systemLanguage: Lang = 'en'): Lang {
  const lang: Lang = setting === 'system' ? systemLanguage : setting;
  langState.current = lang;
  if (typeof document !== 'undefined') document.documentElement.lang = lang;
  return lang;
}

export function currentLang(): Lang {
  return langState.current;
}
