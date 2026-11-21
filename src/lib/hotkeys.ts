// Global hotkey strings ("Ctrl+Alt+V"), as stored in `Settings.hotkey*` and parsed by the
// backend (`src-tauri/src/platform/hotkeys.rs` `parse_hotkey`): modifiers first in the order
// Ctrl, Alt, Shift, Win, then one key. Key names follow the physical key (`KeyboardEvent.code`),
// so a Turkish layout records the same combination Windows registers.

/** The fields of a `KeyboardEvent` these helpers read (a real event works too). */
export interface KeyLike {
  key?: string;
  code?: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  metaKey?: boolean;
}

const MODIFIERS = ['Ctrl', 'Alt', 'Shift', 'Win'] as const;

/** Lowercase name -> display name, for every key the backend accepts besides A–Z, 0–9, F1–F24, Num0–Num9. */
const NAMED: Record<string, string> = {
  space: 'Space',
  enter: 'Enter',
  tab: 'Tab',
  esc: 'Esc',
  escape: 'Esc',
  backspace: 'Backspace',
  insert: 'Insert',
  delete: 'Delete',
  home: 'Home',
  end: 'End',
  pageup: 'PageUp',
  pagedown: 'PageDown',
  left: 'Left',
  up: 'Up',
  right: 'Right',
  down: 'Down',
  arrowleft: 'Left',
  arrowup: 'Up',
  arrowright: 'Right',
  arrowdown: 'Down',
  printscreen: 'PrintScreen',
  pause: 'Pause',
};

const MODIFIER_NAMES: Record<string, (typeof MODIFIERS)[number]> = {
  ctrl: 'Ctrl',
  control: 'Ctrl',
  alt: 'Alt',
  shift: 'Shift',
  win: 'Win',
  meta: 'Win',
  super: 'Win',
};

const FUNCTION_KEY = /^F([1-9]|1\d|2[0-4])$/;

/** Canonical name of one key token ("pagedown" -> "PageDown", "v" -> "V"), null when unsupported. */
function keyToken(raw: string): string | null {
  const s = raw.trim();
  if (/^[a-z0-9]$/i.test(s)) return s.toUpperCase();
  const f = /^f(\d{1,2})$/i.exec(s);
  if (f && FUNCTION_KEY.test(`F${Number(f[1])}`)) return `F${Number(f[1])}`;
  const num = /^num(?:pad)?(\d)$/i.exec(s);
  if (num) return `Num${num[1]}`;
  return NAMED[s.toLowerCase()] ?? null;
}

/** Key name from `code` (physical key), else from `key`. */
function keyName(e: KeyLike): string | null {
  const code = e.code ?? '';
  let m: RegExpExecArray | null;
  if ((m = /^Key([A-Z])$/.exec(code))) return m[1];
  if ((m = /^Digit(\d)$/.exec(code))) return m[1];
  if ((m = /^Numpad(\d)$/.exec(code))) return `Num${m[1]}`;
  if (code === 'NumpadEnter') return 'Enter';
  if (code) return keyToken(code);
  const key = e.key ?? '';
  return key === ' ' ? 'Space' : keyToken(key);
}

/** Modifiers held in `e`, in hotkey order. */
export function heldModifiers(e: KeyLike): string[] {
  const held = [e.ctrlKey, e.altKey, e.shiftKey, e.metaKey];
  return MODIFIERS.filter((_, i) => held[i]);
}

/**
 * The hotkey a key press makes ("Ctrl+Alt+V"), or null while only modifiers are held, for a key
 * the backend cannot register, and when no Ctrl / Alt / Win is held (Shift alone is not enough).
 * F1–F24 are allowed alone.
 */
export function fromKeyboardEvent(e: KeyLike): string | null {
  const key = keyName(e);
  if (!key) return null;
  if (!FUNCTION_KEY.test(key) && !e.ctrlKey && !e.altKey && !e.metaKey) return null;
  return [...heldModifiers(e), key].join('+');
}

function tokens(s: string): string[] {
  return s.split('+').map((p) => p.trim()).filter(Boolean);
}

/** Display form: canonical case; Turkish spaces the plus signs ("Ctrl + Alt + V"). "" stays "". */
export function formatHotkey(s: string, lang: string): string {
  const parts = tokens(s).map((p) => MODIFIER_NAMES[p.toLowerCase()] ?? keyToken(p) ?? p);
  return parts.join(lang === 'tr' ? ' + ' : '+');
}

/** Same combination (any case, spacing or modifier order). An empty hotkey matches nothing. */
export function sameHotkey(a: string, b: string): boolean {
  const norm = (s: string) => {
    const parts = tokens(s);
    const mods = new Set(parts.map((p) => MODIFIER_NAMES[p.toLowerCase()]).filter(Boolean));
    const key = parts.filter((p) => !MODIFIER_NAMES[p.toLowerCase()]).map((p) => keyToken(p) ?? p.toLowerCase());
    return key.length === 1 ? [...MODIFIERS.filter((m) => mods.has(m)), key[0]].join('+') : '';
  };
  const x = norm(a);
  return x !== '' && x === norm(b);
}
