import { api, on } from '../ipc';
import type { NavPayload, View } from '../types';

export const VIEWS: View[] = ['convert', 'download', 'clipboard', 'recent', 'settings'];

export class NavStore {
  view = $state<View>('convert');
  /** Payload (dropped paths / url) waiting for the target view to consume it. */
  pending = $state<NavPayload | null>(null);
  private started = false;

  go(view: View) {
    this.view = view;
  }

  /** Navigate and leave the payload for the destination view (`consume()` clears it). */
  goto(payload: NavPayload) {
    this.pending = payload.paths.length > 0 || payload.url ? payload : null;
    this.view = payload.view;
  }

  /** Take the pending payload if it is addressed to `view`. */
  consume(view: View): NavPayload | null {
    const p = this.pending;
    if (!p || p.view !== view) return null;
    this.pending = null;
    return p;
  }

  /** Apply the navigation that launched the app, then follow `nav://goto`. */
  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    try {
      await on.navGoto((p) => this.goto(p));
    } catch {
      /* no event bridge */
    }
    try {
      const p = await api.takePendingNav();
      if (p) this.goto(p);
    } catch {
      /* nothing pending */
    }
  }
}

export const nav = new NavStore();
