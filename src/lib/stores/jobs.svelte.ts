import { api, on } from '../ipc';
import type { JobSnapshot } from '../types';

/** Merge a snapshot into a list by id (replace in place, or prepend when new). Pure helper. */
export function mergeJob(list: JobSnapshot[], snap: JobSnapshot): JobSnapshot[] {
  const i = list.findIndex((j) => j.id === snap.id);
  if (i < 0) return [snap, ...list];
  const next = list.slice();
  next[i] = snap;
  return next;
}

class JobsStore {
  /** Newest first. */
  list = $state<JobSnapshot[]>([]);
  private started = false;

  get active(): JobSnapshot[] {
    return this.list.filter((j) => j.state === 'queued' || j.state === 'running');
  }

  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    try {
      await on.jobUpdate((s) => (this.list = mergeJob(this.list, s)));
    } catch {
      /* no event bridge */
    }
    await this.refresh();
  }

  async refresh(): Promise<void> {
    try {
      const fresh = await api.listJobs();
      // Keep any newer live snapshots that arrived while the list was loading.
      let merged = fresh.slice().sort((a, b) => b.createdAt - a.createdAt);
      for (const live of this.list) merged = mergeJob(merged, live);
      this.list = merged;
    } catch {
      /* leave as is */
    }
  }

  async clear(): Promise<void> {
    try {
      await api.clearJobs();
      this.list = this.list.filter((j) => j.state === 'queued' || j.state === 'running');
    } catch {
      /* ignore */
    }
  }
}

export const jobs = new JobsStore();
