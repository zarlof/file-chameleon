import { describe, expect, it } from 'vitest';
import { formatHotkey, fromKeyboardEvent, heldModifiers, sameHotkey } from './hotkeys';

describe('fromKeyboardEvent', () => {
  it('builds "Ctrl+Alt+V"', () => {
    expect(fromKeyboardEvent({ ctrlKey: true, altKey: true, key: 'v', code: 'KeyV' })).toBe('Ctrl+Alt+V');
  });

  it('uses the physical key, not the character of the layout', () => {
    // Turkish Q: the I key types "ı"; AltGr (= Ctrl+Alt) + Q types "@".
    expect(fromKeyboardEvent({ ctrlKey: true, altKey: true, key: 'ı', code: 'KeyI' })).toBe('Ctrl+Alt+I');
    expect(fromKeyboardEvent({ ctrlKey: true, altKey: true, key: '@', code: 'KeyQ' })).toBe('Ctrl+Alt+Q');
  });

  it('orders modifiers Ctrl, Alt, Shift, Win', () => {
    expect(fromKeyboardEvent({ metaKey: true, shiftKey: true, ctrlKey: true, key: 'S', code: 'KeyS' })).toBe('Ctrl+Shift+Win+S');
    expect(fromKeyboardEvent({ altKey: true, shiftKey: true, key: '!', code: 'Digit1' })).toBe('Alt+Shift+1');
  });

  it('needs Ctrl, Alt or Win: Shift alone or no modifier is not enough', () => {
    expect(fromKeyboardEvent({ shiftKey: true, key: 'a', code: 'KeyA' })).toBeNull();
    expect(fromKeyboardEvent({ key: 'v', code: 'KeyV' })).toBeNull();
    expect(fromKeyboardEvent({ key: ' ', code: 'Space' })).toBeNull();
  });

  it('allows F1–F24 alone (and with any modifier)', () => {
    expect(fromKeyboardEvent({ key: 'F5', code: 'F5' })).toBe('F5');
    expect(fromKeyboardEvent({ key: 'F24', code: 'F24' })).toBe('F24');
    expect(fromKeyboardEvent({ shiftKey: true, key: 'F5', code: 'F5' })).toBe('Shift+F5');
  });

  it('is null while only modifiers are held', () => {
    expect(fromKeyboardEvent({ ctrlKey: true, key: 'Control', code: 'ControlLeft' })).toBeNull();
    expect(fromKeyboardEvent({ ctrlKey: true, altKey: true, key: 'Alt', code: 'AltLeft' })).toBeNull();
    expect(fromKeyboardEvent({ metaKey: true, key: 'Meta', code: 'MetaLeft' })).toBeNull();
    expect(fromKeyboardEvent({ ctrlKey: true, altKey: true, key: 'AltGraph', code: 'AltRight' })).toBeNull();
  });

  it('names digits, numpad and navigation keys the way the backend parses them', () => {
    expect(fromKeyboardEvent({ ctrlKey: true, key: '1', code: 'Digit1' })).toBe('Ctrl+1');
    expect(fromKeyboardEvent({ ctrlKey: true, key: '5', code: 'Numpad5' })).toBe('Ctrl+Num5');
    expect(fromKeyboardEvent({ ctrlKey: true, key: ' ', code: 'Space' })).toBe('Ctrl+Space');
    expect(fromKeyboardEvent({ altKey: true, key: 'PageDown', code: 'PageDown' })).toBe('Alt+PageDown');
    expect(fromKeyboardEvent({ ctrlKey: true, key: 'ArrowUp', code: 'ArrowUp' })).toBe('Ctrl+Up');
    expect(fromKeyboardEvent({ ctrlKey: true, altKey: true, key: 'Home', code: 'Home' })).toBe('Ctrl+Alt+Home');
    expect(fromKeyboardEvent({ ctrlKey: true, key: 'Insert', code: 'Insert' })).toBe('Ctrl+Insert');
  });

  it('ignores keys the backend cannot register', () => {
    expect(fromKeyboardEvent({ ctrlKey: true, key: ',', code: 'Comma' })).toBeNull();
    expect(fromKeyboardEvent({ ctrlKey: true, key: 'ğ', code: 'BracketLeft' })).toBeNull();
    expect(fromKeyboardEvent({ ctrlKey: true, key: 'Unidentified', code: '' })).toBeNull();
  });

  it('falls back to the key when there is no code', () => {
    expect(fromKeyboardEvent({ ctrlKey: true, key: 'k' })).toBe('Ctrl+K');
    expect(fromKeyboardEvent({ ctrlKey: true, key: 'PageUp' })).toBe('Ctrl+PageUp');
  });
});

describe('heldModifiers', () => {
  it('lists the held modifiers in order', () => {
    expect(heldModifiers({ shiftKey: true, ctrlKey: true })).toEqual(['Ctrl', 'Shift']);
    expect(heldModifiers({})).toEqual([]);
  });
});

describe('formatHotkey', () => {
  it('spaces the plus signs in Turkish', () => {
    expect(formatHotkey('Ctrl+Alt+V', 'tr')).toBe('Ctrl + Alt + V');
    expect(formatHotkey('Ctrl+Alt+V', 'en')).toBe('Ctrl+Alt+V');
  });

  it('normalizes the case of stored values', () => {
    expect(formatHotkey('ctrl + alt + v', 'en')).toBe('Ctrl+Alt+V');
    expect(formatHotkey('win+shift+pagedown', 'en')).toBe('Win+Shift+PageDown');
  });

  it('is empty when off', () => {
    expect(formatHotkey('', 'tr')).toBe('');
    expect(formatHotkey('  ', 'en')).toBe('');
  });
});

describe('sameHotkey', () => {
  it('ignores case, spacing and modifier order', () => {
    expect(sameHotkey('Ctrl+Alt+V', 'alt + ctrl + v')).toBe(true);
    expect(sameHotkey('Ctrl+Alt+V', 'Ctrl+V')).toBe(false);
    expect(sameHotkey('Ctrl+Alt+V', 'Ctrl+Alt+B')).toBe(false);
  });

  it('never matches an empty (off) hotkey', () => {
    expect(sameHotkey('', '')).toBe(false);
    expect(sameHotkey('Ctrl+Alt+V', '')).toBe(false);
  });
});
