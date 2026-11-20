import { describe, expect, it } from 'vitest';
import type { DownloadSnapshot } from '../../../lib/types';
import {
  formatDuration,
  hostOf,
  isActive,
  missingToolOf,
  resolutionLabel,
  safeThumbnail,
  sortedDownloads,
  upsert,
} from './helpers';

const snap = (id: number, createdAt: number, extra: Partial<DownloadSnapshot> = {}): DownloadSnapshot => ({
  id,
  url: 'https://example.com/a.mp4',
  title: 't',
  state: 'downloading',
  progress: null,
  speedBps: null,
  etaSecs: null,
  output: null,
  errorCode: null,
  error: null,
  createdAt,
  ...extra,
});

describe('download helpers', () => {
  it('formats durations', () => {
    expect(formatDuration(75)).toBe('1:15');
    expect(formatDuration(9)).toBe('0:09');
    expect(formatDuration(3725.4)).toBe('1:02:05');
    expect(formatDuration(null)).toBe('');
    expect(formatDuration(NaN)).toBe('');
    expect(formatDuration(-3)).toBe('');
  });

  it('reads the missing tool from an error', () => {
    expect(missingToolOf('missingTool', 'yt-dlp')).toBe('yt-dlp');
    expect(missingToolOf('missingTool', ' deno ')).toBe('deno');
    expect(missingToolOf('missingTool', 'pdfium')).toBeNull();
    expect(missingToolOf('network', 'yt-dlp')).toBeNull();
    expect(missingToolOf(null, null)).toBeNull();
  });

  it('labels resolution and activity', () => {
    expect(resolutionLabel(1920, 1080)).toBe('1920×1080');
    expect(resolutionLabel(null, 1080)).toBe('');
    expect(isActive('processing')).toBe(true);
    expect(isActive('done')).toBe(false);
    expect(isActive('failed')).toBe(false);
  });

  it('sorts newest first and upserts by id', () => {
    const list = [snap(1, 100), snap(2, 300), snap(3, 200)];
    expect(sortedDownloads(list).map((d) => d.id)).toEqual([2, 3, 1]);
    const next = upsert(list, snap(3, 200, { state: 'done' }));
    expect(next).toHaveLength(3);
    expect(next.find((d) => d.id === 3)?.state).toBe('done');
    expect(list.find((d) => d.id === 3)?.state).toBe('downloading');
    expect(upsert(list, snap(4, 400))).toHaveLength(4);
  });

  it('only allows https thumbnails', () => {
    expect(safeThumbnail('https://i.ytimg.com/a.jpg')).toBe('https://i.ytimg.com/a.jpg');
    expect(safeThumbnail('http://x.com/a.jpg')).toBeNull();
    expect(safeThumbnail('file:///C:/a.jpg')).toBeNull();
    expect(safeThumbnail(null)).toBeNull();
  });

  it('shows the host of a link', () => {
    expect(hostOf('https://www.youtube.com/watch?v=1')).toBe('youtube.com');
    expect(hostOf('not a url')).toBe('not a url');
  });
});
