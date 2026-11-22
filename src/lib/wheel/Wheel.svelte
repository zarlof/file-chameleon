<!--
  Wheel — the Filee-style donut: light slices with even gaps, the hovered slice in the wheel
  colour, a centre disc, an outer ring for an open group, and a wave when the face flips.
  A pure view: the parent owns face, hover and fan state (the overlay hit-tests the pointer
  with geometry.ts / hover.ts; the wheel editor passes `selected` + `onslotclick`).
  Drawn at scale 1 (540 px box) and CSS-scaled to `size` (default: `style.scale`).
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { SVGAttributes } from 'svelte/elements';
  import Ban from '@lucide/svelte/icons/ban';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import type { WheelFace, WheelSlot, WheelStyle } from '../types';
  import {
    BASE_SIZE,
    fanLayout,
    fanSlicePath,
    polar,
    ringRadii,
    sliceCenterAngle,
    slicePath,
    slotAtAngle,
    waveSteps,
    type Hit,
  } from './geometry';
  import { slotIcon } from './icons';
  import { isGroupSlot, slotLabel, type Translate } from './labels';

  interface Props {
    front: WheelSlot[];
    back: WheelSlot[];
    face: WheelFace;
    hovered: Hit;
    /** Group slot whose fan ring is open, -1 = none. */
    fan: number;
    style: WheelStyle;
    center: { title: string; subtitle: string };
    missingTools: string[];
    /** i18n of the host (overlay or main app); keys "presets.*" and "overlay.*". */
    t: Translate;
    /** Box size in px; default `ringRadii(style.scale).size`. */
    size?: number;
    /** Editor highlight (slot index), -1 = none. */
    selected?: number;
    /** Editor: makes slots clickable / focusable. */
    onslotclick?: (i: number) => void;
  }
  let { front, back, face, hovered, fan, style, center, missingTools, t, size, selected = -1, onslotclick }: Props = $props();

  const R = ringRadii(1);
  const GAP = 6;
  /** Rounded corners: paths are inset by this and stroked twice as wide in their own colour. */
  const CORNER = 5;
  /** One flip: the old face spins and fades out while the new one spins in (2D only, so it stays
   *  smooth on the CPU compositor the overlay runs on). */
  const FLIP_MS = 280;
  const WAVE_MS = 0;
  const TURN_MS = 0;

  interface View {
    slot: WheelSlot;
    path: string;
    /** Label centre (and the slice's turning axis). */
    x: number;
    y: number;
    angle: number;
    width: number;
    icon: boolean;
    text: boolean;
    dense: boolean;
    chevron: boolean;
    delay: number;
  }

  /** `auto` = icon + text up to 6 slots, else text only. */
  function labelMode(count: number): { icon: boolean; text: boolean } {
    if (style.labels === 'icon') return { icon: true, text: false };
    if (style.labels === 'text') return { icon: false, text: true };
    if (style.labels === 'iconText') return { icon: true, text: true };
    return { icon: count <= 6, text: true };
  }

  /** Room for a horizontal label: the slice's chord at the top and bottom of the wheel, the
   *  ring's thickness at its sides. */
  function labelWidth(chord: number, thickness: number, angle: number): number {
    return Math.min(112, chord + (thickness - chord) * Math.abs(Math.sin(angle)) - 12);
  }

  /** Label font size (px) so its longest word fits `width` (Inter bold caps ≈ 0.72 em/char). */
  function fitFont(label: string, width: number, dense: boolean): number {
    const longest = Math.max(1, ...label.split(/\s+/).map((w) => w.length));
    return Math.max(8.5, Math.min(dense ? 10 : 11.5, width / (longest * 0.72)));
  }

  function ringViews(list: WheelSlot[], origin: number, baseDelay: number): View[] {
    const count = list.length;
    const dense = count > 8;
    // Dense rings put labels a little further out, where the slices are wider.
    const mid = (R.inner + R.outer) / 2 + (dense ? 8 : 0);
    const chord = count <= 1 ? 2 * mid : 2 * mid * Math.sin(Math.PI / count);
    const mode = labelMode(count);
    return list.map((slot, i) => {
      const angle = sliceCenterAngle(i, count);
      const p = polar(angle, mid);
      return {
        slot,
        path: slicePath(i, count, R.inner + CORNER, R.outer - CORNER, GAP + 2 * CORNER),
        x: p.x,
        y: p.y,
        angle,
        width: labelWidth(chord, R.outer - R.inner, angle),
        ...mode,
        dense,
        chevron: isGroupSlot(slot) && slot.children.length > 0,
        delay: baseDelay + WAVE_MS * waveSteps(i, count, origin),
      };
    });
  }

  // ---- flip: the old face spins out and fades, the new one spins in from the other side.
  let flip = $state.raw<{ out: View[]; hot: number; origin: number; dir: 1 | -1 } | null>(null);
  let flipKey = $state(0);
  let shown: WheelFace | null = null;
  let flipTimer: ReturnType<typeof setTimeout> | undefined;

  function flipTo(f: WheelFace) {
    if (shown === null || shown === f) {
      shown = f;
      return;
    }
    const before = shown === 'front' ? front : back;
    const after = f === 'front' ? front : back;
    shown = f;
    if (style.reduceMotion) {
      flip = null;
      return;
    }
    const pointed = hovered.slot >= 0 && hovered.slot < after.length;
    const origin = pointed ? sliceCenterAngle(hovered.slot, after.length) : 0;
    clearTimeout(flipTimer);
    flip = {
      out: ringViews(before, origin, 0),
      hot: pointed && before.length ? slotAtAngle(origin, before.length) : -1,
      origin,
      dir: f === 'back' ? 1 : -1,
    };
    flipKey++;
    flipTimer = setTimeout(() => (flip = null), FLIP_MS + 40);
  }

  $effect.pre(() => {
    const f = face;
    untrack(() => flipTo(f));
    return () => clearTimeout(flipTimer);
  });

  const box = $derived(size ?? ringRadii(style.scale).size);
  const slots = $derived(face === 'front' ? front : back);
  const other = $derived(face === 'front' ? back : front);
  const views = $derived(ringViews(slots, flip?.origin ?? 0, flip ? TURN_MS : 0));
  const alpha = $derived(Math.min(1, Math.max(0.2, (style.opacity || 100) / 100)));

  const fanSlot = $derived(fan >= 0 && fan < slots.length && slots[fan].children.length > 0 ? slots[fan] : null);
  const fanViews = $derived.by((): View[] => {
    if (!fanSlot) return [];
    const kids = fanSlot.children;
    const { start, step } = fanLayout(fan, slots.length, kids.length);
    const mid = (R.fanInner + R.fanOuter) / 2;
    const chord = 2 * mid * Math.sin(Math.min(step, Math.PI) / 2);
    const mode = labelMode(kids.length);
    return kids.map((slot, c) => {
      const angle = start + (c + 0.5) * step;
      const p = polar(angle, mid);
      return {
        slot,
        path: fanSlicePath(fan, slots.length, c, kids.length, R.fanInner + CORNER, R.fanOuter - CORNER, GAP + 2 * CORNER),
        x: p.x,
        y: p.y,
        angle,
        width: labelWidth(chord, R.fanOuter - R.fanInner, angle),
        ...mode,
        dense: false,
        chevron: false,
        delay: 0,
      };
    });
  });

  const needsTool = (s: WheelSlot) => s.needsTools.some((x) => missingTools.includes(x));
  /** Detail line, unless hidden or already part of the label ("16:9 kırp" + "16:9"). */
  const detailOf = (s: WheelSlot, label: string) =>
    style.showDetail && s.detail && !label.toLowerCase().includes(s.detail.toLowerCase()) ? s.detail : null;

  function interactive(i: number, slot: WheelSlot): SVGAttributes<SVGPathElement> {
    if (i < 0 || !onslotclick) return {};
    const go = () => onslotclick?.(i);
    return {
      role: 'button',
      tabindex: 0,
      'aria-label': slotLabel(slot, t),
      onclick: go,
      onkeydown: (e: KeyboardEvent) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          go();
        }
      },
    };
  }
