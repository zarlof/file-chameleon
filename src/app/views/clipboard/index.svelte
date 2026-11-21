<!--
  Clipboard library ("Pano") — filters on the left, toolbar and a windowed list/grid in the middle,
  inspector on the right (a drawer over the content when the view is narrow).
  Keys: arrows / Home / End move (Shift extends), Ctrl+A, Shift/Ctrl+click, Enter or Ctrl+C copy,
  Delete trash (forever in the trash), Ctrl+Z restores the last trashed batch, Ctrl+F search,
  Space preview, Esc deselect.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { Clipboard as ClipboardIcon, CirclePause, FolderOpen, Search, Settings2, ShieldCheck, Trash2 } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { api, errorOf, on } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import type { ClipFolder, ClipItem, ClipStats, ClipTag } from '../../../lib/types';
  import { Button, EmptyState, IconButton, modal, Sheet, toast } from '../../../lib/ui';
  import FilterColumn from './FilterColumn.svelte';
  import Inspector from './Inspector.svelte';
  import ItemGrid from './ItemGrid.svelte';
  import ItemList from './ItemList.svelte';
  import PreviewSheet from './PreviewSheet.svelte';
  import Toolbar from './Toolbar.svelte';
  import {
    acceptsAdded, arrange, EMPTY_SELECTION, filterKey, LoadGate, pruneSelection, pushUndo, queryFor, selectAll, selectionAfterClick,
    selectionAfterMove, withAdded, type BatchAction, type LibraryFilter, type QuickAction, type Selection,
  } from './clips';
  import { guard } from './ui';

  /** Below this width the inspector is a drawer over the content instead of a column. */
  const WIDE = 960;

  let filter = $state<LibraryFilter>({ type: 'all' });
  let search = $state('');
  let items = $state<ClipItem[]>([]);
  let loaded = $state(false);
  let folders = $state<ClipFolder[]>([]);
  let tags = $state<ClipTag[]>([]);
  let stats = $state<ClipStats | null>(null);
  let paused = $state(false);
  let selection = $state<Selection>(EMPTY_SELECTION);
  let focus = $state<number | null>(null);
  let undoStack: number[][] = [];
  let mode = $state<'grid' | 'list'>(readPref('fc.clipboard.mode') === 'grid' ? 'grid' : 'list');
  let inspectorPref = $state(readPref('fc.clipboard.inspector') !== '0');
  let drawer = $state(false);
  let width = $state(1200);
  let columns = $state(1);
  let now = $state(Date.now());
  /** Bumped when the filter or search changes: remounts the list (scroll back to the top). */
  let epoch = $state(0);
  let previewOpen = $state(false);
  let previewId = $state<number | null>(null);
  let pendingDelete = $state<number[]>([]);
  let deleteOpen = $state(false);
  let emptyOpen = $state(false);
  let toolbar: ReturnType<typeof Toolbar> | undefined = $state();
  let listView: ReturnType<typeof ItemList> | undefined = $state();
  let gridView: ReturnType<typeof ItemGrid> | undefined = $state();
  let content: HTMLElement | undefined = $state();

  const shown = $derived(arrange(items, filter));
  const order = $derived(shown.map((i) => i.id));
  const selected = $derived(new Set(selection.ids));
  const selectedItems = $derived(shown.filter((i) => selected.has(i.id)));
  const current = $derived(selectedItems.length === 1 ? selectedItems[0] : null);
  const inTrash = $derived(filter.type === 'trash');
  const folderNames = $derived(new Map(folders.map((f) => [f.id, f.name])));
  const wide = $derived(width >= WIDE);
  const inspectorShown = $derived(wide ? inspectorPref : drawer);
  const previewItem = $derived(shown.find((i) => i.id === previewId) ?? null);

  function readPref(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  }
  function writePref(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      /* storage unavailable: keep the choice for this session only */
    }
  }
  $effect(() => writePref('fc.clipboard.mode', mode));
  $effect(() => writePref('fc.clipboard.inspector', inspectorPref ? '1' : '0'));

  // ------------------------------------------------------------ loading

  const gate = new LoadGate();
  let loadFailed = false;
  async function loadItems() {
    const token = gate.start();
    try {
      const list = await api.clipList(queryFor(filter, search));
      if (!gate.isCurrent(token)) return;
      items = list;
      const ids = arrange(list, filter).map((i) => i.id);
      selection = pruneSelection(ids, selection);
      if (focus !== null && !ids.includes(focus)) focus = selection.ids[0] ?? null;
      loadFailed = false;
    } catch (e) {
      if (!gate.isCurrent(token)) return;
      if (!loadFailed) toast.show(errorOf(e).message, 'error');
      loadFailed = true;
    } finally {
      gate.finish();
      if (gate.isCurrent(token)) loaded = true;
    }
  }

  /** Edit the shown list directly (live insert, optimistic pin, trash...). A load already on its way
   *  carries older data: it is dropped and a fresh one is started. */
  function setItems(next: ClipItem[]) {
    items = next;
    if (gate.edited()) void loadItems();
  }

  async function loadSide() {
    const [f, tg, st] = await Promise.allSettled([api.clipFolders(), api.clipTags(), api.clipStats()]);
    if (f.status === 'fulfilled') {
      folders = f.value;
      const shownFolder = filter.type === 'folder' ? filter.id : null;
      if (shownFolder !== null && !f.value.some((x) => x.id === shownFolder)) setFilter({ type: 'all' });
    }
    if (tg.status === 'fulfilled') tags = tg.value;
    if (st.status === 'fulfilled') stats = st.value;
  }

  let itemsTimer: ReturnType<typeof setTimeout> | undefined;
  let sideTimer: ReturnType<typeof setTimeout> | undefined;
  /** Coalesced refetch (events and our own actions often arrive together). */
  function refresh(delay = 60) {
    clearTimeout(itemsTimer);
    itemsTimer = setTimeout(() => void loadItems(), delay);
    refreshSide(delay);
  }
  function refreshSide(delay = 200) {
    clearTimeout(sideTimer);
    sideTimer = setTimeout(() => void loadSide(), delay);
  }

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      epoch++;
      void loadItems();
    }, 150);
  }

  function setFilter(f: LibraryFilter) {
    if (filterKey(f) === filterKey(filter)) return;
    // Never show the old view's items under the new filter (live items in the trash view could be
    // "deleted forever"): empty the list and block actions until the new list arrives.
    filter = f;
    items = [];
    loaded = false;
    selection = EMPTY_SELECTION;
    focus = null;
    epoch++;
    void loadItems();
  }

  onMount(() => {
    void loadItems();
    void loadSide();
    api.clipGetPaused().then((p) => (paused = p)).catch(() => {});
    const clock = setInterval(() => (now = Date.now()), 60_000);
    const unlisten: (() => void)[] = [];
    let disposed = false;
    const keep = (p: Promise<() => void>) => p.then((u) => (disposed ? u() : unlisten.push(u))).catch(() => {});
    keep(on.clipAdded((c) => {
      if (loaded && acceptsAdded(filter, search, c)) setItems(withAdded(items, c));
      else refresh();
      refreshSide();
    }));
    keep(on.clipUpdated((c) => {
      // Still belongs to this view (live vs trash)? Replace it; otherwise drop it.
      if ((c.trashedAt !== null) === inTrash) setItems(items.map((i) => (i.id === c.id ? c : i)));
      else setItems(items.filter((i) => i.id !== c.id));
      refreshSide();
    }));
    keep(on.clipChanged(() => refresh()));
    keep(on.clipPaused((p) => (paused = p)));
    return () => {
      disposed = true;
      clearInterval(clock);
      clearTimeout(searchTimer);
      clearTimeout(itemsTimer);
      clearTimeout(sideTimer);
      unlisten.forEach((u) => u());
    };
  });

  // ------------------------------------------------------------ actions

  const count = (ids: number[]) => ({ count: ids.length });

  async function copy(ids: number[]) {
    if (ids.length && (await guard(() => api.clipCopy(ids)))) toast.show(t('clipboard.toast.copied', count(ids)), 'success', 1500);
  }

  async function pin(ids: number[], pinned: boolean) {
    if (!ids.length) return;
    setItems(items.map((i) => (ids.includes(i.id) ? { ...i, pinned } : i)));
    await guard(() => api.clipPin(ids, pinned));
    refresh();
  }

  async function trash(ids: number[]) {
    if (!ids.length) return;
    const at = order.indexOf(ids[0]);
    if (!(await guard(() => api.clipTrash(ids)))) return;
    undoStack = pushUndo(undoStack, ids);
    setItems(items.filter((i) => !ids.includes(i.id)));
    // Keep the cursor where it was: the next item takes the place of the first trashed one.
    const next = order.filter((id) => !ids.includes(id));
    const nextFocus = next[Math.min(Math.max(0, at), next.length - 1)] ?? null;
    selection = nextFocus !== null ? { ids: [nextFocus], anchor: nextFocus } : EMPTY_SELECTION;
    focus = nextFocus;
    toast.show(t('clipboard.toast.trashed', count(ids)), 'info');
    refreshSide();
  }

  async function undoTrash() {
    const batch = undoStack[undoStack.length - 1];
    if (!batch) return;
    undoStack = undoStack.slice(0, -1);
    if (await guard(() => api.clipRestore(batch))) {
      toast.show(t('clipboard.toast.restored', count(batch)), 'success', 2000);
      refresh(0);
    }
  }

  async function restore(ids: number[]) {
    if (ids.length && (await guard(() => api.clipRestore(ids)))) {
      setItems(items.filter((i) => !ids.includes(i.id)));
      selection = EMPTY_SELECTION;
      toast.show(t('clipboard.toast.restored', count(ids)), 'success', 2000);
      refreshSide();
    }
  }

  function askDelete(ids: number[]) {
    if (!ids.length) return;
    pendingDelete = ids;
    deleteOpen = true;
  }

  async function deleteForever() {
    // Only items shown as trashed can be deleted for good from here.
    const ids = pendingDelete.filter((id) => items.some((i) => i.id === id && i.trashedAt !== null));
    deleteOpen = false;
    if (ids.length && (await guard(() => api.clipDeleteForever(ids)))) {
      setItems(items.filter((i) => !ids.includes(i.id)));
      selection = EMPTY_SELECTION;
      refreshSide();
    }
  }

  async function emptyTrash() {
    emptyOpen = false;
    if (await guard(() => api.clipEmptyTrash())) {
      if (inTrash) setItems([]);
      selection = EMPTY_SELECTION;
      refreshSide(0);
    }
  }

  async function exportTo(ids: number[]) {
    let dir: string | null = null;
    try {
      const r = await openDialog({ directory: true });
      dir = typeof r === 'string' ? r : null;
    } catch {
      /* dialog unavailable */
    }
    if (!dir) return;
    const target = dir;
    await guard(async () => {
      const n = await api.clipExport(ids, target);
      toast.show(t('clipboard.toast.exported', { count: n }), 'success');
    });
  }

  async function batch(a: BatchAction) {
    if (!loaded) return; // a filter switch is loading: the selection belongs to no list yet
    const ids = selection.ids;
    switch (a.type) {
      case 'copy':
        return copy(ids);
      case 'pin':
        return pin(ids, a.pinned);
      case 'move':
        if (await guard(() => api.clipMove(ids, a.folder))) {
          toast.show(a.folder === null ? t('clipboard.toast.unfiled', count(ids)) : t('clipboard.toast.moved', { ...count(ids), folder: folderNames.get(a.folder) ?? '' }), 'success', 2000);
          refresh(0);
        }
        return;
      case 'moveNew':
        if (await guard(async () => {
          const f = await api.clipFolderCreate(a.name);
          await api.clipMove(ids, f.id);
        })) {
          toast.show(t('clipboard.toast.moved', { ...count(ids), folder: a.name }), 'success', 2000);
        }
        refresh(0);
        return;
      case 'tag':
        if (await guard(() => api.clipTag(ids, a.add, a.remove))) refresh(0);
        return;
      case 'export':
        return exportTo(ids);
      case 'trash':
        return trash(ids);
      case 'restore':
        return restore(ids);
      case 'delete':
        return askDelete(ids);
      case 'deselect':
        selection = EMPTY_SELECTION;
        return;
    }
  }

  function quick(id: number, action: QuickAction) {
    const it = items.find((i) => i.id === id);
    if (!it) return;
    if (action === 'copy') void copy([id]);
    else if (action === 'pin') void pin([id], !it.pinned);
    else if (action === 'trash') void trash([id]);
    else void restore([id]);
  }

  function updated(c: ClipItem) {
    setItems(items.map((i) => (i.id === c.id ? c : i)));
    refreshSide();
  }

  function edit(c: ClipItem) {
    nav.goto({ view: 'editor', paths: [`clip:${c.id}`], url: null });
  }

  function preview(id: number | null) {
    if (id === null) return;
    previewId = id;
    previewOpen = true;
  }

  async function resume() {
    try {
      await api.clipSetPaused(false);
      paused = false;
    } catch (e) {
      toast.show(errorOf(e).message, 'error');
    }
  }

  // ------------------------------------------------------------ selection & keys

  function pick(id: number, e: MouseEvent) {
    selection = selectionAfterClick(order, selection, id, { shift: e.shiftKey, ctrl: e.ctrlKey || e.metaKey });
    focus = id;
  }

  function activate(id: number) {
    if (inTrash) preview(id);
    else void copy([id]);
  }

  async function moveBy(delta: number, shift: boolean) {
    const r = selectionAfterMove(order, selection, focus ?? selection.ids[selection.ids.length - 1] ?? null, delta, shift);
    selection = r.selection;
    focus = r.focus;
    await tick();
    const index = r.focus === null ? -1 : order.indexOf(r.focus);
    if (mode === 'list') listView?.reveal(index);
    else gridView?.reveal(index);
  }

  function editable(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    return !!el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName));
  }

  /**
   * Navigation keys only act on the content area (or nothing focused), never on a focused control:
   * the hover buttons of a row live inside the content, and Space / Enter must press them.
   */
  function inContent(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (!el || el === document.body) return true;
    return !!content?.contains(el) && !el.closest('button, a, [role="button"]');
  }

  function onKey(e: KeyboardEvent) {
    if (previewOpen) {
      if (e.key === ' ') {
        e.preventDefault();
        previewOpen = false;
      }
      return;
    }
    if (modal.open > 0 || e.altKey) return;
    const ctrl = e.ctrlKey || e.metaKey;
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (ctrl && key === 'f') {
      e.preventDefault();
      toolbar?.focusSearch();
      return;
    }
    // While a filter switch loads nothing is shown yet: no selection or actions.
    if (editable(e.target) || !loaded) return;
    if (ctrl) {
      if (key === 'a') {
        e.preventDefault();
        selection = selectAll(order);
      } else if (key === 'z' && !e.shiftKey) {
        e.preventDefault();
        void undoTrash();
      } else if (key === 'c' && !inTrash && selection.ids.length && !window.getSelection()?.toString()) {
        e.preventDefault();
        void copy(selection.ids);
      }
      return;
    }
    if (!inContent(e.target)) return;
    const step = mode === 'grid' ? columns : 1;
    const moves: Record<string, number> = {
      ArrowDown: step,
      ArrowUp: -step,
      Home: -order.length,
      End: order.length,
      ...(mode === 'grid' ? { ArrowRight: 1, ArrowLeft: -1 } : {}),
    };
    if (key in moves) {
      e.preventDefault();
      void moveBy(moves[key], e.shiftKey);
    } else if (key === 'Enter' && selection.ids.length) {
      e.preventDefault();
      if (inTrash) void restore(selection.ids);
      else void copy(selection.ids);
    } else if (key === 'Delete' && selection.ids.length) {
      e.preventDefault();
      if (inTrash) askDelete(selection.ids);
      else void trash(selection.ids);
    } else if (key === ' ') {
      e.preventDefault();
      preview(focus ?? current?.id ?? null);
    } else if (key === 'Escape') {
      if (!wide && drawer) drawer = false;
      else selection = EMPTY_SELECTION;
    }
  }

  function toggleInspector(v: boolean) {
    if (wide) inspectorPref = v;
    else drawer = v;
  }
