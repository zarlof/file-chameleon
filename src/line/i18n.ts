// Tiny line-only i18n: reads just line.json (the full i18n lives in the main app).
import trLine from '../locales/tr/line.json';
import enLine from '../locales/en/line.json';

type Lang = 'tr' | 'en';
const dicts: Record<Lang, Record<string, string>> = { tr: trLine, en: enLine };
let lang: Lang = 'en';

export function setLang(settingsLanguage: 'system' | Lang, systemLanguage: Lang) {
  lang = settingsLanguage === 'system' ? systemLanguage : settingsLanguage;
}

export function getLang() {
  return lang;
}

function own(o: Record<string, string>, k: string): string | undefined {
  return Object.hasOwn(o, k) ? o[k] : undefined;
}

function lookup(l: Lang, key: string, count: number | undefined): string | undefined {
  if (count !== undefined) {
    const form = new Intl.PluralRules(l).select(count) === 'one' ? 'one' : 'other';
    const plural = own(dicts[l], `${key}_${form}`);
    if (plural !== undefined) return plural;
  }
  return own(dicts[l], key);
}

/** Looks up `key`; with `count` in params prefers `key_one` / `key_other`. Falls back to English, then the key. */
export function t(key: string, params: Record<string, string | number> = {}): string {
  const count = typeof params.count === 'number' ? params.count : undefined;
  const s = lookup(lang, key, count) ?? lookup('en', key, count) ?? key;
  return s.replace(/\{(\w+)\}/g, (_, n: string) => (n in params ? String(params[n]) : `{${n}}`));
}
