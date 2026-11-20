import { api, on } from '../ipc';
import type { JobRequest, JobSnapshot } from '../types';

/** Merge a snapshot into a list by id (replace in place, or prepend when new). Pure helper. */
export function mergeJob(list: JobSnapshot[], snap: JobSnapshot): JobSnapshot[] {
  const i = list.findIndex((j) => j.id === snap.id);
  if (i < 0) return [snap, ...list];
  const next = list.slice();
  next[i] = snap;
  return next;
}

/** The request to resubmit for `job`: the remembered one when the UI submitted it, else `{ inputs, op }`. */
export function requestFor(remembered: Map<number, JobRequest>, job: JobSnapshot): JobRequest {
  return remembered.get(job.id) ?? { inputs: job.inputs, op: job.op };
}

class JobsStore {
  /** Requests submitted from this UI (in memory only), by job id, so Retry keeps the same options. */
  private requests = new Map<number, JobRequest>();

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

  /** Submit a request and remember it for each created job. */
  async submit(req: JobRequest): Promise<number[]> {
    const ids = await api.submitJob(req);
    for (const id of ids) this.requests.set(id, req);
    return ids;
  }

  /** Resubmit the same request a job was created with. */
  retry(job: JobSnapshot): Promise<number[]> {
    return this.submit(requestFor(this.requests, job));
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
