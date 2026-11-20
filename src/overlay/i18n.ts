// Tiny overlay-only i18n: reads just overlay.json and presets.json (the full i18n lives in the
// main app). Keys work like the main app's ("overlay.files", "presets.builtin.to-jpg"), so the
// shared wheel code runs with either; bare keys ("toast.jobDone.title") read overlay.json.
import trOverlay from '../locales/tr/overlay.json';
import enOverlay from '../locales/en/overlay.json';
import trPresets from '../locales/tr/presets.json';
import enPresets from '../locales/en/presets.json';

type Lang = 'tr' | 'en';
type Tree = { [k: string]: string | Tree };
const dicts: Record<Lang, { overlay: Record<string, string>; presets: Tree }> = {
  tr: { overlay: trOverlay, presets: trPresets },
  en: { overlay: enOverlay, presets: enPresets },
};
const OVERLAY = 'overlay.';
const PRESETS = 'presets.';
let lang: Lang = 'en';

export function setLang(settingsLanguage: 'system' | Lang, systemLanguage: Lang) {
  lang = settingsLanguage === 'system' ? systemLanguage : settingsLanguage;
}

export function getLang() {
  return lang;
}

export type T = (key: string, params?: Record<string, string | number>) => string;

function own(o: Record<string, unknown>, k: string): string | undefined {
  const v = Object.hasOwn(o, k) ? o[k] : undefined;
  return typeof v === 'string' ? v : undefined;
}

function lookup(l: Lang, key: string, count: number | undefined): string | undefined {
  if (key.startsWith(PRESETS)) {
    let node: string | Tree | undefined = dicts[l].presets;
    for (const part of key.slice(PRESETS.length).split('.')) {
      if (node === undefined || typeof node === 'string') return undefined;
      node = Object.hasOwn(node, part) ? node[part] : undefined;
    }
    return typeof node === 'string' ? node : undefined;
  }
  const k = key.startsWith(OVERLAY) ? key.slice(OVERLAY.length) : key;
  const d = dicts[l].overlay;
  if (count !== undefined) {
    const form = new Intl.PluralRules(l).select(count) === 'one' ? 'one' : 'other';
    const plural = own(d, `${k}_${form}`);
    if (plural !== undefined) return plural;
  }
  return own(d, k);
}

/** Looks up `key`; with `count` in params, prefers `key_one` / `key_other` by plural rule.
 *  Falls back to English, then to the key itself. */
export const t: T = (key, params = {}) => {
  const count = typeof params.count === 'number' ? params.count : undefined;
  const s = lookup(lang, key, count) ?? lookup('en', key, count) ?? key;
  return s.replace(/\{(\w+)\}/g, (_, n: string) => (n in params ? String(params[n]) : `{${n}}`));
};
