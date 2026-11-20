<!--
  Clipboard view — history of recent copies: search, filter, pin, copy back, save images, hand links to Download.
  Renders at most `limit` rows (PAGE at a time, more on scroll) so very long histories stay cheap.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Clipboard as ClipboardIcon, Search, ShieldCheck, Trash2 } from '@lucide/svelte';
  import { currentLang, t } from '../../../lib/i18n';
  import { api, errorOf, on } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import type { ClipItem } from '../../../lib/types';
  import { Button, EmptyState, Sheet, SegmentedControl, TextField, Toggle, toast } from '../../../lib/ui';
  import ClipRow from './ClipRow.svelte';
  import { baseName, type ClipFilter, ordered, relativeTime, ROW_H, rowWindow, scrollToReveal, withAdded } from './clips';

  let items = $state<ClipItem[]>([]);
  let loaded = $state(false);
  let query = $state('');
  let filter = $state<ClipFilter>('all');
  let paused = $state(false);
  let confirmClear = $state(false);
  let now = $state(Date.now());
  let list: HTMLElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(600);

  const shown = $derived(ordered(items, filter));
  const win = $derived(rowWindow(scrollTop, viewport, shown.length));
  const visible = $derived(shown.slice(win.start, win.end));
  const options = $derived([
    { value: 'all' as const, label: t('clipboard.filter.all') },
    { value: 'links' as const, label: t('clipboard.filter.links') },
    { value: 'text' as const, label: t('clipboard.filter.text') },
    { value: 'images' as const, label: t('clipboard.filter.images') },
  ]);

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let requestSeq = 0;

  async function load() {
    const seq = ++requestSeq;
    try {
      const list = await api.clipList(query.trim() || undefined);
      if (seq === requestSeq) items = list;
    } catch (e) {
      toast.show(errorOf(e).message, 'error');
    } finally {
      loaded = true;
    }
  }

  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      resetScroll();
      void load();
    }, 150);
  }

  function resetScroll() {
    scrollTop = 0;
    if (list) list.scrollTop = 0;
  }

  onMount(() => {
    void load();
    api.clipGetPaused().then((p) => (paused = p)).catch(() => {});
    const clock = setInterval(() => (now = Date.now()), 60_000);
    const unlisten: (() => void)[] = [];
    let disposed = false;
    const keep = (p: Promise<() => void>) => p.then((u) => (disposed ? u() : unlisten.push(u))).catch(() => {});
    keep(on.clipAdded((c) => {
      if (query.trim()) void load();
      else items = withAdded(items, c);
    }));
    keep(on.clipPaused((p) => (paused = p)));
    return () => {
      disposed = true;
      clearInterval(clock);
      clearTimeout(searchTimer);
      unlisten.forEach((u) => u());
    };
  });

  async function guarded(action: () => Promise<unknown>) {
    try {
      await action();
    } catch (e) {
      toast.show(errorOf(e).message, 'error');
    }
  }

  const copy = (c: ClipItem) =>
    guarded(async () => {
      await api.clipCopy(c.id);
      toast.show(t('clipboard.copied'), 'success', 1500);
    });

  const pin = (c: ClipItem) =>
    guarded(async () => {
      if (await api.clipPin(c.id, !c.pinned)) items = items.map((i) => (i.id === c.id ? { ...i, pinned: !c.pinned } : i));
    });

  const remove = (c: ClipItem) =>
    guarded(async () => {
      if (await api.clipDelete(c.id)) items = items.filter((i) => i.id !== c.id);
    });

  const save = (c: ClipItem, format: 'png' | 'jpg' | 'webp') =>
    guarded(async () => {
      const path = await api.clipSaveImage(c.id, format);
      toast.show(t('clipboard.saved', { name: baseName(path) }), 'success');
    });

  function download(c: ClipItem) {
    if (c.text) nav.goto({ view: 'download', paths: [], url: c.text.trim() });
  }

  function convert(c: ClipItem) {
    if (c.image) nav.goto({ view: 'convert', paths: [c.image], url: null });
  }

  async function clearUnpinned() {
    confirmClear = false;
    await guarded(async () => {
      await api.clipClear();
      await load();
    });
  }

  async function togglePause(value: boolean) {
    try {
      await api.clipSetPaused(value);
    } catch (e) {
      paused = !value; // roll the switch back: the backend state did not change
      toast.show(errorOf(e).message, 'error');
    }
  }

  /** Arrow keys move focus between rows; the target row is scrolled in and mounted first. */
  async function onListKey(e: KeyboardEvent) {
    const delta = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
    const row = (e.target as HTMLElement).closest<HTMLElement>('[data-index]');
    if (!delta || !list || !row) return;
    e.preventDefault();
    const index = Math.min(shown.length - 1, Math.max(0, Number(row.dataset.index) + delta));
    list.scrollTop = scrollTop = scrollToReveal(scrollTop, viewport, index);
    await tick();
    list.querySelector<HTMLElement>(`[data-index="${index}"] .row`)?.focus({ preventScroll: true });
  }
