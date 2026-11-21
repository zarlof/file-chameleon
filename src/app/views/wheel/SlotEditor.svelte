<!--
  SlotEditor — column 2 of the wheel editor: Front/Back switch, a live donut (the overlay's Wheel)
  whose slices select a slot, and the ordered slot list (move ←/→, remove; Alt+←/→ and Delete
  work on the selected slot). No drag and drop: Tauri's file-drop handler disables it on Windows.
  Props: config, profile, face (bindable), selectedId (bindable slot id, null = none),
         onchange(profile), oneditgroup(groupId).
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { ChevronLeft, ChevronRight, Pencil, RefreshCw, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { Profile, WheelConfig, WheelFace, WheelStyle } from '../../../lib/types';
  import { IconButton, SegmentedControl } from '../../../lib/ui';
  import { slotIcon } from '../../../lib/wheel/icons';
  import { isGroupSlot, profileLabel, slotLabel } from '../../../lib/wheel/labels';
  import Wheel from '../../../lib/wheel/Wheel.svelte';
  import { FACE_LIMIT, GROUP_PREFIX, moveSlot, removeSlot, slotsFor } from './logic';

  /** Fan ring diameter (2 × 246 + slice stroke) as a share of the 540 px box. */
  const FAN_SPAN = 510 / 540;

  interface Props {
    config: WheelConfig;
    profile: Profile;
    face: WheelFace;
    selectedId: string | null;
    onchange: (p: Profile) => void;
    oneditgroup: (groupId: string) => void;
  }
  let { config, profile, face = $bindable(), selectedId = $bindable(), onchange, oneditgroup }: Props = $props();

  let width = $state(320);
  let list: HTMLElement | undefined = $state();

  const front = $derived(slotsFor(config, profile.front));
  const back = $derived(slotsFor(config, profile.back));
  const slots = $derived(face === 'front' ? front : back);
  const selected = $derived(slots.findIndex((s) => s.id === selectedId));
  /** The box also holds the group fan ring (outer radius 246 of 270): size it so that ring spans
   *  the column, and trim the empty corners above and below. */
  const size = $derived(Math.round(Math.max(240, Math.min(480, (width - 12) / FAN_SPAN))));
  const trim = $derived(Math.round((size * (1 - FAN_SPAN)) / 2));
  /** Diameter of the slot ring (outer radius 168 of 270). */
  const ring = $derived(Math.round((size * 336) / 540));

  const style = $derived.by((): WheelStyle => {
    const s = settings.value;
    return {
      labels: s.wheelLabels,
      opacity: s.wheelOpacity,
      color: s.wheelColor === 'accent' ? 'var(--accent)' : `var(--${s.wheelColor})`,
      scale: 1,
      showDetail: s.wheelShowDetail,
      reduceMotion: s.reduceMotion,
    };
  });
  const faces = $derived([
    { value: 'front' as const, label: `${t('wheel.arrange.front')} ${profile.front.length}` },
    { value: 'back' as const, label: `${t('wheel.arrange.back')} ${profile.back.length}` },
  ]);
  const center = $derived({
    title: profileLabel(profile, t),
    subtitle: face === 'front' ? t('wheel.arrange.front') : t('wheel.arrange.back'),
  });
  /** An open group fans its presets out, as on the real wheel. */
  const fan = $derived(selected >= 0 && slots[selected].children.length > 0 ? selected : -1);

  function select(i: number) {
    selectedId = slots[i]?.id ?? null;
  }

  async function move(from: number, to: number) {
    const id = slots[from]?.id;
    const target = slots[to]?.id;
    if (!id || !target) return;
    onchange(moveSlot(profile, face, profile[face].indexOf(id), profile[face].indexOf(target)));
    selectedId = id;
    // Keyed rows move in the DOM, which drops focus: put it back on the moved row.
    await tick();
    list?.querySelector<HTMLElement>(`[data-slot="${CSS.escape(id)}"] .pick`)?.focus();
  }

  function remove(i: number) {
    const id = slots[i]?.id;
    if (!id) return;
    onchange(removeSlot(profile, face, profile[face].indexOf(id)));
    const rest = slots.filter((s) => s.id !== id);
    selectedId = rest[Math.min(i, rest.length - 1)]?.id ?? null;
  }

  function onkey(e: KeyboardEvent) {
    if (selected < 0) return;
    if (e.altKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      void move(selected, selected + (e.key === 'ArrowLeft' ? -1 : 1));
    } else if (e.key === 'Delete' && !e.altKey && !e.ctrlKey) {
      e.preventDefault();
      remove(selected);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="editor" onkeydown={onkey}>
  <div class="faces">
    <SegmentedControl options={faces} bind:value={face} size="sm" onchange={() => (selectedId = null)} />
    <span class="hint"><RefreshCw size={12} strokeWidth={2} />{t('wheel.arrange.flipHint')}</span>
  </div>

  <div class="stage" bind:clientWidth={width} style:margin-block="-{trim}px">
    {#if slots.length === 0}
      <!-- The Wheel would say "not supported" here; in the editor an empty side is just empty. -->
      <div class="blank" style:height="{size}px">
        <span style:width="{ring}px" style:height="{ring}px">{t('wheel.arrange.emptySide')}</span>
      </div>
    {:else}
      <Wheel {front} {back} {face} hovered={{ slot: selected, child: -1 }} {fan} {style} {center} missingTools={[]} {t} {size}
        {selected} onslotclick={select} />
    {/if}
  </div>

  <div class="slots-head">
    <span>{t('wheel.arrange.slots')}</span>
    <span class="cap">{t('wheel.arrange.capacity', { n: profile[face].length, max: FACE_LIMIT[face] })}</span>
  </div>
  {#if slots.length === 0}
    <p class="empty">{t('wheel.arrange.empty')}</p>
  {:else}
    <ol class="slots" bind:this={list}>
      {#each slots as s, i (s.id)}
        {@const Icon = slotIcon(s.icon)}
        <li class="slot" class:sel={i === selected} data-slot={s.id}>
          <button type="button" class="pick" aria-pressed={i === selected} onclick={() => select(i)}>
            <span class="n">{i + 1}</span>
            <Icon size={14} strokeWidth={1.9} />
            <span class="label">{slotLabel(s, t)}</span>
            {#if s.detail && !slotLabel(s, t).includes(s.detail)}<span class="detail">{s.detail}</span>{/if}
          </button>
          <span class="acts">
            {#if isGroupSlot(s)}
              <IconButton label={t('wheel.arrange.editGroup')} size={24} onclick={() => oneditgroup(s.id.slice(GROUP_PREFIX.length))}>
                <Pencil size={13} />
              </IconButton>
            {/if}
            <IconButton label={t('wheel.arrange.moveEarlier')} size={24} disabled={i === 0} onclick={() => move(i, i - 1)}>
              <ChevronLeft size={14} />
            </IconButton>
            <IconButton label={t('wheel.arrange.moveLater')} size={24} disabled={i === slots.length - 1} onclick={() => move(i, i + 1)}>
              <ChevronRight size={14} />
            </IconButton>
            <IconButton label={t('wheel.arrange.remove')} size={24} onclick={() => remove(i)}>
              <X size={13} />
            </IconButton>
          </span>
        </li>
      {/each}
    </ol>
    <p class="keys">{t('wheel.arrange.keys')}</p>
  {/if}
</div>

<style>
  .editor { display: flex; flex-direction: column; gap: var(--space-2); }
  .faces { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: var(--space-2); }
  .hint { display: inline-flex; align-items: center; gap: 5px; font: var(--text-callout); color: var(--text-tertiary); }
  .stage { display: flex; justify-content: center; overflow: hidden; }
  .blank { display: grid; place-items: center; width: 100%; }
  .blank span {
    display: grid; place-items: center; border: 2px dashed var(--hairline); border-radius: 50%;
    font: var(--text-callout); color: var(--text-tertiary);
  }
  .slots-head { display: flex; justify-content: space-between; padding: 0 var(--space-1); font: var(--text-caption); color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.02em; }
  .cap { font-variant-numeric: tabular-nums; text-transform: none; }
  .empty, .keys { margin: 0; padding: 0 var(--space-1); font: var(--text-callout); color: var(--text-tertiary); }
  .empty { padding: var(--space-3) var(--space-1); }
  .slots {
    display: flex; flex-direction: column; margin: 0; padding: 3px; list-style: none; border-radius: var(--radius-md);
    background: var(--card-bg); box-shadow: var(--shadow-sm), var(--glass-edge);
  }
  .slot { display: flex; align-items: center; gap: 2px; padding-right: 3px; border-radius: var(--radius-sm); }
  .slot:hover { background: var(--control-bg); }
  .slot.sel { background: var(--accent-soft); }
  .pick {
    display: flex; align-items: center; gap: var(--space-2); flex: 1; min-width: 0; height: 30px; padding: 0 8px; border: 0;
    border-radius: var(--radius-sm); background: transparent; color: var(--text); font: var(--text-body); text-align: left; cursor: default;
  }
  .pick :global(svg) { flex: none; color: var(--text-secondary); }
  .sel .pick :global(svg) { color: var(--accent); }
  .n { flex: none; width: 16px; font: var(--text-caption); color: var(--text-tertiary); font-variant-numeric: tabular-nums; text-align: right; }
  .label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .detail { flex: none; font: var(--text-caption); color: var(--text-secondary); }
  .acts { display: inline-flex; flex: none; opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); }
  .slot:hover .acts, .slot.sel .acts, .slot:focus-within .acts { opacity: 1; }
</style>
