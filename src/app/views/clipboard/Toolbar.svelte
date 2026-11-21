<!--
  Toolbar — search, grid/list switch, inspector toggle and, while items are selected, batch actions
  (copy, pin, move to folder, tag, export, trash; restore / delete in the trash).
  Props: search (bindable), mode (bindable 'grid' | 'list'), inspector (shown), selected (items, display order),
         trash (the trash is shown), folders, tags, onsearch(), onbatch(action), oninspector(show).
  Exports: focusSearch() (Ctrl+F).
-->
<script lang="ts">
  import {
    Check, Copy, FolderInput, FolderPlus, Hash, LayoutGrid, List, PanelRight, Pin, PinOff, Plus, RotateCcw, Search, Share, Trash2, X,
  } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import type { ClipFolder, ClipItem, ClipTag } from '../../../lib/types';
  import { Button, IconButton, TextField } from '../../../lib/ui';
  import { fold, folderTree, tagState, type BatchAction } from './clips';
  import Popover from './Popover.svelte';

  interface Props {
    search: string;
    mode: 'grid' | 'list';
    inspector: boolean;
    selected: ClipItem[];
    trash: boolean;
    folders: ClipFolder[];
    tags: ClipTag[];
    onsearch: () => void;
    onbatch: (a: BatchAction) => void;
    oninspector: (show: boolean) => void;
  }
  let { search = $bindable(), mode = $bindable(), inspector, selected, trash, folders, tags, onsearch, onbatch, oninspector }: Props = $props();

  let searchBox: HTMLElement | undefined = $state();
  let newFolder = $state('');
  let newTag = $state('');

  const count = $derived(selected.length);
  const allPinned = $derived(count > 0 && selected.every((i) => i.pinned));
  const tree = $derived(folderTree(folders));
  /** Existing tags plus the tags of the selection (which may not be counted yet). */
  const tagNames = $derived.by(() => {
    const names = tags.map((x) => x.tag);
    for (const i of selected) for (const tg of i.tags) if (!names.some((n) => fold(n) === fold(tg))) names.push(tg);
    return names;
  });

  export function focusSearch() {
    const input = searchBox?.querySelector('input');
    input?.focus();
    input?.select();
  }

  function searchKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && search) {
      e.stopPropagation();
      search = '';
      onsearch();
    }
  }

  function addTag(close: () => void) {
    const tag = newTag.trim();
    if (!tag) return;
    newTag = '';
    onbatch({ type: 'tag', add: [tag], remove: [] });
    close();
  }

  function moveNew(close: () => void) {
    const name = newFolder.trim();
    if (!name) return;
    newFolder = '';
    onbatch({ type: 'moveNew', name });
    close();
  }
</script>

