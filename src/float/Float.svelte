<!--
  Float — an image pinned on screen, in its own small window. The path comes as ?path=...
  Drag anywhere to move, Ctrl+wheel zooms (0.25x-4x), x or Esc closes. Never imports from src/app/.
-->
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';
  import { api } from '../lib/ipc';
  import type { Settings } from '../lib/types';
  import { setLang, t } from '../line/i18n';

  const path = new URLSearchParams(window.location.search).get('path') ?? '';
  const src = path ? convertFileSrc(path) : '';
  let zoom = $state(1);
  let failed = $state(false);

  const dark = window.matchMedia('(prefers-color-scheme: dark)');
  let themeSetting: Settings['theme'] = 'system';
  function applyTheme() {
    document.documentElement.dataset.theme = themeSetting === 'system' ? (dark.matches ? 'dark' : 'light') : themeSetting;
  }

  function close() {
    void getCurrentWindow().close().catch(() => {});
  }

  function onwheel(e: WheelEvent) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    zoom = Math.min(4, Math.max(0.25, zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1)));
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }

  onMount(() => {
    dark.addEventListener('change', applyTheme);
    applyTheme();
    void (async () => {
      try {
        const [s, info] = await Promise.all([api.getSettings(), api.appInfo()]);
        themeSetting = s.theme;
        document.documentElement.dataset.accent = s.accent;
        if (s.reduceMotion) document.documentElement.dataset.reduceMotion = '';
        setLang(s.language, info.systemLanguage);
        applyTheme();
      } catch {
        /* defaults are fine */
      }
    })();
    return () => dark.removeEventListener('change', applyTheme);
  });
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="frame" data-tauri-drag-region {onwheel}>
  {#if src && !failed}
    <img {src} alt="" draggable="false" style:transform="scale({zoom})" onerror={() => (failed = true)} />
  {:else}
    <span class="missing" data-tauri-drag-region>{t('float.missing')}</span>
  {/if}
  <button type="button" class="x" tabindex="-1" aria-label={t('float.close')} title={t('float.close')} onclick={close}>
    <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M2 2l6 6M8 2L2 8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
  </button>
</div>

<style>
  .frame {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    overflow: hidden;
    border-radius: var(--radius-sm);
    background: var(--elevated-bg);
    box-shadow: inset 0 0 0 1px var(--glass-edge);
  }
  img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    pointer-events: none;
    transform-origin: center;
    transition: transform var(--dur-fast) var(--ease-out);
  }
  :global([data-reduce-motion]) img { transition: none; }
  .missing { color: var(--text-secondary); font: var(--text-callout); }
  .x {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--control-bg-active);
    color: var(--text);
    opacity: 0;
    cursor: default;
    transition: opacity var(--dur-fast);
  }
  .frame:hover .x { opacity: 1; }
  .x:hover { background: var(--red); color: var(--on-accent); }
</style>
