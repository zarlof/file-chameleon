<!--
  ItemList — the library's list view, windowed: only the rows near the viewport are mounted.
  Props: items (display order), selected (ids), focus (id or null), now (ms, for relative times), trash,
         folders (id -> name), onpick(id, e), onopen(id), onquick(id, action).
  Exports: reveal(index) scrolls the row into view.
-->
<script lang="ts">
  import { currentLang, t } from '../../../lib/i18n';
  import type { ClipItem } from '../../../lib/types';
  import ClipRow from './ClipRow.svelte';
  import { relativeTime, ROW_H, rowWindow, scrollToReveal, type QuickAction } from './clips';

  interface Props {
    items: ClipItem[];
    selected: Set<number>;
    focus: number | null;
    now: number;
    trash: boolean;
    folders: Map<number, string>;
    onpick: (id: number, e: MouseEvent) => void;
    onopen: (id: number) => void;
    onquick: (id: number, action: QuickAction) => void;
  }
  let { items, selected, focus, now, trash, folders, onpick, onopen, onquick }: Props = $props();

  let box: HTMLElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(600);
  const win = $derived(rowWindow(scrollTop, viewport, items.length));
  const visible = $derived(items.slice(win.start, win.end));

  export function reveal(index: number) {
    if (!box || index < 0) return;
    box.scrollTop = scrollTop = scrollToReveal(scrollTop, viewport, index);
  }
</script>

<div class="list" bind:this={box} bind:clientHeight={viewport} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  role="listbox" tabindex="0" aria-multiselectable="true" aria-label={t('clipboard.title')}
  aria-activedescendant={focus !== null ? `clip-${focus}` : undefined}>
  <div style:height="{win.top}px" aria-hidden="true"></div>
  {#each visible as item (item.id)}
    <div class="slot" style:height="{ROW_H}px">
      <ClipRow {item} {trash} selected={selected.has(item.id)} focused={focus === item.id}
        when={relativeTime(trash ? (item.trashedAt ?? item.createdAt) : item.createdAt, now, currentLang(), t('clipboard.ago.now'))}
        folder={item.folder !== null ? (folders.get(item.folder) ?? null) : null}
        onpick={(e) => onpick(item.id, e)} onopen={() => onopen(item.id)} onquick={(a) => onquick(item.id, a)} />
    </div>
  {/each}
  <div style:height="{win.bottom}px" aria-hidden="true"></div>
</div>

<style>
  /* Bottom room for the floating batch bar. */
  .list { height: 100%; overflow-y: auto; padding: 0 var(--space-1) 72px 0; outline: 0; }
  .slot { box-sizing: border-box; padding-bottom: 4px; }
</style>
