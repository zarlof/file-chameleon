<!--
  Pin — a small realistic clothespin, drawn here from scratch: two slanted jaws that meet at the bottom,
  a bevel (a lighter and a darker strip per jaw), a metal spring coil in the middle and, for the wood
  style, a few grain lines. About 10 x 24 px at medium size. The base colour comes from the style
  (plastic = light grey, wood = warm beige, metal = cool grey) or from `color`; a pinned card gets the accent.
-->
<script lang="ts">
  let {
    pinStyle = 'plastic',
    color = 'auto',
    pinned = false,
    scale = 1,
  }: {
    pinStyle?: 'plastic' | 'wood' | 'metal';
    /** 'auto' (the style's own colour) or a #rrggbb hex. */
    color?: string;
    /** Pinned cards get an accent coloured pin. */
    pinned?: boolean;
    scale?: number;
  } = $props();

  const BASE = { plastic: '#cfd2d6', wood: '#d6b583', metal: '#a4adb7' } as const;
  const base = $derived(pinned ? 'var(--accent)' : /^#[0-9a-f]{6}$/i.test(color) ? color : (BASE[pinStyle] ?? BASE.plastic));
</script>

<svg class="pin {pinStyle}" viewBox="0 0 10 24" width={10 * scale} height={24 * scale} style:--pin={base} aria-hidden="true">
  <!-- left and right jaw -->
  <path d="M1 1.4Q1 0.6 1.8 0.6H3Q3.8 0.6 3.8 1.4L5 22.6Q5 23.4 4.2 23.4H3.4Q2.8 23.4 2.7 22.8Z" class="jaw" />
  <path d="M9 1.4Q9 0.6 8.2 0.6H7Q6.2 0.6 6.2 1.4L5 22.6Q5 23.4 5.8 23.4H6.6Q7.2 23.4 7.3 22.8Z" class="jaw" />
  <!-- bevel -->
  <path d="M1.5 1.6L3.1 22.4" class="hi" />
  <path d="M8.5 1.6L6.9 22.4" class="hi" />
  <path d="M3.5 2L4.7 22.4" class="lo" />
  <path d="M6.5 2L5.3 22.4" class="lo" />
  {#if pinStyle === 'wood'}
    <path d="M2 6.5L3.3 6.2M2.2 15L3.8 15.4M8 7L6.8 6.8M7.8 16L6.4 16.4" class="grain" />
  {/if}
  <!-- the spring: a coil and two arms across both jaws -->
  <path d="M1.7 9.6H8.3M1.9 12.4H8.1" class="wire" />
  <ellipse cx="5" cy="11" rx="1.9" ry="2.1" class="coil" />
  <path d="M3.7 11H6.3" class="wire thin" />
  <!-- where the jaws grip -->
  <path d="M3.3 21.6H6.7" class="grip" />
</svg>

<style>
  .pin { display: block; overflow: visible; filter: drop-shadow(0 1px 1.2px rgba(0, 0, 0, 0.4)); }
  .jaw { fill: var(--pin); stroke: color-mix(in srgb, var(--pin) 55%, black); stroke-width: 0.35; stroke-linejoin: round; }
  .hi { fill: none; stroke: color-mix(in srgb, var(--pin) 45%, white); stroke-width: 0.7; stroke-linecap: round; opacity: 0.85; }
  .lo { fill: none; stroke: color-mix(in srgb, var(--pin) 70%, black); stroke-width: 0.5; stroke-linecap: round; opacity: 0.55; }
  .grain { fill: none; stroke: color-mix(in srgb, var(--pin) 60%, black); stroke-width: 0.35; stroke-linecap: round; opacity: 0.6; }
  .wire { fill: none; stroke: #8b9097; stroke-width: 0.6; stroke-linecap: round; }
  .wire.thin { stroke: #e9ecef; stroke-width: 0.4; }
  .coil { fill: none; stroke: #7b8087; stroke-width: 0.7; }
  .grip { stroke: color-mix(in srgb, var(--pin) 50%, black); stroke-width: 0.5; stroke-linecap: round; opacity: 0.5; }
  .metal .jaw { stroke: #e8edf2; stroke-width: 0.5; }
  .metal .hi { stroke: #ffffff; }
</style>
