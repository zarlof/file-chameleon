// Pure helpers for the wheel editor (and the presets page): slot lists with the core's limits,
// preset categories and search, ids, and the editor preview's slots. The slot description
// mirrors `describe()` in crates/fc-core/src/presets/mod.rs (no files are dragged here, so no
// tool needs are known).
import type {
  ConvertOptions, ImageOp, Kind, Preset, PresetGroup, Profile, SlotIcon, WheelConfig, WheelFace, WheelSlot,
} from '../../../lib/types';
import { presetLabel, profileLabel, type Translate } from '../../../lib/wheel/labels';

/** Same limits as `fc_core::presets` (MAX_FRONT, MAX_BACK, MAX_GROUP). */
export const FACE_LIMIT: Record<WheelFace, number> = { front: 8, back: 12 };
export const MAX_GROUP = 6;
export const GROUP_PREFIX = 'group:';

export type PresetFilter = 'all' | 'image' | 'document' | 'media' | 'edit' | 'files';
export const PRESET_FILTERS: PresetFilter[] = ['all', 'image', 'document', 'media', 'edit', 'files'];

export interface FormatDef {
  id: string;
  label: string;
  kind: Kind;
}

/** Output formats (the core's convert presets), in presets-page order; labels as in `formats::FORMATS`. */
export const OUTPUT_FORMATS: FormatDef[] = [
  ['png', 'PNG', 'image'], ['jpg', 'JPG', 'image'], ['webp', 'WebP', 'image'], ['gif', 'GIF', 'image'],
  ['bmp', 'BMP', 'image'], ['tiff', 'TIFF', 'image'], ['ico', 'ICO', 'image'], ['pdf', 'PDF', 'pdf'],
  ['mp4', 'MP4', 'video'], ['webm', 'WebM', 'video'], ['mkv', 'MKV', 'video'], ['mov', 'MOV', 'video'], ['avi', 'AVI', 'video'],
  ['mp3', 'MP3', 'audio'], ['m4a', 'M4A', 'audio'], ['wav', 'WAV', 'audio'], ['flac', 'FLAC', 'audio'], ['ogg', 'OGG', 'audio'],
  ['opus', 'Opus', 'audio'], ['m4r', 'M4R', 'audio'],
  ['csv', 'CSV', 'data'], ['tsv', 'TSV', 'data'], ['json', 'JSON', 'data'], ['yaml', 'YAML', 'data'], ['toml', 'TOML', 'data'],
  ['html', 'HTML', 'data'], ['txt', 'TXT', 'data'],
  ['srt', 'SRT', 'subtitle'], ['vtt', 'WebVTT', 'subtitle'],
].map(([id, label, kind]) => ({ id, label, kind: kind as Kind }));

export function formatDef(id: string | null | undefined): FormatDef | null {
  return OUTPUT_FORMATS.find((f) => f.id === id) ?? null;
}

/** "JPG" for "jpg"; unknown ids upper-cased. */
export function formatLabel(id: string): string {
  return formatDef(id)?.label ?? id.toUpperCase();
}

// ---------------------------------------------------------------- slot lists

export function slotCounts(p: Profile): string {
  return `${p.front.length}·${p.back.length}`;
}

/** Insert `slotId` at `at` (default: the end). Unchanged (same object) when the face is full or
 *  already has it. */
export function addSlot(p: Profile, face: WheelFace, slotId: string, at?: number): Profile {
  const list = p[face];
  if (list.length >= FACE_LIMIT[face] || list.includes(slotId)) return p;
  const i = at === undefined ? list.length : Math.max(0, Math.min(list.length, at));
  return { ...p, [face]: [...list.slice(0, i), slotId, ...list.slice(i)] };
}

/** Move the slot at `from` to `to`; unchanged when either is out of range. */
export function moveSlot(p: Profile, face: WheelFace, from: number, to: number): Profile {
  const list = p[face];
  if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return p;
  const next = list.slice();
  const [id] = next.splice(from, 1);
  next.splice(to, 0, id);
  return { ...p, [face]: next };
}

export function removeSlot(p: Profile, face: WheelFace, index: number): Profile {
  if (index < 0 || index >= p[face].length) return p;
  return { ...p, [face]: p[face].filter((_, i) => i !== index) };
}

// ---------------------------------------------------------------- categories and search

export function presetCategory(p: Preset): Exclude<PresetFilter, 'all'> {
  switch (p.op.type) {
    case 'convert': {
      const kind = formatDef(p.op.to)?.kind;
      if (kind === 'image' || kind === 'vector') return 'image';
      if (kind === 'video' || kind === 'audio') return 'media';
      return 'document';
    }
    case 'edit':
    case 'annotate':
    case 'cropAspect':
    case 'crop':
    case 'compress':
      return 'edit';
    case 'mute':
    case 'speed':
    case 'trim':
      return 'media';
    case 'mergePdf':
    case 'splitPdf':
      return 'document';
    case 'zip':
    case 'extract':
    case 'copyTo':
      return 'files';
  }
}

