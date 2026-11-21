<!--
  PresetPicker — column 3 of the wheel editor: presets and groups to add to the current face
  (click adds), with search, category filters, "New preset", "New group" and "Copy to folder".
  Presets that cannot handle the profile's files are dimmed, with the reason on hover.
  Props: config, profile, face, fit (preset ids that fit the profile; null = no dimming),
         onadd(slotId), onnewpreset(), onnewgroup(), oncopyfolder().
-->
<script lang="ts">
  import { Check, FolderPlus, Layers, Plus, Search } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import type { Profile, WheelConfig, WheelFace } from '../../../lib/types';
  import { Button, TextField } from '../../../lib/ui';
  import { slotIcon } from '../../../lib/wheel/icons';
  import { profileLabel } from '../../../lib/wheel/labels';
  import { filterPresets, PRESET_FILTERS, presetSlot, type PickItem, type PresetFilter } from './logic';

  interface Props {
    config: WheelConfig;
    profile: Profile;
    face: WheelFace;
    fit: Set<string> | null;
    onadd: (slotId: string) => void;
    onnewpreset: () => void;
    onnewgroup: () => void;
    oncopyfolder: () => void;
  }
  let { config, profile, face, fit, onadd, onnewpreset, onnewgroup, oncopyfolder }: Props = $props();

  let query = $state('');
  let filter = $state<PresetFilter>('all');
  const items = $derived(filterPresets(config.presets, config.groups, query, filter, t));

  function unfit(item: PickItem): boolean {
    const ok = fit;
    if (!ok) return false;
    return item.kind === 'group' ? !item.members.some((p) => ok.has(p.id)) : !ok.has(item.id);
  }

  /** Detail shown next to the name ("70%", "16:9", "Group · 3"), unless the name already says it. */
  function meta(item: PickItem): string | null {
    if (item.kind === 'group') return t('wheel.picker.groupOf', { n: item.members.length });
    const d = presetSlot(item.preset).detail;
    return d && !item.label.includes(d) ? d : null;
  }
</script>

<div class="picker">
  <h2>{t('wheel.picker.title')}</h2>
  <div class="actions">
    <Button size="sm" onclick={onnewpreset}>
      {#snippet icon()}<Plus size={14} />{/snippet}
      {t('presets.newPreset')}
    </Button>
    <Button size="sm" onclick={onnewgroup}>
      {#snippet icon()}<Layers size={14} />{/snippet}
      {t('presets.newGroup')}
    </Button>
    <Button size="sm" onclick={oncopyfolder}>
      {#snippet icon()}<FolderPlus size={14} />{/snippet}
      {t('wheel.picker.copyToFolder')}
    </Button>
  </div>
  <TextField bind:value={query} type="search" placeholder={t('wheel.picker.search')}>
    {#snippet leading()}<Search size={14} />{/snippet}
  </TextField>
  <div class="filters">
    {#each PRESET_FILTERS as f (f)}
      <button type="button" class="filter" class:on={filter === f} aria-pressed={filter === f} onclick={() => (filter = f)}>
        {t(`presets.filter.${f}`)}
      </button>
    {/each}
  </div>
  <div class="chips">
    {#each items as item (item.id)}
      {@const on = profile[face].includes(item.id)}
      {@const off = unfit(item)}
      {@const Icon = slotIcon(item.kind === 'group' ? 'group' : presetSlot(item.preset).icon)}
      {@const m = meta(item)}
      <button type="button" class="chip" class:on class:off onclick={() => onadd(item.id)}
        title={off ? t('wheel.picker.unfit', { profile: profileLabel(profile, t) }) : on ? t('wheel.picker.onFace') : undefined}>
        <Icon size={13} strokeWidth={1.9} />
        <span class="label">{item.label}</span>
        {#if m}<span class="meta">{m}</span>{/if}
        {#if on}<Check size={12} strokeWidth={2.6} />{/if}
      </button>
    {:else}
      <p class="none">{t('wheel.picker.none')}</p>
    {/each}
  </div>
</div>

<style>
  .picker { display: flex; flex-direction: column; gap: var(--space-2); min-height: 0; }
  .picker > :global(*) { flex: none; }
  h2 { margin: 0 0 var(--space-1); font: var(--text-headline); }
  .actions { display: flex; flex-wrap: wrap; gap: 6px; }
  .filters { display: flex; flex-wrap: wrap; gap: 4px; }
  .filter {
    height: 22px; padding: 0 9px; border: 0; border-radius: 99px; background: var(--control-bg); color: var(--text-secondary);
    font: var(--text-caption); cursor: default; transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast);
  }
  .filter:hover { background: var(--control-bg-hover); }
  .filter.on { background: var(--accent); color: var(--on-accent); }
  .picker > .chips { flex: 1 1 0; }
  .chips { display: flex; flex-wrap: wrap; align-content: flex-start; gap: 6px; min-height: 0; overflow: auto; margin: 0 -4px; padding: 2px 4px 4px; }
  .chip {
    display: inline-flex; align-items: center; gap: 6px; max-width: 100%; height: 28px; padding: 0 10px; border: 0;
    border-radius: var(--radius-sm); background: var(--card-bg); box-shadow: var(--shadow-sm); color: var(--text);
    font: var(--text-callout); font-weight: 500; cursor: default;
    transition: background var(--dur-fast) var(--ease-out), opacity var(--dur-fast), transform var(--dur-fast) var(--ease-out);
  }
  .chip:hover { background: var(--card-bg-hover); }
  .chip:active { transform: scale(0.97); }
  .chip :global(svg) { flex: none; color: var(--text-secondary); }
  .chip.on { background: var(--accent-soft); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent); }
  .chip.on :global(svg) { color: var(--accent); }
  .chip.off { opacity: 0.45; }
  .label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { flex: none; font: var(--text-caption); color: var(--text-secondary); }
  .none { margin: var(--space-2) 4px; font: var(--text-callout); color: var(--text-tertiary); }
</style>
