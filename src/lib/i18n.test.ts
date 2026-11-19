import { describe, expect, it } from 'vitest';
import { currentLang, setLanguage, t, translate } from './i18n';

describe('i18n', () => {
  it('translates by namespace.path in both languages', () => {
    expect(translate('en', 'common.nav.convert')).toBe('Convert');
    expect(translate('tr', 'common.nav.convert')).toBe('Dönüştür');
  });

  it('interpolates {params}', () => {
    expect(translate('en', 'common.files', { count: 1 })).toBe('1 file');
    expect(translate('en', 'common.files', { count: 3 })).toBe('3 files');
  });

  it('selects plural forms by count (one / other)', () => {
    expect(translate('en', 'common.files', { count: 0 })).toBe('0 files');
    expect(translate('tr', 'common.files', { count: 5 })).toBe('5 dosya');
  });

  it('keeps unknown placeholders intact', () => {
    expect(translate('en', 'common.files', { other: 1 })).toBe('{count} files');
  });

  it('falls back to the key itself when missing everywhere', () => {
    expect(translate('tr', 'nope.missing.key')).toBe('nope.missing.key');
    expect(translate('en', 'common.nav')).toBe('common.nav'); // a branch, not a string/plural
  });

  it('falls back to en for unknown languages', () => {
    expect(translate('de', 'common.nav.settings')).toBe('Settings');
  });

  it('setLanguage resolves "system" and drives reactive t()', () => {
    expect(setLanguage('system', 'tr')).toBe('tr');
    expect(currentLang()).toBe('tr');
    expect(t('common.nav.download')).toBe('İndir');
    expect(setLanguage('en', 'tr')).toBe('en');
    expect(t('common.nav.download')).toBe('Download');
  });
});
