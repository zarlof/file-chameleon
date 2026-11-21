<!--
  Popover — a small button that opens a floating panel below (or above) it: the batch "move" and "tag" menus.
  Closes on Esc, on a click outside, or when the content calls the `close` it is given.
  Props: label, disabled, up (open above the button), icon?: Snippet, children: Snippet<[close: () => void]>.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Button } from '../../../lib/ui';

  interface Props { label: string; disabled?: boolean; up?: boolean; icon?: Snippet; children: Snippet<[() => void]> }
  let { label, disabled = false, up = false, icon, children }: Props = $props();

  let open = $state(false);
  let root: HTMLElement | undefined = $state();
  const close = () => (open = false);

  function outside(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  function key(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      e.stopPropagation();
      open = false;
    }
  }
</script>

<svelte:window onpointerdown={outside} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pop" bind:this={root} onkeydown={key}>
  <Button size="sm" variant="plain" {disabled} {icon} onclick={() => (open = !open)}>{label}</Button>
  {#if open}
    <div class="panel" class:up role="dialog" aria-label={label}>{@render children(close)}</div>
  {/if}
</div>

<style>
  .pop { position: relative; }
  .panel {
    position: absolute; top: calc(100% + 6px); left: 0; z-index: 30; display: flex; flex-direction: column; gap: 2px;
    min-width: 220px; max-height: 320px; overflow-y: auto; padding: 6px; border-radius: var(--radius-md);
    background: var(--elevated-bg); box-shadow: var(--shadow-lg), var(--glass-edge); backdrop-filter: blur(24px) saturate(1.6);
    animation: drop var(--dur) var(--ease-out);
  }
  .panel.up { top: auto; bottom: calc(100% + 8px); }
  @keyframes drop { from { opacity: 0; transform: translateY(-4px); } }
</style>
