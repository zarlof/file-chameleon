import { describe, expect, it } from 'vitest';
import { setLang, t } from './i18n';

describe('overlay i18n', () => {
  it('reads overlay keys with or without the namespace', () => {
    setLang('tr', 'en');
    expect(t('overlay.escToClose')).toBe('Esc ile kapat');
    expect(t('notSupported')).toBe('Desteklenmiyor');
    expect(t('overlay.files', { n: 3, count: 3 })).toBe('3 dosya');
    setLang('en', 'en');
    expect(t('overlay.files', { n: 1, count: 1 })).toBe('1 file');
    expect(t('overlay.files', { n: 2, count: 2 })).toBe('2 files');
  });
  it('reads the nested presets namespace', () => {
    setLang('tr', 'en');
    expect(t('presets.builtin.to-jpg-70')).toBe('JPG %70');
    expect(t('presets.groupName.files')).toBe('Dosya işlemleri');
    expect(t('presets.group')).toBe('Grup');
    setLang('en', 'en');
    expect(t('presets.profile.other')).toBe('Other files');
  });
  it('returns the key itself when it is missing or a branch', () => {
    setLang('tr', 'en');
    expect(t('presets.builtin.nope')).toBe('presets.builtin.nope');
    expect(t('presets.builtin')).toBe('presets.builtin');
    expect(t('overlay.nope')).toBe('overlay.nope');
  });
});
