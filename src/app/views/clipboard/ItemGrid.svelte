<!--
  ItemGrid — the library's grid of cards, windowed by card rows (fixed row height GRID_ROW_H).
  Props: items (display order), selected (ids), focus, now, trash, columns (bindable out: cards per row, for the
         arrow keys), onpick(id, e), onopen(id), onquick(id, action).
  Exports: reveal(index) scrolls the card's row into view.
-->
<script lang="ts">
  import { currentLang, t } from '../../../lib/i18n';
  import type { ClipItem } from '../../../lib/types';
  import ClipCard from './ClipCard.svelte';
  import { GRID_GAP, GRID_ROW_H, gridColumns, relativeTime, rowWindow, scrollToReveal, type QuickAction } from './clips';

  interface Props {
    items: ClipItem[];
    selected: Set<number>;
    focus: number | null;
    now: number;
    trash: boolean;
    columns?: number;
    onpick: (id: number, e: MouseEvent) => void;
    onopen: (id: number) => void;
    onquick: (id: number, action: QuickAction) => void;
  }
  let { items, selected, focus, now, trash, columns = $bindable(1), onpick, onopen, onquick }: Props = $props();

  let box: HTMLElement | undefined = $state();
  let width = $state(600);
  let scrollTop = $state(0);
  let viewport = $state(600);

  $effect(() => {
    columns = gridColumns(width);
  });
  const rows = $derived(Math.ceil(items.length / columns));
  const win = $derived(rowWindow(scrollTop, viewport, rows, GRID_ROW_H, 3));
  const visible = $derived(items.slice(win.start * columns, win.end * columns));

  export function reveal(index: number) {
    if (!box || index < 0) return;
    box.scrollTop = scrollTop = scrollToReveal(scrollTop, viewport, Math.floor(index / columns), GRID_ROW_H);
  }
</script>

<div class="scroll" bind:this={box} bind:clientHeight={viewport} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  role="listbox" tabindex="0" aria-multiselectable="true" aria-label={t('clipboard.title')}
  aria-activedescendant={focus !== null ? `clip-${focus}` : undefined}>
  <div class="inner" bind:clientWidth={width}>
    <div style:height="{win.top}px" aria-hidden="true"></div>
    <div class="grid" style:grid-template-columns="repeat({columns}, minmax(0, 1fr))" style:gap="{GRID_GAP}px">
      {#each visible as item (item.id)}
        <ClipCard {item} {trash} selected={selected.has(item.id)} focused={focus === item.id}
          when={relativeTime(trash ? (item.trashedAt ?? item.createdAt) : item.createdAt, now, currentLang(), t('clipboard.ago.now'))}
          onpick={(e) => onpick(item.id, e)} onopen={() => onopen(item.id)} onquick={(a) => onquick(item.id, a)} />
      {/each}
    </div>
    <div style:height="{win.bottom}px" aria-hidden="true"></div>
  </div>
</div>

<style>
  .scroll { height: 100%; overflow-y: auto; outline: 0; }
  /* Bottom room for the floating batch bar. */
  .inner { padding: 2px var(--space-1) 72px 2px; }
  /* Row pitch = card (172) + gap (12) = GRID_ROW_H. */
  .grid { display: grid; grid-auto-rows: 172px; }
</style>
