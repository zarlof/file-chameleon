<!--
  Card — rounded translucent grouped surface (System Settings style "inset grouped" block).
  Props: padded = false (adds 16px padding; leave off when holding ListRows), interactive = false (hover lift),
         onclick?, children: Snippet.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props { padded?: boolean; interactive?: boolean; onclick?: (e: MouseEvent) => void; children?: Snippet }
  let { padded = false, interactive = false, onclick, children }: Props = $props();
</script>

{#if interactive}
  <button type="button" class="card interactive" class:padded {onclick}>{@render children?.()}</button>
{:else}
  <div class="card" class:padded>{@render children?.()}</div>
{/if}

<style>
  .card {
    display: block; width: 100%; text-align: left; border: 0; font: inherit; color: inherit;
    background: var(--card-bg); border-radius: var(--radius-md); box-shadow: var(--shadow-sm), var(--glass-edge);
    overflow: hidden;
  }
  .padded { padding: var(--space-4); }
  .interactive { cursor: default; transition: background var(--dur-fast), transform var(--dur) var(--ease-out), box-shadow var(--dur); }
  .interactive:hover { background: var(--card-bg-hover); box-shadow: var(--shadow-md), var(--glass-edge); }
  .interactive:active { transform: scale(0.992); }
</style>
