<!--
  Preview — a large in-window overlay for one clip (Space). Image fitted to the height, text as light
  Markdown, a link as title + domain + URL. Navigation and closing are handled by Line.svelte (keys).
  {@html} only ever receives renderMd output, which escapes everything first.
-->
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { ClipItem } from '../lib/types';
  import { renderMd } from './markdown';
  import { t } from './i18n';

  let { item, onclose }: { item: ClipItem; onclose: () => void } = $props();

  const file = $derived(item.edited ?? item.image ?? item.thumb);
  const html = $derived(item.kind === 'text' ? renderMd(item.text ?? '') : '');
  const host = $derived.by(() => {
    try {
      return new URL((item.text ?? '').trim()).host.replace(/^www\./, '');
    } catch {
      return '';
    }
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="preview" onclick={onclose} role="presentation">
  <div class="sheet" onclick={(e) => e.stopPropagation()} role="dialog" aria-label={t('preview')} tabindex="-1">
    {#if item.kind === 'image'}
      {#if file}<img src={convertFileSrc(file)} alt="" draggable="false" />{:else}<span class="none">{t('noPreview')}</span>{/if}
    {:else if item.kind === 'text'}
      <div class="md">{@html html}</div>
    {:else}
      <div class="link">
        {#if item.thumb}<img class="lt" src={convertFileSrc(item.thumb)} alt="" draggable="false" />{/if}
        <b>{item.linkTitle ?? item.title ?? host}</b>
        {#if host}<span class="host">{host}</span>{/if}
        <span class="url">{(item.text ?? '').trim()}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .preview {
    position: fixed;
    inset: 0;
    z-index: 8;
    display: grid;
    place-items: center;
    padding: 8px;
    background: color-mix(in srgb, var(--window-bg) 72%, transparent);
    backdrop-filter: blur(6px);
  }
  .sheet {
    display: flex;
    max-width: min(92vw, 900px);
    max-height: 100%;
    min-height: 0;
    border-radius: var(--radius-md);
    background: var(--elevated-bg);
    box-shadow: var(--shadow-lg), inset 0 0 0 1px var(--glass-edge);
    overflow: hidden;
  }
  .sheet img { display: block; max-width: 100%; max-height: calc(100vh - 16px); object-fit: contain; }
  .none { padding: var(--space-5); color: var(--text-secondary); }
  .md { padding: var(--space-3) var(--space-5); overflow: auto; font: var(--text-body); line-height: 1.45; user-select: text; }
  .md :global(h1), .md :global(h2), .md :global(h3) { margin: 0.2em 0; font-weight: 650; }
  .md :global(h1) { font-size: 1.35em; }
  .md :global(h2) { font-size: 1.2em; }
  .md :global(h3) { font-size: 1.08em; }
  .md :global(p) { margin: 0.25em 0; white-space: pre-wrap; word-break: break-word; }
  .md :global(ul) { margin: 0.25em 0; padding-left: 1.3em; }
  .md :global(code) { padding: 0 4px; border-radius: 4px; background: var(--control-bg); font-family: var(--font-mono, monospace); font-size: 0.92em; }
  .link { display: flex; flex-direction: column; gap: 4px; padding: var(--space-4) var(--space-5); max-width: 640px; }
  .link .lt { max-height: 90px; width: auto; border-radius: var(--radius-sm); margin-bottom: 4px; }
  .link b { font: var(--text-headline, var(--text-callout)); font-weight: 650; word-break: break-word; }
  .host { color: var(--accent); font-weight: 600; }
  .url { color: var(--text-secondary); font: var(--text-caption); word-break: break-all; user-select: text; }
</style>
