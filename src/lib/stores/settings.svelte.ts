import { api, on } from '../ipc';
import { setLanguage } from '../i18n';
import type { AppInfo, Settings } from '../types';

export const DEFAULT_SETTINGS: Settings = {
  language: 'system',
  theme: 'light',
  accent: 'blue',
  launchAtLogin: true,
  closeToTray: true,
  contextMenu: true,
  uiScale: 100,
  reduceMotion: false,
  outputDir: null,
  imageQuality: 100,
  downloadDir: null,
  downloadMode: 'original',
  downloadQuality: 'best',
  cookiesBrowser: null,
  wheelEnabled: true,
  wheelAnyApp: true,
  recentTargets: [],
  wheelFlipScroll: true,
  wheelFlipTab: true,
  wheelShiftBack: true,
  wheelExcludedApps: [],
  wheelLabels: 'text',
  wheelOpacity: 100,
  wheelColor: 'purple',
  wheelScale: 100,
  wheelShowDetail: true,
  hotkeyShelf: '',
  hotkeyLibrary: 'Ctrl+2',
  hotkeyDownloadLink: 'Ctrl+3',
  hotkeyLine: 'Ctrl+4',
  hotkeyLineSearch: 'Ctrl+Alt+F',
  hotkeyCapture: '',
  hotkeyCopyLatestImage: '',
  lineEnabled: true,
  lineEdgeReveal: true,
  linePeekOnCopy: false,
  lineBreeze: true,
  lineRopes: { images: true, text: true, links: true, files: true, shortcuts: true },
  lineColor: '#ffffff',
  lineThickness: 3,
  lineOpacity: 100,
  lineSize: 'large',
  lineZoomOnHover: false,
  lineSound: true,
  lineRopeStyle: 'web',
  lineSag: 14,
  linePinStyle: 'spider',
  linePinColor: 'auto',
  lineCardStyle: 'paper',
  lineCardRadius: 10,
  lineCardShadow: true,
  lineShowLabels: true,
  lineShowNumbers: true,
  lineRopeOrder: ['images', 'text', 'links', 'files', 'shortcuts'],
  lineRopeNames: { images: '', text: '', links: '', files: '', shortcuts: '' },
  lineShortcuts: [],
  lineFolded: [],
  lineFilesSources: { added: true, converted: true, downloaded: true },
  lineMaxPerRope: 0,
  lineEdge: 'top',
  linePeekMs: 2500,
  lineRevealDwellMs: 350,
  lineHideDelayMs: 400,
  lineClickAction: 'open',
  lineSwing: 100,
  lineDamping: 100,
  lineBreezeStrength: 100,
  lineFall: true,
  lineSoundVolume: 60,
  lineLookVersion: 5,
  watchFolders: [],
  clipboardEnabled: true,
  clipboardCaptureText: true,
  clipboardCaptureLinks: true,
  clipboardImages: true,
  clipboardSuggestLinks: true,
  clipboardSuggestImages: true,
  // Same defaults as `Settings::default()` in crates/fc-core/src/settings.rs.
  clipboardMaxItems: 200,
  clipboardRetentionText: 30,
  clipboardRetentionLinks: 30,
  clipboardRetentionImages: 30,
  clipboardIgnoreApps: [],
  clipboardOcr: true,
  clipboardLinkTitles: false,
  clipboardWatchFolders: [],
  shelfRows: ['images', 'texts', 'links'],
  shelfCardSize: 'medium',
  shelfClickCopies: true,
  clipboardPaused: false,
  onboarded: false,
};

class SettingsStore {
  /** Always usable: defaults until the backend answers. */
  value = $state<Settings>({ ...DEFAULT_SETTINGS });
  appInfo = $state<AppInfo | null>(null);
  loaded = $state(false);
  private started = false;

  private apply(s: Settings) {
    this.value = s;
    setLanguage(s.language, this.appInfo?.systemLanguage ?? 'en');
  }

  /** Load settings + app info and follow backend changes. Safe to call more than once. */
  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    try {
      this.appInfo = await api.appInfo();
    } catch {
      /* backend not ready: keep defaults */
    }
    try {
      this.apply(await api.getSettings());
    } catch {
      this.apply(this.value);
    }
    this.loaded = true;
    try {
      await on.settingsChanged((s) => this.apply(s));
    } catch {
      /* no event bridge (tests / plain browser) */
    }
  }

  /** Optimistically merge `patch`, persist it, and adopt the backend's normalized result. */
  async update(patch: Partial<Settings>): Promise<boolean> {
    const before = this.value;
    this.apply({ ...before, ...patch });
    try {
      this.apply(await api.setSettings(this.value));
      return true;
    } catch {
      this.apply(before);
      return false;
    }
  }
}

export const settings = new SettingsStore();
