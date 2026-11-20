<!--
  ClipRow — one clipboard history entry.
  Props: item, when (relative time label), oncopy / onpin / ondelete / ondownload / onconvert / onsave(format).
  Enter or double-click copies, Delete removes (only when the row itself has focus).
-->
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { Copy, Download, FileText, Link, Pin, PinOff, RefreshCw, Trash2 } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import type { ClipItem } from '../../../lib/types';
  import { Badge, Button, IconButton } from '../../../lib/ui';

  interface Props {
    item: ClipItem;
    when: string;
    oncopy: () => void;
    onpin: () => void;
    ondelete: () => void;
    ondownload: () => void;
    onconvert: () => void;
    onsave: (format: 'png' | 'jpg' | 'webp') => void;
  }
  let { item, when, oncopy, onpin, ondelete, ondownload, onconvert, onsave }: Props = $props();

  const isLink = $derived(item.kind === 'link' || item.kind === 'mediaLink');
  const thumb = $derived(item.thumb ? convertFileSrc(item.thumb) : null);
  const formats = [
    { id: 'png', label: 'PNG' },
    { id: 'jpg', label: 'JPG' },
    { id: 'webp', label: 'WebP' },
  ] as const;

  function key(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      oncopy();
    } else if (e.key === 'Delete') {
      e.preventDefault();
      ondelete();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="row" class:pinned={item.pinned} role="group" tabindex="0" aria-label={item.text?.slice(0, 80) ?? when}
  ondblclick={oncopy} onkeydown={key}>
  <div class="lead">
    {#if item.kind === 'image' && thumb}
      <img src={thumb} alt="" loading="lazy" draggable="false" />
    {:else if isLink}
      <span class="glyph"><Link size={18} /></span>
    {:else}
      <span class="glyph"><FileText size={18} /></span>
    {/if}
  </div>

  <div class="main">
    {#if item.kind === 'image'}
      <span class="preview">{t('clipboard.imageSize', { width: item.width ?? 0, height: item.height ?? 0 })}</span>
    {:else}
      <span class="preview" class:url={isLink}>{item.text}</span>
    {/if}
    <span class="meta">
      {#if item.site}<Badge tone={item.kind === 'mediaLink' ? 'accent' : 'neutral'}>{item.site}</Badge>{/if}
      <span>{when}</span>
    </span>
    {#if item.kind === 'mediaLink'}
      <span class="actions">
        <Button size="sm" variant="primary" onclick={ondownload}>
          {#snippet icon()}<Download size={13} />{/snippet}
          {t('clipboard.action.download')}
        </Button>
      </span>
    {:else if item.kind === 'image'}
      <span class="actions">
        {#each formats as f (f.id)}
          <Button size="sm" onclick={() => onsave(f.id)}>{t('clipboard.action.save', { format: f.label })}</Button>
        {/each}
        <Button size="sm" variant="plain" onclick={onconvert}>
          {#snippet icon()}<RefreshCw size={13} />{/snippet}
          {t('clipboard.action.convert')}
        </Button>
      </span>
    {/if}
  </div>

  <div class="tools">
    <IconButton label={t('clipboard.action.copy')} onclick={oncopy}><Copy size={15} /></IconButton>
    <IconButton label={item.pinned ? t('clipboard.action.unpin') : t('clipboard.action.pin')} active={item.pinned} onclick={onpin}>
      {#if item.pinned}<PinOff size={15} />{:else}<Pin size={15} />{/if}
    </IconButton>
    <IconButton label={t('clipboard.action.delete')} onclick={ondelete}><Trash2 size={15} /></IconButton>
  </div>
</div>

<style>
  .row {
    display: flex; align-items: flex-start; gap: var(--space-3); padding: 10px var(--space-3); border-radius: var(--radius-md);
    outline: 0; transition: background var(--dur-fast) var(--ease-out);
  }
  .row:hover, .row:focus-visible { background: var(--control-bg); }
  .row:focus-visible { box-shadow: 0 0 0 2px var(--accent-soft), inset 0 0 0 1px var(--accent); }
  .row.pinned { background: var(--accent-soft); }
  .lead { flex: none; width: 48px; height: 48px; display: grid; place-items: center; }
  .lead img { max-width: 48px; max-height: 48px; border-radius: var(--radius-sm); object-fit: contain; background: var(--control-bg); box-shadow: 0 0 0 0.5px var(--separator); }
  .glyph { display: grid; place-items: center; width: 36px; height: 36px; border-radius: var(--radius-sm); background: var(--control-bg); color: var(--text-secondary); }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .preview {
    font: var(--text-body); display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical;
    overflow: hidden; overflow-wrap: anywhere; white-space: pre-wrap;
  }
  .preview.url { color: var(--accent); }
  .meta { display: flex; align-items: center; gap: var(--space-2); font: var(--text-caption); color: var(--text-secondary); }
  .actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 2px; }
  .tools { flex: none; display: flex; gap: 2px; opacity: 0; transition: opacity var(--dur-fast); }
  .row:hover .tools, .row:focus-within .tools { opacity: 1; }
</style>
