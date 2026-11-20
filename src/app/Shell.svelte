<!--
  Shell — the main window frame: title bar, sidebar, content card, global shortcuts, file drop, toasts.
  On mount it loads settings, applies the theme, then shows the (initially hidden) window after first paint.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { FileDown } from '@lucide/svelte';
  import { t } from '../lib/i18n';
  import { jobs } from '../lib/stores/jobs.svelte';
  import { nav, VIEWS } from '../lib/stores/nav.svelte';
  import { settings } from '../lib/stores/settings.svelte';
  import { applyTheme, watchSystemTheme } from '../lib/stores/theme';
  import { tools } from '../lib/stores/tools.svelte';
  import { modal, Toast } from '../lib/ui';
  import Sidebar from './Sidebar.svelte';
  import TitleBar from './TitleBar.svelte';
  import Clipboard from './views/clipboard/index.svelte';
  import Convert from './views/convert/index.svelte';
  import Download from './views/download/index.svelte';
  import Recent from './views/recent/index.svelte';
  import Settings from './views/settings/index.svelte';

  let width = $state(1000);
  let dragging = $state(false);
  const collapsed = $derived(width < 860);

  // Theme + accent follow the setting (and the OS while on "system").
  $effect(() => {
    const { theme, accent } = settings.value;
    applyTheme(theme, accent);
    if (theme !== 'system') return;
    return watchSystemTheme(() => applyTheme(theme, accent));
  });

  function onKeydown(e: KeyboardEvent) {
    if (!e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === ',') {
      e.preventDefault();
      nav.go('general');
    } else if (/^[1-5]$/.test(e.key)) {
      e.preventDefault();
      nav.go(VIEWS[Number(e.key) - 1]);
    }
  }

  onMount(() => {
    let disposed = false;
    const unlisten: (() => void)[] = [];

    (async () => {
      await settings.init();
      applyTheme(settings.value.theme, settings.value.accent);
      void nav.init();
      void jobs.init();
      void tools.init();
      // Two frames: the first paint with the final theme must land before the window becomes visible.
      await new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));
      getCurrentWindow().show().catch(() => {});
    })();
    // Safety net: never leave the window hidden if the backend is slow.
    const failsafe = setTimeout(() => getCurrentWindow().show().catch(() => {}), 2500);

    // CONTRACT: this is the ONLY onDragDropEvent listener in the main window. Views must not register
    // their own; they react to `nav.pending` via $effect (see nav.svelte.ts). Drops are ignored while
    // a modal Sheet is open.
    getCurrentWebview()
      .onDragDropEvent((e) => {
        const p = e.payload;
        if (modal.open > 0) {
          dragging = false;
          return;
        }
        if (p.type === 'enter') dragging = true;
        else if (p.type === 'leave') dragging = false;
        else if (p.type === 'drop') {
          dragging = false;
          if (p.paths.length > 0) nav.goto({ view: 'convert', paths: p.paths, url: null });
        }
      })
      .then((un) => (disposed ? un() : unlisten.push(un)))
      .catch(() => {});

    return () => {
      disposed = true;
      clearTimeout(failsafe);
      unlisten.forEach((u) => u());
    };
  });
</script>

<svelte:window bind:innerWidth={width} onkeydown={onKeydown} />

<div class="shell">
  <TitleBar />
  <Sidebar {collapsed} version={settings.appInfo?.version} />
  <main class="content">
    {#key nav.view}
      <div class="view">
        {#if nav.view === 'convert'}<Convert />
        {:else if nav.view === 'download'}<Download />
        {:else if nav.view === 'clipboard'}<Clipboard />
        {:else if nav.view === 'recent'}<Recent />
        {:else}<Settings />{/if}
      </div>
    {/key}
    {#if dragging}
      <div class="drop" aria-hidden="true">
        <FileDown size={30} strokeWidth={1.6} />
        <span>{t('common.dropOverlay')}</span>
      </div>
    {/if}
  </main>
  <Toast />
</div>

<style>
  .shell { position: relative; display: flex; width: 100vw; height: 100vh; overflow: hidden; background: var(--window-bg); }
  .content {
    position: relative; flex: 1; min-width: 0; margin: 8px 8px 8px 0; overflow: hidden; border-radius: var(--radius-lg);
    background: var(--content-bg); box-shadow: var(--shadow-sm), var(--glass-edge);
  }
  .view { position: absolute; inset: 0; padding-top: calc(var(--titlebar-h) - 8px); overflow: auto; animation: enter var(--dur-slow) var(--ease-out); }
  .drop {
    position: absolute; inset: 8px; z-index: 20; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px;
    border-radius: var(--radius-md); border: 2px dashed var(--accent); color: var(--accent); font: var(--text-headline);
    background: color-mix(in srgb, var(--accent) 10%, var(--content-bg)); backdrop-filter: blur(8px); animation: fade var(--dur) var(--ease-out);
  }
  @keyframes enter { from { opacity: 0; transform: translateY(6px); } }
  @keyframes fade { from { opacity: 0; } }
</style>
