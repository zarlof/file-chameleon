// The wheel config (presets, groups, profiles) for the Wheel and Presets pages.
// Edits are optimistic: `save(next)` shows `next` at once and sends it after a 300 ms pause
// (only the latest edit is sent). When the backend rejects it ("invalid": limits, references,
// a changed built-in) or fails, the last stored config comes back and an error toast says so.
import { t } from '../i18n';
import { api, errorOf } from '../ipc';
import type { WheelConfig } from '../types';
import { toast } from '../ui/toaster.svelte';

export const SAVE_DELAY_MS = 300;

export class WheelConfigStore {
  /** null until the first load succeeds. Never mutate: pass a new object to `save`. */
  value = $state.raw<WheelConfig | null>(null);
  /** The first load failed (no backend): the pages show a retry state. */
  failed = $state(false);
  /** Bumped whenever the backend confirms a config (load, save, reset), e.g. to re-ask
   *  `presetsForExtensions`. */
  revision = $state(0);

  /** Last config the backend confirmed (the rollback target). */
  private stored: WheelConfig | null = null;
  private queued: WheelConfig | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** Saves and resets run one after another. */
  private chain: Promise<void> = Promise.resolve();

  private adopt(c: WheelConfig) {
    this.stored = c;
    this.revision++;
  }

  /** Fetch the stored config; skipped while local edits are waiting to be saved. */
  async load(): Promise<void> {
    if (this.queued) return;
    try {
      const c = await api.getWheelConfig();
      this.adopt(c);
      if (!this.queued) this.value = c;
      this.failed = false;
    } catch {
      this.failed = this.value === null;
    }
  }

  save(next: WheelConfig): void {
    this.value = next;
    this.queued = next;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.flush(), SAVE_DELAY_MS);
  }

  /** Send a waiting edit now; resolves when every earlier save has finished. */
  flush(): Promise<void> {
    clearTimeout(this.timer);
    this.chain = this.chain.then(() => this.send());
    return this.chain;
  }

  private async send(): Promise<void> {
    const next = this.queued;
    if (!next) return;
    this.queued = null;
    try {
      const c = await api.setWheelConfig(next);
      this.adopt(c);
      if (!this.queued) this.value = c;
    } catch (e) {
      // A newer edit is already waiting: it replaces this one, so do not undo it.
      if (!this.queued) this.value = this.stored;
      toast.show(t(errorOf(e).code === 'invalid' ? 'wheel.errors.invalid' : 'wheel.errors.saveFailed'), 'error', 5000);
    }
  }

  /** Built-in profile back to its default arrangement. */
  resetProfile(id: string): Promise<boolean> {
    return this.reset(() => api.resetWheelProfile(id));
  }

  /** Everything back to the built-ins (user presets, groups and profiles are removed). */
  resetAll(): Promise<boolean> {
    return this.reset(() => api.resetWheelConfig());
  }

  private async reset(call: () => Promise<WheelConfig>): Promise<boolean> {
    // Waiting edits go first, or they would land after the reset and undo it.
    await this.flush();
    try {
      const c = await call();
      this.adopt(c);
      this.queued = null;
      clearTimeout(this.timer);
      this.value = c;
      return true;
    } catch {
      toast.show(t('wheel.errors.resetFailed'), 'error');
      return false;
    }
  }
}

export const wheelConfig = new WheelConfigStore();
