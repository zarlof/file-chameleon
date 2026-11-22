// The line's right-click menu as data: which entries a card gets, in order. LineMenu.svelte draws
// them; Line.svelte supplies what each entry does.
import type { LineCard } from './logic';

export type MenuEntry = { label: string; run: () => unknown; danger?: boolean } | 'sep';

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
  removeShortcut: () => unknown;
}

export function menuEntries(c: LineCard, a: MenuActions, t: (key: string) => string): MenuEntry[] {
  const e = (key: string, run: () => unknown, danger = false): MenuEntry => ({ label: t(key), run, danger });
  if (c.app) {
    const path = c.app.path;
    return [e('menu.open', a.open), e('menu.reveal', () => a.reveal(path)), e('menu.copyPath', () => a.copyPath(path)), 'sep', e('menu.removeShortcut', a.removeShortcut, true)];
  }
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
      'sep',
      e('menu.removeFromList', a.trash, true),
    ];
  }
  const it = c.item;
  if (!it) return [];
  const viewable = it.kind === 'image' || it.kind === 'text';
  const out: MenuEntry[] = [e(viewable ? 'menu.open' : 'menu.openLink', a.open), e('menu.copy', a.copy), e('menu.paste', a.paste)];
  if (viewable) out.push(e('menu.edit', a.edit));
  out.push(e(it.pinned ? 'menu.unpin' : 'menu.pin', a.pin), e('menu.share', a.share));
  if (it.kind === 'image') {
    const file = it.edited ?? it.image;
    if (file) out.push(e('menu.float', () => a.float(file)));
    out.push(e('menu.save', a.save));
  }
  out.push('sep', e('menu.delete', a.trash, true));
  return out;
}

/** About how tall the menu is (to keep it on screen): rows, separators and the close bar. */
export function menuHeight(entries: MenuEntry[]): number {
  return 26 + 8 + entries.reduce((h, x) => h + (x === 'sep' ? 9 : 30), 0);
}
