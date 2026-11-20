// Pure helpers shared by the Recent view and the Convert queue.
import type { JobSnapshot, Operation } from '../../../lib/types';

export const ERROR_CODES = [
  'cancelled', 'missingTool', 'unsupported', 'notFound', 'tooLarge', 'network', 'checksum', 'toolFailed', 'failed', 'io',
] as const;

/** i18n key for a job's error code (unknown / missing codes fall back to the generic "failed" text). */
export function errorKey(code: string | null | undefined): string {
  return `recent.errors.${code && (ERROR_CODES as readonly string[]).includes(code) ? code : 'failed'}`;
}

export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
}

/** First file name, plus "+N" for the rest. */
export function jobTitle(inputs: string[]): string {
  if (inputs.length === 0) return '';
  const first = baseName(inputs[0]);
  return inputs.length > 1 ? `${first} +${inputs.length - 1}` : first;
}

/** i18n key + params describing an operation, e.g. `recent.op.convert` with `{ to: 'PNG' }`. */
export function opLabel(op: Operation): { key: string; params?: Record<string, string> } {
  switch (op.type) {
    case 'convert':
      return { key: 'recent.op.convert', params: { to: op.to.toUpperCase() } };
    case 'compress':
      return { key: 'recent.op.compress', params: { level: op.level } };
    default:
      return { key: `recent.op.${op.type}` };
  }
}

export function isActive(j: JobSnapshot): boolean {
  return j.state === 'queued' || j.state === 'running';
}

export function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Newest first, split into jobs created today (local time) and earlier. */
export function groupJobs(list: JobSnapshot[], now: number): { today: JobSnapshot[]; earlier: JobSnapshot[] } {
  const sorted = list.slice().sort((a, b) => b.createdAt - a.createdAt);
  const cut = startOfDay(now);
  return { today: sorted.filter((j) => j.createdAt >= cut), earlier: sorted.filter((j) => j.createdAt < cut) };
}