<div class="bar">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="search" bind:this={searchBox} onkeydown={searchKey}>
    <TextField bind:value={search} type="search" placeholder={t('clipboard.search')} oninput={onsearch}>
      {#snippet leading()}<Search size={14} />{/snippet}
    </TextField>
  </div>

  <!-- One item with the inspector open: the inspector has the actions, the bar is for batches. -->
  {#if count > 1 || (count === 1 && !inspector)}
    <div class="batch" role="toolbar" aria-label={t('clipboard.batch.label')}>
      <span class="n">{t('clipboard.batch.selected', { count })}</span>
      {#if trash}
        <Button size="sm" variant="plain" onclick={() => onbatch({ type: 'restore' })}>
          {#snippet icon()}<RotateCcw size={13} />{/snippet}{t('clipboard.action.restore')}
        </Button>
        <Button size="sm" variant="plain" onclick={() => onbatch({ type: 'delete' })}>
          {#snippet icon()}<Trash2 size={13} />{/snippet}{t('clipboard.action.deleteForever')}
        </Button>
      {:else}
        <IconButton label={t('clipboard.action.copy')} onclick={() => onbatch({ type: 'copy' })}><Copy size={15} /></IconButton>
        <IconButton label={allPinned ? t('clipboard.action.unpin') : t('clipboard.action.pin')} active={allPinned}
          onclick={() => onbatch({ type: 'pin', pinned: !allPinned })}>
          {#if allPinned}<PinOff size={15} />{:else}<Pin size={15} />{/if}
        </IconButton>

        <Popover label={t('clipboard.batch.move')} up>
          {#snippet icon()}<FolderInput size={13} />{/snippet}
          {#snippet children(close)}
            <button type="button" class="opt" onclick={() => { onbatch({ type: 'move', folder: null }); close(); }}>
              <X size={13} /><span>{t('clipboard.folder.none')}</span>
            </button>
            {#each tree as node (node.folder.id)}
              <button type="button" class="opt" style:padding-left="{8 + node.depth * 12}px"
                onclick={() => { onbatch({ type: 'move', folder: node.folder.id }); close(); }}>
                {#if selected.every((i) => i.folder === node.folder.id)}<Check size={13} />{:else}<span class="sp"></span>{/if}
                <span>{node.folder.name}</span>
              </button>
            {/each}
            <div class="new">
              <TextField bind:value={newFolder} placeholder={t('clipboard.folder.namePlaceholder')} label={t('clipboard.folder.new')}
                onenter={() => moveNew(close)} />
              <IconButton label={t('clipboard.batch.moveNew')} disabled={!newFolder.trim()} onclick={() => moveNew(close)}><FolderPlus size={14} /></IconButton>
            </div>
          {/snippet}
        </Popover>

        <Popover label={t('clipboard.batch.tag')} up>
          {#snippet icon()}<Hash size={13} />{/snippet}
          {#snippet children(close)}
            {#each tagNames as name (name)}
              {@const state = tagState(selected, name)}
              <button type="button" class="opt" aria-pressed={state === 'all'}
                onclick={() => onbatch(state === 'all' ? { type: 'tag', add: [], remove: [name] } : { type: 'tag', add: [name], remove: [] })}>
                {#if state === 'all'}<Check size={13} />{:else if state === 'some'}<span class="dash">–</span>{:else}<span class="sp"></span>{/if}
                <span>{name}</span>
              </button>
            {/each}
            <div class="new">
              <TextField bind:value={newTag} placeholder={t('clipboard.tags.placeholder')} label={t('clipboard.tags.add')} onenter={() => addTag(close)} />
              <IconButton label={t('clipboard.tags.add')} disabled={!newTag.trim()} onclick={() => addTag(close)}><Plus size={14} /></IconButton>
            </div>
          {/snippet}
        </Popover>

        <IconButton label={t('clipboard.batch.export')} onclick={() => onbatch({ type: 'export' })}><Share size={15} /></IconButton>
        <IconButton label={t('clipboard.action.trash')} onclick={() => onbatch({ type: 'trash' })}><Trash2 size={15} /></IconButton>
      {/if}
      <IconButton label={t('clipboard.batch.deselect')} onclick={() => onbatch({ type: 'deselect' })}><X size={15} /></IconButton>
    </div>
  {/if}

  <div class="end">
    <IconButton label={t('clipboard.view.list')} active={mode === 'list'} onclick={() => (mode = 'list')}><List size={15} /></IconButton>
    <IconButton label={t('clipboard.view.grid')} active={mode === 'grid'} onclick={() => (mode = 'grid')}><LayoutGrid size={15} /></IconButton>
    <span class="vsep"></span>
    <IconButton label={t('clipboard.view.inspector')} active={inspector} onclick={() => oninspector(!inspector)}><PanelRight size={15} /></IconButton>
  </div>
</div>

<style>
  .bar { display: flex; align-items: center; gap: var(--space-2); flex: none; min-height: 40px; padding-bottom: var(--space-2); }
  .search { flex: 1 1 160px; min-width: 120px; max-width: 360px; }
  /* Floats over the bottom of the content (the parent's .main is the positioned box): no layout jump on select. */
  .batch {
    position: absolute; left: 50%; bottom: var(--space-4); z-index: 20; display: flex; align-items: center; gap: 2px;
    max-width: calc(100% - 16px); padding: 4px 4px 4px 14px; border-radius: 99px; transform: translateX(-50%);
    background: var(--elevated-bg); box-shadow: var(--shadow-lg), var(--glass-edge); backdrop-filter: blur(24px) saturate(1.6);
    animation: rise var(--dur) var(--ease-out);
  }
  .n { margin-right: 6px; font: var(--text-headline); white-space: nowrap; }
  .end { display: flex; align-items: center; gap: 2px; margin-left: auto; flex: none; }
  .vsep { width: 0.5px; height: 18px; margin: 0 4px; background: var(--separator); }
  .opt {
    display: flex; align-items: center; gap: 8px; width: 100%; height: 28px; padding: 0 8px; border: 0; border-radius: var(--radius-sm);
    background: transparent; color: var(--text); font: var(--text-body); text-align: left; cursor: default; white-space: nowrap;
  }
  .opt span:last-child { overflow: hidden; text-overflow: ellipsis; }
  .opt:hover { background: var(--accent); color: var(--on-accent); }
  .opt :global(svg) { flex: none; }
  .sp, .dash { display: inline-block; width: 13px; flex: none; text-align: center; }
  .new { display: flex; align-items: center; gap: 4px; margin-top: 4px; padding-top: 6px; border-top: 0.5px solid var(--separator); }
  .new :global(.field) { flex: 1; }
  @keyframes rise { from { opacity: 0; transform: translate(-50%, 8px); } }
</style>
