<!--
  LineMenu — the right-click menu of a card: a close button at the top right and the entries from
  menu.ts. Choosing an entry closes the menu first, then runs it; an entry with a sub-list shows
  that list in place, under a "back" row.
-->
<script lang="ts">
  import type { MenuEntry } from './menu';
  import { t } from './i18n';

  let {
    x,
    y,
    width,
    entries,
    el = $bindable(null),
    onclose,
    onerror,
  }: {
    x: number;
    y: number;
    width: number;
    entries: MenuEntry[];
    el?: HTMLElement | null;
    onclose: () => void;
    onerror: (e: unknown) => void;
  } = $props();

  /** The sub-list shown instead of `entries` (and its title), if one was opened. */
  let sub = $state<{ label: string; entries: MenuEntry[] } | null>(null);
  const shown = $derived(sub?.entries ?? entries);

  async function choose(run: () => unknown) {
    onclose();
    try {
      await run();
    } catch (e) {
      onerror(e);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="menu" role="menu" tabindex="-1" bind:this={el} style:left="{x}px" style:top="{y}px" style:width="{width}px">
  <button type="button" class="close" tabindex="-1" aria-label={t('close')} title={t('close')} onclick={onclose}>
    <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true"><path d="M2 2l6 6M8 2L2 8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
  </button>
  {#if sub}
    <button type="button" role="menuitem" class="back" onclick={() => (sub = null)}>‹ {sub.label}</button>
  {/if}
  {#each shown as entry, i (i)}
    {#if entry === 'sep'}
      <hr />
    {:else if 'sub' in entry}
      <button type="button" role="menuitem" class="more" onclick={() => (sub = { label: entry.label, entries: entry.sub })}>{entry.label}<span aria-hidden="true">›</span></button>
    {:else}
      <button type="button" role="menuitem" class:danger={entry.danger} onclick={() => choose(entry.run)}>{entry.label}</button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 10;
    display: flex;
    flex-direction: column;
    padding: 26px var(--space-1) var(--space-1);
    border-radius: var(--radius-md);
    background: var(--elevated-bg);
    box-shadow: var(--shadow-lg), inset 0 0 0 1px var(--glass-edge);
    font: var(--text-callout);
  }
  button {
    padding: 6px var(--space-3);
    border: 0;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: default;
  }
  button:hover { background: var(--accent); color: var(--on-accent); }
  .more { display: flex; justify-content: space-between; gap: var(--space-2); }
  .back { color: var(--text-secondary); }
  .danger { color: var(--red); }
  .danger:hover { background: var(--red); color: var(--on-accent); }
  .close {
    position: absolute;
    top: 4px;
    right: 4px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border-radius: 50%;
    color: var(--text-secondary);
  }
  .close:hover { background: var(--red); color: #fff; }
  hr { width: 100%; height: 1px; margin: var(--space-1) 0; border: 0; background: var(--separator); }
</style>
