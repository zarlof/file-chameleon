<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api, on } from '../lib/ipc';
  import type { ClipItem, ClipShelf, Settings } from '../lib/types';
  import Row from './Row.svelte';
  import Preview from './Preview.svelte';
  import { setLang, getLang, t } from './i18n';
  import { firstSelectable, moveSelection, numberPick, toggleSelect, type NavKey, type Pos } from './logic';

  const LIMIT = 60;
  const SIZES = {
    small: { w: 120, h: 88 },
    medium: { w: 160, h: 118 },
    large: { w: 210, h: 156 },
  } as const;

  let settings = $state<Settings | null>(null);
  let data = $state<Record<ClipShelf, ClipItem[]>>({ images: [], texts: [], links: [] });
  let query = $state('');
  let pinnedOnly = $state(false);
  let cursor = $state<Pos>({ row: 0, col: 0 });
  let selected = $state<number[]>([]);
  /** Fixed end of a Shift range; moves on plain navigation and plain / Ctrl clicks. */
  let anchor: number | null = null;
  /** false until the first list after (re)open is in: nothing stale is ever shown. */
  let ready = $state(false);
  /** Set by a refresh that wants the cursor on the first card; survives a superseding refresh. */
  let resetPending = false;
  let preview = $state<ClipItem | null>(null);
  let toast = $state<{ text: string; undo: boolean } | null>(null);
  let openCount = $state(0);
  let searchEl = $state<HTMLInputElement | null>(null);

  const undoStack: number[][] = [];
  let seq = 0;
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let systemLanguage: 'tr' | 'en' = 'en';

  const rows = $derived<ClipShelf[]>([...new Set<ClipShelf>(settings?.shelfRows ?? ['images', 'texts', 'links'])]);
  const grid = $derived(rows.map((r) => data[r].map((i) => i.id)));
  const size = $derived(SIZES[settings?.shelfCardSize ?? 'medium']);
  const cursorItem = $derived(data[rows[cursor.row]]?.[cursor.col] ?? null);
  const cursorId = $derived(cursorItem?.id ?? null);
  const anything = $derived(grid.some((r) => r.length > 0));

  // ------------------------------------------------------------ settings / theme
  const dark = window.matchMedia('(prefers-color-scheme: dark)');
  let themeSetting: Settings['theme'] = 'system';

  function applyTheme() {
    document.documentElement.dataset.theme = themeSetting === 'system' ? (dark.matches ? 'dark' : 'light') : themeSetting;
  }

  function applySettings(s: Settings) {
    settings = s;
    themeSetting = s.theme;
    const root = document.documentElement;
    root.dataset.accent = s.accent;
    if (s.reduceMotion) root.dataset.reduceMotion = '';
    else delete root.dataset.reduceMotion;
    setLang(s.language, systemLanguage);
    root.lang = getLang();
    applyTheme();
  }

  // ------------------------------------------------------------ data
  function clampPos(g: number[][], p: Pos): Pos {
    const row = Math.min(p.row, g.length - 1);
    if (row >= 0 && g[row].length > 0) return { row, col: Math.min(p.col, g[row].length - 1) };
    return firstSelectable(g) ?? { row: 0, col: 0 };
  }

  function markReady() {
    if (!ready) {
      ready = true;
      openCount++;
    }
  }

  async function refresh(resetCursor = false) {
    if (resetCursor) resetPending = true;
    const my = ++seq;
    const wanted = rows;
    const text = query.trim() || null;
    let lists: ClipItem[][];
    try {
      lists = await Promise.all(wanted.map((shelf) => api.clipList({ shelf, text, pinned: pinnedOnly || undefined, limit: LIMIT })));
    } catch (e) {
      console.error(e);
      if (my === seq) markReady();
      return;
    }
    if (my !== seq) return;
    const reset = resetPending;
    resetPending = false;
    const keepId = reset ? null : cursorId;
    const next = { images: [], texts: [], links: [] } as Record<ClipShelf, ClipItem[]>;
    wanted.forEach((shelf, i) => (next[shelf] = lists[i]));
    data = next;
    const g = wanted.map((r) => next[r].map((x) => x.id));
    let pos: Pos | null = null;
    if (keepId !== null) {
      const row = g.findIndex((ids) => ids.includes(keepId));
      if (row >= 0) pos = { row, col: g[row].indexOf(keepId) };
    }
    cursor = pos ?? (reset ? (firstSelectable(g) ?? { row: 0, col: 0 }) : clampPos(g, cursor));
    const live = new Set(g.flat());
    selected = selected.filter((id) => live.has(id));
    if (reset || (anchor !== null && !live.has(anchor))) anchor = cursorId;
    markReady();
  }

  function scheduleSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => void refresh(true), 120);
  }

  async function reopen() {
    clearTimeout(searchTimer);
    ready = false;
    query = '';
    pinnedOnly = false;
    preview = null;
    selected = [];
    anchor = null;
    toast = null;
    data = { images: [], texts: [], links: [] };
    visible = true;
    window.focus();
    await refresh(true);
    await tick();
  }

  // ------------------------------------------------------------ actions
  function targets(): number[] {
    if (selected.length > 0) return selected;
    return cursorId === null ? [] : [cursorId];
  }

  function showToast(text: string, undo = false) {
    clearTimeout(toastTimer);
    toast = { text, undo };
    toastTimer = setTimeout(() => (toast = null), 4000);
  }

  // While the window is hidden, clip / settings events only mark the lists stale; `shelf://open`
  // (reopen) reloads them. Saves the queries and renders for a window nobody sees.
  let visible = true;
  function refreshLive() {
    if (visible) void refresh();
  }

  async function hide() {
    visible = false;
    try {
      await api.shelfHide();
    } catch (e) {
      console.error(e);
    }
  }

  async function copyAndClose(ids: number[]) {
    if (ids.length === 0) return;
    try {
      await api.clipCopy(ids);
      await hide();
    } catch (e) {
      console.error(e);
    }
  }

  async function pasteInto(ids: number[]) {
    if (ids.length === 0) return;
    try {
      await api.shelfPaste(ids);
    } catch (e) {
      console.error(e);
    }
  }

  async function copyStay() {
    const ids = targets();
    if (ids.length === 0) return;
    try {
      await api.clipCopy(ids);
    } catch (e) {
      console.error(e);
    }
  }

  async function togglePin() {
    const ids = targets();
    if (ids.length === 0) return;
    const items = rows.flatMap((r) => data[r]);
    const allPinned = ids.every((id) => items.find((i) => i.id === id)?.pinned);
    try {
      await api.clipPin(ids, !allPinned);
      await refresh();
    } catch (e) {
      console.error(e);
    }
  }

  async function trash() {
    const ids = targets();
    if (ids.length === 0) return;
    try {
      await api.clipTrash(ids);
      undoStack.push(ids);
      selected = [];
      showToast(t('trashed', { count: ids.length }), true);
      await refresh();
    } catch (e) {
      console.error(e);
    }
  }

  async function undo() {
    const ids = undoStack[undoStack.length - 1];
    if (!ids) return;
    try {
      await api.clipRestore(ids);
      undoStack.pop();
      showToast(t('restored'));
      await refresh();
    } catch (e) {
      console.error(e);
    }
  }

  function setPin(p: boolean) {
    pinnedOnly = p;
    void refresh(true);
  }

  // ------------------------------------------------------------ input
  function pick(item: ClipItem, e: MouseEvent) {
    const row = grid.findIndex((ids) => ids.includes(item.id));
    if (row < 0) return;
    cursor = { row, col: grid[row].indexOf(item.id) };
    const mods = { shift: e.shiftKey, ctrl: e.ctrlKey || e.metaKey };
    if (settings?.shelfClickCopies && !mods.shift && !mods.ctrl) {
      void copyAndClose([item.id]);
      return;
    }
    selected = toggleSelect(selected, item.id, mods, grid[row], anchor);
    if (!mods.shift) anchor = item.id;
  }

  function focusSearch(initial = '') {
    searchEl?.focus();
    if (initial) {
      query += initial;
      scheduleSearch();
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const inInput = document.activeElement === searchEl;
    // AltGr (reported as Ctrl+Alt on Windows) only produces characters: treat as plain typing.
    const altGr = e.getModifierState('AltGraph');
    const ctrl = !altGr && (e.ctrlKey || e.metaKey);
    const alt = !altGr && e.altKey;
    const plain = !ctrl && !alt;

    if (e.key === 'Escape') {
      e.preventDefault();
      if (preview) preview = null;
      else if (query) {
        query = '';
        void refresh(true);
      } else void hide();
      return;
    }
    if (preview) {
      if (e.key === ' ' || e.key === 'Enter') {
        e.preventDefault();
        preview = null;
      }
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      if (e.repeat || e.isComposing) return;
      if (ctrl) void pasteInto(targets());
      else void copyAndClose(targets());
      return;
    }
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown' || (!inInput && (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'Home' || e.key === 'End'))) {
      e.preventDefault();
      if (inInput && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) searchEl?.blur();
      const next = moveSelection(grid, cursor, e.key as NavKey);
      const nextId = grid[next.row]?.[next.col];
      if (e.shiftKey && next.row === cursor.row && nextId !== undefined) {
        anchor ??= cursorId;
        selected = toggleSelect([], nextId, { shift: true }, grid[next.row], anchor);
      } else {
        selected = [];
        anchor = nextId ?? null;
      }
      cursor = next;
      return;
    }
    if (ctrl && !alt && e.key.toLowerCase() === 'c') {
      if (inInput && searchEl && searchEl.selectionStart !== searchEl.selectionEnd) return;
      e.preventDefault();
      void copyStay();
      return;
    }
    if (ctrl && !alt && e.key.toLowerCase() === 'z' && !inInput) {
      e.preventDefault();
      void undo();
      return;
    }
    if (inInput) return;
    if (plain && e.key === 'Delete') {
      e.preventDefault();
      if (!e.repeat) void trash();
      return;
    }
    if (plain && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      const id = numberPick(grid[cursor.row] ?? [], Number(e.key));
      if (id !== null && !e.repeat) {
        cursor = { row: cursor.row, col: grid[cursor.row].indexOf(id) };
        selected = [];
        void copyAndClose([id]);
      }
      return;
    }
    if (plain && e.key === ' ') {
      e.preventDefault();
      if (cursorItem) preview = cursorItem;
      return;
    }
    if (plain && (e.key === 'p' || e.key === 'P')) {
      e.preventDefault();
      if (!e.repeat) void togglePin();
      return;
    }
    if (plain && (e.key === 'f' || e.key === 'F')) {
      e.preventDefault();
      if (!e.repeat) setPin(!pinnedOnly);
      return;
    }
    if (plain && e.key === '/') {
      e.preventDefault();
      focusSearch();
      return;
    }
    // Any other printable key starts a search.
    if (plain && e.key.length === 1 && !inInput) {
      e.preventDefault();
      focusSearch(e.key);
    }
  }

  onMount(() => {
    const unlisten: Array<() => void> = [];
    dark.addEventListener('change', applyTheme);
    applyTheme();
    (async () => {
      try {
        const [s, info] = await Promise.all([api.getSettings(), api.appInfo()]);
        systemLanguage = info.systemLanguage;
        applySettings(s);
      } catch (e) {
        console.error(e);
      }
      await refresh(true);
      unlisten.push(
        await on.shelfOpen(() => void reopen()),
        await on.clipAdded(refreshLive),
        await on.clipUpdated(refreshLive),
        await on.clipChanged(refreshLive),
        await on.settingsChanged((s) => {
          applySettings(s);
          refreshLive();
        }),
      );
    })();
    return () => {
      dark.removeEventListener('change', applyTheme);
      clearTimeout(searchTimer);
      clearTimeout(toastTimer);
      unlisten.forEach((u) => u());
    };
  });
