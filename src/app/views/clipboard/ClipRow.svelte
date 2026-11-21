<!--
  ClipRow — one item in the list view (fixed height: the slot is ROW_H).
  Props: item, selected, focused, when (relative time), folder (name or null), trash (shown in the trash),
         onpick(e) (click: select), onopen() (double-click), onquick(action) (hover buttons).
-->
<script lang="ts">
  import { Copy, FileText, Folder, Link, Pin, PinOff, RotateCcw, Trash2 } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import type { ClipItem } from '../../../lib/types';
  import { Badge, IconButton } from '../../../lib/ui';
  import { clipLabel, type QuickAction } from './clips';
  import { thumbUrl } from './ui';

  interface Props {
    item: ClipItem;
    selected: boolean;
    focused: boolean;
    when: string;
    folder: string | null;
    trash: boolean;
    onpick: (e: MouseEvent) => void;
    onopen: () => void;
    onquick: (action: QuickAction) => void;
  }
  let { item, selected, focused, when, folder, trash, onpick, onopen, onquick }: Props = $props();

  const isLink = $derived(item.kind === 'link' || item.kind === 'mediaLink');
  const thumb = $derived(item.kind === 'image' ? thumbUrl(item) : null);
  const label = $derived(clipLabel(item));
  /** A title (or link title) replaces the text on the first line; the text then moves to the second. */
  const titled = $derived(!!item.title || (isLink && !!item.linkTitle));
  const sub = $derived(
    item.kind === 'image'
      ? t('clipboard.imageSize', { width: item.width ?? 0, height: item.height ?? 0 })
      : titled
        ? (item.text?.trim().split(/\r?\n/)[0] ?? '')
        : '',
  );

  function quick(e: MouseEvent, action: QuickAction) {
    e.stopPropagation();
    onquick(action);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
<div class="row" class:selected class:focused id="clip-{item.id}" role="option" aria-selected={selected}
  onclick={onpick} ondblclick={onopen}>
  <div class="lead">
    {#if thumb}
      <img src={thumb} alt="" loading="lazy" draggable="false" />
    {:else}
      <span class="glyph">{#if isLink}<Link size={16} />{:else}<FileText size={16} />{/if}</span>
    {/if}
  </div>

  <div class="main">
    <span class="title" class:url={isLink && !titled}>{label ?? (item.kind === 'image' ? t('clipboard.kind.image') : item.text)}</span>
    <span class="meta">
      {#if item.pinned}<Pin size={11} />{/if}
      {#if item.kind === 'mediaLink' && item.site}<Badge tone="accent">{item.site}</Badge>{/if}
      {#if sub}<span class="sub" class:url={isLink}>{sub}</span>{/if}
      {#if folder}<span class="folder"><Folder size={11} />{folder}</span>{/if}
      {#each item.tags.slice(0, 2) as tag (tag)}<span class="tag">#{tag}</span>{/each}
      <span class="when">{when}</span>
    </span>
  </div>

  <div class="tools">
    {#if trash}
      <IconButton label={t('clipboard.action.restore')} onclick={(e) => quick(e, 'restore')}><RotateCcw size={15} /></IconButton>
    {:else}
      <IconButton label={t('clipboard.action.copy')} onclick={(e) => quick(e, 'copy')}><Copy size={15} /></IconButton>
      <IconButton label={item.pinned ? t('clipboard.action.unpin') : t('clipboard.action.pin')} active={item.pinned} onclick={(e) => quick(e, 'pin')}>
        {#if item.pinned}<PinOff size={15} />{:else}<Pin size={15} />{/if}
      </IconButton>
      <IconButton label={t('clipboard.action.trash')} onclick={(e) => quick(e, 'trash')}><Trash2 size={15} /></IconButton>
    {/if}
  </div>
</div>

<style>
  .row {
    display: flex; align-items: center; gap: var(--space-3); height: 60px; box-sizing: border-box; overflow: hidden;
    padding: 0 var(--space-2) 0 var(--space-2); border-radius: var(--radius-md); transition: background var(--dur-fast) var(--ease-out);
  }
  .row:hover { background: var(--control-bg); }
  .row.selected { background: var(--accent-soft); }
  .row.focused { box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 70%, transparent); }
  .lead { flex: none; width: 44px; height: 44px; display: grid; place-items: center; }
  .lead img { max-width: 44px; max-height: 44px; border-radius: var(--radius-sm); object-fit: cover; background: var(--control-bg); box-shadow: 0 0 0 0.5px var(--separator); }
  .glyph { display: grid; place-items: center; width: 36px; height: 36px; border-radius: var(--radius-sm); background: var(--control-bg); color: var(--text-secondary); }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .title { font: var(--text-body); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .url { color: var(--accent); }
  .meta { display: flex; align-items: center; gap: var(--space-2); min-width: 0; font: var(--text-caption); color: var(--text-secondary); white-space: nowrap; }
  .meta :global(svg) { flex: none; }
  .sub { min-width: 0; overflow: hidden; text-overflow: ellipsis; font-weight: 400; }
  .folder { display: inline-flex; align-items: center; gap: 3px; }
  .tag { color: var(--text-tertiary); }
  .when { margin-left: auto; flex: none; color: var(--text-tertiary); }
  .tools { flex: none; display: flex; gap: 2px; opacity: 0; transition: opacity var(--dur-fast); }
  .row:hover .tools, .row.focused .tools { opacity: 1; }
</style>
