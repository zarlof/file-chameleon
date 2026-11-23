// Quick text actions on the line's text and link cards (right-click > "Change text"). Each one is
// a pure function: the line copies the result as a new clip. `null` = nothing to do for this text
// (the menu leaves the entry out).

export type TextActionId = 'upper' | 'lower' | 'title' | 'spaces' | 'cleanLink' | 'json' | 'ascii';

/** Locale for case changes: Turkish "i" ↔ "İ" and "ı" ↔ "I" only in Turkish. */
type CaseLocale = 'tr' | 'en';

/** Query parameters that only track where a click came from. */
const TRACKING = /^(utm_[a-z_]+|fbclid|gclid|dclid|gbraid|wbraid|msclkid|yclid|igshid|igsh|mc_cid|mc_eid|_hsenc|_hsmi|mkt_tok|si|ref_src|ref_url|spm|share_source|feature)$/i;

const TR_ASCII: Record<string, string> = { ç: 'c', Ç: 'C', ğ: 'g', Ğ: 'G', ı: 'i', İ: 'I', ö: 'o', Ö: 'O', ş: 's', Ş: 'S', ü: 'u', Ü: 'U' };

export function upper(text: string, locale: CaseLocale): string {
  return text.toLocaleUpperCase(locale);
}

export function lower(text: string, locale: CaseLocale): string {
  return text.toLocaleLowerCase(locale);
}

/** Every word starting with a capital, the rest small. */
export function titleCase(text: string, locale: CaseLocale): string {
  return text
    .toLocaleLowerCase(locale)
    .replace(/(^|[\s\-–—("'“‘/])(\p{L})/gu, (_, pre: string, ch: string) => pre + ch.toLocaleUpperCase(locale));
}

/** Runs of spaces and tabs become one space, lines are trimmed, at most one empty line in a row. */
export function tidySpaces(text: string): string {
  return text
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((l) => l.replace(/[ \t ]+/g, ' ').trim())
    .join('\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

/** The link without tracking parameters (utm_*, fbclid, ...); `null` when it is not a link or has none. */
export function cleanLink(text: string): string | null {
  const raw = text.trim();
  if (!/^https?:\/\/\S+$/i.test(raw)) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  const drop = [...url.searchParams.keys()].filter((k) => TRACKING.test(k));
  if (drop.length === 0) return null;
  for (const k of drop) url.searchParams.delete(k);
  let out = url.toString();
  // No query left: no lone "?" (URL keeps none), and no trailing slash the original did not have.
  if (!raw.includes('/?') && out.endsWith('/') && !raw.split('?')[0].endsWith('/')) out = out.slice(0, -1);
  return out;
}

/** JSON indented with two spaces; `null` when the text is not a JSON object or array. */
export function prettyJson(text: string): string | null {
  const t = text.trim();
  if (!(t.startsWith('{') || t.startsWith('['))) return null;
  try {
    return JSON.stringify(JSON.parse(t), null, 2);
  } catch {
    return null;
  }
}

/** Turkish letters spelled without accents (ş → s, ğ → g, ı → i...); `null` when there are none. */
export function asciiTurkish(text: string): string | null {
  if (!/[çÇğĞıİöÖşŞüÜ]/.test(text)) return null;
  return text.replace(/[çÇğĞıİöÖşŞüÜ]/g, (c) => TR_ASCII[c] ?? c);
}

/** The result of `id` on `text`; `null` when the action does nothing for it. */
export function applyTextAction(id: TextActionId, text: string, locale: CaseLocale): string | null {
  let out: string | null;
  switch (id) {
    case 'upper':
      out = upper(text, locale);
      break;
    case 'lower':
      out = lower(text, locale);
      break;
    case 'title':
      out = titleCase(text, locale);
      break;
    case 'spaces':
      out = tidySpaces(text);
      break;
    case 'cleanLink':
      out = cleanLink(text);
      break;
    case 'json':
      out = prettyJson(text);
      break;
    case 'ascii':
      out = asciiTurkish(text);
      break;
  }
  return out === null || out === text ? null : out;
}

/** The actions offered for `text`, in menu order: only those that would change it. */
export function textActionsFor(text: string, locale: CaseLocale): TextActionId[] {
  // A link: changing its case or letters would break it.
  if (/^https?:\/\/\S+$/i.test(text.trim())) return applyTextAction('cleanLink', text, locale) === null ? [] : ['cleanLink'];
  // JSON: only tidied (case changes would break its keys).
  if (prettyJson(text) !== null) return applyTextAction('json', text, locale) === null ? [] : ['json'];
  const all: TextActionId[] = ['upper', 'lower', 'title', 'spaces', 'ascii'];
  return all.filter((id) => applyTextAction(id, text, locale) !== null);
}
