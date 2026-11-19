<!--
  ListRow — a row inside a Card; hairline separators between consecutive rows.
  Props: title, subtitle?, leading?: Snippet, trailing?: Snippet, onclick? (makes the row a hover/press target),
         selected = false, children?: Snippet (extra content under the text, e.g. a ProgressBar).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props {
    title: string;
    subtitle?: string;
    leading?: Snippet;
    trailing?: Snippet;
    children?: Snippet;
    selected?: boolean;
    onclick?: (e: MouseEvent) => void;
  }
  let { title, subtitle, leading, trailing, children, selected = false, onclick }: Props = $props();
</script>

{#snippet body()}
  {#if leading}<span class="lead">{@render leading()}</span>{/if}
  <span class="text">
    <span class="title">{title}</span>
    {#if subtitle}<span class="sub">{subtitle}</span>{/if}
    {#if children}<span class="extra">{@render children()}</span>{/if}
  </span>
  {#if trailing}<span class="trail">{@render trailing()}</span>{/if}
{/snippet}

{#if onclick}
  <button type="button" class="row clickable" class:selected {onclick}>{@render body()}</button>
{:else}
  <div class="row" class:selected>{@render body()}</div>
{/if}

<style>
  .row {
    position: relative; display: flex; align-items: center; gap: var(--space-3); width: 100%; min-height: 44px;
    padding: 8px var(--space-4); border: 0; background: transparent; color: inherit; font: inherit; text-align: left;
  }
  .row:not(:first-child)::before {
    content: ''; position: absolute; top: 0; left: var(--space-4); right: 0; height: 0.5px; background: var(--separator);
  }
  .clickable { cursor: default; transition: background var(--dur-fast); }
  .clickable:hover { background: var(--control-bg); }
  .clickable:active { background: var(--control-bg-hover); }
  .selected { background: var(--accent-soft); }
  .lead { display: inline-flex; flex: none; color: var(--text-secondary); }
  .text { display: flex; flex-direction: column; flex: 1; min-width: 0; gap: 2px; }
  .title { font: var(--text-body); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { font: var(--text-callout); color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .extra { display: block; margin-top: 4px; }
  .trail { display: inline-flex; align-items: center; gap: var(--space-2); flex: none; color: var(--text-secondary); }
</style>
