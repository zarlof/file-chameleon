<!--
  ProgressBar — thin determinate/indeterminate bar.
  Props: value: number | null (0..1; null = indeterminate), height = 4 (px), tone = 'accent' | 'green' | 'red'.
-->
<script lang="ts">
  interface Props { value?: number | null; height?: number; tone?: 'accent' | 'green' | 'red' }
  let { value = null, height = 4, tone = 'accent' }: Props = $props();
  const pct = $derived(value === null ? 0 : Math.round(Math.min(1, Math.max(0, value)) * 1000) / 10);
</script>

<div class="bar {tone}" class:indet={value === null} style:height="{height}px" role="progressbar"
  aria-valuemin="0" aria-valuemax="100" aria-valuenow={value === null ? undefined : pct}>
  <div class="fill" style:width={value === null ? undefined : `${pct}%`}></div>
</div>

<style>
  .bar { position: relative; width: 100%; overflow: hidden; border-radius: 99px; background: var(--control-bg-hover); }
  .fill { height: 100%; border-radius: inherit; background: var(--accent); transition: width var(--dur) var(--ease-out); }
  .green .fill { background: var(--green); }
  .red .fill { background: var(--red); }
  .indet .fill { position: absolute; width: 38%; animation: slide 1.3s var(--ease-out) infinite; }
  @keyframes slide { 0% { left: -40%; } 100% { left: 102%; } }
</style>
