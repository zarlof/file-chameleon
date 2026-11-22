<!--
  Rope — one natural cord across the whole screen width (no band, no backdrop), a knot near each end,
  a small label pill at the left end and the cards hanging from it by clothespins. The strokes come from
  ropeStyle.ts (twisted / cord / thin); the swing angles and falling progress are owned by Line.svelte's
  shared animation loop. While a card is dragged along the rope (`drag`, owned by Line.svelte) it follows
  the pointer and the others step aside to show where it will land. The settings page draws its live
  preview with this same component.
-->
<script lang="ts">
  import { fall, ropePath, ropeY } from './physics';
  import { ropeStrokes, type RopeStyle } from './ropeStyle';
  import type { LineCard } from './logic';
  import { cardCenter } from './layout';
  import Peg from './Peg.svelte';
  import { t } from './i18n';

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
    ropeOp = 1,
    previewOn = false,
    onpreview,
    drag = null,
    emptyText = '',
    collapsed = false,
    ontoggle,
    onpin,
    onclick,
    ondblclick,
    ondragstart,
    onremove,
    oncontext,
    onhover,
    onmore,
    ongear,
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
    pinStyle?: 'spider' | 'plastic' | 'wood' | 'metal';
    pinColor?: string;
    /** Card opacity 0.9..1. */
    cardOp: number;
    /** Opacity of the rope itself (cord, knots, label), 0.3..1. */
    ropeOp?: number;
    previewOn?: boolean;
    onpreview?: (c: LineCard, rect: DOMRect | null) => void;
    /** A card of this rope being dragged: its offset from where it hangs and the slot it would take
     *  among the others. */
    drag?: { id: number; dx: number; dy: number; slot: number } | null;
    /** Shown on the rope when a search leaves it empty. */
    emptyText?: string;
    /** Folded away: only the label (with its show button) is drawn. */
    collapsed?: boolean;
    /** The hide / show button next to the label; without it the label is plain text. */
    ontoggle?: () => void;
    /** The pin button on a card. */
    onpin?: (c: LineCard) => void;
    onclick: (c: LineCard, e: MouseEvent) => void;
    ondblclick: (c: LineCard) => void;
    ondragstart: (c: LineCard, e: PointerEvent) => void;
    onremove: (c: LineCard) => void;
    oncontext: (c: LineCard, e: MouseEvent) => void;
    onhover: (c: LineCard, dir: number) => void;
    onmore: () => void;
    /** The gear on a card (its own settings). */
    ongear?: (c: LineCard, rect: DOMRect) => void;
  } = $props();

  const TOP = $derived(22 * scale);
  const sagPx = $derived(Math.min(40, Math.max(0, sag)) * scale);
  /** Where each card hangs. A dragged card follows the pointer; the others make room at its slot. */
  const dragFrom = $derived(drag ? cards.findIndex((c) => c.id === drag.id) : -1);
  const centers = $derived(
    cards.map((_, i) => {
      if (dragFrom < 0 || !drag) return cardCenter(i, card);
      if (i === dragFrom) return cardCenter(i, card) + drag.dx;
      const rest = i < dragFrom ? i : i - 1;
      return cardCenter(rest < drag.slot ? rest : rest + 1, card);
    }),
  );
  const d = $derived(ropePath(width, centers, sagPx));
  const layers = $derived(ropeStrokes(ropeStyle, color, thickness));
  const knot = $derived(2 + thickness * 0.9);
  const knotY = (x: number) => ropeY(width, x, centers, sagPx);
</script>

<div class="rope" class:collapsed style:top="{top}px" style:height="{collapsed ? 26 : band}px">
  {#if !collapsed}
  <svg class="cord" style:opacity={ropeOp} {width} height={TOP + sagPx + 24} viewBox="0 0 {width} {TOP + sagPx + 24}" aria-hidden="true">
    <g transform="translate(0 {TOP})" class="g" class:thin={ropeStyle === 'thin'}>
      {#each layers as l, i (i)}
        <path {d} fill="none" stroke={l.stroke} stroke-width={l.width} stroke-dasharray={l.dash} stroke-linecap="round" stroke-linejoin="round" opacity={l.opacity} />
      {/each}
      {#if ropeStyle !== 'thin' && ropeStyle !== 'web'}
        <circle cx={KNOT_INSET} cy={knotY(KNOT_INSET)} r={knot} fill={color} stroke="rgba(0,0,0,0.35)" stroke-width="0.6" />
        <circle cx={width - KNOT_INSET} cy={knotY(width - KNOT_INSET)} r={knot} fill={color} stroke="rgba(0,0,0,0.35)" stroke-width="0.6" />
      {/if}
    </g>
  </svg>
  {/if}
  {#if ontoggle}
    <span class="label tool" style:opacity={collapsed ? Math.max(0.7, ropeOp) : ropeOp}>
      {#if showLabel || collapsed}<span class="name">{label}</span>{/if}
      <button type="button" class="eye" tabindex="-1" title={t(collapsed ? 'rope.show' : 'rope.hide')} aria-label={t(collapsed ? 'rope.show' : 'rope.hide')} aria-pressed={collapsed} onclick={ontoggle}>
        <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
          <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8s-2.4 4.5-6.5 4.5S1.5 8 1.5 8Z" />
          <circle cx="8" cy="8" r="2" />
          {#if collapsed}<path d="M2.5 13.5l11-11" class="slash" />{/if}
        </svg>
      </button>
    </span>
  {:else if showLabel && label}<span class="label" style:opacity={ropeOp}>{label}</span>{/if}
  {#if !collapsed}
  {#if cards.length === 0 && emptyText}<span class="none">{emptyText}</span>{/if}
  {#each cards as c, i (c.id)}
    {@const lifted = i === dragFrom}
    <div
      class="slot"
      class:lifted
      style:left="{centers[i] - card / 2}px"
      style:top="{TOP + ropeY(width, lifted ? cardCenter(i, card) : centers[i], centers, sagPx) + 8 * scale + (lifted && drag ? drag.dy : 0)}px"
      style:width="{card}px"
    >
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
        {previewOn}
        {lifted}
        onpreview={onpreview ? (rect) => onpreview(c, rect) : undefined}
        onclick={(e) => onclick(c, e)}
        ondblclick={() => ondblclick(c)}
        ondragstart={(e) => ondragstart(c, e)}
        onremove={() => onremove(c)}
        oncontext={(e) => oncontext(c, e)}
        onhover={(dir) => onhover(c, dir)}
        onpin={onpin ? () => onpin(c) : undefined}
        ongear={ongear ? (rect) => ongear(c, rect) : undefined}
      />
    </div>
  {/each}
  {#if more > 0}
    <button type="button" class="more" tabindex="-1" title={t('more', { count: more })} aria-label={t('more', { count: more })} onclick={onmore}>+{more}</button>
  {/if}
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
  .label.tool { display: inline-flex; align-items: center; gap: 6px; padding: 0 3px 0 9px; pointer-events: auto; z-index: 3; }
  .collapsed .label.tool { padding-left: 9px; }
  .eye {
    display: grid;
    place-items: center;
    width: 15px;
    height: 15px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.9);
    cursor: default;
  }
  .eye:hover { background: var(--accent); color: var(--on-accent); }
  .eye svg { fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; stroke-linejoin: round; }
  .eye .slash { stroke-width: 1.7; }
  .slot { position: absolute; transition: left var(--dur) var(--ease-out), top var(--dur) var(--ease-out); }
  .slot.lifted { z-index: 5; transition: none; }
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
