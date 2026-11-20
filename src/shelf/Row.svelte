<script lang="ts">
  import type { ClipItem } from '../lib/types';
  import Card from './Card.svelte';
  import { t } from './i18n';

  let {
    title,
    items,
    active,
    cursorId,
    selected,
    emptyText,
    onpick,
  }: {
    title: string;
    items: ClipItem[];
    active: boolean;
    cursorId: number | null;
    selected: number[];
    emptyText: string;
    onpick: (item: ClipItem, e: MouseEvent) => void;
  } = $props();
</script>

<section class="row" class:active aria-label={title}>
  <h2>{title}{#if selected.length > 1 && active}<span class="count">{t('selected', { count: selected.length })}</span>{/if}</h2>
  {#if items.length === 0}
    <div class="empty">{emptyText}</div>
  {:else}
    <div class="strip">
      {#each items as item, i (item.id)}
        <Card
          {item}
          cursor={item.id === cursorId}
          selected={selected.includes(item.id)}
          number={active && i < 9 ? i + 1 : null}
          onclick={(e) => onpick(item, e)}
        />
      {/each}
    </div>
  {/if}
</section>

<style>
  .row { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
  h2 {
    margin: 0;
    padding: 0 var(--space-1);
    font: var(--text-caption);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-tertiary);
    display: flex;
    gap: var(--space-2);
  }
  .row.active h2 { color: var(--text-secondary); }
  .count { color: var(--accent); text-transform: none; letter-spacing: 0; }
  .strip {
    display: flex;
    gap: var(--space-2);
    overflow-x: auto;
    overflow-y: hidden;
    padding: 3px 3px var(--space-2);
    scrollbar-width: thin;
  }
  .empty {
    height: var(--card-h);
    display: grid;
    place-items: center;
    border-radius: var(--radius-md);
    border: 1px dashed var(--separator);
    color: var(--text-tertiary);
    font: var(--text-callout);
  }
</style>
