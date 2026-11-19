<!--
  SectionHeader — small caption above a Card group, with an optional trailing action.
  Props: title, subtitle?, action?: Snippet.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props { title: string; subtitle?: string; action?: Snippet }
  let { title, subtitle, action }: Props = $props();
</script>

<div class="sh">
  <div class="t">
    <h3>{title}</h3>
    {#if subtitle}<p>{subtitle}</p>{/if}
  </div>
  {#if action}<div class="a">{@render action()}</div>{/if}
</div>

<style>
  .sh { display: flex; align-items: flex-end; justify-content: space-between; gap: var(--space-3); padding: 0 var(--space-1) 6px; }
  h3 { margin: 0; font: var(--text-caption); font-size: 11.5px; letter-spacing: 0.02em; text-transform: uppercase; color: var(--text-secondary); }
  p { margin: 2px 0 0; font: var(--text-callout); color: var(--text-tertiary); }
</style>
