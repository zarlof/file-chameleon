import { describe, expect, it } from 'vitest';
import { translate } from '../../../lib/i18n';
import type { Preset, PresetGroup, Profile, WheelConfig } from '../../../lib/types';
import {
  addSlot, deleteGroup, deletePreset, deleteProfile, filterPresets, filterProfiles, folderName, moveSlot, newPresetId,
  normalizeExt, presetCategory, presetSlot, removeSlot, replaceProfile, shadowedBy, slotCounts, slotsFor, upsertGroup,
  upsertPreset,
} from './logic';

const en = (key: string, params?: Record<string, string | number>) => translate('en', key, params);
const tr = (key: string, params?: Record<string, string | number>) => translate('tr', key, params);

const preset = (id: string, op: Preset['op'], extra: Partial<Preset> = {}): Preset => ({
  id, name: '', builtin: true, op, options: {}, ...extra,
});
const profile = (front: string[], back: string[] = [], extra: Partial<Profile> = {}): Profile => ({
  id: 'p', name: '', builtin: false, extensions: [], mixed: false, front, back, ...extra,
});

const PRESETS: Preset[] = [
  preset('to-png', { type: 'convert', to: 'png' }),
  preset('to-jpg-70', { type: 'convert', to: 'jpg' }, { options: { quality: 70 } }),
  preset('to-mp4', { type: 'convert', to: 'mp4' }),
  preset('to-pdf', { type: 'convert', to: 'pdf' }),
  preset('grayscale', { type: 'edit', ops: [{ type: 'grayscale' }], to: null }),
  preset('fit-1920', { type: 'edit', ops: [], to: null }, { options: { maxWidth: 1920, maxHeight: 1920 } }),
  preset('crop-16-9', { type: 'cropAspect', w: 16, h: 9 }),
  preset('speed-2x', { type: 'speed', factor: 2 }),
  preset('mute', { type: 'mute' }),
  preset('merge-pdf', { type: 'mergePdf' }),
  preset('zip', { type: 'zip' }),
  preset('copy-to', { type: 'copyTo', dir: null, moveFiles: false }),
  preset('move-to', { type: 'copyTo', dir: null, moveFiles: true }),
];
const GROUPS: PresetGroup[] = [{ id: 'files', name: '', presets: ['zip', 'copy-to', 'move-to'] }];
const ids = (items: { id: string }[]) => items.map((i) => i.id);

describe('slot lists', () => {
  it('counts front and back', () => {
    expect(slotCounts(profile(['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'], ['x', 'y', 'z', 'w', 'v', 'u', 't', 's', 'r', 'q', 'o', 'n']))).toBe('8·12');
    expect(slotCounts(profile([]))).toBe('0·0');
  });

  it('adds at the end or at a position', () => {
    const p = profile(['a', 'b']);
    expect(addSlot(p, 'front', 'c').front).toEqual(['a', 'b', 'c']);
    expect(addSlot(p, 'front', 'c', 1).front).toEqual(['a', 'c', 'b']);
    expect(addSlot(p, 'front', 'c', 99).front).toEqual(['a', 'b', 'c']);
    expect(addSlot(p, 'back', 'c').back).toEqual(['c']);
    expect(p.front).toEqual(['a', 'b']); // not mutated
  });

  it('refuses a 9th front slot, a 13th back slot and duplicates', () => {
    const full = profile(['1', '2', '3', '4', '5', '6', '7', '8'], ['1', '2', '3', '4', '5', '6', '7', '8', '9', '10', '11', '12']);
    expect(addSlot(full, 'front', 'x')).toBe(full);
    expect(addSlot(full, 'back', 'x')).toBe(full);
    const p = profile(['a'], ['b']);
    expect(addSlot(p, 'front', 'a')).toBe(p);
    // The same slot may sit on the other face.
    expect(addSlot(p, 'back', 'a').back).toEqual(['b', 'a']);
  });

  it('moves and removes', () => {
    const p = profile(['a', 'b', 'c']);
    expect(moveSlot(p, 'front', 0, 2).front).toEqual(['b', 'c', 'a']);
    expect(moveSlot(p, 'front', 2, 1).front).toEqual(['a', 'c', 'b']);
    expect(moveSlot(p, 'front', 0, -1)).toBe(p);
    expect(moveSlot(p, 'front', 2, 3)).toBe(p);
    expect(moveSlot(p, 'front', 5, 0)).toBe(p);
    expect(removeSlot(p, 'front', 1).front).toEqual(['a', 'c']);
    expect(removeSlot(p, 'front', 7)).toBe(p);
  });
});