/** Lowercase without accents, so "dondur" finds "Döndür" and "I" finds "ı". */
export function fold(s: string): string {
  return s.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/ı/g, 'i');
}

export function groupLabel(g: PresetGroup, t: Translate): string {
  if (g.name) return g.name;
  const key = `presets.groupName.${g.id}`;
  const s = t(key);
  return s === key ? t('presets.group') : s;
}

/** One entry of the "available presets" list; `id` is the slot id. */
export type PickItem =
  | { kind: 'preset'; id: string; label: string; preset: Preset }
  | { kind: 'group'; id: string; label: string; group: PresetGroup; members: Preset[] };

/** Groups first, then presets, in config order. A group shows under a category when one of its
 *  presets does, and matches a search through its own name or its presets' names. */
export function filterPresets(
  presets: Preset[], groups: PresetGroup[], query: string, filter: PresetFilter, t: Translate,
): PickItem[] {
  const q = fold(query.trim());
  const inFilter = (p: Preset) => filter === 'all' || presetCategory(p) === filter;
  const matches = (label: string, id: string) => !q || fold(label).includes(q) || id.includes(q);
  const out: PickItem[] = [];
  for (const g of groups) {
    const members = g.presets.map((id) => presets.find((p) => p.id === id)).filter((p): p is Preset => !!p);
    const label = groupLabel(g, t);
    const shown = filter === 'all' || members.some(inFilter);
    const found = matches(label, g.id) || members.some((p) => matches(presetLabel(p, t), p.id));
    if (shown && found) out.push({ kind: 'group', id: GROUP_PREFIX + g.id, label, group: g, members });
  }
  for (const p of presets) {
    const label = presetLabel(p, t);
    if (inFilter(p) && matches(label, p.id)) out.push({ kind: 'preset', id: p.id, label, preset: p });
  }
  return out;
}

export function filterProfiles(profiles: Profile[], query: string, t: Translate): Profile[] {
  const q = fold(query.trim()).replace(/^\./, '');
  if (!q) return profiles;
  return profiles.filter((p) => fold(profileLabel(p, t)).includes(q) || p.extensions.some((e) => e.includes(q)));
}

/** The profile listed before `profileId` that also has `ext` (the core opens the first one), or null. */
export function shadowedBy(profiles: Profile[], profileId: string, ext: string): Profile | null {
  for (const p of profiles) {
    if (p.id === profileId) return null;
    if (p.extensions.includes(ext)) return p;
  }
  return null;
}

// ---------------------------------------------------------------- ids, extensions, folders

/**
 * A fresh "u-<base36 time><random>" id (also used for user groups and profiles, each in its own
 * list). Ids are never reused, even after a delete: a watch folder or profile that still names a
 * deleted preset must not silently point at a newer one. `existing` only guards against a clash.
 */
