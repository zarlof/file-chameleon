<!--
  JobRow — one job as a ListRow: status icon / ring, title, operation, progress, error, actions.
  Props: job, onretry? (shows "Try again" for failed/cancelled jobs).
  Used by the Recent view and the Convert queue.
-->
<script lang="ts">
  import { CircleAlert, CircleCheck, CircleSlash, Clock } from '@lucide/svelte';
  import { api } from '../../../lib/ipc';
  import { t } from '../../../lib/i18n';
  import type { JobSnapshot } from '../../../lib/types';
  import { Button, ListRow, ProgressBar, ProgressRing } from '../../../lib/ui';
  import { errorKey, isActive, jobTitle, opLabel } from './logic';

  interface Props { job: JobSnapshot; onretry?: (job: JobSnapshot) => void }
  let { job, onretry }: Props = $props();

  const label = $derived(opLabel(job.op));
  const opText = $derived(t(label.key, label.params));
  const subtitle = $derived(
    job.state === 'failed' ? `${opText} · ${t(errorKey(job.errorCode))}`
    : job.state === 'cancelled' ? `${opText} · ${t('recent.state.cancelled')}`
    : job.state === 'queued' ? `${opText} · ${t('recent.state.queued')}`
    : opText,
  );
  const output = $derived(job.outputs[0]);
</script>

<ListRow title={jobTitle(job.inputs)} {subtitle}>
  {#snippet leading()}
    <span class="status" class:ok={job.state === 'done'} class:bad={job.state === 'failed'}>
      {#if job.state === 'running'}
        <ProgressRing value={job.progress > 0 ? job.progress : null} size={20} />
      {:else if job.state === 'queued'}
        <Clock size={18} />
      {:else if job.state === 'done'}
        <CircleCheck size={18} />
      {:else if job.state === 'failed'}
        <CircleAlert size={18} />
      {:else}
        <CircleSlash size={18} />
      {/if}
    </span>
  {/snippet}
  {#snippet trailing()}
    <span class="actions">
      {#if isActive(job)}
        <Button size="sm" onclick={() => api.cancelJob(job.id).catch(() => {})}>{t('recent.cancel')}</Button>
      {:else if job.state === 'done' && output}
        <Button size="sm" onclick={() => api.revealPath(output).catch(() => {})}>{t('recent.reveal')}</Button>
        <Button size="sm" onclick={() => api.openPath(output).catch(() => {})}>{t('recent.open')}</Button>
      {:else if onretry}
        <Button size="sm" onclick={() => onretry(job)}>{t('recent.retry')}</Button>
      {/if}
    </span>
  {/snippet}
  {#if job.state === 'running'}
    <div class="bar"><ProgressBar value={job.progress > 0 ? job.progress : null} /></div>
  {/if}
  {#if job.state === 'done' && job.outputs.length > 1}
    <div class="outs">{t('recent.outputs', { count: job.outputs.length })}</div>
  {/if}
</ListRow>

<style>
  .status { display: inline-flex; align-items: center; justify-content: center; width: 24px; color: var(--text-secondary); }
  .status.ok { color: var(--green); }
  .status.bad { color: var(--red); }
  .actions { display: inline-flex; gap: var(--space-2); }
  .bar { margin-top: var(--space-2); }
  .outs { margin-top: 2px; font: var(--text-callout); color: var(--text-secondary); }
</style>
