import { api, on } from '../ipc';
import { setLanguage } from '../i18n';
import type { AppInfo, Settings } from '../types';

export const DEFAULT_SETTINGS: Settings = {
  language: 'system',
  theme: 'system',
  accent: 'blue',
  launchAtLogin: false,
  closeToTray: true,
  contextMenu: false,
  uiScale: 100,
  reduceMotion: false,
  outputDir: null,
  imageQuality: 85,
  downloadDir: null,
  downloadMode: 'original',
  downloadQuality: 'best',
  cookiesBrowser: null,
  wheelEnabled: true,
  wheelAnyApp: false,
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
  hotkeyShelf: 'Ctrl+Alt+V',
  hotkeyLibrary: '',
  hotkeyDownloadLink: 'Ctrl+Alt+D',
  watchFolders: [],
  clipboardEnabled: true,
  clipboardCaptureText: true,
  clipboardCaptureLinks: true,
  clipboardImages: true,
  clipboardSuggestLinks: true,
  clipboardSuggestImages: true,
  // Same defaults as `Settings::default()` in crates/fc-core/src/settings.rs.
  clipboardMaxItems: 500,
  clipboardRetentionText: 30,
  clipboardRetentionLinks: 30,
  clipboardRetentionImages: 7,
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
