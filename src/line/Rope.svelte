<!--
  Rope — one softly sagging cord across the strip with a knot at each end, a tiny label and the pegs
  hanging from it. Positions are derived from the rope maths in physics.ts; the swing angles and
  falling progress are owned by Line.svelte's shared animation loop.
-->
<script lang="ts">
  import type { ClipItem } from '../lib/types';
  import { fall, ropePath, ropeY } from './physics';
  import Peg from './Peg.svelte';
  import { t } from './i18n';

  /** Band height in px. Keep in step with ROPE_HEIGHT (this + 4 px margin) in src-tauri/src/line.rs. */
  const BAND = 178;
  const PEG_W = 112;
  const GAP = 16;
  const MARGIN = 24;
  const TOP = 16;

  let {
    label,
    items,
    width,
    more,
    angles,
    falls,
    selected,
    copiedId,
    onclick,
    ondblclick,
    ondragout,
    onremove,
    oncontext,
    onhover,
    onmore,
  }: {
    label: string;
    items: ClipItem[];
    width: number;
    /** Items that did not fit (shown as "+N"). */
    more: number;
    angles: Record<number, number>;
    falls: Record<number, number>;
    selected: Set<number>;
    copiedId: number | null;
    onclick: (item: ClipItem, e: MouseEvent) => void;
    ondblclick: (item: ClipItem) => void;
    ondragout: (item: ClipItem) => void;
    onremove: (item: ClipItem) => void;
    oncontext: (item: ClipItem, e: MouseEvent) => void;
    onhover: (item: ClipItem, dir: number) => void;
    onmore: () => void;
  } = $props();

  const centers = $derived(items.map((_, i) => MARGIN + i * (PEG_W + GAP) + PEG_W / 2));
  const d = $derived(ropePath(width, centers));
</script>

<div class="band" style:height="{BAND}px">
  <svg class="cord" {width} height={TOP + 40} viewBox="0 0 {width} {TOP + 40}" aria-hidden="true">
    <g transform="translate(0 {TOP})">
      <path {d} class="shade" transform="translate(0 2)" />
      <path {d} class="strand" />
      <circle cx="3" cy="0" r="4.5" class="knot" />
      <circle cx={width - 3} cy="0" r="4.5" class="knot" />
    </g>
  </svg>
  <span class="label">{label}</span>
  {#each items as item, i (item.id)}
    <div class="slot" style:left="{centers[i] - PEG_W / 2}px" style:top="{TOP + ropeY(width, centers[i], centers) + 8}px">
      <Peg
        {item}
        theta={angles[item.id] ?? 0}
        drop={fall(falls[item.id] ?? 1)}
        selected={selected.has(item.id)}
        copied={copiedId === item.id}
        onclick={(e) => onclick(item, e)}
        ondblclick={() => ondblclick(item)}
        ondragout={() => ondragout(item)}
        onremove={() => onremove(item)}
        oncontext={(e) => oncontext(item, e)}
        onhover={(dir) => onhover(item, dir)}
      />
    </div>
  {/each}
  {#if more > 0}
    <button type="button" class="more" tabindex="-1" title={t('more', { count: more })} aria-label={t('more', { count: more })} onclick={onmore}>+{more}</button>
  {/if}
</div>

<style>
  .band {
    position: relative;
    margin-bottom: 4px;
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--window-bg) 38%, transparent);
  }
  .cord { position: absolute; inset: 0 auto auto 0; overflow: visible; pointer-events: none; }
  .strand { fill: none; stroke: var(--text-secondary); stroke-width: 2.4; stroke-linecap: round; stroke-linejoin: round; opacity: 0.85; }
  .shade { fill: none; stroke: var(--text); stroke-width: 2.4; stroke-linecap: round; opacity: 0.12; }
  .knot { fill: var(--text-secondary); opacity: 0.9; }
  .label {
    position: absolute;
    left: 16px;
    top: 6px;
    padding: 1px 9px;
    border-radius: 999px;
    background: var(--elevated-bg);
    box-shadow: inset 0 0 0 1px var(--glass-edge);
    font: var(--text-caption);
    color: var(--text-secondary);
    pointer-events: none;
    z-index: 1;
  }
  .slot { position: absolute; width: 112px; }
  .more {
    position: absolute;
    right: 14px;
    top: 34px;
    min-width: 40px;
    height: 28px;
    padding: 0 var(--space-2);
    border: 0;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
    font: var(--text-callout);
    font-weight: 600;
    cursor: default;
  }
  .more:hover { background: var(--accent); color: var(--on-accent); }
</style>
