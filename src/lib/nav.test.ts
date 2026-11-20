import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NavPayload } from './types';

const handlers: { goto?: (p: NavPayload) => void } = {};
const takePendingNav = vi.fn<() => Promise<NavPayload | null>>();

vi.mock('./ipc', () => ({
  api: { takePendingNav: () => takePendingNav() },
  on: {
    navGoto: async (cb: (p: NavPayload) => void) => {
      handlers.goto = cb;
      return () => {};
    },
  },
}));

const { NavStore, NAV_MAIN, NAV_SETTINGS } = await import('./stores/nav.svelte');

describe('nav lists', () => {
  it('lists main and settings views', () => {
    expect(NAV_MAIN).toEqual(['convert', 'download', 'clipboard', 'recent']);
    expect(NAV_SETTINGS).toHaveLength(9);
    expect(new Set([...NAV_MAIN, ...NAV_SETTINGS]).size).toBe(13);
  });
});

describe('nav store', () => {
  let nav: InstanceType<typeof NavStore>;
  beforeEach(() => {
    nav = new NavStore();
    takePendingNav.mockReset();
    handlers.goto = undefined;
  });

  it('goto switches view and keeps a payload for the destination view', () => {
    nav.goto({ view: 'convert', paths: ['C:\\a.png'], url: null });
    expect(nav.view).toBe('convert');
    expect(nav.pending?.paths).toEqual(['C:\\a.png']);
  });

  it('consume only hands the payload to the addressed view, once', () => {
    nav.goto({ view: 'convert', paths: ['x'], url: null });
    expect(nav.consume('download')).toBeNull();
    expect(nav.consume('convert')?.paths).toEqual(['x']);
    expect(nav.consume('convert')).toBeNull();
  });

  it('an empty payload only navigates', () => {
    nav.goto({ view: 'general', paths: [], url: null });
    expect(nav.view).toBe('general');
    expect(nav.pending).toBeNull();
  });

  it('a url payload is kept for the download view', () => {
    nav.goto({ view: 'download', paths: [], url: 'https://example.com/v' });
    expect(nav.consume('download')?.url).toBe('https://example.com/v');
  });

  it('init applies the pending launch navigation and follows nav://goto', async () => {
    takePendingNav.mockResolvedValue({ view: 'clipboard', paths: [], url: null });
    await nav.init();
    expect(nav.view).toBe('clipboard');
    handlers.goto?.({ view: 'recent', paths: [], url: null });
    expect(nav.view).toBe('recent');
  });

  it('init survives a rejecting backend', async () => {
    takePendingNav.mockRejectedValue(new Error('not ready'));
    await expect(nav.init()).resolves.toBeUndefined();
    expect(nav.view).toBe('convert');
  });
});
