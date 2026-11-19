<!--
  Sidebar — translucent navigation column. Props: collapsed (icons only), version?: string.
  The header (below the traffic lights) is a drag region; nav buttons are not.
-->
<script lang="ts">
  import { Clipboard, Clock, Download, RefreshCw, Settings } from '@lucide/svelte';
  import appIcon from '../../src-tauri/icons/128x128.png';
  import type { Component } from 'svelte';
  import { t } from '../lib/i18n';
  import { jobs } from '../lib/stores/jobs.svelte';
  import { nav } from '../lib/stores/nav.svelte';
  import type { View } from '../lib/types';

  let { collapsed = false, version }: { collapsed?: boolean; version?: string } = $props();

  const items: { view: View; icon: Component<any> }[] = [
    { view: 'convert', icon: RefreshCw },
    { view: 'download', icon: Download },
    { view: 'clipboard', icon: Clipboard },
    { view: 'recent', icon: Clock },
    { view: 'settings', icon: Settings },
  ];
  const activeJobs = $derived(jobs.active.length);
</script>

<aside class="sidebar" class:collapsed>
  <div class="head" data-tauri-drag-region>
    <img class="logo" src={appIcon} alt="" width="28" height="28" draggable="false" />
    {#if !collapsed}<span class="name">{t('common.appName')}</span>{/if}
  </div>

  <nav aria-label={t('common.appName')}>
    {#each items as { view, icon: Icon } (view)}
      {@const selected = nav.view === view}
      <button type="button" class="item" class:selected aria-current={selected ? 'page' : undefined}
        title={collapsed ? t(`common.nav.${view}`) : undefined} onclick={() => nav.go(view)}>
        <Icon size={17} strokeWidth={selected ? 2.1 : 1.8} />
        {#if !collapsed}<span class="label">{t(`common.nav.${view}`)}</span>{/if}
        {#if view === 'convert' && activeJobs > 0}<span class="count" class:dot={collapsed}>{collapsed ? '' : activeJobs}</span>{/if}
      </button>
    {/each}
  </nav>

  {#if version && !collapsed}<div class="foot">v{version}</div>{/if}
</aside>

<style>
  .sidebar {
    display: flex; flex-direction: column; flex: none; width: var(--sidebar-w); height: 100%; padding-top: var(--titlebar-h);
    background: var(--sidebar-bg); box-shadow: inset -0.5px 0 0 var(--separator);
    transition: width var(--dur-slow) var(--ease-out);
  }
  .sidebar.collapsed { width: 72px; }
  .head { display: flex; align-items: center; gap: 9px; height: 36px; margin: 0 12px 4px; padding: 0 8px; }
  .collapsed .head { justify-content: center; padding: 0; }
  .head > * { pointer-events: none; }
  .logo { width: 28px; height: 28px; flex: none; object-fit: contain; }
  .name { font: var(--text-headline); letter-spacing: var(--tracking-tight); white-space: nowrap; }
  nav { display: flex; flex-direction: column; gap: 2px; padding: 4px 10px; }
  .item {
    position: relative; display: flex; align-items: center; gap: 10px; height: 32px; padding: 0 10px; border: 0; border-radius: 8px;
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
  .foot { margin-top: auto; padding: 12px 22px; font: var(--text-caption); color: var(--text-tertiary); }
  @keyframes pop { from { transform: scale(0.4); opacity: 0; } }
</style>