describe('preset categories and filtering', () => {
  it('sorts presets into filter categories', () => {
    const cat = (id: string) => presetCategory(PRESETS.find((p) => p.id === id)!);
    expect(cat('to-png')).toBe('image');
    expect(cat('to-mp4')).toBe('media');
    expect(cat('to-pdf')).toBe('document');
    expect(cat('grayscale')).toBe('edit');
    expect(cat('crop-16-9')).toBe('edit');
    expect(cat('speed-2x')).toBe('media');
    expect(cat('merge-pdf')).toBe('document');
    expect(cat('zip')).toBe('files');
    expect(cat('move-to')).toBe('files');
  });

  it('filters by category: edit has grayscale, not to-png', () => {
    const edit = ids(filterPresets(PRESETS, GROUPS, '', 'edit', en));
    expect(edit).toContain('grayscale');
    expect(edit).not.toContain('to-png');
  });

  it('lists groups first, as group slot ids, under any category of their members', () => {
    expect(ids(filterPresets(PRESETS, GROUPS, '', 'all', en)).slice(0, 2)).toEqual(['group:files', 'to-png']);
    expect(ids(filterPresets(PRESETS, GROUPS, '', 'files', en))).toEqual(['group:files', 'zip', 'copy-to', 'move-to']);
    expect(ids(filterPresets(PRESETS, GROUPS, '', 'image', en))).not.toContain('group:files');
  });

  it('searches labels, ignoring case and Turkish accents', () => {
    expect(ids(filterPresets(PRESETS, [], 'GRAY', 'all', en))).toEqual(['grayscale']);
    expect(ids(filterPresets(PRESETS, [], 'gri ton', 'all', tr))).toEqual(['grayscale']);
    expect(ids(filterPresets(PRESETS, [], 'kirp', 'all', tr))).toEqual(['crop-16-9']);
    // A group matches through its members' names too.
    expect(ids(filterPresets(PRESETS, GROUPS, 'zip', 'all', en))).toEqual(['group:files', 'zip']);
    expect(filterPresets(PRESETS, GROUPS, 'nothing like this', 'all', en)).toEqual([]);
  });

  it('labels user presets by name', () => {
    const mine = preset('u-1', { type: 'convert', to: 'webp' }, { name: 'Web', builtin: false });
    const [item] = filterPresets([mine], [], 'web', 'all', en);
    expect(item.label).toBe('Web');
  });
});

describe('profiles', () => {
  const profiles: Profile[] = [
    profile([], [], { id: 'images', builtin: true, extensions: ['png', 'jpg'] }),
    profile([], [], { id: 'u-1', name: 'Photos', extensions: ['jpg', 'heic'] }),
  ];
  it('searches names and extensions', () => {
    expect(ids(filterProfiles(profiles, 'imag', en))).toEqual(['images']);
    expect(ids(filterProfiles(profiles, 'heic', en))).toEqual(['u-1']);
    expect(ids(filterProfiles(profiles, '', en))).toEqual(['images', 'u-1']);
  });
  it('finds an earlier profile that takes an extension', () => {
    expect(shadowedBy(profiles, 'u-1', 'jpg')?.id).toBe('images');
    expect(shadowedBy(profiles, 'u-1', 'heic')).toBeNull();
    expect(shadowedBy(profiles, 'images', 'jpg')).toBeNull();
  });
});

describe('ids and extensions', () => {
  it('makes unique u- ids that are never reused', () => {
    const ids = new Set<string>();
    for (let i = 0; i < 200; i++) ids.add(newPresetId(ids));
    expect(ids.size).toBe(200);
    for (const id of ids) expect(id).toMatch(/^u-[a-z0-9]+$/);
    // Deleting the newest item does not free its id for the next one.
    const a = newPresetId(['u-1']);
    const b = newPresetId(['u-1']);
    expect(a).not.toBe(b);
    expect(newPresetId(['to-png', 'u-1', 'u-7', 'u-x'])).not.toBe('u-8');
  });
  it('normalizes extensions', () => {
    expect(normalizeExt('.JPG')).toBe('jpg');
    expect(normalizeExt('  docx ')).toBe('docx');
    expect(normalizeExt('..Tar')).toBe('tar');
    expect(normalizeExt('a b')).toBeNull();
    expect(normalizeExt('')).toBeNull();
    expect(normalizeExt('.')).toBeNull();
    expect(normalizeExt('abcdefghijk')).toBeNull();
    expect(normalizeExt('tar.gz')).toBeNull();
    expect(normalizeExt('ğ')).toBeNull();
  });
  it('names a folder', () => {
    expect(folderName('C:\\Users\\me\\Pictures\\')).toBe('Pictures');
    expect(folderName('D:\\')).toBe('D:');
    expect(folderName('/home/me/out')).toBe('out');
  });
});

