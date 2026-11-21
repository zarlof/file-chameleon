<!--
  Rope — one natural cord across the whole screen width (no band, no backdrop), a knot near each end,
  a small label pill at the left end and the cards hanging from it by clothespins. The strokes come from
  ropeStyle.ts (twisted / cord / thin); the swing angles and falling progress are owned by Line.svelte's
  shared animation loop. The settings page draws its live preview with this same component.
-->
<script lang="ts">
  import { fall, ropePath, ropeY } from './physics';
  import { ropeStrokes, type RopeStyle } from './ropeStyle';
  import type { LineCard } from './logic';
  import Peg from './Peg.svelte';
  import { t } from './i18n';

  /** Space between two cards, and the rope's inset at the first card (not scaled). */
  const GAP = 16;
  const MARGIN = 24;
  /** The knots sit this far inside each screen edge. */
  const KNOT_INSET = 6;

  let {
    label,
    showLabel = true,
    cards,
    width,
    more,
    angles,
    falls,
    selected,
    copiedId,
    focusedId = null,
    numbers = false,
    top,
    band,
    card,
    scale,
    color,
    thickness,
    ropeStyle = 'twisted',
    sag = 14,
    pinStyle = 'plastic',
    pinColor = 'auto',
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
    showLabel?: boolean;
    cards: LineCard[];
    width: number;
    /** Items that did not fit (shown as "+N"). */
    more: number;
    angles: Record<number, number>;
    falls: Record<number, number>;
    selected: Set<number>;
    copiedId: number | null;
    focusedId?: number | null;
    /** Show the 1-9 badges on the first nine cards. */
    numbers?: boolean;
    /** Where the rope's area starts in the window and how tall it is (bandTops / SIZES in layout.ts). */
    top: number;
    band: number;
    /** Card width and size factor. */
    card: number;
    scale: number;
    /** Rope colour (any CSS colour), thickness 1..6, style and sag in px (0..40, before the size factor). */
    color: string;
    thickness: number;
    ropeStyle?: RopeStyle;
    sag?: number;
    pinStyle?: 'plastic' | 'wood' | 'metal';
    pinColor?: string;
    /** Card opacity 0.9..1. */
    cardOp: number;
    zoomOn?: boolean;
    onzoom?: (item: NonNullable<LineCard['item']> | null, rect: DOMRect | null) => void;
    /** Shown on the rope when a search leaves it empty. */
    emptyText?: string;
    onclick: (c: LineCard, e: MouseEvent) => void;
    ondblclick: (c: LineCard) => void;
    ondragout: (c: LineCard) => void;
    onremove: (c: LineCard) => void;
    oncontext: (c: LineCard, e: MouseEvent) => void;
    onhover: (c: LineCard, dir: number) => void;
    onmore: () => void;
  } = $props();

  const TOP = $derived(22 * scale);
  const sagPx = $derived(Math.min(40, Math.max(0, sag)) * scale);
  const centers = $derived(cards.map((_, i) => MARGIN + i * (card + GAP) + card / 2));
  const d = $derived(ropePath(width, centers, sagPx));
  const layers = $derived(ropeStrokes(ropeStyle, color, thickness));
  const knot = $derived(2 + thickness * 0.9);
  const knotY = (x: number) => ropeY(width, x, centers, sagPx);
</script>

<div class="rope" style:top="{top}px" style:height="{band}px">
  <svg class="cord" {width} height={TOP + sagPx + 24} viewBox="0 0 {width} {TOP + sagPx + 24}" aria-hidden="true">
    <g transform="translate(0 {TOP})" class="g" class:thin={ropeStyle === 'thin'}>
      {#each layers as l, i (i)}
        <path {d} fill="none" stroke={l.stroke} stroke-width={l.width} stroke-dasharray={l.dash} stroke-linecap="round" stroke-linejoin="round" opacity={l.opacity} />
      {/each}
      {#if ropeStyle !== 'thin'}
        <circle cx={KNOT_INSET} cy={knotY(KNOT_INSET)} r={knot} fill={color} stroke="rgba(0,0,0,0.35)" stroke-width="0.6" />
        <circle cx={width - KNOT_INSET} cy={knotY(width - KNOT_INSET)} r={knot} fill={color} stroke="rgba(0,0,0,0.35)" stroke-width="0.6" />
      {/if}
    </g>
  </svg>
  {#if showLabel && label}<span class="label">{label}</span>{/if}
  {#if cards.length === 0 && emptyText}<span class="none">{emptyText}</span>{/if}
  {#each cards as c, i (c.id)}
    <div class="slot" style:left="{centers[i] - card / 2}px" style:top="{TOP + ropeY(width, centers[i], centers, sagPx) + 8 * scale}px" style:width="{card}px">
      <Peg
        card={c}
        theta={angles[c.id] ?? 0}
        drop={fall(falls[c.id] ?? 1)}
        selected={selected.has(c.id)}
        copied={copiedId === c.id}
        focused={focusedId === c.id}
        {scale}
        width={card}
        {cardOp}
        {pinStyle}
        {pinColor}
        num={numbers && i < 9 ? i + 1 : null}
        {zoomOn}
        {onzoom}
        onclick={(e) => onclick(c, e)}
        ondblclick={() => ondblclick(c)}
        ondragout={() => ondragout(c)}
        onremove={() => onremove(c)}
        oncontext={(e) => oncontext(c, e)}
        onhover={(dir) => onhover(c, dir)}
      />
    </div>
  {/each}
  {#if more > 0}
    <button type="button" class="more" tabindex="-1" title={t('more', { count: more })} aria-label={t('more', { count: more })} onclick={onmore}>+{more}</button>
  {/if}
</div>

<style>
  .rope { position: absolute; left: 0; right: 0; }
  .cord { position: absolute; inset: 0 auto auto 0; overflow: visible; pointer-events: none; }
  .g { filter: drop-shadow(0 1.5px 1.5px rgba(0, 0, 0, 0.45)); }
  .g.thin { filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.5)); }
  .label {
    position: absolute;
    left: 12px;
    top: 0;
    padding: 0 9px;
    height: 17px;
    line-height: 17px;
    border-radius: 999px;
    background: rgba(20, 20, 24, 0.72);
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    font: var(--text-caption);
    font-size: 11px;
    color: rgba(255, 255, 255, 0.82);
    pointer-events: none;
    z-index: 1;
  }
  .slot { position: absolute; transition: left var(--dur) var(--ease-out), top var(--dur) var(--ease-out); }
  :global([data-reduce-motion]) .slot { transition: none; }
  .none {
    position: absolute;
    inset: 22px 0 0 0;
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.8);
    font: var(--text-callout);
    text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
    pointer-events: none;
  }
  .more {
    position: absolute;
    right: 14px;
    top: 52px;
    min-width: 40px;
    height: 26px;
    padding: 0 var(--space-2);
    border: 0;
    border-radius: 999px;
    background: rgba(20, 20, 24, 0.78);
    color: #fff;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16);
    font: var(--text-callout);
    font-weight: 600;
    cursor: default;
  }
  .more:hover { background: var(--accent); color: var(--on-accent); }
</style>
