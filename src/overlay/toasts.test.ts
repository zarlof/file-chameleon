import { describe, expect, it } from 'vitest';
import { setLang, t } from './i18n';
import { shortenUrl, toastView } from './toasts';
import type { ToastPayload } from '../lib/types';

const p = (kind: ToastPayload['kind'], data: Record<string, unknown>): ToastPayload => ({
  id: 1,
  kind,
  data,
  progress: null,
  timeoutMs: null,
});

describe('toastView', () => {
  it('localizes jobDone with the output file name', () => {
    setLang('en', 'en');
    const v = toastView(p('jobDone', { outputs: ['C:\\x\\photo.webp'], inputName: 'photo.png' }), t);
    expect(v.title).toBe('Converted');
    expect(v.body).toBe('photo.webp');
    expect(v.actions.map((a) => a.id)).toEqual(['reveal', 'open']);
  });
  it('uses Turkish and plural rules for batches', () => {
    setLang('tr', 'en');
    expect(toastView(p('jobsBatch', { done: 3, failed: 1, total: 4 }), t).title).toBe('3 dosya dönüştürüldü');
    setLang('en', 'en');
    expect(toastView(p('jobsBatch', { done: 1, failed: 0, total: 1 }), t).title).toBe('1 file converted');
    expect(toastView(p('jobsBatch', { done: 2, failed: 1, total: 3 }), t).body).toBe('1 failed');
  });
  it('shows a reason for failures and falls back for unknown codes', () => {
    setLang('en', 'en');
    expect(toastView(p('jobFailed', { inputName: 'a.png', errorCode: 'unsupported' }), t).body).toBe('a.png · Unsupported format');
    expect(toastView(p('jobFailed', { inputName: 'a.png', errorCode: 'weird' }), t).body).toBe(`a.png · ${t('error.failed')}`);
  });
  it('formats tool size and clip image dimensions', () => {
    setLang('en', 'en');
    expect(toastView(p('toolNeeded', { tool: 'ffmpeg', name: 'FFmpeg', size: 80_000_000 }), t).body).toBe('80 MB download');
    const c = toastView(p('clipImage', { width: 800, height: 600, thumb: 'C:\\t.png' }), t);
    expect(c.body).toBe('800×600');
    expect(c.thumb).toBe('C:\\t.png');
  });
  it('shortens urls', () => {
    expect(shortenUrl('https://www.example.com/')).toBe('example.com');
    expect(shortenUrl('https://example.com/' + 'a'.repeat(80)).length).toBe(44);
  });
});
