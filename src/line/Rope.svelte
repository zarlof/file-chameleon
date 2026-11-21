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

  /** Space between two cards, and the rope's inset at both ends (not scaled). */
  const GAP = 16;
  const MARGIN = 24;

  let {
    label,
    items,
    width,
    more,
    angles,
    falls,
    selected,
    copiedId,
    focusedId = null,
    top,
    band,
    card,
    scale,
    color,
    thickness,
    opacity,
    cardOp,
    zoomOn = false,
    onzoom,
    emptyText = '',
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
    focusedId?: number | null;
    /** Where the band starts in the window and how tall it is (bandTops / SIZES in layout.ts). */
    top: number;
    band: number;
    /** Card width and size factor. */
    card: number;
    scale: number;
    /** Rope colour (any CSS colour), stroke width, backdrop opacity 0..1 and card opacity 0.9..1. */
    color: string;
    thickness: number;
    opacity: number;
    cardOp: number;
    zoomOn?: boolean;
    onzoom?: (item: ClipItem | null, rect: DOMRect | null) => void;
    /** Shown in the band when a search leaves this rope empty. */
    emptyText?: string;
    onclick: (item: ClipItem, e: MouseEvent) => void;
    ondblclick: (item: ClipItem) => void;
    ondragout: (item: ClipItem) => void;
    onremove: (item: ClipItem) => void;
    oncontext: (item: ClipItem, e: MouseEvent) => void;
    onhover: (item: ClipItem, dir: number) => void;
    onmore: () => void;
  } = $props();

  const TOP = $derived(16 * scale);
  const sag = $derived(18 * scale);
  const centers = $derived(items.map((_, i) => MARGIN + i * (card + GAP) + card / 2));
  const d = $derived(ropePath(width, centers, sag));
</script>

<div class="band" style:top="{top}px" style:height="{band}px" style:--op={opacity} style:--rope={color} style:--w="{thickness}px">
  <svg class="cord" {width} height={TOP + sag + 22} viewBox="0 0 {width} {TOP + sag + 22}" aria-hidden="true">
    <g transform="translate(0 {TOP})">
      <path {d} class="shade" transform="translate(0 {1 + thickness / 2})" />
      <path {d} class="strand" />
      <circle cx="3" cy="0" r={3 + thickness * 0.9} class="knot" />
      <circle cx={width - 3} cy="0" r={3 + thickness * 0.9} class="knot" />
    </g>
  </svg>
  <span class="label">{label}</span>
  {#if items.length === 0 && emptyText}<span class="none">{emptyText}</span>{/if}
  {#each items as item, i (item.id)}
    <div class="slot" style:left="{centers[i] - card / 2}px" style:top="{TOP + ropeY(width, centers[i], centers, sag) + 8 * scale}px" style:width="{card}px">
      <Peg
        {item}
        theta={angles[item.id] ?? 0}
        drop={fall(falls[item.id] ?? 1)}
        selected={selected.has(item.id)}
        copied={copiedId === item.id}
        focused={focusedId === item.id}
        {scale}
        width={card}
        {cardOp}
        {zoomOn}
        {onzoom}
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
    position: absolute;
    left: 0;
    right: 0;
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--window-bg) calc(var(--op) * 38%), transparent);
  }
  .cord { position: absolute; inset: 0 auto auto 0; overflow: visible; pointer-events: none; }
  .strand { fill: none; stroke: var(--rope); stroke-width: var(--w); stroke-linecap: round; stroke-linejoin: round; opacity: calc(0.35 + 0.65 * var(--op)); }
  .shade { fill: none; stroke: var(--text); stroke-width: var(--w); stroke-linecap: round; opacity: calc(0.12 * var(--op)); }
  .knot { fill: var(--rope); opacity: calc(0.35 + 0.65 * var(--op)); }
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
  .slot { position: absolute; transition: left var(--dur) var(--ease-out), top var(--dur) var(--ease-out); }
  :global([data-reduce-motion]) .slot { transition: none; }
  .none { position: absolute; inset: 0; display: grid; place-items: center; color: var(--text-secondary); font: var(--text-callout); pointer-events: none; }
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
