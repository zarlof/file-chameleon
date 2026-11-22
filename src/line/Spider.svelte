<!--
  Spider — a small, simple black spider that hangs from the rope on its own silk thread and holds a
  card: the two upper legs hold the thread, the two lower ones grip the card's top edge, two short
  side legs give it its shape. The thread is white with a faint dark edge so it reads on any
  background; the spider is one colour (black) with a thin light halo for dark desktops.
  Drawn in a 26 x 36 box: y = 0 is the rope, y = 34 the card's top edge.
-->
<script lang="ts">
  let { scale = 1 }: { scale?: number } = $props();
</script>

<svg class="spider" viewBox="0 0 26 36" width={26 * scale} height={36 * scale} aria-hidden="true">
  <!-- its own thread -->
  <path d="M13 0V10" class="silk-edge" />
  <path d="M13 0V10" class="silk" />
  <g class="body">
    <!-- upper legs: hold the thread -->
    <path d="M10.6 12.2L7.4 7.6L12.3 5" />
    <path d="M15.4 12.2L18.6 7.6L13.7 5" />
    <!-- side legs -->
    <path d="M9.2 15.6L4.6 14L2.6 18" />
    <path d="M16.8 15.6L21.4 14L23.4 18" />
    <!-- lower legs: hold the card -->
    <path d="M11 22.4L6.2 25.4L8 34.2" />
    <path d="M15 22.4L19.8 25.4L18 34.2" />
    <ellipse cx="13" cy="14.6" rx="4.4" ry="5.1" class="fill" />
    <circle cx="13" cy="21.6" r="3.1" class="fill" />
  </g>
  <circle cx="11.9" cy="22.5" r="0.6" class="eye" />
  <circle cx="14.1" cy="22.5" r="0.6" class="eye" />
</svg>

<style>
  .spider { display: block; overflow: visible; filter: drop-shadow(0 0 0.7px rgba(255, 255, 255, 0.55)) drop-shadow(0 1px 1px rgba(0, 0, 0, 0.35)); }
  .silk-edge { stroke: rgba(0, 0, 0, 0.45); stroke-width: 1.7; stroke-linecap: round; }
  .silk { stroke: #ffffff; stroke-width: 0.9; stroke-linecap: round; }
  .body path { fill: none; stroke: #0b0b0d; stroke-width: 1.15; stroke-linecap: round; stroke-linejoin: round; }
  .fill { fill: #0b0b0d; }
  .eye { fill: rgba(255, 255, 255, 0.85); }
</style>
