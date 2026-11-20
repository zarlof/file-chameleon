import { describe, expect, it } from 'vitest';
import { addExe, exeName } from './logic';

describe('exeName', () => {
  it('lowercases and appends .exe when missing', () => {
    expect(exeName('Code')).toBe('code.exe');
    expect(exeName('  Photoshop.EXE ')).toBe('photoshop.exe');
    expect(exeName('my app')).toBe('my app.exe');
  });

  it('keeps only the file name of a pasted path', () => {
    expect(exeName('C:\\Program Files\\Tool\\Tool.exe')).toBe('tool.exe');
    expect(exeName('/usr/bin/thing')).toBe('thing.exe');
  });

  it('rejects empty input and bare ".exe"', () => {
    expect(exeName('')).toBeNull();
    expect(exeName('   ')).toBeNull();
    expect(exeName('.exe')).toBeNull();
    expect(exeName('C:\\folder\\')).toBeNull();
  });
});

describe('addExe', () => {
  it('adds a normalized name once', () => {
    expect(addExe(['code.exe'], 'Explorer')).toEqual(['code.exe', 'explorer.exe']);
    expect(addExe(['code.exe'], 'CODE.exe')).toEqual(['code.exe']);
  });

  it('returns the list unchanged for invalid input', () => {
    const list = ['a.exe'];
    expect(addExe(list, '  ')).toBe(list);
  });
});
