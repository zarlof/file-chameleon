<!--
  TitleBar — frameless window chrome: a full-width drag region (double-click maximizes) with
  Windows 11 caption buttons at the top right (minimize, maximize / restore, close).
  Glyphs come from the system icon font; they dim while the window is unfocused.
  The maximize glyph follows `isMaximized()`, re-read on every resize. No props.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { t } from '../lib/i18n';

  let focused = $state(true);
  let maximized = $state(false);

  function run(fn: () => Promise<unknown>) {
    try {
      fn().catch(() => {});
    } catch {
      /* not running inside Tauri */
    }
  }

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const sync = () => run(async () => (maximized = await getCurrentWindow().isMaximized()));
    sync();
    run(async () => {
      const un = await getCurrentWindow().onResized(sync);
      if (disposed) un();
      else unlisten = un;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });
</script>

<svelte:window onfocus={() => (focused = true)} onblur={() => (focused = false)} />

<header class="titlebar" data-tauri-drag-region>
  <div class="caption" class:blur={!focused}>
    <button type="button" class="cb" aria-label={t('common.window.minimize')} title={t('common.window.minimize')}
      onclick={() => run(() => getCurrentWindow().minimize())}>
      <span class="glyph" aria-hidden="true">&#xE921;</span>
    </button>
    <button type="button" class="cb" aria-label={t(maximized ? 'common.window.restore' : 'common.window.maximize')}
      title={t(maximized ? 'common.window.restore' : 'common.window.maximize')}
      onclick={() => run(() => getCurrentWindow().toggleMaximize())}>
      <span class="glyph" aria-hidden="true">{maximized ? '' : ''}</span>
    </button>
    <button type="button" class="cb close" aria-label={t('common.window.close')} title={t('common.window.close')}
      onclick={() => run(() => getCurrentWindow().close())}>
      <span class="glyph" aria-hidden="true">&#xE8BB;</span>
    </button>
  </div>
</header>

<style>
  .titlebar { position: absolute; z-index: 50; top: 0; left: 0; right: 0; height: var(--titlebar-h); display: flex; justify-content: flex-end; }
  /* Windows 11 caption metrics and colours (system chrome, so literal values rather than theme tokens). */
  .caption {
    --cb-hover: rgba(0, 0, 0, 0.06);
    --cb-press: rgba(0, 0, 0, 0.04);
    display: flex; height: 32px;
  }
  :global(:root[data-theme='dark']) .caption { --cb-hover: rgba(255, 255, 255, 0.08); --cb-press: rgba(255, 255, 255, 0.05); }
  .cb {
    display: grid; place-items: center; width: 46px; height: 32px; padding: 0; border: 0; border-radius: 0;
    background: transparent; color: var(--text); cursor: default; transition: background var(--dur-fast), color var(--dur-fast);
  }
  .cb:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; border-radius: 0; }
  .glyph { font-family: 'Segoe Fluent Icons', 'Segoe MDL2 Assets'; font-size: 10px; line-height: 1; }
  .cb:hover { background: var(--cb-hover); }
  .cb:active { background: var(--cb-press); }
  .cb.close:hover { background: #c42b1c; color: #fff; }
  .cb.close:active { background: rgba(196, 43, 28, 0.9); color: rgba(255, 255, 255, 0.7); }
  .caption.blur .cb:not(:hover) { color: var(--text-tertiary); }
</style>
