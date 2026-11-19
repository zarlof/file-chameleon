<!--
  SegmentedControl — macOS segmented picker with a sliding thumb.
  Props: options: { value: string; label: string; icon?: Snippet }[], value (bindable), size = 'md' | 'sm',
         disabled, onchange?(value).
-->
<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';

  interface Option { value: T; label: string; icon?: Snippet }
  interface Props {
    options: Option[];
    value: T;
    size?: 'sm' | 'md';
    disabled?: boolean;
    onchange?: (value: T) => void;
  }
  let { options, value = $bindable(), size = 'md', disabled = false, onchange }: Props = $props();
  const index = $derived(Math.max(0, options.findIndex((o) => o.value === value)));

  function pick(v: T) {
    if (disabled || v === value) return;
    value = v;
    onchange?.(v);
  }
  function key(e: KeyboardEvent) {
    const d = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    pick(options[(index + d + options.length) % options.length].value);
  }
</script>

<div class="seg {size}" role="radiogroup" tabindex="-1" onkeydown={key} class:disabled
  style:--n={options.length} style:--i={index}>
  <span class="thumb" aria-hidden="true"></span>
  {#each options as o (o.value)}
    <button type="button" role="radio" aria-checked={o.value === value} tabindex={o.value === value ? 0 : -1}
      class:on={o.value === value} {disabled} onclick={() => pick(o.value)}>
      {#if o.icon}<span class="ico">{@render o.icon()}</span>{/if}
      <span>{o.label}</span>
    </button>
  {/each}
</div>

<style>
  .seg {
    position: relative; display: inline-grid; grid-template-columns: repeat(var(--n), 1fr); padding: 2px;
    background: var(--control-bg); border-radius: 8px; height: var(--control-h);
  }
  .seg.sm { height: 24px; }
  .seg.disabled { opacity: 0.5; }
  .thumb {
    position: absolute; top: 2px; bottom: 2px; left: 2px; border-radius: 6px;
    width: calc((100% - 4px) / var(--n)); transform: translateX(calc(var(--i) * 100%));
    background: var(--elevated-bg); box-shadow: var(--shadow-sm);
    transition: transform var(--dur) var(--ease-spring);
  }
  button {
    position: relative; z-index: 1; display: inline-flex; align-items: center; justify-content: center; gap: 5px;
    border: 0; background: transparent; padding: 0 12px; font: var(--text-callout); font-weight: 500;
    color: var(--text-secondary); border-radius: 6px; cursor: default; white-space: nowrap;
    transition: color var(--dur-fast);
  }
  button.on { color: var(--text); }
  button:hover:not(.on):not(:disabled) { color: var(--text); }
  .ico { display: inline-flex; }
  .ico :global(svg) { width: 14px; height: 14px; }
</style>
