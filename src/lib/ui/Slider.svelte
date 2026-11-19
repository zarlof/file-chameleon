<!--
  Slider — macOS-style range input with accent fill.
  Props: value (bindable number), min = 0, max = 100, step = 1, disabled, label (aria-label), oninput?(value).
-->
<script lang="ts">
  interface Props { value: number; min?: number; max?: number; step?: number; disabled?: boolean; label?: string; oninput?: (v: number) => void }
  let { value = $bindable(), min = 0, max = 100, step = 1, disabled = false, label, oninput }: Props = $props();
  const pct = $derived(max > min ? ((value - min) / (max - min)) * 100 : 0);
</script>

<input class="slider" type="range" {min} {max} {step} {disabled} aria-label={label} bind:value style:--p="{pct}%"
  oninput={() => oninput?.(value)} />

<style>
  .slider { -webkit-appearance: none; appearance: none; width: 100%; height: 20px; background: transparent; margin: 0; }
  .slider:disabled { opacity: 0.45; }
  .slider:focus-visible { outline: none; }
  .slider::-webkit-slider-runnable-track {
    height: 4px; border-radius: 2px;
    background: linear-gradient(to right, var(--accent) var(--p), var(--control-bg-active) var(--p));
  }
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none; appearance: none; width: 18px; height: 18px; margin-top: -7px; border-radius: 50%;
    background: #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.32), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
    transition: transform var(--dur-fast) var(--ease-out);
  }
  .slider:active::-webkit-slider-thumb { transform: scale(1.12); }
  .slider:focus-visible::-webkit-slider-thumb { box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 35%, transparent), 0 1px 4px rgba(0, 0, 0, 0.32); }
</style>