describe('editor slots', () => {
  const find = (id: string) => PRESETS.find((p) => p.id === id)!;
  it('describes presets like the core', () => {
    expect(presetSlot(find('to-jpg-70'))).toMatchObject({ id: 'to-jpg-70', format: 'JPG', detail: '70%', icon: 'image', kind: 'image', children: [] });
    expect(presetSlot(find('fit-1920'))).toMatchObject({ format: null, detail: '1920 px', icon: 'resize', kind: 'image' });
    expect(presetSlot(find('grayscale'))).toMatchObject({ detail: null, icon: 'grayscale' });
    expect(presetSlot(find('crop-16-9'))).toMatchObject({ detail: '16:9', icon: 'crop', kind: null });
    expect(presetSlot(find('speed-2x')).detail).toBe('2×');
    expect(presetSlot(find('move-to'))).toMatchObject({ icon: 'move', kind: null });
    expect(presetSlot(preset('to-m4r', { type: 'convert', to: 'm4r' })).icon).toBe('ringtone');
    expect(presetSlot(preset('u-1', { type: 'convert', to: 'png' }, { options: { maxWidth: 1280, maxHeight: 720 } })).detail).toBe('1280×720 px');
    expect(presetSlot(preset('u-2', { type: 'edit', ops: [{ type: 'scale', percent: 25 }], to: 'webp' }))).toMatchObject({ format: 'WebP', detail: '25%', icon: 'resize' });
  });

  it('builds slots and group children, skipping unknown ids', () => {
    const config: WheelConfig = { version: 1, presets: PRESETS, groups: GROUPS, profiles: [] };
    const slots = slotsFor(config, ['to-png', 'gone', 'group:files', 'group:gone']);
    expect(ids(slots)).toEqual(['to-png', 'group:files']);
    expect(slots[1]).toMatchObject({ icon: 'group', format: null });
    expect(ids(slots[1].children)).toEqual(['zip', 'copy-to', 'move-to']);
  });
});

describe('config edits', () => {
  const base = (): WheelConfig => ({
    version: 1,
    presets: [...PRESETS, preset('u-1', { type: 'zip' }, { name: 'Mine', builtin: false })],
    groups: [...GROUPS, { id: 'u-1', name: 'G', presets: ['u-1', 'zip'] }],
    profiles: [
      profile(['u-1', 'group:u-1', 'to-png'], ['u-1'], { id: 'a' }),
      profile(['group:files'], [], { id: 'b' }),
    ],
  });

  it('replaces a profile and upserts presets and groups', () => {
    const c = base();
    const next = replaceProfile(c, { ...c.profiles[1], mixed: true });
    expect(next.profiles[1].mixed).toBe(true);
    expect(c.profiles[1].mixed).toBe(false);
    const p = preset('u-2', { type: 'mute' }, { builtin: false, name: 'x' });
    expect(ids(upsertPreset(c, p).presets).at(-1)).toBe('u-2');
    expect(upsertPreset(c, { ...c.presets.at(-1)!, name: 'Renamed' }).presets.at(-1)!.name).toBe('Renamed');
    expect(upsertGroup(c, { id: 'u-2', name: 'New', presets: [] }).groups.map((g) => g.id)).toEqual(['files', 'u-1', 'u-2']);
    expect(upsertGroup(c, { id: 'files', name: 'Files', presets: ['zip'] }).groups[0].presets).toEqual(['zip']);
  });

  it('deleting a preset drops it from groups and faces', () => {
    const next = deletePreset(base(), 'u-1');
    expect(ids(next.presets)).not.toContain('u-1');
    expect(next.groups[1].presets).toEqual(['zip']);
    expect(next.profiles[0].front).toEqual(['group:u-1', 'to-png']);
    expect(next.profiles[0].back).toEqual([]);
  });

  it('deleting a group drops its slots; deleting a profile removes it', () => {
    const next = deleteGroup(base(), 'u-1');
    expect(next.groups.map((g) => g.id)).toEqual(['files']);
    expect(next.profiles[0].front).toEqual(['u-1', 'to-png']);
    expect(deleteProfile(base(), 'a').profiles.map((p) => p.id)).toEqual(['b']);
  });
});
