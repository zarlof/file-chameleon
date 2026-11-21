import { describe, expect, it } from 'vitest';
import { isCommandKey, nextFocus } from './keys';

const ev = (key: string, mods: Partial<KeyboardEvent> = {}) => ({ key, ctrlKey: false, metaKey: false, altKey: false, ...mods }) as KeyboardEvent;

describe('keys', () => {
  it('keys_focus', () => {
    expect(nextFocus([3, 0, 2], { r: 0, i: 2 }, 'ArrowDown')).toEqual({ r: 2, i: 1 });
    expect(nextFocus([3, 0, 2], { r: 0, i: 2 }, 'ArrowRight')).toEqual({ r: 0, i: 2 });
    expect(nextFocus([3, 0, 2], { r: 0, i: 2 }, 'Home')).toEqual({ r: 0, i: 0 });
    expect(nextFocus([3, 0, 2], { r: 0, i: 2 }, 'End')).toEqual({ r: 0, i: 2 });
    expect(nextFocus([3, 0, 2], { r: 2, i: 0 }, 'ArrowUp')).toEqual({ r: 0, i: 0 });
    expect(nextFocus([3, 0, 2], null, 'ArrowRight')).toEqual({ r: 0, i: 0 });
    expect(nextFocus([0, 0], null, 'ArrowRight')).toBeNull();
  });

  it('keys_in_search_are_text', () => {
    for (const k of ['Delete', 'p', 'e', '1', ' ']) {
      expect(isCommandKey(ev(k), true)).toBe(false);
      expect(isCommandKey(ev(k), false)).toBe(true);
    }
    expect(isCommandKey(ev('q'), false)).toBe(false);
    expect(isCommandKey(ev('c', { ctrlKey: true }), true)).toBe(false); // native text copy
    expect(isCommandKey(ev('c', { ctrlKey: true }), false)).toBe(true);
    expect(isCommandKey(ev('ArrowLeft'), true)).toBe(false);
  });
});
