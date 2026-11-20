import { describe, expect, it, vi } from 'vitest';
import type { JobRequest, JobSnapshot } from './types';

const submitJob = vi.fn<(r: JobRequest) => Promise<number[]>>();
vi.mock('./ipc', () => ({ api: { submitJob: (r: JobRequest) => submitJob(r) }, on: {} }));

const { requestFor, jobs } = await import('./stores/jobs.svelte');

const snap = (id: number): JobSnapshot => ({
  id, inputs: ['a.png'], op: { type: 'convert', to: 'jpg' }, state: 'failed', progress: 0, outputs: [],
  errorCode: 'failed', error: null, createdAt: 0, finishedAt: null,
});

describe('retry request lookup', () => {
  it('falls back to inputs + op', () => {
    expect(requestFor(new Map(), snap(1))).toEqual({ inputs: ['a.png'], op: { type: 'convert', to: 'jpg' } });
  });
  it('prefers the remembered request and remembers retries too', async () => {
    const req: JobRequest = { inputs: ['a.png'], op: { type: 'convert', to: 'jpg' }, options: { quality: 40, outputDir: 'C:\\o' } };
    submitJob.mockResolvedValueOnce([7]).mockResolvedValueOnce([8]).mockResolvedValueOnce([9]);
    await jobs.submit(req);
    await jobs.retry(snap(7));
    expect(submitJob).toHaveBeenLastCalledWith(req);
    await jobs.retry(snap(8));
    expect(submitJob).toHaveBeenLastCalledWith(req);
  });
});
