<!--
  FilterColumn — the library's left column: kinds, pinned, folders (create / rename / delete), tags, trash.
  Props: filter (current), folders, tags, stats (counts; null = unknown), onselect(filter),
         onchanged() (a folder was created, renamed or deleted: refetch).
-->
<script lang="ts">
  import { FileText, Folder, FolderPlus, Hash, Image, Layers, Link, Pencil, Pin, Trash2, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { api } from '../../../lib/ipc';
  import type { ClipFolder, ClipStats, ClipTag } from '../../../lib/types';
  import { Button, IconButton, Sheet } from '../../../lib/ui';
  import { filterKey, folderTree, type LibraryFilter } from './clips';
  import { guard } from './ui';

  interface Props {
    filter: LibraryFilter;
    folders: ClipFolder[];
    tags: ClipTag[];
    stats: ClipStats | null;
    onselect: (f: LibraryFilter) => void;
    onchanged: () => void;
  }
  let { filter, folders, tags, stats, onselect, onchanged }: Props = $props();

  const tree = $derived(folderTree(folders));
  const active = $derived(filterKey(filter));

  const kinds = [
    { f: { type: 'all' } as LibraryFilter, key: 'all', icon: Layers },
    { f: { type: 'shelf', shelf: 'images' } as LibraryFilter, key: 'images', icon: Image },
    { f: { type: 'shelf', shelf: 'texts' } as LibraryFilter, key: 'texts', icon: FileText },
    { f: { type: 'shelf', shelf: 'links' } as LibraryFilter, key: 'links', icon: Link },
    { f: { type: 'pinned' } as LibraryFilter, key: 'pinned', icon: Pin },
  ];

  let creating = $state(false);
  let draft = $state('');
  let renaming = $state<number | null>(null);
  let deleting = $state<ClipFolder | null>(null);
  let deleteOpen = $state(false);

  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function startCreate() {
    renaming = null;
    draft = '';
    creating = true;
  }

  function startRename(f: ClipFolder) {
    creating = false;
    draft = f.name;
    renaming = f.id;
  }

  /** Enter and blur both end here; whichever comes second finds nothing to do. */
  async function commit() {
    if (!creating && renaming === null) return;
    const name = draft.trim();
    const id = renaming;
    const wasCreating = creating;
    creating = false;
    renaming = null;
    if (!name) return;
    if (wasCreating) {
      await guard(async () => {
        const f = await api.clipFolderCreate(name);
        onchanged();
        onselect({ type: 'folder', id: f.id });
      });
    } else if (id !== null && folders.find((f) => f.id === id)?.name !== name) {
      await guard(async () => {
        await api.clipFolderRename(id, name);
        onchanged();
      });
    }
  }

  function inputKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      void commit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      creating = false;
      renaming = null;
    }
  }

  function askDelete(f: ClipFolder) {
    deleting = f;
    deleteOpen = true;
  }

  async function confirmDelete() {
    const f = deleting;
    deleteOpen = false;
    if (!f) return;
    await guard(async () => {
      await api.clipFolderDelete(f.id);
      if (active === `folder:${f.id}`) onselect({ type: 'all' });
      onchanged();
    });
  }
</script>

