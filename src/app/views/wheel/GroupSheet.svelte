<!--
  GroupSheet — create or edit a group: a name and up to 6 presets in order (they fan out around
  the group's wheel slot). Mount it to open it; it calls onclose() when dismissed.
  Props: group (null = new), presets (all presets), onsave(group), ondelete?(), onclose().
  onsave gets the edited group; a new group has id "" (the caller assigns one).
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { Check, ChevronLeft, ChevronRight, Search, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import type { Preset, PresetGroup } from '../../../lib/types';
  import { Button, IconButton, Sheet, TextField } from '../../../lib/ui';
  import { presetLabel } from '../../../lib/wheel/labels';
  import { filterPresets, groupLabel, MAX_GROUP } from './logic';

  interface Props {
    group: PresetGroup | null;
    presets: Preset[];
    onsave: (g: PresetGroup) => void;
    ondelete?: () => void;
    onclose: () => void;
  }
  let { group, presets, onsave, ondelete, onclose }: Props = $props();

  const initial = untrack(() => group);
  let open = $state(true);
  let name = $state(initial?.name ?? '');
  let members = $state<string[]>(initial ? [...initial.presets] : []);
  let query = $state('');

  const options = $derived(filterPresets(presets, [], query, 'all', t));
  const full = $derived(members.length >= MAX_GROUP);
  const labelOf = (id: string) => {
    const p = presets.find((x) => x.id === id);
    return p ? presetLabel(p, t) : id;
  };

  function toggle(id: string) {
    if (members.includes(id)) members = members.filter((m) => m !== id);
    else if (!full) members = [...members, id];
  }

  function move(i: number, to: number) {
    if (to < 0 || to >= members.length) return;
    const next = members.slice();
    const [id] = next.splice(i, 1);
    next.splice(to, 0, id);
    members = next;
  }

  function close() {
    open = false;
    onclose();
  }

  function save() {
    // A built-in group keeps "" (shown localized) until renamed; other groups never end up nameless.
    const builtin = initial !== null && initial.name === '';
    const fallback = builtin ? '' : t('wheel.group.defaultName');
    onsave({ id: initial?.id ?? '', name: name.trim() || fallback, presets: members });
    close();
  }
</script>

<Sheet bind:open title={initial ? t('wheel.group.editTitle') : t('wheel.group.newTitle')} width={460} {onclose}>
  <div class="form">
    <label class="field">
      <span>{t('wheel.group.name')}</span>
      <TextField bind:value={name} placeholder={initial ? groupLabel(initial, t) : t('wheel.group.defaultName')} label={t('wheel.group.name')} />
    </label>

    <div class="head">{t('wheel.group.members', { n: members.length, max: MAX_GROUP })}</div>
    {#if members.length === 0}
      <p class="hint">{t('wheel.group.noMembers')}</p>
    {:else}
      <ol class="members">
        {#each members as id, i (id)}
          <li>
            <span class="n">{i + 1}</span>
            <span class="label">{labelOf(id)}</span>
            <IconButton label={t('wheel.group.moveEarlier')} size={22} disabled={i === 0} onclick={() => move(i, i - 1)}><ChevronLeft size={13} /></IconButton>
            <IconButton label={t('wheel.group.moveLater')} size={22} disabled={i === members.length - 1} onclick={() => move(i, i + 1)}><ChevronRight size={13} /></IconButton>
            <IconButton label={t('wheel.group.remove')} size={22} onclick={() => toggle(id)}><X size={12} /></IconButton>
          </li>
        {/each}
      </ol>
    {/if}

    <TextField bind:value={query} type="search" placeholder={t('wheel.group.search')}>
      {#snippet leading()}<Search size={14} />{/snippet}
    </TextField>
    <div class="options" title={full ? t('wheel.group.full', { max: MAX_GROUP }) : undefined}>
      {#each options as item (item.id)}
        {@const on = members.includes(item.id)}
        <button type="button" class="opt" class:on aria-pressed={on} disabled={!on && full} onclick={() => toggle(item.id)}>
          {#if on}<Check size={12} strokeWidth={2.6} />{/if}
          {item.label}
        </button>
      {/each}
    </div>
  </div>
  {#snippet footer()}
    {#if initial && ondelete}
      <Button variant="destructive" onclick={() => { ondelete?.(); close(); }}>{t('wheel.group.delete')}</Button>
      <span class="grow"></span>
    {/if}
    <Button onclick={close}>{t('common.actions.cancel')}</Button>
    <Button variant="primary" disabled={members.length === 0} onclick={save}>
      {initial ? t('wheel.group.save') : t('wheel.group.create')}
    </Button>
  {/snippet}
</Sheet>

<style>
  .form { display: flex; flex-direction: column; gap: var(--space-3); }
  .field { display: flex; flex-direction: column; gap: 6px; font: var(--text-callout); color: var(--text-secondary); }
  .head { font: var(--text-caption); color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.02em; }
  .hint { margin: 0; font: var(--text-callout); color: var(--text-tertiary); }
  .members { display: flex; flex-direction: column; gap: 2px; margin: 0; padding: 3px; list-style: none; border-radius: var(--radius-md); background: var(--card-bg); box-shadow: var(--shadow-sm); }
  .members li { display: flex; align-items: center; gap: var(--space-2); height: 28px; padding: 0 4px 0 8px; border-radius: var(--radius-sm); }
  .members li:hover { background: var(--control-bg); }
  .n { width: 14px; font: var(--text-caption); color: var(--text-tertiary); text-align: right; font-variant-numeric: tabular-nums; }
  .label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .options { display: flex; flex-wrap: wrap; gap: 5px; max-height: 210px; overflow: auto; padding: 2px; }
  .opt {
    display: inline-flex; align-items: center; gap: 5px; height: 26px; padding: 0 9px; border: 0; border-radius: var(--radius-sm);
    background: var(--control-bg); color: var(--text); font: var(--text-callout); cursor: default; transition: background var(--dur-fast);
  }
  .opt:hover:not(:disabled) { background: var(--control-bg-hover); }
  .opt.on { background: var(--accent-soft); color: var(--accent); }
  .opt:disabled { opacity: 0.4; }
  .grow { flex: 1; }
</style>
