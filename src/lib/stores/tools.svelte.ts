import { api, errorOf, on } from '../ipc';
import type { ToolStatus } from '../types';

function without<T>(rec: Record<string, T>, id: string): Record<string, T> {
  const next = { ...rec };
  delete next[id];
  return next;
}

class ToolsStore {
  list = $state<ToolStatus[]>([]);
  /** Install progress 0..1 by tool id (present only while installing). */
  progress = $state<Record<string, number>>({});
  /** Last install error message by tool id. */
  errors = $state<Record<string, string>>({});
  private started = false;

  get(id: string): ToolStatus | undefined {
    return this.list.find((x) => x.id === id);
  }

  isInstalled(id: string): boolean {
    const s = this.get(id);
    return !!s && s.source !== 'missing';
  }

  isInstalling(id: string): boolean {
    return id in this.progress;
  }

  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    try {
      await on.toolProgress((p) => {
        this.progress = { ...this.progress, [p.id]: p.total > 0 ? Math.min(1, p.done / p.total) : 0 };
      });
      await on.toolDone((d) => {
        this.progress = without(this.progress, d.id);
        this.errors = d.ok ? without(this.errors, d.id) : { ...this.errors, [d.id]: d.error ?? d.errorCode ?? 'failed' };
        void this.refresh();
      });
    } catch {
      /* no event bridge */
    }
    await this.refresh();
  }

  async refresh(): Promise<void> {
    try {
      this.list = await api.toolsStatus();
    } catch {
      /* keep previous */
    }
  }

  async install(id: string): Promise<void> {
    this.progress = { ...this.progress, [id]: 0 };
    this.errors = without(this.errors, id);
    try {
      await api.installTool(id);
    } catch (e) {
      this.progress = without(this.progress, id);
      this.errors = { ...this.errors, [id]: errorOf(e).message };
    }
  }

  async uninstall(id: string): Promise<void> {
    try {
      await api.uninstallTool(id);
    } catch (e) {
      this.errors = { ...this.errors, [id]: errorOf(e).message };
    }
    await this.refresh();
  }
}

export const tools = new ToolsStore();
