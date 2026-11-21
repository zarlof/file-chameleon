<!--
  Presets page ("Hazır ayarlar") — every preset in a searchable, filterable list, and an editor
  for the selected one. Built-ins are read-only ("Duplicate" makes an editable u-<n> copy);
  user presets can be edited (saved as you type, through the wheelConfig store) and deleted.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { Copy, Plus, Search, SlidersHorizontal, Trash2 } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { settings } from '../../../lib/stores/settings.svelte';
  import { wheelConfig } from '../../../lib/stores/wheelConfig.svelte';
  import type { Preset } from '../../../lib/types';
  import { Badge, Button, EmptyState, Sheet, TextField } from '../../../lib/ui';
  import { slotIcon } from '../../../lib/wheel/icons';
  import { presetLabel } from '../../../lib/wheel/labels';
  import { deletePreset, filterPresets, newPresetId, PRESET_FILTERS, presetSlot, upsertPreset, type PresetFilter } from '../wheel/logic';
  import PresetEditor from './PresetEditor.svelte';
  import { defaultOp, describeOp, describePreset, duplicatePreset, EMPTY_OPTIONS, presetUsage } from './logic';

  let query = $state('');
  let filter = $state<PresetFilter>('all');
  let selectedId = $state<string | null>(null);
  let confirmDelete = $state(false);

  const config = $derived(wheelConfig.value);
  const items = $derived(config ? filterPresets(config.presets, [], query, filter, t) : []);
  const current = $derived(config ? (config.presets.find((p) => p.id === selectedId) ?? config.presets[0] ?? null) : null);
  const usage = $derived(config && current ? presetUsage(config, current.id) : { profiles: 0, groups: 0 });
  const watching = $derived(current ? settings.value.watchFolders.filter((w) => w.preset === current.id).length : 0);

  onMount(() => void wheelConfig.load());

  function save(p: Preset) {
    if (wheelConfig.value) wheelConfig.save(upsertPreset(wheelConfig.value, p));
  }

  function create() {
    if (!config) return;
    const op = defaultOp('convert');
    const p: Preset = {
      id: newPresetId(config.presets.map((x) => x.id)),
      name: describeOp(op, t),
      builtin: false,
      op,
      options: { ...EMPTY_OPTIONS },
    };
    save(p);
    selectedId = p.id;
    query = '';
    filter = 'all';
  }

  function duplicate(p: Preset) {
    if (!config) return;
    const copy = duplicatePreset(p, config.presets.map((x) => x.id), t);
    save(copy);
    selectedId = copy.id;
  }

  function remove() {
    confirmDelete = false;
    if (!config || !current || current.builtin) return;
    const i = config.presets.findIndex((p) => p.id === current.id);
    const next = deletePreset(config, current.id);
    selectedId = next.presets[Math.min(i, next.presets.length - 1)]?.id ?? null;
    wheelConfig.save(next);
  }
</script>

