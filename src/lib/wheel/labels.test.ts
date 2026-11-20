import { describe, expect, it } from 'vitest';
import { translate } from '../i18n';
import type { Preset, WheelSlot } from '../types';
import { centerTitle, commonExt, isGroupSlot, presetLabel, profileLabel, slotLabel } from './labels';

const tr = (k: string, p?: Record<string, string | number>) => translate('tr', k, p);
const en = (k: string, p?: Record<string, string | number>) => translate('en', k, p);

const slot = (over: Partial<WheelSlot> & { id: string }): WheelSlot => ({
  name: '',
  format: null,
  detail: null,
  icon: 'image',
  kind: 'image',
  needsTools: [],
  children: [],
  ...over,
});

const preset = (id: string, name = ''): Preset => ({
  id,
  name,
  builtin: name === '',
  op: { type: 'convert', to: 'png' },
  options: {},
});

describe('slotLabel', () => {
  it('prefers the user name, then the output format, then the built-in label', () => {
    expect(slotLabel(slot({ id: 'to-jpg-70', name: 'Web', format: 'JPG' }), en)).toBe('Web');
    expect(slotLabel(slot({ id: 'to-jpg-70', format: 'JPG', detail: '70%' }), en)).toBe('JPG');
    expect(slotLabel(slot({ id: 'grayscale', icon: 'grayscale' }), tr)).toBe('Gri tonlama');
    expect(slotLabel(slot({ id: 'crop-16-9', icon: 'crop' }), en)).toBe('Crop 16:9');
  });
  it('falls back to the id for an unnamed unknown preset', () => {
    expect(slotLabel(slot({ id: 'u-7' }), en)).toBe('u-7');
  });
  it('names groups: own name, then the built-in group name, then "Group"', () => {
    expect(slotLabel(slot({ id: 'group:files', icon: 'group' }), tr)).toBe('Dosya işlemleri');
    expect(slotLabel(slot({ id: 'group:g-2', icon: 'group' }), en)).toBe('Group');
    expect(slotLabel(slot({ id: 'group:g-2', name: 'Paylaş', icon: 'group' }), en)).toBe('Paylaş');
  });
});

describe('presetLabel / profileLabel', () => {
  it('uses the name or the localized built-in label', () => {
    expect(presetLabel(preset('to-m4r'), tr)).toBe('Zil sesi (iPhone)');
    expect(presetLabel(preset('u-1', 'Küçük WebP'), tr)).toBe('Küçük WebP');
    expect(presetLabel(preset('u-2'), en)).toBe('u-2');
    expect(profileLabel({ id: 'mixed', name: '' }, en)).toBe('Mixed selection');
    expect(profileLabel({ id: 'p-1', name: 'Word' }, en)).toBe('Word');
    expect(profileLabel({ id: 'p-2', name: '' }, en)).toBe('p-2');
  });
});

describe('centerTitle', () => {
  it('shows the count and the shared extension', () => {
    expect(centerTitle(1, 'jpg', en)).toBe('1 JPG');
    expect(centerTitle(3, 'png', tr)).toBe('3 PNG');
  });
  it('uses the file count text when the extensions differ', () => {
    expect(centerTitle(3, null, tr)).toBe('3 dosya');
    expect(centerTitle(3, null, en)).toBe('3 files');
    expect(centerTitle(1, null, en)).toBe('1 file');
  });
});

describe('commonExt', () => {
  it('returns the extension every path shares, lowercase', () => {
    expect(commonExt(['C:\\a\\x.JPG', 'C:\\b\\y.jpg'])).toBe('jpg');
    expect(commonExt(['C:\\a\\x.jpg', 'C:\\a\\y.png'])).toBeNull();
    expect(commonExt(['C:\\a\\folder', 'C:\\a\\y.png'])).toBeNull();
    expect(commonExt(['C:\\a.b\\noext'])).toBeNull();
    expect(commonExt([])).toBeNull();
  });
});

describe('isGroupSlot', () => {
  it('recognizes group slot ids', () => {
    expect(isGroupSlot({ id: 'group:files' })).toBe(true);
    expect(isGroupSlot({ id: 'to-png' })).toBe(false);
  });
});
