<!--
  ClipCard — one item in the grid view (fixed height: the row slot is GRID_ROW_H).
  Props: item, selected, focused, when (relative time), trash, onpick(e), onopen(), onquick(action).
-->
<script lang="ts">
  import { Copy, Link, Pin, PinOff, RotateCcw, Trash2 } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import type { ClipItem } from '../../../lib/types';
  import { IconButton } from '../../../lib/ui';
  import { clipLabel, linkHost, type QuickAction } from './clips';
  import { thumbUrl } from './ui';

  interface Props {
    item: ClipItem;
    selected: boolean;
    focused: boolean;
    when: string;
    trash: boolean;
    onpick: (e: MouseEvent) => void;
    onopen: () => void;
    onquick: (action: QuickAction) => void;
  }
  let { item, selected, focused, when, trash, onpick, onopen, onquick }: Props = $props();

  const isLink = $derived(item.kind === 'link' || item.kind === 'mediaLink');
  const thumb = $derived(item.kind === 'image' ? thumbUrl(item) : null);
  const label = $derived(clipLabel(item) ?? (item.kind === 'image' ? t('clipboard.imageSize', { width: item.width ?? 0, height: item.height ?? 0 }) : ''));

  function quick(e: MouseEvent, action: QuickAction) {
    e.stopPropagation();
    onquick(action);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
<div class="card" class:selected class:focused id="clip-{item.id}" role="option" aria-selected={selected} onclick={onpick} ondblclick={onopen}>
  <div class="face">
    {#if item.kind === 'image'}
      {#if thumb}<img src={thumb} alt="" loading="lazy" draggable="false" />{/if}
    {:else if isLink}
      <div class="link">
        <Link size={18} />
        <span class="host">{item.site ?? linkHost(item.text ?? '') ?? ''}</span>
      </div>
    {:else}
      <p class="text">{item.text}</p>
    {/if}
    <span class="tools">
      {#if trash}
        <IconButton label={t('clipboard.action.restore')} size={24} onclick={(e) => quick(e, 'restore')}><RotateCcw size={13} /></IconButton>
      {:else}
        <IconButton label={t('clipboard.action.copy')} size={24} onclick={(e) => quick(e, 'copy')}><Copy size={13} /></IconButton>
        <IconButton label={item.pinned ? t('clipboard.action.unpin') : t('clipboard.action.pin')} size={24} onclick={(e) => quick(e, 'pin')}>
          {#if item.pinned}<PinOff size={13} />{:else}<Pin size={13} />{/if}
        </IconButton>
        <IconButton label={t('clipboard.action.trash')} size={24} onclick={(e) => quick(e, 'trash')}><Trash2 size={13} /></IconButton>
      {/if}
    </span>
  </div>
  <div class="foot">
    <span class="label" class:url={isLink && !item.title && !item.linkTitle}>{label}</span>
    <span class="when">{#if item.pinned}<Pin size={10} />{/if}{when}</span>
  </div>
</div>

<style>
  .card {
    display: flex; flex-direction: column; height: 172px; overflow: hidden; border-radius: var(--radius-md);
    background: var(--card-bg); box-shadow: var(--shadow-sm); transition: box-shadow var(--dur-fast), background var(--dur-fast);
  }
  .card:hover { background: var(--card-bg-hover); }
  .card.selected { box-shadow: 0 0 0 2px var(--accent), var(--shadow-sm); }
  .card.focused:not(.selected) { box-shadow: 0 0 0 1.5px color-mix(in srgb, var(--accent) 60%, transparent), var(--shadow-sm); }
  .face { position: relative; flex: 1; min-height: 0; display: grid; place-items: center; background: var(--control-bg); overflow: hidden; }
  .face img { width: 100%; height: 100%; object-fit: cover; }
  .text {
    align-self: start; justify-self: stretch; margin: 0; padding: 10px 12px; font: var(--text-callout); color: var(--text);
    display: -webkit-box; -webkit-line-clamp: 6; line-clamp: 6; -webkit-box-orient: vertical; overflow: hidden; overflow-wrap: anywhere; white-space: pre-wrap;
  }
  .link { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 0 12px; color: var(--accent); }
  .host { max-width: 100%; font: var(--text-callout); color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tools {
    position: absolute; top: 4px; right: 4px; display: flex; gap: 1px; padding: 1px; border-radius: var(--radius-sm);
    background: var(--elevated-bg); box-shadow: var(--shadow-sm); opacity: 0; transition: opacity var(--dur-fast);
  }
  .card:hover .tools, .card.focused .tools { opacity: 1; }
  .foot { flex: none; display: flex; flex-direction: column; gap: 2px; padding: 7px 10px 8px; }
  .label { font: var(--text-callout); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .label.url { color: var(--accent); font-weight: 400; }
  .when { display: flex; align-items: center; gap: 4px; font: var(--text-caption); color: var(--text-tertiary); }
</style>