</script>

<section>
  <header>
    <h1>{t('clipboard.title')}</h1>
    <div class="head-actions">
      <label class="pause">
        <span>{paused ? t('clipboard.paused') : t('clipboard.pause')}</span>
        <Toggle bind:checked={paused} label={t('clipboard.pause')} onchange={togglePause} />
      </label>
      <Button size="sm" variant="secondary" disabled={items.every((i) => i.pinned)} onclick={() => (confirmClear = true)}>
        {#snippet icon()}<Trash2 size={13} />{/snippet}
        {t('clipboard.clearUnpinned')}
      </Button>
    </div>
  </header>

  <div class="tools">
    <div class="search">
      <TextField bind:value={query} placeholder={t('clipboard.search')} type="search" oninput={onSearch}>
        {#snippet leading()}<Search size={14} />{/snippet}
      </TextField>
    </div>
    <SegmentedControl {options} bind:value={filter} size="sm" onchange={resetScroll} />
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="list" bind:this={list} bind:clientHeight={viewport} onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
    onkeydown={onListKey}>
    {#if !loaded}
      <!-- first load: keep the space, avoid flashing the empty state -->
    {:else if items.length === 0 && !query.trim()}
      <EmptyState title={t('clipboard.empty.title')} message={t('clipboard.empty.message')}>
        {#snippet icon()}<ClipboardIcon />{/snippet}
        {#snippet action()}
          <span class="privacy"><ShieldCheck size={14} />{t('clipboard.empty.privacy')}</span>
        {/snippet}
      </EmptyState>
    {:else if shown.length === 0}
      <EmptyState title={t('clipboard.noResults.title')} message={t('clipboard.noResults.message')}>
        {#snippet icon()}<Search />{/snippet}
      </EmptyState>
    {:else}
      <div style:height="{win.top}px" aria-hidden="true"></div>
      {#each visible as item, k (item.id)}
        <div class="slot" style:height="{ROW_H}px" data-index={win.start + k}>
          <ClipRow {item} when={relativeTime(item.createdAt, now, currentLang(), t('clipboard.ago.now'))}
            oncopy={() => copy(item)} onpin={() => pin(item)} ondelete={() => remove(item)}
            ondownload={() => download(item)} onconvert={() => convert(item)} onsave={(f) => save(item, f)} />
        </div>
      {/each}
      <div style:height="{win.bottom}px" aria-hidden="true"></div>
    {/if}
  </div>
</section>

<Sheet bind:open={confirmClear} title={t('clipboard.clear.title')}>
  <p class="confirm">{t('clipboard.clear.message')}</p>
  {#snippet footer()}
    <Button onclick={() => (confirmClear = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={clearUnpinned}>{t('clipboard.clear.confirm')}</Button>
  {/snippet}
</Sheet>

<style>
  section { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--space-6); }
  header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex: none; }
  h1 { margin: 0; font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .head-actions { display: flex; align-items: center; gap: var(--space-4); }
  .pause { display: flex; align-items: center; gap: var(--space-2); font: var(--text-callout); color: var(--text-secondary); }
  .tools { display: flex; align-items: center; gap: var(--space-3); flex: none; padding: var(--space-3) 0; }
  .search { flex: 1; min-width: 0; }
  .list { flex: 1; min-height: 0; overflow: auto; margin: 0 calc(-1 * var(--space-3)); padding: 0 var(--space-3) var(--space-6); }
  .slot { box-sizing: border-box; padding-bottom: 4px; }
  .privacy { display: inline-flex; align-items: center; gap: 6px; font: var(--text-callout); color: var(--text-secondary); }
  .confirm { margin: 0; color: var(--text-secondary); }
</style>
