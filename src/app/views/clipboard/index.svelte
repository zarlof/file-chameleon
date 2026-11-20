<!--
  Clipboard view — history of recent copies: search, filter, pin, copy back, save images, hand links to Download.
  Renders at most `limit` rows (PAGE at a time, more on scroll) so very long histories stay cheap.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { Clipboard as ClipboardIcon, Search, ShieldCheck, Trash2 } from '@lucide/svelte';
  import { currentLang, t } from '../../../lib/i18n';
  import { api, errorOf, on } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import type { ClipItem } from '../../../lib/types';
  import { Button, EmptyState, Sheet, SegmentedControl, TextField, Toggle, toast } from '../../../lib/ui';
  import ClipRow from './ClipRow.svelte';
  import { baseName, type ClipFilter, ordered, PAGE, relativeTime, withAdded } from './clips';

  let items = $state<ClipItem[]>([]);
  let loaded = $state(false);
  let query = $state('');
  let filter = $state<ClipFilter>('all');
  let paused = $state(false);
  let confirmClear = $state(false);
  let limit = $state(PAGE);
  let now = $state(Date.now());

  const shown = $derived(ordered(items, filter));
  const visible = $derived(shown.slice(0, limit));
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
      limit = PAGE;
      void load();
    }, 150);
  }

  onMount(() => {
    void load();
    const tick = setInterval(() => (now = Date.now()), 60_000);
    let unlisten: (() => void) | undefined;
    let disposed = false;
    on.clipAdded((c) => {
      if (query.trim()) void load();
      else items = withAdded(items, c);
    })
      .then((u) => (disposed ? u() : (unlisten = u)))
      .catch(() => {});
    return () => {
      disposed = true;
      clearInterval(tick);
      clearTimeout(searchTimer);
      unlisten?.();
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

  function togglePause(value: boolean) {
    void guarded(() => api.clipSetPaused(value));
  }

  function onScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    if (limit < shown.length && el.scrollTop + el.clientHeight > el.scrollHeight - 240) limit += PAGE;
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
    <SegmentedControl {options} bind:value={filter} size="sm" onchange={() => (limit = PAGE)} />
  </div>

  <div class="list" onscroll={onScroll}>
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
      {#each visible as item (item.id)}
        <ClipRow {item} when={relativeTime(item.createdAt, now, currentLang(), t('clipboard.ago.now'))}
          oncopy={() => copy(item)} onpin={() => pin(item)} ondelete={() => remove(item)}
          ondownload={() => download(item)} onconvert={() => convert(item)} onsave={(f) => save(item, f)} />
      {/each}
      {#if shown.length > visible.length}
        <div class="more"><Button size="sm" variant="plain" onclick={() => (limit += PAGE)}>{t('clipboard.showMore')}</Button></div>
      {/if}
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
  .more { display: flex; justify-content: center; padding: var(--space-3) 0; }
  .privacy { display: inline-flex; align-items: center; gap: 6px; font: var(--text-callout); color: var(--text-secondary); }
  .confirm { margin: 0; color: var(--text-secondary); }
</style>
