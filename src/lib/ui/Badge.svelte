<!--
  Badge — small pill label.
  Props: tone = 'neutral' | 'accent' | 'green' | 'red' | 'orange', children: Snippet.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props { tone?: 'neutral' | 'accent' | 'green' | 'red' | 'orange'; children: Snippet }
  let { tone = 'neutral', children }: Props = $props();
</script>

<span class="badge {tone}">{@render children()}</span>

<style>
  .badge { display: inline-flex; align-items: center; height: 18px; padding: 0 7px; border-radius: 99px; font: var(--text-caption); white-space: nowrap; }
  .neutral { background: var(--control-bg-hover); color: var(--text-secondary); }
  .accent { background: var(--accent-soft); color: var(--accent); }
  .green { background: color-mix(in srgb, var(--green) 18%, transparent); color: var(--green); }
  .red { background: color-mix(in srgb, var(--red) 16%, transparent); color: var(--red); }
  .orange { background: color-mix(in srgb, var(--orange) 18%, transparent); color: var(--orange); }
</style>