</script>

<svelte:window onkeydown={onKey} />

<section class="lib" bind:clientWidth={width}>
  <header>
    <h1>{t('clipboard.title')}</h1>
    <div class="head">
      {#if paused}
        <span class="paused">
          <CirclePause size={14} />{t('clipboard.paused')}
          <Button size="sm" variant="plain" onclick={resume}>{t('clipboard.resume')}</Button>
        </span>
      {/if}
      <IconButton label={t('common.nav.clipboardSettings')} onclick={() => nav.go('clipboardSettings')}><Settings2 size={16} /></IconButton>
    </div>
  </header>

  <div class="body">
    <FilterColumn {filter} {folders} {tags} {stats} onselect={setFilter} onchanged={() => refresh(0)} />

    <div class="main">
      <Toolbar bind:this={toolbar} bind:search bind:mode inspector={inspectorShown} selected={selectedItems} trash={inTrash} {folders} {tags}
        onsearch={onSearch} onbatch={(a) => void batch(a)} oninspector={toggleInspector} />

      {#if inTrash}
        <div class="trash-note">
          <span>{t('clipboard.trash.note')}</span>
          <Button size="sm" variant="plain" disabled={!stats?.trash && shown.length === 0} onclick={() => (emptyOpen = true)}>
            {#snippet icon()}<Trash2 size={13} />{/snippet}{t('clipboard.trash.empty')}
          </Button>
        </div>
      {/if}

      <div class="content" bind:this={content}>
        {#if !loaded}
          <!-- first load: keep the space, avoid flashing the empty state -->
        {:else if shown.length === 0}
          {#if search.trim()}
            <EmptyState title={t('clipboard.noResults.title')} message={t('clipboard.noResults.message')}>
              {#snippet icon()}<Search />{/snippet}
            </EmptyState>
          {:else if inTrash}
            <EmptyState title={t('clipboard.trash.emptyTitle')} message={t('clipboard.trash.emptyMessage')}>
              {#snippet icon()}<Trash2 />{/snippet}
            </EmptyState>
          {:else if filter.type === 'all'}
            <EmptyState title={t('clipboard.empty.title')} message={t('clipboard.empty.message')}>
              {#snippet icon()}<ClipboardIcon />{/snippet}
              {#snippet action()}<span class="privacy"><ShieldCheck size={14} />{t('clipboard.empty.privacy')}</span>{/snippet}
            </EmptyState>
          {:else}
            <EmptyState title={t('clipboard.emptyFilter.title')} message={t('clipboard.emptyFilter.message')}>
              {#snippet icon()}<FolderOpen />{/snippet}
            </EmptyState>
          {/if}
        {:else}
          {#key `${epoch}:${mode}`}
            {#if mode === 'list'}
              <ItemList bind:this={listView} items={shown} {selected} {focus} {now} trash={inTrash} folders={folderNames}
                onpick={pick} onopen={activate} onquick={quick} />
            {:else}
              <ItemGrid bind:this={gridView} bind:columns items={shown} {selected} {focus} {now} trash={inTrash}
                onpick={pick} onopen={activate} onquick={quick} />
            {/if}
          {/key}
        {/if}
      </div>
    </div>

    {#if inspectorShown}
      <div class="side" class:drawer={!wide}>
        <Inspector item={current} count={selectedItems.length} {folders} trash={inTrash}
          onupdated={updated} onrefresh={() => refresh(0)} onquick={(a) => current && quick(current.id, a)}
          ondelete={() => current && askDelete([current.id])} onedit={() => current && edit(current)} onpreview={() => preview(current?.id ?? null)} />
      </div>
    {/if}
  </div>
</section>

<PreviewSheet bind:open={previewOpen} item={previewItem} />

<Sheet bind:open={deleteOpen} title={t('clipboard.delete.title', { count: pendingDelete.length })}>
  <p class="confirm">{t('clipboard.delete.message')}</p>
  {#snippet footer()}
    <Button onclick={() => (deleteOpen = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={deleteForever}>{t('clipboard.delete.confirm')}</Button>
  {/snippet}
</Sheet>

<Sheet bind:open={emptyOpen} title={t('clipboard.trash.emptyConfirmTitle')}>
  <p class="confirm">{t('clipboard.delete.message')}</p>
  {#snippet footer()}
    <Button onclick={() => (emptyOpen = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={emptyTrash}>{t('clipboard.trash.empty')}</Button>
  {/snippet}
</Sheet>

<style>
  .lib { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--space-5) var(--space-4); }
  header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex: none; padding-bottom: var(--space-3); }
  h1 { margin: 0; font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .head { display: flex; align-items: center; gap: var(--space-2); }
  .paused {
    display: inline-flex; align-items: center; gap: 6px; padding: 0 2px 0 10px; border-radius: 99px;
    font: var(--text-callout); color: var(--orange); background: color-mix(in srgb, var(--orange) 12%, transparent);
  }
  .body { position: relative; display: flex; gap: var(--space-3); flex: 1; min-height: 0; }
  /* Positioned: the toolbar's floating batch bar sits at its bottom. */
  .main { position: relative; display: flex; flex-direction: column; flex: 1; min-width: 0; min-height: 0; }
  .content { flex: 1; min-height: 0; }
  .trash-note {
    display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex: none; margin-bottom: var(--space-2);
    padding: 4px 4px 4px 12px; border-radius: var(--radius-sm); background: var(--control-bg); font: var(--text-callout); color: var(--text-secondary);
  }
  .side { display: flex; min-height: 0; }
  .side.drawer {
    /* Below the toolbar row, so its toggle stays reachable. */
    position: absolute; top: 48px; right: 0; bottom: 0; z-index: 10; border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg); animation: slide var(--dur) var(--ease-out);
  }
  .side.drawer :global(.insp) { background: var(--elevated-bg); backdrop-filter: blur(24px) saturate(1.6); }
  .privacy { display: inline-flex; align-items: center; gap: 6px; font: var(--text-callout); color: var(--text-secondary); }
  .confirm { margin: 0; color: var(--text-secondary); }
  @keyframes slide { from { opacity: 0; transform: translateX(12px); } }
</style>
