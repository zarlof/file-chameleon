// Tiny i18n: src/locales/<lang>/<namespace>.json, keys are "<namespace>.<path>".
// Values may contain {name} placeholders, or be { one, other } plural objects chosen by params.count.
import { langState, type Lang } from './lang.svelte';

export type { Lang };
export type Params = Record<string, string | number>;
type Tree = { [k: string]: string | Tree };

const modules = import.meta.glob('../locales/*/*.json', { eager: true, import: 'default' }) as Record<string, Tree>;

const dicts: Record<string, Tree> = {};
for (const [path, tree] of Object.entries(modules)) {
  const m = /locales\/([^/]+)\/([^/]+)\.json$/.exec(path);
  if (!m) continue;
  (dicts[m[1]] ??= {})[m[2]] = tree;
}

function lookup(lang: string, key: string): string | Tree | undefined {
  let node: string | Tree | undefined = dicts[lang];
  for (const part of key.split('.')) {
    if (node === undefined || typeof node === 'string') return undefined;
    node = node[part];
  }
  return node;
}

function pick(value: string | Tree | undefined, params?: Params): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value === 'string') return value;
  // Plural object.
  const count = Number(params?.count);
  const form = count === 1 ? 'one' : 'other';
  const chosen = value[form] ?? value.other;
  return typeof chosen === 'string' ? chosen : undefined;
}

function interpolate(s: string, params?: Params): string {
  if (!params) return s;
  return s.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** Translate `key` in `lang` (pure; falls back to en, then the key itself). */
export function translate(lang: string, key: string, params?: Params): string {
  const raw = pick(lookup(lang, key), params) ?? pick(lookup('en', key), params);
  return raw === undefined ? key : interpolate(raw, params);
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
