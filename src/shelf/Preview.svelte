<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { ClipItem } from '../lib/types';
  import { t } from './i18n';

  let { item, onclose }: { item: ClipItem; onclose: () => void } = $props();
  const src = $derived(item.edited ?? item.image);
  const linkLabel = $derived(item.linkTitle ?? item.title);
</script>

<div class="scrim" role="presentation" onclick={onclose}>
  <div class="box" role="dialog" aria-label={t('preview')}>
    {#if item.kind === 'image'}
      {#if src}<img src={convertFileSrc(src)} alt={item.title ?? t('image')} draggable="false" />{/if}
    {:else if item.kind === 'text'}
      <pre>{item.text}</pre>
    {:else}
      {#if linkLabel}<div class="title">{linkLabel}</div>{/if}
      <div class="url">{item.text}</div>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: var(--space-6);
    background: color-mix(in srgb, var(--window-bg) 60%, transparent);
    border-radius: inherit;
  }
  .box {
    max-width: 100%;
    max-height: 100%;
    overflow: auto;
    padding: var(--space-4);
    border-radius: var(--radius-lg);
    background: var(--elevated-bg);
    box-shadow: var(--shadow-lg);
    color: var(--text);
  }
  img { display: block; max-width: 100%; max-height: calc(100vh - 120px); border-radius: var(--radius-sm); }
  pre { margin: 0; font: var(--text-body); white-space: pre-wrap; word-break: break-word; }
  .title { font: var(--text-title); margin-bottom: var(--space-2); word-break: break-word; }
  .url { font: var(--text-body); color: var(--accent); word-break: break-all; }
</style>