</script>

{#snippet sliceView(v: View, index: number, hot: boolean, open: boolean, sel: boolean, turn: '' | 'in' | 'out')}
  {@const Icon = slotIcon(v.slot.icon)}
  {@const label = slotLabel(v.slot, t)}
  {@const detail = v.text ? detailOf(v.slot, label) : null}
  {@const fontPx = fitFont(label, v.width, v.dense)}
  <div class="turn {turn}" style:transform-origin="{v.x}px {v.y}px" style:animation-delay={turn ? `${v.delay}ms` : undefined}>
    <div class="body" class:hot class:open class:sel>
      <svg width={BASE_SIZE} height={BASE_SIZE} viewBox="0 0 {BASE_SIZE} {BASE_SIZE}" aria-hidden={index >= 0 && onslotclick ? undefined : 'true'}>
        <path class="slice" d={v.path} {...interactive(index, v.slot)} />
      </svg>
      <div class="label" class:dense={v.dense} style:left="{v.x}px" style:top="{v.y}px" style:max-width="{v.width}px">
        {#if v.icon}<Icon size={v.text ? 18 : 24} strokeWidth={1.9} />{/if}
        {#if v.text}<span class="name" style:font-size="{fontPx}px">{label}</span>{/if}
        {#if detail}<span class="detail">{detail}</span>{/if}
        {#if needsTool(v.slot)}<span class="tool"><Download size={10} strokeWidth={2.4} /></span>{/if}
      </div>
      {#if v.chevron}
        {@const c = polar(v.angle, R.outer - 11)}
        <span class="chev" style:left="{c.x}px" style:top="{c.y}px" style:transform="translate(-50%, -50%) rotate({(v.angle * 180) / Math.PI}deg)">
          <ChevronUp size={12} strokeWidth={2.6} />
        </span>
      {/if}
    </div>
  </div>
{/snippet}

<div
  class="wheel"
  class:still={style.reduceMotion}
  class:editable={!!onslotclick}
  style:width="{box}px"
  style:height="{box}px"
  style:--wheel-hot={style.color || 'var(--accent)'}
  style:--wheel-alpha={alpha}
>
  <div class="stage" style:transform="scale({box / BASE_SIZE})">
    <div class="pop">
      <div class="slices">
        {#if flip}
          {#key flipKey}
            <div class="face out" style:--flip-dir={flip.dir}>
              {#each flip.out as v, i (i)}
                {@render sliceView(v, -1, i === flip.hot, false, false, '')}
              {/each}
            </div>
          {/key}
        {/if}
        {#key flipKey}
          <div class="face" class:in={!!flip} style:--flip-dir={flip?.dir ?? 1}>
            {#each views as v, i (i)}
              {@render sliceView(v, i, hovered.slot === i && hovered.child < 0, fan === i && hovered.child >= 0, i === selected, '')}
            {/each}
          </div>
        {/key}
        {#if fanSlot}
          <div class="fan">
            {#each fanViews as v, c (c)}
              {@render sliceView(v, -1, hovered.slot === fan && hovered.child === c, false, false, '')}
            {/each}
          </div>
        {/if}
      </div>

      <div class="hub">
        <div class="disc" class:empty={slots.length === 0}>
          {#if slots.length === 0}
            <Ban size={18} strokeWidth={1.8} />
            <span class="title">{t('overlay.notSupported')}</span>
          {:else}
            <span class="title">{center.title}</span>
          {/if}
          {#if center.subtitle}<span class="sub">{center.subtitle}</span>{/if}
          {#if other.length > 0}
            <span class="hint"><RefreshCw size={12} strokeWidth={2.2} />{t('overlay.flipKey')}</span>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .wheel {
    /* Slice / disc colours of the donut (light); dark below. */
    --wheel-slice: #f3f2ef;
    --wheel-disc: #ffffff;
    --wheel-ring: var(--control-bg);
    --wheel-ink: var(--text);
    --wheel-muted: var(--text-secondary);
    --wheel-hot-ink: #ffffff;
    --wheel-shadow: rgba(0, 0, 0, 0.16);
    position: relative;
    flex: none;
    font-family: var(--font);
    color: var(--wheel-ink);
    pointer-events: none;
  }
  :global(:root[data-theme='dark']) .wheel {
    --wheel-slice: #2c2c30;
    --wheel-disc: var(--elevated-bg);
    --wheel-shadow: rgba(0, 0, 0, 0.45);
  }
  .stage {
    position: absolute;
    left: 0;
    top: 0;
    width: 540px;
    height: 540px;
    transform-origin: 0 0;
  }
  .pop {
    position: absolute;
    inset: 0;
    filter: drop-shadow(0 6px 18px var(--wheel-shadow));
    animation: appear 200ms var(--ease-spring) both;
  }
  .slices {
    position: absolute;
    inset: 0;
  }
  .turn,
  .body,
  .fan {
    position: absolute;
    inset: 0;
  }
  .face {
    position: absolute;
    inset: 0;
    transform-origin: 270px 270px;
  }
  .face.out {
    animation: face-out 280ms cubic-bezier(0.4, 0, 0.7, 0.2) both;
    will-change: transform, opacity;
  }
  .face.in {
    animation: face-in 280ms cubic-bezier(0.16, 1, 0.3, 1) both;
    will-change: transform, opacity;
  }
  .body {
    transition: transform var(--dur) var(--ease-spring);
  }
  .body.hot {
    transform: scale(1.04);
  }
  .fan {
    animation: fan-in 160ms var(--ease-out) both;
  }
  svg {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .slice {
    fill: var(--wheel-slice);
    stroke: var(--wheel-slice);
    stroke-width: 10px;
    stroke-linejoin: round;
    opacity: var(--wheel-alpha);
    transition:
      fill var(--dur-fast) var(--ease-out),
      stroke var(--dur-fast) var(--ease-out),
      opacity var(--dur-fast) var(--ease-out);
  }
  .open .slice {
    fill: color-mix(in srgb, var(--wheel-hot) 22%, var(--wheel-slice));
    stroke: color-mix(in srgb, var(--wheel-hot) 22%, var(--wheel-slice));
  }
  .sel .slice {
    stroke: var(--wheel-hot);
    opacity: 1;
  }
  .hot .slice {
    fill: var(--wheel-hot);
    stroke: var(--wheel-hot);
    opacity: 1;
  }
  .editable .slice {
    pointer-events: visiblePainted;
    cursor: pointer;
  }
  .slice:focus-visible {
    outline: none;
    stroke: var(--wheel-hot);
    opacity: 1;
  }
  .label {
    position: absolute;
    width: max-content;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    text-align: center;
    color: var(--wheel-ink);
    transition: color var(--dur-fast) var(--ease-out);
  }
  .hot .label,
  .hot .chev {
    color: var(--wheel-hot-ink);
  }
  .name {
    max-width: 100%;
    font-weight: 700;
    line-height: 1.15;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    text-wrap: balance;
    overflow-wrap: anywhere;
    hyphens: auto;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .dense .name {
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }
  .detail {
    font-size: 10px;
    font-weight: 600;
    line-height: 12px;
    color: var(--wheel-muted);
  }
  .hot .detail {
    color: color-mix(in srgb, var(--wheel-hot-ink) 82%, transparent);
  }
  .tool {
    position: absolute;
    right: -18px;
    top: -8px;
    width: 16px;
    height: 16px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--wheel-disc);
    color: var(--wheel-muted);
    box-shadow: var(--shadow-sm);
  }
  .hot .tool {
    color: var(--wheel-hot);
  }
  .chev {
    position: absolute;
    display: grid;
    color: var(--wheel-muted);
  }
  /* Centre: disc r 52 inside a soft 8 px ring. */
  .hub {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 120px;
    height: 120px;
    transform: translate(-50%, -50%);
    display: grid;
    place-items: center;
  }
  .hub::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 8px solid var(--wheel-ring);
    opacity: var(--wheel-alpha);
  }
  .disc {
    position: relative;
    isolation: isolate;
    width: 104px;
    height: 104px;
    border-radius: 50%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    text-align: center;
    color: var(--wheel-ink);
  }
  .disc::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: 50%;
    background: var(--wheel-disc);
    opacity: var(--wheel-alpha);
  }
  .title {
    max-width: 90px;
    font: 700 15px/18px var(--font);
    letter-spacing: var(--tracking-tight);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    color: var(--wheel-muted);
  }
  .empty .title {
    font-size: 12px;
    line-height: 15px;
    white-space: normal;
  }
  .sub {
    font: var(--text-caption);
    color: var(--wheel-muted);
  }
  .hint {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-top: 2px;
    font-size: 10px;
    font-weight: 600;
    line-height: 12px;
    color: var(--wheel-muted);
  }
  .still,
  .still * {
    animation: none !important;
    transition: none !important;
  }
  @keyframes appear {
    from {
      opacity: 0;
      transform: scale(0.86);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
  @keyframes face-out {
    from {
      opacity: 1;
      transform: rotate(0deg) scale(1);
    }
    to {
      opacity: 0;
      transform: rotate(calc(var(--flip-dir) * 28deg)) scale(0.9);
    }
  }
  @keyframes face-in {
    from {
      opacity: 0;
      transform: rotate(calc(var(--flip-dir) * -28deg)) scale(0.9);
    }
    to {
      opacity: 1;
      transform: rotate(0deg) scale(1);
    }
  }
  @keyframes fan-in {
    from {
      opacity: 0;
      transform: scale(0.94);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
</style>
