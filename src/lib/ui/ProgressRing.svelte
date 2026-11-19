<!--
  ProgressRing — circular progress.
  Props: value: number | null (0..1; null = indeterminate spin), size = 20 (px), stroke = 2.5.
-->
<script lang="ts">
  interface Props { value?: number | null; size?: number; stroke?: number }
  let { value = null, size = 20, stroke = 2.5 }: Props = $props();
  const r = $derived((size - stroke) / 2);
  const c = $derived(2 * Math.PI * r);
  const v = $derived(value === null ? 0.25 : Math.min(1, Math.max(0, value)));
</script>

<svg class="ring" class:spin={value === null} width={size} height={size} viewBox="0 0 {size} {size}" role="progressbar"
  aria-valuemin="0" aria-valuemax="100" aria-valuenow={value === null ? undefined : Math.round(v * 100)}>
  <circle cx={size / 2} cy={size / 2} {r} fill="none" stroke="var(--control-bg-active)" stroke-width={stroke} />
  <circle class="arc" cx={size / 2} cy={size / 2} {r} fill="none" stroke="var(--accent)" stroke-width={stroke}
    stroke-linecap="round" stroke-dasharray={c} stroke-dashoffset={c * (1 - v)} transform="rotate(-90 {size / 2} {size / 2})" />
</svg>

<style>
  .ring { flex: none; }
  .arc { transition: stroke-dashoffset var(--dur) var(--ease-out); }
  .spin { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
