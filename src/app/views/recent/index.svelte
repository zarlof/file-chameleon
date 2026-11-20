<!-- Recent — every job, newest first, grouped Today / Earlier, with reveal / open / retry. -->
<script lang="ts">
  import { Clock } from '@lucide/svelte';
  import { api } from '../../../lib/ipc';
  import { t } from '../../../lib/i18n';
  import { jobs } from '../../../lib/stores/jobs.svelte';
  import type { JobSnapshot } from '../../../lib/types';
  import { Button, Card, EmptyState, SectionHeader } from '../../../lib/ui';
  import JobRow from './JobRow.svelte';
  import { groupJobs, isActive } from './logic';

  const groups = $derived(groupJobs(jobs.list, Date.now()));
  const hasFinished = $derived(jobs.list.some((j) => !isActive(j)));

  function retry(job: JobSnapshot) {
    api.submitJob({ inputs: job.inputs, op: job.op }).catch(() => {});
  }
</script>

<section>
  <header>
    <h1>{t('recent.title')}</h1>
    {#if hasFinished}
      <Button size="sm" onclick={() => jobs.clear()}>{t('recent.clear')}</Button>
    {/if}
  </header>

  {#if jobs.list.length === 0}
    <EmptyState title={t('recent.empty.title')} message={t('recent.empty.message')}>
      {#snippet icon()}<Clock />{/snippet}
    </EmptyState>
  {:else}
    {#each [['today', groups.today], ['earlier', groups.earlier]] as const as [key, list] (key)}
      {#if list.length > 0}
        <div class="group">
          <SectionHeader title={t(`recent.${key}`)} />
          <Card>
            {#each list as job (job.id)}
              <JobRow {job} onretry={retry} />
            {/each}
          </Card>
        </div>
      {/if}
    {/each}
  {/if}
</section>

<style>
  section { padding: 0 var(--space-6) var(--space-6); }
  header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); margin-bottom: var(--space-4); }
  h1 { margin: 0; font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .group { margin-bottom: var(--space-5); }
</style>
