<!--
  Select — styled native <select> (keeps native popup, keyboard and accessibility).
  Props: value (bindable string), options: { value: string; label: string }[], disabled, label (aria-label),
         onchange?(value).
-->
<script lang="ts">
  import { ChevronDown } from '@lucide/svelte';

  interface Props {
    value: string;
    options: { value: string; label: string }[];
    disabled?: boolean;
    label?: string;
    onchange?: (value: string) => void;
  }
  let { value = $bindable(), options, disabled = false, label, onchange }: Props = $props();
</script>

<span class="sel" class:disabled>
  <select bind:value {disabled} aria-label={label} onchange={() => onchange?.(value)}>
    {#each options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
  </select>
  <ChevronDown size={13} strokeWidth={2.25} />
</span>

<style>
  .sel { position: relative; display: inline-flex; align-items: center; min-width: 120px; }
  .sel.disabled { opacity: 0.45; }
  select {
    -webkit-appearance: none; appearance: none; width: 100%; height: var(--control-h); padding: 0 28px 0 10px;
    border: 0; border-radius: var(--radius-sm); font: var(--text-body); color: var(--text);
    background: var(--card-bg-hover); box-shadow: var(--shadow-sm); cursor: default;
    transition: background var(--dur-fast);
  }
  select:hover:not(:disabled) { background: var(--elevated-bg); }
  select option { background: var(--elevated-bg); color: var(--text); }
  .sel :global(svg) { position: absolute; right: 9px; pointer-events: none; color: var(--text-secondary); }
</style>
