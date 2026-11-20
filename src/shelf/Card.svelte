<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { ClipItem } from '../lib/types';
  import { t } from './i18n';

  let {
    item,
    cursor,
    selected,
    number,
    onclick,
  }: {
    item: ClipItem;
    cursor: boolean;
    selected: boolean;
    number: number | null;
    onclick: (e: MouseEvent) => void;
  } = $props();

  const thumb = $derived(item.editedThumb ?? item.thumb);
  const linkLabel = $derived(item.linkTitle ?? item.title);
  const link = $derived.by(() => {
    const raw = (item.text ?? '').trim();
    try {
      const u = new URL(raw);
      const rest = (u.pathname === '/' ? '' : u.pathname) + u.search;
      return { host: u.host.replace(/^www\./, ''), path: rest };
    } catch {
      return { host: raw, path: '' };
    }
  });
  const lines = $derived((item.text ?? '').split(/\r?\n/).filter((l) => l.trim()).slice(0, 6).join('\n'));

  function reveal(node: HTMLElement, active: boolean) {
    if (active) node.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    return {
      update(a: boolean) {
        if (a) node.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      },
    };
  }
</script>

<button type="button" class="card" class:cursor class:selected {onclick} use:reveal={cursor} tabindex="-1">
  {#if item.kind === 'image'}
    {#if thumb}
      <img src={convertFileSrc(thumb)} alt={item.title ?? t('image')} draggable="false" />
    {/if}
  {:else if item.kind === 'text'}
    <span class="text">{lines}</span>
  {:else}
    <span class="link">
      {#if linkLabel}<span class="link-title">{linkLabel}</span>{/if}
      <span class="link-url"><b>{link.host}</b>{link.path}</span>
    </span>
  {/if}
  {#if item.pinned}
    <span class="pin" title={t('pinned')}>
      <svg viewBox="0 0 16 16" width="11" height="11" aria-hidden="true"><path d="M5 1.5h6l-1 4.2 2.5 2.3v1H8.7V14L8 15l-.7-1V9H3.5V8L6 5.7 5 1.5z" fill="currentColor" /></svg>
    </span>
  {/if}
  {#if number !== null}<span class="num">{number}</span>{/if}
</button>

<style>
  .card {
    position: relative;
    flex: none;
    width: var(--card-w);
    height: var(--card-h);
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: var(--card-bg);
    color: var(--text);
    font: var(--text-callout);
    text-align: left;
    overflow: hidden;
    cursor: default;
    box-shadow: var(--shadow-sm);
    transition: background var(--dur-fast), box-shadow var(--dur-fast);
  }
  .card:hover { background: var(--card-bg-hover); }
  .card.selected { background: var(--accent-soft); box-shadow: inset 0 0 0 1.5px var(--accent); }
  .card.cursor { box-shadow: inset 0 0 0 2px var(--accent), var(--shadow-md); }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    pointer-events: none;
  }
  .text {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    overflow: hidden;
    padding: var(--space-3);
    white-space: pre-wrap;
    word-break: break-word;
    height: 100%;
  }
  .link {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: var(--space-1);
    height: 100%;
    padding: var(--space-3);
    overflow: hidden;
  }
  .link-title {
    font: var(--text-headline);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
    word-break: break-word;
  }
  .link-url {
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link-url b { color: var(--accent); font-weight: 500; }
  .pin {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
  }
  .num {
    position: absolute;
    left: 6px;
    bottom: 6px;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    display: grid;
    place-items: center;
    border-radius: var(--radius-xs);
    background: var(--elevated-bg);
    color: var(--text-secondary);
    font: var(--text-caption);
    box-shadow: var(--shadow-sm);
  }
</style>
