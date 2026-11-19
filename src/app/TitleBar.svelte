<!--
  TitleBar — frameless window chrome: full-width drag region + macOS traffic lights (left).
  Lights go grey while the window is unfocused; glyphs appear when hovering the group.
  No props.
-->
<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { t } from '../lib/i18n';

  let focused = $state(true);

  function run(fn: () => Promise<unknown>) {
    fn().catch(() => {});
  }
</script>

<svelte:window onfocus={() => (focused = true)} onblur={() => (focused = false)} />

<header class="titlebar" data-tauri-drag-region>
  <div class="lights" class:blur={!focused} role="group" aria-label={t('common.appName')}>
    <button type="button" class="light close" aria-label={t('common.window.close')} onclick={() => run(() => getCurrentWindow().close())}>
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3.6 3.6l4.8 4.8M8.4 3.6L3.6 8.4" /></svg>
    </button>
    <button type="button" class="light min" aria-label={t('common.window.minimize')} onclick={() => run(() => getCurrentWindow().minimize())}>
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 6h6" /></svg>
    </button>
    <button type="button" class="light zoom" aria-label={t('common.window.zoom')} onclick={() => run(() => getCurrentWindow().toggleMaximize())}>
      <svg viewBox="0 0 12 12" aria-hidden="true"><path class="fill" d="M3.4 3.4h3.6L3.4 7zM8.6 8.6H5L8.6 5z" /></svg>
    </button>
  </div>
</header>

<style>
  .titlebar { position: absolute; z-index: 50; top: 0; left: 0; right: 0; height: var(--titlebar-h); display: flex; align-items: center; padding-left: 20px; }
  .lights { display: flex; gap: 8px; }
  .light {
    position: relative; display: grid; place-items: center; width: 12px; height: 12px; padding: 0; border: 0; border-radius: 50%;
    box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.18); transition: background var(--dur-fast), filter var(--dur-fast);
  }
  .close { background: #ff5f57; }
  .min { background: #febc2e; }
  .zoom { background: #28c840; }
  .light:active { filter: brightness(0.8); }
  .light svg { width: 12px; height: 12px; opacity: 0; fill: none; stroke: rgba(0, 0, 0, 0.62); stroke-width: 1.15; stroke-linecap: round; transition: opacity var(--dur-fast); }
  .light svg .fill { fill: rgba(0, 0, 0, 0.62); stroke: none; }
  .lights:hover .light svg { opacity: 1; }
  .lights.blur:not(:hover) .light { background: var(--control-bg-active); box-shadow: inset 0 0 0 0.5px var(--hairline); }
</style>
