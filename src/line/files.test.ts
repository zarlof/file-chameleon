import { describe, expect, it } from 'vitest';
import type { LineFile } from '../lib/types';
import { fileCard, fileId } from './files';

function file(over: Partial<LineFile> = {}): LineFile {
  return { path: 'C:/x/a.PNG', name: 'a.PNG', source: 'added', at: 1, size: 1258291, kind: 'image', thumb: null, ...over };
}

describe('files', () => {
  it('file_card_model', () => {
    const c = fileCard(file(), 'tr');
    expect(c.ext).toBe('png');
    expect(c.isImage).toBe(true);
    expect(c.sizeText).toBe('1,2 MB');
    expect(c.sourceIcon).toBe('added');
    expect(c.name).toBe('a.PNG');
    expect(fileCard(file({ size: 500, name: 'b' }), 'tr').sizeText).toBe('500 B');
    expect(fileCard(file({ size: 1258291 }), 'en').sizeText).toBe('1.2 MB');
    expect(fileCard(file({ size: 980 * 1024 }), 'tr').sizeText).toBe('980 KB');
    expect(fileCard(file({ name: 'noext', kind: 'video' }), 'en')).toMatchObject({ ext: '', isImage: false });
    expect(fileCard(file({ source: 'converted' }), 'en').sourceIcon).toBe('converted');
  });
  it('file_id_is_stable_negative_and_distinct', () => {
    expect(fileId('C:/a.png')).toBeLessThan(0);
    expect(fileId('C:/a.png')).toBe(fileId('c:/A.PNG'));
    expect(fileId('C:/a.png')).not.toBe(fileId('C:/b.png'));
  });
});
