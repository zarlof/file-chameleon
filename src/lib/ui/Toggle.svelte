<!--
  Toggle — macOS switch.
  Props: checked (bindable), disabled, label (aria-label), onchange?(checked).
-->
<script lang="ts">
  interface Props { checked: boolean; disabled?: boolean; label?: string; onchange?: (checked: boolean) => void }
  let { checked = $bindable(), disabled = false, label, onchange }: Props = $props();

  function flip() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button type="button" class="tg" class:on={checked} role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={flip}>
  <span class="knob"></span>
</button>

<style>
  .tg {
    position: relative; flex: none; width: 38px; height: 22px; padding: 0; border: 0; border-radius: 11px;
    background: var(--control-bg-active); cursor: default;
    transition: background var(--dur) var(--ease-out);
  }
  .tg.on { background: var(--green); }
  .tg:disabled { opacity: 0.45; }
  .knob {
    position: absolute; top: 2px; left: 2px; width: 18px; height: 18px; border-radius: 50%; background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3), 0 0 0 0.5px rgba(0, 0, 0, 0.06);
    transition: transform var(--dur) var(--ease-spring), width var(--dur-fast);
  }
  .tg.on .knob { transform: translateX(16px); }
  .tg:active:not(:disabled) .knob { width: 22px; }
  .tg.on:active:not(:disabled) .knob { transform: translateX(12px); }
</style>