</script>

<svelte:window {onkeydown} onblur={() => void hide()} />

{#if ready}
{#key openCount}
  <div class="panel" style:--card-w="{size.w}px" style:--card-h="{size.h}px">
    <header>
      <div class="search">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.5" /><path d="M10.5 10.5 14 14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
        <input
          bind:this={searchEl}
          bind:value={query}
          oninput={scheduleSearch}
          placeholder={t('search')}
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <button type="button" class="chip" class:on={pinnedOnly} aria-pressed={pinnedOnly} onclick={() => setPin(!pinnedOnly)} tabindex="-1">
        {t('pinnedOnly')} <kbd>F</kbd>
      </button>
    </header>

    <div class="rows">
      {#each rows as shelf, i (shelf)}
        <Row
          title={t(`rows.${shelf}`)}
          items={data[shelf]}
          active={i === cursor.row}
          {cursorId}
          {selected}
          emptyText={query.trim() || pinnedOnly ? t('empty.search') : t(`empty.${shelf}`)}
          onpick={pick}
        />
      {/each}
    </div>

    <footer>
      {#if toast}
        <span class="toast">{toast.text}{#if toast.undo}<button type="button" onclick={() => void undo()} tabindex="-1">{t('undo')}</button>{/if}</span>
      {:else if anything}
        <span class="hint"><kbd>Enter</kbd> {t('hint.copy')}</span>
        <span class="hint"><kbd>Ctrl</kbd>+<kbd>Enter</kbd> {t('hint.paste')}</span>
        <span class="hint"><kbd>P</kbd> {t('hint.pin')}</span>
        <span class="hint"><kbd>Space</kbd> {t('hint.preview')}</span>
        <span class="hint"><kbd>Del</kbd> {t('hint.delete')}</span>
      {/if}
    </footer>

    {#if preview}<Preview item={preview} onclose={() => (preview = null)} />{/if}
  </div>
{/key}
{/if}

<style>
  .panel {
    position: absolute;
    inset: 6px;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
    border-radius: var(--radius-xl);
    background: var(--elevated-bg);
    border: 1px solid var(--hairline);
    box-shadow: var(--shadow-lg), var(--glass-edge);
    color: var(--text);
    overflow: hidden;
    animation: slide-down var(--dur-slow) var(--ease-out) both;
  }
  :global([data-reduce-motion]) .panel { animation: none; }
  @keyframes slide-down {
    from { transform: translateY(-16px); opacity: 0; }
    to { transform: none; opacity: 1; }
  }
  header { display: flex; align-items: center; gap: var(--space-2); }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--control-h-lg);
    padding: 0 var(--space-3);
    border-radius: var(--radius-md);
    background: var(--field-bg);
    box-shadow: inset 0 0 0 0.5px var(--hairline);
    color: var(--text-secondary);
  }
  .search:focus-within { box-shadow: inset 0 0 0 1.5px var(--accent); }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text);
    font: var(--text-body);
  }
  input::placeholder { color: var(--text-tertiary); }
  .chip {
    height: var(--control-h-lg);
    padding: 0 var(--space-3);
    border: 0;
    border-radius: var(--radius-md);
    background: var(--control-bg);
    color: var(--text-secondary);
    font: var(--text-callout);
    cursor: default;
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .chip:hover { background: var(--control-bg-hover); }
  .chip.on { background: var(--accent); color: var(--on-accent); }
  kbd {
    font: var(--text-caption);
    padding: 1px 5px;
    border-radius: var(--radius-xs);
    background: var(--control-bg);
    color: var(--text-secondary);
  }
  .chip.on kbd { background: rgba(255, 255, 255, 0.25); color: inherit; }
  .rows {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    overflow-y: auto;
    scrollbar-width: thin;
  }
  footer {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    min-height: 18px;
    color: var(--text-tertiary);
    font: var(--text-caption);
  }
  .hint { display: inline-flex; align-items: center; gap: var(--space-1); }
  .toast { color: var(--text-secondary); display: inline-flex; gap: var(--space-3); align-items: center; }
  .toast button {
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    cursor: default;
    padding: 0;
  }
</style>
