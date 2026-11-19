<!--
  IconButton — square, borderless icon-only button.
  Props: label (required, aria-label + tooltip), size = 28 (px), active = false, disabled, onclick,
         children: Snippet (the icon, e.g. <X size={14} />).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    size?: number;
    active?: boolean;
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  }
  let { label, size = 28, active = false, disabled = false, onclick, children }: Props = $props();
</script>

<button class="ib" class:active type="button" aria-label={label} title={label} {disabled} {onclick}
  style:width="{size}px" style:height="{size}px">
  {@render children()}
</button>

<style>
  .ib {
    display: inline-grid; place-items: center; border: 0; padding: 0; flex: none;
    border-radius: var(--radius-sm); background: transparent; color: var(--text-secondary); cursor: default;
    transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast), transform var(--dur-fast) var(--ease-out);
  }
  .ib:hover:not(:disabled) { background: var(--control-bg); color: var(--text); }
  .ib:active:not(:disabled) { background: var(--control-bg-hover); transform: scale(0.94); }
  .ib.active { background: var(--accent-soft); color: var(--accent); }
  .ib:disabled { opacity: 0.4; }
</style>