export function newPresetId(existing: Iterable<string>): string {
  const taken = new Set(existing);
  let id: string;
  do {
    id = `u-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  } while (taken.has(id));
  return id;
}

/** ".JPG" -> "jpg"; null unless 1..10 ASCII letters or digits. */
export function normalizeExt(s: string): string | null {
  const e = s.trim().replace(/^\.+/, '').toLowerCase();
  return /^[a-z0-9]{1,10}$/.test(e) ? e : null;
}

/** Last part of a folder path ("D:\" -> "D:"). */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
}

// ---------------------------------------------------------------- editor preview slots

const KIND_ICON: Record<Kind, SlotIcon> = {
  image: 'image', vector: 'vector', video: 'video', audio: 'audio', pdf: 'pdf', data: 'data', subtitle: 'subtitle', archive: 'archive',
};

const EDIT_ICON: Record<ImageOp['type'], SlotIcon> = {
  grayscale: 'grayscale', rotateLeft: 'rotateLeft', rotateRight: 'rotateRight', rotate180: 'rotate180', flipH: 'flipH',
  flipV: 'flipV', trim: 'trim', pixelate: 'pixelate', blur: 'blur', scale: 'resize',
};

/** "1920 px" / "1280×720 px" from the max size options. */
function sizeDetail(o: ConvertOptions): string | null {
  const w = o.maxWidth ?? null;
  const h = o.maxHeight ?? null;
  if (w !== null && h !== null && w !== h) return `${w}×${h} px`;
  const n = w ?? h;
  return n === null ? null : `${n} px`;
}

/** The wheel slot of a preset, as the core describes it (without files). */
export function presetSlot(p: Preset): WheelSlot {
  let format: string | null = null;
  let detail: string | null = null;
  let icon: SlotIcon;
  let kind: Kind | null = null;
  const op = p.op;
  switch (op.type) {
    case 'convert': {
      const f = formatDef(op.to);
      format = f?.label ?? null;
      const parts: string[] = [];
      if (p.options.quality != null) parts.push(`${p.options.quality}%`);
      const size = sizeDetail(p.options);
      if (size) parts.push(size);
      detail = parts.length ? parts.join(' · ') : null;
      kind = f?.kind ?? null;
      icon = op.to === 'm4r' ? 'ringtone' : kind ? KIND_ICON[kind] : 'data';
      break;
    }
    case 'edit': {
      format = op.to ? (formatDef(op.to)?.label ?? null) : null;
      const scale = op.ops.find((s) => s.type === 'scale');
      detail = scale && scale.type === 'scale' ? `${scale.percent}%` : sizeDetail(p.options);
      icon = op.ops.length ? EDIT_ICON[op.ops[0].type] : 'resize';
      kind = 'image';
      break;
    }
    case 'annotate':
      icon = 'annotate';
      kind = 'image';
      break;
    case 'cropAspect':
      detail = `${op.w}:${op.h}`;
      icon = 'crop';
      break;
    case 'crop':
      icon = 'crop';
      break;
    case 'speed':
      detail = `${op.factor}×`;
      icon = 'speed';
      break;
    case 'trim':
      icon = 'trim';
      break;
    case 'compress':
      icon = 'compress';
      break;
    case 'mute':
      icon = 'mute';
      kind = 'video';
      break;
    case 'mergePdf':
      icon = 'merge';
      kind = 'pdf';
      break;
    case 'splitPdf':
      icon = 'split';
      kind = 'pdf';
      break;
    case 'copyTo':
      icon = op.moveFiles ? 'move' : 'copy';
      break;
    case 'zip':
      icon = 'zip';
      break;
    case 'extract':
      icon = 'extract';
      break;
  }
  return { id: p.id, name: p.name, format, detail, icon, kind, needsTools: [], children: [] };
}

/** Slots for a face's ids; unknown ids are skipped, groups carry their presets as children. */
export function slotsFor(config: WheelConfig, ids: string[]): WheelSlot[] {
  const preset = (id: string) => config.presets.find((p) => p.id === id);
  const out: WheelSlot[] = [];
  for (const id of ids) {
    if (id.startsWith(GROUP_PREFIX)) {
      const g = config.groups.find((x) => x.id === id.slice(GROUP_PREFIX.length));
      if (!g) continue;
      const children = g.presets.map(preset).filter((p): p is Preset => !!p).map(presetSlot);
      out.push({ id, name: g.name, format: null, detail: null, icon: 'group', kind: null, needsTools: [], children });
    } else {
      const p = preset(id);
      if (p) out.push(presetSlot(p));
    }
  }
  return out;
}

// ---------------------------------------------------------------- config edits (immutable)

export function replaceProfile(c: WheelConfig, profile: Profile): WheelConfig {
  return { ...c, profiles: c.profiles.map((p) => (p.id === profile.id ? profile : p)) };
}

/** Replace the preset with the same id, or append it. */
export function upsertPreset(c: WheelConfig, preset: Preset): WheelConfig {
  const has = c.presets.some((p) => p.id === preset.id);
  return { ...c, presets: has ? c.presets.map((p) => (p.id === preset.id ? preset : p)) : [...c.presets, preset] };
}

export function upsertGroup(c: WheelConfig, group: PresetGroup): WheelConfig {
  const has = c.groups.some((g) => g.id === group.id);
  return { ...c, groups: has ? c.groups.map((g) => (g.id === group.id ? group : g)) : [...c.groups, group] };
}

function withoutSlot(profiles: Profile[], slotId: string): Profile[] {
  return profiles.map((p) =>
    p.front.includes(slotId) || p.back.includes(slotId)
      ? { ...p, front: p.front.filter((s) => s !== slotId), back: p.back.filter((s) => s !== slotId) }
      : p,
  );
}

/** Remove a preset and every reference to it (groups, faces), so the config stays valid. */
export function deletePreset(c: WheelConfig, id: string): WheelConfig {
  return {
    ...c,
    presets: c.presets.filter((p) => p.id !== id),
    groups: c.groups.map((g) => (g.presets.includes(id) ? { ...g, presets: g.presets.filter((x) => x !== id) } : g)),
    profiles: withoutSlot(c.profiles, id),
  };
}

export function deleteGroup(c: WheelConfig, id: string): WheelConfig {
  return { ...c, groups: c.groups.filter((g) => g.id !== id), profiles: withoutSlot(c.profiles, GROUP_PREFIX + id) };
}

export function deleteProfile(c: WheelConfig, id: string): WheelConfig {
  return { ...c, profiles: c.profiles.filter((p) => p.id !== id) };
}
