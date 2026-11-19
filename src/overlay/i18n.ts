// Tiny overlay-only i18n: reads just overlay.json (the full i18n lives in the main app).
import tr from '../locales/tr/overlay.json';
import en from '../locales/en/overlay.json';

type Dict = Record<string, string>;
const dicts: Record<'tr' | 'en', Dict> = { tr, en };
let lang: 'tr' | 'en' = 'en';

export function setLang(settingsLanguage: 'system' | 'tr' | 'en', systemLanguage: 'tr' | 'en') {
  lang = settingsLanguage === 'system' ? systemLanguage : settingsLanguage;
}

export function getLang() {
  return lang;
}

export type T = (key: string, params?: Record<string, string | number>) => string;

/** Looks up `key`; with `count` in params, prefers `key_one` / `key_other` by plural rule. */
export const t: T = (key, params = {}) => {
  let k = key;
  if (typeof params.count === 'number') {
    const form = new Intl.PluralRules(lang).select(params.count);
    const pk = `${key}_${form === 'one' ? 'one' : 'other'}`;
    if (pk in dicts[lang]) k = pk;
  }
  const s = dicts[lang][k] ?? dicts.en[k] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, n: string) => (n in params ? String(params[n]) : `{${n}}`));
};
