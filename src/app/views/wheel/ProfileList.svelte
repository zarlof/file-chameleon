<!--
  ProfileList — column 1 of the wheel editor: searchable list of profiles with their
  "front·back" slot counts, and "New profile".
  Props: profiles, selected (profile id), onselect(id), oncreate().
-->
<script lang="ts">
  import { Plus, Search } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import type { Profile, SlotIcon } from '../../../lib/types';
  import { Button, TextField } from '../../../lib/ui';
  import { slotIcon } from '../../../lib/wheel/icons';
  import { profileLabel } from '../../../lib/wheel/labels';
  import { filterProfiles, slotCounts } from './logic';

  interface Props {
    profiles: Profile[];
    selected: string;
    onselect: (id: string) => void;
    oncreate: () => void;
  }
  let { profiles, selected, onselect, oncreate }: Props = $props();

  let query = $state('');
  const shown = $derived(filterProfiles(profiles, query, t));

  /** Icons of the built-in profiles; user profiles get a plain file. */
  const ICONS: Record<string, SlotIcon> = {
    images: 'image', vector: 'vector', video: 'video', audio: 'audio', pdf: 'pdf', text: 'data', data: 'data',
    subtitles: 'subtitle', archives: 'archive', mixed: 'group',
  };
</script>

<div class="profiles">
  <h2>{t('wheel.profiles.title')}</h2>
  <TextField bind:value={query} type="search" placeholder={t('wheel.profiles.search')}>
    {#snippet leading()}<Search size={14} />{/snippet}
  </TextField>
  <div class="list">
    {#each shown as p (p.id)}
      {@const Icon = slotIcon(ICONS[p.id] ?? null)}
      <button type="button" class="item" class:on={p.id === selected} aria-pressed={p.id === selected}
        title={t('wheel.profiles.counts', { front: p.front.length, back: p.back.length })} onclick={() => onselect(p.id)}>
        <Icon size={15} strokeWidth={1.8} />
        <span class="name">{profileLabel(p, t)}</span>
        <span class="count">{slotCounts(p)}</span>
      </button>
    {:else}
      <p class="none">{t('wheel.profiles.none')}</p>
    {/each}
  </div>
  <div class="add">
    <Button size="sm" variant="plain" onclick={oncreate}>
      {#snippet icon()}<Plus size={14} />{/snippet}
      {t('presets.newProfile')}
    </Button>
  </div>
</div>

<style>
  .profiles { display: flex; flex-direction: column; gap: var(--space-2); min-height: 0; }
  .profiles > :global(*) { flex: none; }
  .profiles > .list { flex: 0 1 auto; }
  h2 { margin: 0 0 var(--space-1); font: var(--text-headline); }
  .list { display: flex; flex-direction: column; gap: 2px; min-height: 0; overflow: auto; margin: 0 -4px; padding: 2px 4px; }
  .item {
    display: flex; align-items: center; gap: var(--space-2); width: 100%; min-height: 30px; padding: 4px 8px; border: 0;
    border-radius: var(--radius-sm); background: transparent; color: var(--text-secondary); font: var(--text-body); text-align: left;
    cursor: default; transition: background var(--dur-fast) var(--ease-out);
  }
  .item:hover { background: var(--control-bg); }
  .item.on { background: var(--accent-soft); color: var(--text); }
  .item.on :global(svg) { color: var(--accent); }
  .name { flex: 1; min-width: 0; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count {
    flex: none; padding: 1px 7px; border-radius: 99px; background: var(--control-bg-hover); color: var(--text-secondary);
    font: var(--text-caption); font-variant-numeric: tabular-nums;
  }
  .on .count { background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--accent); }
  .none { margin: var(--space-2) 8px; font: var(--text-callout); color: var(--text-tertiary); }
  .add { flex: none; }
</style>
