// Display names for wheel slots, presets and profiles, plus the wheel's centre title.
// `t` is either i18n (main app `src/lib/i18n.ts` or the overlay's `src/overlay/i18n.ts`); both
// use the same "<namespace>.<path>" keys and return the key itself when it is missing.
import type { Preset, WheelSlot } from '../types';

export type Translate = (key: string, params?: Record<string, string | number>) => string;

const GROUP_PREFIX = 'group:';

/** Slot ids of groups are "group:<group id>". */
export function isGroupSlot(slot: { id: string }): boolean {
  return slot.id.startsWith(GROUP_PREFIX);
}

/** `t(key)`, or null when the key has no translation. */
function lookup(t: Translate, key: string): string | null {
  const s = t(key);
  return s === key ? null : s;
}

/** User name, else the output format ("JPG"), else the built-in label; groups: user name,
 *  else the built-in group name, else "Group". */
export function slotLabel(slot: WheelSlot, t: Translate): string {
  if (slot.name) return slot.name;
  if (isGroupSlot(slot)) {
    return lookup(t, `presets.groupName.${slot.id.slice(GROUP_PREFIX.length)}`) ?? t('presets.group');
  }
  return slot.format || (lookup(t, `presets.builtin.${slot.id}`) ?? slot.id);
}

export function presetLabel(p: Preset, t: Translate): string {
  return p.name || (lookup(t, `presets.builtin.${p.id}`) ?? p.id);
}

export function profileLabel(p: { id: string; name: string }, t: Translate): string {
  return p.name || (lookup(t, `presets.profile.${p.id}`) ?? p.id);
}

/** "1 JPG" / "3 PNG" when every file has `ext`, else "3 dosya" / "3 files". */
export function centerTitle(count: number, ext: string | null, t: Translate): string {
  return ext ? `${count} ${ext.toUpperCase()}` : t('overlay.files', { n: count, count });
}

/** Lowercase extension shared by every path, or null (mixed, missing, or no paths). */
export function commonExt(paths: readonly string[]): string | null {
  let ext: string | null = null;
  for (const p of paths) {
    const name = p.split(/[\\/]/).pop() ?? '';
    const dot = name.lastIndexOf('.');
    const e = dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
    if (!e || (ext !== null && e !== ext)) return null;
    ext = e;
  }
  return ext;
}
