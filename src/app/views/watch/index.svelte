<!--
  Watch folders ("İzlenen klasörler") — rules of folder + preset (+ subfolders, on/off) that convert new
  files automatically. Rules live in the settings; the status line comes from the backend
  (`watch_status` + `watch://status`, refreshed every 5 s for folder errors). Presets that cannot run
  unattended are not offered; a rule whose preset is gone shows an error row.
-->
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { Folder, FolderPlus, FolderSync, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { api, on } from '../../../lib/ipc';
  import { settings } from '../../../lib/stores/settings.svelte';
  import { wheelConfig } from '../../../lib/stores/wheelConfig.svelte';
  import type { WatchFolder, WatchStatus } from '../../../lib/types';
  import { Button, Card, EmptyState, IconButton, ListRow, Select, Toggle } from '../../../lib/ui';
  import { folderName } from '../wheel/logic';
  import { newRule, presetOptions, statusLine, usablePresets } from './logic';

  const rules = $derived(settings.value.watchFolders);
  const presets = $derived(wheelConfig.value?.presets ?? []);
  const canAdd = $derived(usablePresets(presets).length > 0);
  let statuses = $state<Record<string, WatchStatus>>({});

  function save(next: WatchFolder[]) {
    void settings.update({ watchFolders: next });
  }
  function change(id: string, patch: Partial<WatchFolder>) {
    save(rules.map((r) => (r.id === id ? { ...r, ...patch } : r)));
  }

  async function add() {
    try {
      const path = await open({ directory: true });
      if (typeof path !== 'string') return;
      // The same folder twice would process every file twice.
      if (rules.some((r) => r.path.toLowerCase() === path.toLowerCase())) return;
      save([...rules, newRule(path, presets, rules.map((r) => r.id))]);
    } catch {
      /* dialog unavailable */
    }
  }

  async function refresh() {
    try {
      const all = await api.watchStatus();
      statuses = Object.fromEntries(all.map((s) => [s.id, s]));
    } catch {
      /* no backend */
    }
  }

  // The backend reconfigures when the rules change; ask again for the fresh state.
  $effect(() => {
    void rules;
    void wheelConfig.revision;
    const timer = setTimeout(() => void refresh(), 300);
    return () => clearTimeout(timer);
  });

  onMount(() => {
    void wheelConfig.load();
    void refresh();
    const poll = setInterval(() => void refresh(), 5000);
    let disposed = false;
    let unlisten: (() => void) | undefined;
    on.watchStatus((s) => (statuses = { ...statuses, [s.id]: s }))
      .then((u) => (disposed ? u() : (unlisten = u)))
      .catch(() => {});
    return () => {
      disposed = true;
      clearInterval(poll);
      unlisten?.();
    };
  });
</script>

<section>
  <h1>{t('watch.title')}</h1>

  {#if rules.length === 0}
    <EmptyState title={t('watch.empty.title')} message={t('watch.empty.message')}>
      {#snippet icon()}<FolderSync />{/snippet}
      {#snippet action()}
        <Button variant="primary" disabled={!canAdd} onclick={add}>
          {#snippet icon()}<FolderPlus size={14} />{/snippet}
          {t('watch.add')}
        </Button>
        {#if !canAdd && wheelConfig.value}<p class="hint">{t('watch.noPresets')}</p>{/if}
      {/snippet}
    </EmptyState>
  {:else}
    <div class="top">
      <p class="intro">{t('watch.intro')}</p>
      <Button size="sm" disabled={!canAdd} onclick={add} title={canAdd ? undefined : t('watch.noPresets')}>
        {#snippet icon()}<FolderPlus size={13} />{/snippet}
        {t('watch.add')}
      </Button>
    </div>
    <Card>
      {#each rules as rule (rule.id)}
        {@const line = statusLine(rule, statuses[rule.id], t)}
        <ListRow title={folderName(rule.path)} subtitle={rule.path}>
          {#snippet leading()}<Folder size={16} />{/snippet}
          <span class="line" class:err={line.error} title={line.detail}>{line.text}</span>
          <div class="opts">
            <span class="opt">
              <span class="lbl">{t('watch.preset')}</span>
              <Select value={rule.preset} options={presetOptions(rule, presets, t)} label={t('watch.preset')} onchange={(v) => change(rule.id, { preset: v })} />
            </span>
            <span class="opt">
              <Toggle checked={rule.subfolders} label={t('watch.subfolders')} onchange={(v) => change(rule.id, { subfolders: v })} />
              <span class="lbl">{t('watch.subfolders')}</span>
            </span>
          </div>
          {#snippet trailing()}
            <Toggle checked={rule.enabled} label={t('watch.enabled')} onchange={(v) => change(rule.id, { enabled: v })} />
            <IconButton label={t('watch.remove')} onclick={() => save(rules.filter((r) => r.id !== rule.id))}><X size={14} /></IconButton>
          {/snippet}
        </ListRow>
      {/each}
    </Card>
  {/if}
</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; min-height: calc(100% - var(--space-6)); padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .top { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-4); }
  .intro { margin: 0; font: var(--text-callout); color: var(--text-secondary); }
  .hint { margin: var(--space-2) 0 0; font: var(--text-callout); color: var(--text-tertiary); }
  .line { display: block; font: var(--text-callout); color: var(--text-secondary); white-space: normal; }
  .line.err { color: var(--red); }
  .opts { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4); margin-top: var(--space-2); }
  .opt { display: inline-flex; align-items: center; gap: var(--space-2); }
  .lbl { font: var(--text-callout); color: var(--text-secondary); }
</style>