<section>
  <header>
    <h1>{t('common.nav.presets')}</h1>
    {#if config}
      <Button size="sm" variant="primary" onclick={create}>
        {#snippet icon()}<Plus size={14} />{/snippet}
        {t('presets.newPreset')}
      </Button>
    {/if}
  </header>

  {#if !config}
    {#if wheelConfig.failed}
      <EmptyState title={t('wheel.loadFailed')}>
        {#snippet icon()}<SlidersHorizontal />{/snippet}
        {#snippet action()}<Button onclick={() => wheelConfig.load()}>{t('wheel.retry')}</Button>{/snippet}
      </EmptyState>
    {/if}
  {:else}
    <div class="cols">
      <div class="list-col">
        <TextField bind:value={query} type="search" placeholder={t('presetsPage.search')}>
          {#snippet leading()}<Search size={14} />{/snippet}
        </TextField>
        <div class="filters">
          {#each PRESET_FILTERS as f (f)}
            <button type="button" class="filter" class:on={filter === f} aria-pressed={filter === f} onclick={() => (filter = f)}>
              {t(`presets.filter.${f}`)}
            </button>
          {/each}
        </div>
        <div class="list">
          {#each items as item (item.id)}
            {#if item.kind === 'preset'}
              {@const p = item.preset}
              {@const Icon = slotIcon(presetSlot(p).icon)}
              <button type="button" class="row" class:on={p.id === current?.id} aria-pressed={p.id === current?.id} onclick={() => (selectedId = p.id)}>
                <span class="tile"><Icon size={15} strokeWidth={1.8} /></span>
                <span class="text">
                  <span class="name">{item.label}</span>
                  <span class="sub">{describePreset(p, t)}</span>
                </span>
              </button>
            {/if}
          {:else}
            <p class="none">{t('presetsPage.none')}</p>
          {/each}
        </div>
      </div>

      <div class="edit-col">
        {#if current}
          {@const Icon = slotIcon(presetSlot(current).icon)}
          <div class="head">
            <span class="tile big"><Icon size={20} strokeWidth={1.8} /></span>
            <div class="title">
              <h2>{presetLabel(current, t)}</h2>
              <p>{describePreset(current, t)}</p>
            </div>
            {#if current.builtin}<Badge>{t('presetsPage.builtin')}</Badge>{/if}
            <span class="acts">
              <Button size="sm" onclick={() => duplicate(current)}>
                {#snippet icon()}<Copy size={13} />{/snippet}
                {t('presetsPage.duplicate')}
              </Button>
              {#if !current.builtin}
                <Button size="sm" variant="destructive" onclick={() => (confirmDelete = true)}>
                  {#snippet icon()}<Trash2 size={13} />{/snippet}
                  {t('presetsPage.delete')}
                </Button>
              {/if}
            </span>
          </div>
          {#if current.builtin}<p class="note">{t('presetsPage.builtinHint')}</p>{/if}
          {#key current.id}
            <PresetEditor preset={current} readonly={current.builtin} onchange={save} />
          {/key}
        {:else}
          <EmptyState title={t('presetsPage.pick')}>
            {#snippet icon()}<SlidersHorizontal />{/snippet}
          </EmptyState>
        {/if}
      </div>
    </div>
  {/if}
</section>

<Sheet bind:open={confirmDelete} title={current ? t('presetsPage.deleteTitle', { name: presetLabel(current, t) }) : undefined}>
  <div class="confirm">
    <p>{t('presetsPage.deleteMessage')}</p>
    {#if usage.profiles}<p>{t('presetsPage.usedProfiles', { count: usage.profiles })}</p>{/if}
    {#if usage.groups}<p>{t('presetsPage.usedGroups', { count: usage.groups })}</p>{/if}
    {#if watching}<p class="warn">{t('presetsPage.usedWatch', { count: watching })}</p>{/if}
  </div>
  {#snippet footer()}
    <Button onclick={() => (confirmDelete = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={remove}>{t('presetsPage.delete')}</Button>
  {/snippet}
</Sheet>

<style>
  section { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--space-6); }
  header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex: none; margin-bottom: var(--space-3); }
  h1 { margin: 0; font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .cols { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(220px, 300px) minmax(0, 1fr); gap: var(--space-5); }
  .list-col { display: flex; flex-direction: column; gap: var(--space-2); min-height: 0; padding-bottom: var(--space-4); }
  .edit-col { min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: var(--space-3); padding: 2px 4px var(--space-6) 2px; }
  .filters { display: flex; flex-wrap: wrap; gap: 4px; }
  .filter {
    height: 22px; padding: 0 9px; border: 0; border-radius: 99px; background: var(--control-bg); color: var(--text-secondary);
    font: var(--text-caption); cursor: default; transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast);
  }
  .filter:hover { background: var(--control-bg-hover); }
  .filter.on { background: var(--accent); color: var(--on-accent); }
  .list { flex: 1; min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: 2px; margin: 0 -4px; padding: 2px 4px; }
  .row {
    display: flex; align-items: center; gap: var(--space-2); width: 100%; padding: 5px 8px; border: 0; border-radius: var(--radius-sm);
    background: transparent; color: var(--text); font: inherit; text-align: left; cursor: default; transition: background var(--dur-fast);
  }
  .row:hover { background: var(--control-bg); }
  .row.on { background: var(--accent-soft); }
  .tile { display: grid; place-items: center; flex: none; width: 28px; height: 28px; border-radius: var(--radius-sm); background: var(--control-bg); color: var(--text-secondary); }
  .row.on .tile { background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--accent); }
  .tile.big { width: 40px; height: 40px; border-radius: var(--radius-md); background: var(--accent-soft); color: var(--accent); }
  .text { display: flex; flex-direction: column; min-width: 0; }
  .name { font: var(--text-body); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { font: var(--text-callout); color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .none { margin: var(--space-2) 8px; font: var(--text-callout); color: var(--text-tertiary); }
  .head { display: flex; align-items: center; gap: var(--space-3); flex-wrap: wrap; }
  .title { flex: 1; min-width: 160px; }
  h2 { margin: 0; font: var(--text-title); letter-spacing: var(--tracking-tight); }
  .title p { margin: 2px 0 0; font: var(--text-callout); color: var(--text-secondary); }
  .acts { display: inline-flex; gap: 6px; }
  .note { margin: 0; font: var(--text-callout); color: var(--text-secondary); }
  .confirm p { margin: 0 0 6px; color: var(--text-secondary); }
  .confirm .warn { color: var(--orange); }
</style>