<nav class="col" aria-label={t('clipboard.filters.label')}>
  {#each kinds as k (k.key)}
    <button type="button" class="item" class:on={active === filterKey(k.f)} onclick={() => onselect(k.f)}>
      <k.icon size={15} />
      <span class="name">{t(`clipboard.filters.${k.key}`)}</span>
      {#if k.key === 'all' && stats}<span class="count">{stats.items}</span>{/if}
    </button>
  {/each}

  <div class="cap">
    <span>{t('clipboard.filters.folders')}</span>
    <IconButton label={t('clipboard.folder.new')} size={22} onclick={startCreate}><FolderPlus size={13} /></IconButton>
  </div>
  {#if creating}
    <input class="edit" bind:value={draft} placeholder={t('clipboard.folder.namePlaceholder')} aria-label={t('clipboard.folder.new')}
      maxlength="80" use:focusSelect onkeydown={inputKey} onblur={commit} />
  {/if}
  {#each tree as node (node.folder.id)}
    {#if renaming === node.folder.id}
      <input class="edit" style:margin-left="{node.depth * 12}px" bind:value={draft} aria-label={t('clipboard.folder.rename')}
        maxlength="80" use:focusSelect onkeydown={inputKey} onblur={commit} />
    {:else}
      <div class="item folder" class:on={active === `folder:${node.folder.id}`} style:padding-left="{8 + node.depth * 12}px">
        <button type="button" class="hit" onclick={() => onselect({ type: 'folder', id: node.folder.id })}
          ondblclick={() => startRename(node.folder)}>
          <Folder size={15} />
          <span class="name" title={node.folder.name}>{node.folder.name}</span>
        </button>
        <span class="count">{node.folder.count}</span>
        <span class="tools">
          <IconButton label={t('clipboard.folder.rename')} size={22} onclick={() => startRename(node.folder)}><Pencil size={12} /></IconButton>
          <IconButton label={t('clipboard.folder.delete')} size={22} onclick={() => askDelete(node.folder)}><X size={12} /></IconButton>
        </span>
      </div>
    {/if}
  {:else}
    {#if !creating}<span class="hint">{t('clipboard.filters.noFolders')}</span>{/if}
  {/each}

  <div class="cap"><span>{t('clipboard.filters.tags')}</span></div>
  {#each tags as tag (tag.tag)}
    <button type="button" class="item" class:on={active === `tag:${tag.tag}`} onclick={() => onselect({ type: 'tag', tag: tag.tag })}>
      <Hash size={15} />
      <span class="name" title={tag.tag}>{tag.tag}</span>
      <span class="count">{tag.count}</span>
    </button>
  {:else}
    <span class="hint">{t('clipboard.filters.noTags')}</span>
  {/each}

  <div class="sep"></div>
  <button type="button" class="item" class:on={active === 'trash'} onclick={() => onselect({ type: 'trash' })}>
    <Trash2 size={15} />
    <span class="name">{t('clipboard.filters.trash')}</span>
    {#if stats && stats.trash > 0}<span class="count">{stats.trash}</span>{/if}
  </button>
</nav>

<Sheet bind:open={deleteOpen} title={t('clipboard.folder.deleteTitle', { name: deleting?.name ?? '' })}>
  <p class="confirm">{t('clipboard.folder.deleteMessage')}</p>
  {#snippet footer()}
    <Button onclick={() => (deleteOpen = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={confirmDelete}>{t('clipboard.folder.deleteConfirm')}</Button>
  {/snippet}
</Sheet>

<style>
  .col { display: flex; flex-direction: column; gap: 1px; width: 184px; flex: none; min-height: 0; overflow-y: auto; padding: 2px 6px var(--space-4) 0; }
  .item {
    display: flex; align-items: center; gap: 8px; width: 100%; min-height: 28px; padding: 0 8px; border: 0; border-radius: var(--radius-sm);
    background: transparent; color: var(--text); font: var(--text-body); text-align: left; cursor: default;
    transition: background var(--dur-fast);
  }
  .item :global(svg) { flex: none; color: var(--text-secondary); }
  .item:hover { background: var(--control-bg); }
  .item.on { background: var(--accent-soft); }
  .item.on :global(svg) { color: var(--accent); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { flex: none; font: var(--text-caption); color: var(--text-tertiary); font-variant-numeric: tabular-nums; }
  .folder { padding-right: 4px; }
  .hit {
    display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0; height: 28px; padding: 0; border: 0;
    background: transparent; color: inherit; font: inherit; text-align: left; cursor: default;
  }
  .tools { display: none; gap: 0; }
  .folder:hover .tools, .folder:focus-within .tools { display: inline-flex; }
  .folder:hover .count, .folder:focus-within .count { display: none; }
  .cap {
    display: flex; align-items: center; justify-content: space-between; height: 26px; margin-top: var(--space-3); padding: 0 2px 0 8px;
    font: var(--text-caption); letter-spacing: 0.02em; text-transform: uppercase; color: var(--text-secondary);
  }
  .hint { padding: 2px 8px 4px; font: var(--text-callout); color: var(--text-tertiary); }
  .sep { height: 0.5px; margin: var(--space-3) 8px var(--space-2); background: var(--separator); }
  .edit {
    height: 26px; margin: 1px 0; padding: 0 8px; border: 0; border-radius: var(--radius-sm); outline: 0;
    font: var(--text-body); color: var(--text); background: var(--field-bg); cursor: text; user-select: text;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 40%, transparent), inset 0 0 0 0.5px var(--accent);
  }
  .confirm { margin: 0; color: var(--text-secondary); }
</style>
