import { api, on } from '../ipc';
import { setLanguage } from '../i18n';
import type { AppInfo, Settings } from '../types';

export const DEFAULT_SETTINGS: Settings = {
  language: 'system',
  theme: 'system',
  accent: 'blue',
  launchAtLogin: false,
  outputDir: null,
  imageQuality: 85,
  downloadDir: null,
  downloadMode: 'original',
  downloadQuality: 'best',
  cookiesBrowser: null,
  wheelEnabled: true,
  wheelAnyApp: false,
  recentTargets: [],
  clipboardEnabled: true,
  clipboardImages: true,
  clipboardSuggestLinks: true,
  clipboardSuggestImages: true,
  clipboardMaxItems: 200,
  clipboardRetentionDays: 30,
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
