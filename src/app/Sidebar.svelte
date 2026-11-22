<!--
  Sidebar — translucent navigation column. Props: collapsed (icons only), version?: string.
  Top: logo + name (a drag region). Middle: the main views, a "Settings" caption (a thin separator
  when collapsed) and the settings pages; the list scrolls when the window is short.
  Bottom: GitHub button and the version.
-->
<script lang="ts">
  import {
    CircleDot, Clipboard, ClipboardList, Clock, Cpu, Download, FolderSync, GitBranch, Info, Keyboard, Palette,
    RefreshCw, Settings2, SlidersHorizontal, Spline,
  } from '@lucide/svelte';
  import appIcon from '../../src-tauri/icons/128x128.png';
  import type { Component } from 'svelte';
  import { t } from '../lib/i18n';
  import { api } from '../lib/ipc';
  import { guard } from '../lib/ui/guard';
  import { jobs } from '../lib/stores/jobs.svelte';
  import { nav, NAV_MAIN, NAV_SETTINGS } from '../lib/stores/nav.svelte';
  import type { View } from '../lib/types';

  let { collapsed = false, version }: { collapsed?: boolean; version?: string } = $props();

  const REPO_URL = 'https://github.com/zarlof31/file-chameleon';
  const icons: Partial<Record<View, Component<any>>> = {
    convert: RefreshCw,
    download: Download,
    clipboard: Clipboard,
    recent: Clock,
    general: Settings2,
    wheel: CircleDot,
    presets: SlidersHorizontal,
    shortcuts: Keyboard,
    watch: FolderSync,
    clipboardSettings: ClipboardList,
    line: Spline,
    appearance: Palette,
    engines: Cpu,
    about: Info,
  };
  const activeJobs = $derived(jobs.active.length);
</script>

{#snippet item(view: View)}
  {@const Icon = icons[view] ?? Info}
  {@const selected = nav.view === view}
  <button type="button" class="item" class:selected aria-current={selected ? 'page' : undefined}
    title={collapsed ? t(`common.nav.${view}`) : undefined} onclick={() => nav.go(view)}>
    <Icon size={17} strokeWidth={selected ? 2.1 : 1.8} />
    {#if !collapsed}<span class="label">{t(`common.nav.${view}`)}</span>{/if}
    {#if view === 'convert' && activeJobs > 0}<span class="count" class:dot={collapsed}>{collapsed ? '' : activeJobs}</span>{/if}
  </button>
{/snippet}

<aside class="sidebar" class:collapsed>
  <div class="head" data-tauri-drag-region>
    <img class="logo" src={appIcon} alt="" width="28" height="28" draggable="false" />
    {#if !collapsed}<span class="name">{t('common.appName')}</span>{/if}
  </div>

  <nav aria-label={t('common.appName')}>
    {#each NAV_MAIN as view (view)}{@render item(view)}{/each}
    {#if collapsed}
      <hr class="sep" />
    {:else}
      <div class="caption">{t('common.nav.settings')}</div>
    {/if}
    {#each NAV_SETTINGS as view (view)}{@render item(view)}{/each}
  </nav>

  <div class="foot">
    <button type="button" class="item gh" title={t('common.sidebar.githubHint')} aria-label={t('common.sidebar.githubHint')}
      onclick={() => guard(() => api.openPath(REPO_URL))}>
      <GitBranch size={16} strokeWidth={1.8} />
      {#if !collapsed}<span class="label">{t('common.sidebar.github')}</span>{/if}
    </button>
    {#if version && !collapsed}<span class="ver">v{version}</span>{/if}
  </div>
</aside>

<style>
  .sidebar {
    display: flex; flex-direction: column; flex: none; width: var(--sidebar-w); height: 100%; padding-top: 8px;
    background: var(--sidebar-bg); box-shadow: inset -0.5px 0 0 var(--separator);
    transition: width var(--dur-slow) var(--ease-out);
  }
  .sidebar.collapsed { width: 72px; }
  .head { display: flex; align-items: center; gap: 9px; flex: none; height: 40px; margin: 0 12px 6px; padding: 0 8px; }
  .collapsed .head { justify-content: center; padding: 0; }
  .head > * { pointer-events: none; }
  .logo { width: 28px; height: 28px; flex: none; object-fit: contain; }
  .name { font: var(--text-headline); letter-spacing: var(--tracking-tight); white-space: nowrap; }
  nav { display: flex; flex-direction: column; gap: 2px; flex: 1; min-height: 0; overflow-x: hidden; overflow-y: auto; padding: 4px 10px 8px; }
  .caption {
    flex: none; margin: 14px 10px 4px; font: var(--text-caption); font-size: 11px; letter-spacing: 0.04em; text-transform: uppercase;
    color: var(--text-tertiary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .sep { flex: none; width: 28px; margin: 10px auto; border: 0; border-top: 1px solid var(--separator); }
  .item {
    position: relative; display: flex; align-items: center; flex: none; gap: 10px; height: 32px; padding: 0 10px; border: 0; border-radius: 8px;
    background: transparent; color: var(--text); font: var(--text-body); text-align: left; cursor: default;
    transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast), transform var(--dur-fast) var(--ease-out);
  }
  .collapsed .item { justify-content: center; padding: 0; }
  .item :global(svg) { flex: none; color: var(--text-secondary); transition: color var(--dur-fast); }
  .item:hover:not(.selected) { background: var(--control-bg); }
  .item:active { transform: scale(0.98); }
  .item.selected { background: var(--accent-soft); color: var(--accent); font-weight: 500; }
  .item.selected :global(svg) { color: var(--accent); }
  .label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px; background: var(--accent); color: var(--on-accent); font: var(--text-caption); line-height: 18px; text-align: center; animation: pop var(--dur) var(--ease-spring); }
  .count.dot { position: absolute; top: 6px; right: 14px; min-width: 8px; width: 8px; height: 8px; padding: 0; }
  .foot { display: flex; align-items: center; gap: 8px; flex: none; padding: 8px 10px 12px; box-shadow: inset 0 0.5px 0 var(--separator); }
  .collapsed .foot { justify-content: center; }
  .gh { flex: 1; min-width: 0; height: 30px; color: var(--text-secondary); }
  .collapsed .gh { flex: none; width: 36px; }
  .ver { flex: none; padding-right: 8px; font: var(--text-caption); color: var(--text-tertiary); font-variant-numeric: tabular-nums; }
  @keyframes pop { from { transform: scale(0.4); opacity: 0; } }
</style>
