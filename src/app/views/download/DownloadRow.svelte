<!--
  DownloadRow — one active or finished download.
  Props: d (DownloadSnapshot), tool? (manifest id this row's failure asks to install, with `toolName`,
         `toolBusy`), oncancel, onreveal, onconvert, oninstall.
-->
<script lang="ts">
  import { X } from '@lucide/svelte';
  import { Button, Icon, IconButton, ListRow, ProgressBar, Spinner, formatBytes } from '../../../lib/ui';
  import { t } from '../../../lib/i18n';
  import type { DownloadSnapshot } from '../../../lib/types';
  import { formatDuration, hostOf, isActive } from './helpers';

  interface Props {
    d: DownloadSnapshot;
    tool?: string | null;
    toolName?: string;
    toolBusy?: boolean;
    oncancel: (id: number) => void;
    onreveal: (path: string) => void;
    onconvert: (path: string) => void;
    oninstall: (tool: string) => void;
  }
  let { d, tool = null, toolName = '', toolBusy = false, oncancel, onreveal, onconvert, oninstall }: Props = $props();

  const active = $derived(isActive(d.state));
  const title = $derived(d.title && d.title !== d.url ? d.title : hostOf(d.url));
  const subtitle = $derived.by(() => {
    if (d.state === 'failed') return failureText();
    if (active) {
      const parts = [t(`download.state.${d.state}`)];
      if (d.speedBps) parts.push(t('download.speed', { speed: formatBytes(d.speedBps) }));
      if (d.etaSecs !== null) parts.push(t('download.eta', { time: formatDuration(d.etaSecs) }));
      return parts.join(' · ');
    }
    return t(`download.state.${d.state}`);
  });

  function failureText(): string {
    if (tool) return t(`download.tool.reason.${tool}`, { name: toolName || tool });
    const key = `download.error.${d.errorCode ?? 'failed'}`;
    const text = t(key);
    return text === key ? t('download.error.failed') : text;
  }
</script>

<ListRow {title} {subtitle}>
  {#snippet leading()}
    {#if active}
      <Spinner size={16} />
    {:else if d.state === 'done'}
      <span class="ok"><Icon name="circle-check" size={18} /></span>
    {:else if d.state === 'failed'}
      <span class="bad"><Icon name="circle-alert" size={18} /></span>
    {:else}
      <Icon name="circle-x" size={18} />
    {/if}
  {/snippet}
  {#snippet trailing()}
    {#if active}
      <IconButton label={t('download.action.cancel')} onclick={() => oncancel(d.id)}><X size={14} /></IconButton>
    {:else if d.state === 'done' && d.output}
      {@const out = d.output}
      <Button size="sm" onclick={() => onconvert(out)}>{t('download.action.convert')}</Button>
      <IconButton label={t('download.action.reveal')} onclick={() => onreveal(out)}>
        <Icon name="folder-open" size={15} />
      </IconButton>
    {:else if d.state === 'failed' && tool}
      <Button size="sm" variant="primary" loading={toolBusy} onclick={() => oninstall(tool)}>
        {t('download.tool.install', { name: toolName || tool })}
      </Button>
    {/if}
  {/snippet}
  {#if active}
    <ProgressBar value={d.state === 'processing' && d.progress === null ? null : d.progress} />
  {/if}
</ListRow>

<style>
  .ok { display: inline-flex; color: var(--green); }
  .bad { display: inline-flex; color: var(--red); }
</style>
