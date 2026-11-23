// The line's right-click menu as data: which entries a card gets, in order. LineMenu.svelte draws
// them; Line.svelte supplies what each entry does.
import type { LineCard } from './logic';
import { textActionsFor, type TextActionId } from './textActions';

/** A row that runs something, a row that opens a sub-list in place (`sub`), or a separator. */
export type MenuEntry = { label: string; run: () => unknown; danger?: boolean } | { label: string; sub: MenuEntry[] } | 'sep';

/** What the menu can do; each acts on the card the menu was opened on. */
export interface MenuActions {
  open: () => unknown;
  copy: () => unknown;
  paste: () => unknown;
  edit: () => unknown;
  pin: () => unknown;
  share: () => unknown;
  trash: () => unknown;
  reveal: (path: string) => unknown;
  copyPath: (path: string) => unknown;
  convert: (path: string) => unknown;
  float: (file: string) => unknown;
  save: () => unknown;
  /** The card's own settings (its colour). */
  card: () => unknown;
  /** An image card's text (OCR) copied as a new clip. */
  imageText: () => unknown;
  /** A text action's result on a text or link card copied as a new clip. */
  textAction: (id: TextActionId) => unknown;
}

export function menuEntries(c: LineCard, a: MenuActions, t: (key: string) => string, locale: 'tr' | 'en' = 'en'): MenuEntry[] {
  const e = (key: string, run: () => unknown, danger = false): MenuEntry => ({ label: t(key), run, danger });
  if (c.file) {
    const f = c.file;
    return [
      e('menu.open', a.open),
      e('menu.copy', a.copy),
      e('menu.reveal', () => a.reveal(f.path)),
      e('menu.convert', () => a.convert(f.path)),
      e('menu.copyPath', () => a.copyPath(f.path)),
      e(f.pinned ? 'menu.unpin' : 'menu.pin', a.pin),
      e('menu.share', a.share),
      e('menu.card', a.card),
      'sep',
      e('menu.removeFromList', a.trash, true),
    ];
  }
  const it = c.item;
  if (!it) return [];
  const viewable = it.kind === 'image' || it.kind === 'text';
  const out: MenuEntry[] = [e(viewable ? 'menu.open' : 'menu.openLink', a.open), e('menu.copy', a.copy), e('menu.paste', a.paste)];
  if (viewable) out.push(e('menu.edit', a.edit));
  if (it.kind !== 'image') {
    const actions = textActionsFor(it.text ?? '', locale);
    if (actions.length > 0) out.push({ label: t('menu.textActions'), sub: actions.map((id) => e(`text.${id}`, () => a.textAction(id))) });
  }
  out.push(e(it.pinned ? 'menu.unpin' : 'menu.pin', a.pin), e('menu.share', a.share));
  if (it.kind === 'image') {
    out.push(e('menu.copyText', a.imageText));
    const file = it.edited ?? it.image;
    if (file) out.push(e('menu.float', () => a.float(file)));
    out.push(e('menu.save', a.save));
  }
  out.push(e('menu.card', a.card), 'sep', e('menu.delete', a.trash, true));
  return out;
}

/** About how tall the menu is (to keep it on screen): rows, separators and the close bar; the
 *  tallest of the list and its sub-lists (a sub-list adds its "back" row). */
export function menuHeight(entries: MenuEntry[]): number {
  const own = 26 + 8 + entries.reduce((h, x) => h + (x === 'sep' ? 9 : 30), 0);
  const subs = entries.map((x) => (x !== 'sep' && 'sub' in x ? menuHeight(x.sub) + 30 : 0));
  return Math.max(own, ...subs);
}
