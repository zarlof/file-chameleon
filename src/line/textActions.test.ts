import { describe, expect, it } from 'vitest';
import { applyTextAction, asciiTurkish, cleanLink, prettyJson, textActionsFor, tidySpaces, titleCase } from './textActions';

describe('text actions', () => {
  it('change case the Turkish way in Turkish', () => {
    expect(applyTextAction('upper', 'istanbul ılık', 'tr')).toBe('İSTANBUL ILIK');
    expect(applyTextAction('upper', 'istanbul', 'en')).toBe('ISTANBUL');
    expect(applyTextAction('lower', 'IŞIK İZİ', 'tr')).toBe('ışık izi');
    expect(titleCase('merhaba DÜNYA, iyi günler', 'tr')).toBe('Merhaba Dünya, İyi Günler');
    expect(titleCase('hello-world (big) day', 'en')).toBe('Hello-World (Big) Day');
  });

  it('tidy spaces', () => {
    expect(tidySpaces('  a   b\t\tc  \r\n\r\n\r\n\r\n  d  ')).toBe('a b c\n\nd');
  });

  it('clean tracking parameters off links', () => {
    expect(cleanLink('https://example.com/page?utm_source=x&id=5&fbclid=abc')).toBe('https://example.com/page?id=5');
    expect(cleanLink('https://youtu.be/abc?si=XYZ')).toBe('https://youtu.be/abc');
    expect(cleanLink('https://example.com/?utm_medium=a')).toBe('https://example.com/');
    expect(cleanLink('https://example.com/page?id=5')).toBeNull();
    expect(cleanLink('not a link')).toBeNull();
  });

  it('pretty-print JSON only when it is JSON', () => {
    expect(prettyJson('{"a":1,"b":[1,2]}')).toBe('{\n  "a": 1,\n  "b": [\n    1,\n    2\n  ]\n}');
    expect(prettyJson('{broken')).toBeNull();
    expect(prettyJson('42')).toBeNull();
  });

  it('spell Turkish letters without accents', () => {
    expect(asciiTurkish('Şişli Çarşı ığdır Ü')).toBe('Sisli Carsi igdir U');
    expect(asciiTurkish('plain')).toBeNull();
  });

  it('offer only what changes the text', () => {
    expect(textActionsFor('HELLO', 'en')).toEqual(['lower', 'title']);
    expect(textActionsFor('https://a.com/x?utm_source=1', 'en')).toEqual(['cleanLink']);
    expect(textActionsFor('https://a.com/x', 'en')).toEqual([]);
    expect(textActionsFor('{"a":1}', 'en')).toEqual(['json']);
    expect(textActionsFor('şu  an', 'tr')).toEqual(['upper', 'title', 'spaces', 'ascii']);
  });
});
