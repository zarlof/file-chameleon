<!--
  Line — the clothesline: up to three ropes (images, texts, links) across the top of the screen with the
  latest clips pinned on them. Click copies, drag leaves the window, right-click opens a menu.
  Zero work at rest: one shared rAF loop runs only while a card swings or falls, and there are no
  timers while the line is hidden. Reduce motion = no loop and no breeze.
  Never imports from src/app/.
-->
<script lang="ts">
  import { onMount, tick as afterRender } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api, errorOf, on } from '../lib/ipc';
  import type { ClipItem, ClipShelf, Settings } from '../lib/types';
  import { fitCount, matches, overflow, splitRopes, visibleRopes, type RopeKey } from './logic';
  import { bandTops, cardOpacity, ropeColor, sizeInfo, windowHeight } from './layout';
  import { isCommandKey, nextFocus, type Focus, type NavKey } from './keys';
  import { quiet, tick as hangTick } from './sound';
  import Preview from './Preview.svelte';
  import { setLang, getLang, t } from './i18n';
  import { atRest, impulse, step, type Pend } from './physics';
  import Rope from './Rope.svelte';

  const LIST_LIMIT = 200;
  const SHELF: Record<RopeKey, ClipShelf> = { images: 'images', text: 'texts', links: 'links' };
  const HIDE_DELAY = 400;
  const RELOAD_DEBOUNCE = 150;
  const PEEK_MS = 2500;
  const MENU_W = 196;
  const ZOOM_W = 2;

  let settings = $state<Settings | null>(null);
  let items = $state<ClipItem[]>([]);
  let paused = $state(false);
  let shown = $state(false);
  let width = $state(window.innerWidth);
  let selected = $state<number[]>([]);
  let anchor: number | null = null;
  let copiedId = $state<number | null>(null);
  let hoverId: number | null = null;
  let toast = $state<{ text: string; undo: boolean } | null>(null);
  let menu = $state<{ x: number; y: number; item: ClipItem } | null>(null);
  let menuEl = $state<HTMLElement | null>(null);
  let query = $state('');
  let searchOpen = $state(false);
  let searchEl = $state<HTMLInputElement | null>(null);
  let searchFocused = $state(false);
  /** Keyboard focus (rope row, card index). Null until a key moves it; the mouse clears it. */
  let focus = $state<Focus | null>(null);
  let previewOn = $state(false);
  let zoom = $state<{ item: ClipItem; rect: DOMRect } | null>(null);
  const angles = $state<Record<number, number>>({});
  const falls = $state<Record<number, number>>({});

  const reduce = $derived(settings?.reduceMotion ?? false);
  const ropeCfg = $derived(settings?.lineRopes ?? { images: true, text: true, links: true });
  // The ropes that exist do not depend on the search (the window height does not jump while typing).
  const baseRopes = $derived(visibleRopes(splitRopes(items, ropeCfg)));
  const ropes = $derived(baseRopes.map((r) => ({ ...r, items: query.trim() ? r.items.filter((i) => matches(i, query)) : r.items })));
  const size = $derived(sizeInfo(settings?.lineSize ?? 'medium'));
  const sizeName = $derived(settings?.lineSize ?? 'medium');
  const tops = $derived(bandTops(Math.max(1, ropes.length), sizeName));
  const stripH = $derived(windowHeight(Math.max(1, ropes.length), sizeName));
  const op = $derived(Math.min(100, Math.max(30, settings?.lineOpacity ?? 100)) / 100);
  const cardOp = $derived(cardOpacity(op * 100));
  const thickness = $derived(Math.min(6, Math.max(1, settings?.lineThickness ?? 2)));
  const color = $derived(ropeColor(settings?.lineColor ?? 'accent', 'var(--accent)'));
  const fit = $derived(fitCount(width - 72, size.card));
  const order = $derived(ropes.map((r) => r.items.slice(0, fit).map((i) => i.id)));
  const empty = $derived(ropes.length === 0);
  const focusedId = $derived(focus ? (order[focus.r]?.[focus.i] ?? null) : null);
  const byId = $derived(new Map(items.map((i) => [i.id, i])));
  $effect(() => {
    if (focusedId === null) previewOn = false;
  });
  const previewItem = $derived(previewOn && focusedId !== null ? (byId.get(focusedId) ?? null) : null);

  const pend = new Map<number, Pend>();
  let raf = 0;
  let lastFrame = 0;
  let dirty = true;
  let seq = 0;
  let known = new Set<number>();
  /** false until the first list is in (that one never plays the fall). */
  let knownReady = false;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let reloadTimer: ReturnType<typeof setTimeout> | undefined;
  let peekTimer: ReturnType<typeof setTimeout> | undefined;
  let breezeTimer: ReturnType<typeof setTimeout> | undefined;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  let pointerInside = false;
  let dragging = false;
  let systemLanguage: 'tr' | 'en' = 'en';
  const undoStack: number[][] = [];

  // ------------------------------------------------------------ theme / settings
  const dark = window.matchMedia('(prefers-color-scheme: dark)');
  let themeSetting: Settings['theme'] = 'system';

  function applyTheme() {
    document.documentElement.dataset.theme = themeSetting === 'system' ? (dark.matches ? 'dark' : 'light') : themeSetting;
  }

  function applySettings(s: Settings) {
    const ropesChanged = settings !== null && JSON.stringify(settings.lineRopes) !== JSON.stringify(s.lineRopes);
    settings = s;
    themeSetting = s.theme;
    const root = document.documentElement;
    root.dataset.accent = s.accent;
    if (s.reduceMotion) root.dataset.reduceMotion = '';
    else delete root.dataset.reduceMotion;
    setLang(s.language, systemLanguage);
    root.lang = getLang();
    applyTheme();
    if (s.reduceMotion) stopLoop();
    if (ropesChanged) markDirty();
    scheduleBreeze();
  }

  // ------------------------------------------------------------ animation loop
  function kick() {
    if (reduce || !shown || raf) return;
    lastFrame = performance.now();
    raf = requestAnimationFrame(frame);
  }

  function stopLoop() {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    pend.clear();
    for (const k of Object.keys(angles)) delete angles[Number(k)];
    for (const k of Object.keys(falls)) delete falls[Number(k)];
  }

  function frame(now: number) {
    const dt = Math.min(0.05, (now - lastFrame) / 1000);
    lastFrame = now;
    let moving = false;
    for (const [id, p] of pend) {
      const next = step(p, dt);
      if (atRest(next)) {
        pend.delete(id);
        delete angles[id];
      } else {
        pend.set(id, next);
        angles[id] = next.theta;
        moving = true;
      }
    }
    for (const k of Object.keys(falls)) {
      const id = Number(k);
      const tt = falls[id] + dt / 0.55;
      if (tt >= 1) {
        delete falls[id];
        if (settings?.lineSound) hangTick();
      }
      else {
        falls[id] = tt;
        moving = true;
      }
    }
    raf = moving ? requestAnimationFrame(frame) : 0;
  }

  function nudge(id: number, dOmega: number) {
    if (reduce) return;
    pend.set(id, impulse(pend.get(id) ?? { theta: 0, omega: 0 }, dOmega));
    kick();
  }

  function onHover(item: ClipItem, dir: number) {
    hoverId = item.id;
    focus = null;
    nudge(item.id, 0.6 * dir);
  }

  function scheduleBreeze() {
    clearTimeout(breezeTimer);
    if (!shown || reduce || !settings?.lineBreeze || document.hidden) return;
    breezeTimer = setTimeout(
      () => {
        const ids = order.flat();
        const n = Math.min(ids.length, 1 + Math.floor(Math.random() * 3));
        for (let i = 0; i < n; i++) nudge(ids[Math.floor(Math.random() * ids.length)], (Math.random() < 0.5 ? -1 : 1) * 0.5);
        scheduleBreeze();
      },
      3000 + Math.random() * 3000,
    );
  }

  // ------------------------------------------------------------ data
  /** The list is stale. While shown, reload after a short quiet period (a bulk import fires many events). */
  function markDirty() {
    dirty = true;
    if (!shown) return;
    clearTimeout(reloadTimer);
    reloadTimer = setTimeout(() => shown && dirty && void load(), RELOAD_DEBOUNCE);
  }

  async function load() {
    const my = ++seq;
    dirty = false;
    const wanted = (['images', 'text', 'links'] as const).filter((k) => ropeCfg[k]);
    let lists: ClipItem[][];
    try {
      lists = await Promise.all(wanted.map((k) => api.clipList({ shelf: SHELF[k], limit: LIST_LIMIT })));
    } catch (e) {
      console.error(e);
      dirty = true;
      return;
    }
    if (my !== seq) return;
    const next = lists.flat();
    const fresh = knownReady && shown && !reduce ? next.filter((i) => !known.has(i.id)).slice(0, 4) : [];
    known = new Set(next.map((i) => i.id));
    knownReady = true;
    items = next;
    selected = selected.filter((id) => known.has(id));
    for (const i of fresh) falls[i.id] = 0;
    if (fresh.length > 0) kick();
  }

  // ------------------------------------------------------------ show / hide
  function show(mode: 'hotkey' | 'peek' | 'reveal' | 'search') {
    clearTimeout(hideTimer);
    clearTimeout(peekTimer);
    shown = true;
    if (dirty) void load();
    // A little ripple on arrival.
    if (!reduce) {
      order.flat().slice(0, 14).forEach((id, i) => setTimeout(() => shown && nudge(id, (i % 2 ? 1 : -1) * 0.35), i * 25));
    }
    scheduleBreeze();
    // A reveal under a pointer that is already inside must not auto-hide: ask the browser, and pointermove confirms.
    if (document.documentElement.matches(':hover')) pointerInside = true;
    if (mode === 'search') {
      searchOpen = true;
      void afterRender().then(() => searchEl?.focus());
    }
    if ((mode === 'peek' || mode === 'reveal') && !pointerInside) {
      peekTimer = setTimeout(() => !pointerInside && !document.documentElement.matches(':hover') && void hide(), PEEK_MS);
    }
  }

  function hidden() {
    shown = false;
    quiet();
    // WebView2 may not send pointerleave when the window hides under the pointer.
    pointerInside = false;
    hoverId = null;
    menu = null;
    query = '';
    searchOpen = false;
    searchFocused = false;
    focus = null;
    previewOn = false;
    zoom = null;
    selected = [];
    anchor = null;
    toast = null;
    clearTimeout(hideTimer);
    clearTimeout(peekTimer);
    clearTimeout(breezeTimer);
    clearTimeout(clickTimer);
    clearTimeout(reloadTimer);
    stopLoop();
  }

  async function hide() {
    hidden();
    try {
      await api.lineHide();
    } catch (e) {
      console.error(e);
    }
  }

  function leave() {
    pointerInside = false;
    hoverId = null;
    clearTimeout(peekTimer);
    clearTimeout(hideTimer);
    zoom = null;
    if (menu || searchFocused || query || previewOn) return; // in use: Esc or a click outside closes it
    hideTimer = setTimeout(() => {
      if (!dragging && !menu) void hide();
    }, HIDE_DELAY);
  }

  /** Close the context menu; with the pointer outside the window the hide timer starts again. */
  function closeMenu() {
    if (!menu) return;
    menu = null;
    if (!pointerInside) leave();
  }

  function enter() {
    pointerInside = true;
    clearTimeout(hideTimer);
    clearTimeout(peekTimer);
  }

  // ------------------------------------------------------------ actions
  function showToast(text: string, undo = false) {
    clearTimeout(toastTimer);
    toast = { text, undo };
    toastTimer = setTimeout(() => (toast = null), 3500);
  }

  function fail(e: unknown) {
    console.error(e);
    showToast(errorOf(e).message || t('failed'));
  }

  function targetsFor(item: ClipItem | null): number[] {
    if (item && !selected.includes(item.id)) return [item.id];
    if (selected.length > 0) return selected;
    const id = item?.id ?? focusedId ?? hoverId;
    return id === null ? [] : [id];
  }

  /** True when the copy happened. */
  async function copy(ids: number[]): Promise<boolean> {
    if (ids.length === 0) return false;
    try {
      await api.clipCopy(ids);
      copiedId = ids[0];
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedId = null), 1100);
      return true;
    } catch (e) {
      fail(e);
      return false;
    }
  }

  // A double-click delivers two clicks first: the single-click action waits a moment and is cancelled by dblclick.
  let clickTimer: ReturnType<typeof setTimeout> | undefined;
  function pick(item: ClipItem, e: MouseEvent) {
    clearTimeout(clickTimer);
    const mods = { shiftKey: e.shiftKey, ctrlKey: e.ctrlKey, metaKey: e.metaKey } as MouseEvent;
    clickTimer = setTimeout(() => pickNow(item, mods), 220);
  }

  function pickNow(item: ClipItem, e: MouseEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    if (e.shiftKey || ctrl) {
      const row = order.find((ids) => ids.includes(item.id)) ?? [];
      if (e.shiftKey && anchor !== null && row.includes(anchor)) {
        const a = row.indexOf(anchor);
        const b = row.indexOf(item.id);
        const range = row.slice(Math.min(a, b), Math.max(a, b) + 1);
        selected = ctrl ? [...new Set([...selected, ...range])] : range;
      } else {
        selected = selected.includes(item.id) ? selected.filter((x) => x !== item.id) : [...selected, item.id];
        anchor = item.id;
      }
      return;
    }
    selected = [];
    anchor = item.id;
    void copy([item.id]);
  }

  async function openItem(item: ClipItem) {
    clearTimeout(clickTimer);
    try {
      if (item.kind === 'image' || item.kind === 'text') {
        await api.lineOpenItem(item.id);
        await hide();
      } else if (item.text) {
        await api.openUrl(item.text.trim());
        await hide();
      }
    } catch (e) {
      fail(e);
    }
  }

  async function dragOut(item: ClipItem) {
    dragging = true;
    menu = null;
    zoom = null;
    try {
      await api.lineDragOut(item.id);
    } catch (e) {
      console.error(e);
      showToast(t('dragFailed'));
    } finally {
      dragging = false;
      if (!pointerInside) leave();
    }
  }

  async function trash(ids: number[]) {
    if (ids.length === 0) return;
    try {
      await api.clipTrash(ids);
      undoStack.push(ids);
      selected = [];
      showToast(t('trashed', { count: ids.length }), true);
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function undo() {
    const ids = undoStack[undoStack.length - 1];
    if (!ids) return;
    try {
      await api.clipRestore(ids);
      undoStack.pop();
      showToast(t('restored'));
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function togglePin(item: ClipItem) {
    try {
      await api.clipPin([item.id], !item.pinned);
      await load();
    } catch (e) {
      fail(e);
    }
  }

  // ------------------------------------------------------------ context menu
  function openMenu(item: ClipItem, e: MouseEvent) {
    zoom = null;
    const h = item.kind === 'image' ? 250 : 190;
    menu = { x: Math.max(4, Math.min(e.clientX, width - MENU_W - 4)), y: Math.max(4, Math.min(e.clientY, window.innerHeight - h - 4)), item };
  }

  async function menuDo(fn: () => Promise<void> | void) {
    closeMenu();
    try {
      await fn();
    } catch (e) {
      fail(e);
    }
  }

  // ------------------------------------------------------------ input
  function onpointerdown(e: PointerEvent) {
    // A peek / edge reveal does not own the focus: the first click takes it before anything else.
    if (!document.hasFocus()) void api.lineActivate().catch(() => {});
    if (menu && !menuEl?.contains(e.target as Node)) closeMenu();
  }

  function itemOf(id: number | null): ClipItem | null {
    return id === null ? null : (byId.get(id) ?? null);
  }

  /** Make sure the keyboard has a focus: the hovered card, else the first card. */
  function ensureFocus(): boolean {
    if (focus && focusedId !== null) return true;
    const counts = order.map((r) => r.length);
    let f: Focus | null = null;
    if (hoverId !== null) {
      const r = order.findIndex((ids) => ids.includes(hoverId!));
      if (r >= 0) f = { r, i: order[r].indexOf(hoverId) };
    }
    focus = f ?? nextFocus(counts, null, 'ArrowRight');
    return focus !== null;
  }

  function startSearch(text: string) {
    searchOpen = true;
    query = text;
    previewOn = false;
    void afterRender().then(() => {
      searchEl?.focus();
      searchEl?.setSelectionRange(query.length, query.length);
    });
  }

  /** Enter in the search field: copy the newest match and close. */
  async function copyNewestMatch() {
    const all = ropes.flatMap((r) => r.items);
    if (all.length === 0) return;
    const best = all.reduce((a, b) => (b.createdAt > a.createdAt || (b.createdAt === a.createdAt && b.id > a.id) ? b : a));
    if (await copy([best.id])) await hide();
  }

  function onkeydown(e: KeyboardEvent) {
    const inField = e.target === searchEl;
    if (e.key === 'Escape') {
      e.preventDefault();
      if (previewOn) previewOn = false;
      else if (menu) closeMenu();
      else if (selected.length > 0) selected = [];
      else if (query) query = '';
      else void hide();
      return;
    }
    if (inField) {
      if (e.key === 'Enter') {
        e.preventDefault();
        void copyNewestMatch();
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || (e.key === 'Tab' && !e.shiftKey)) {
        // leave the field: arrows, digits, Space and Del now act on the cards
        e.preventDefault();
        searchEl?.blur();
        ensureFocus();
      }
      return; // everything else is text
    }
    const altGr = e.getModifierState('AltGraph');
    const ctrl = !altGr && (e.ctrlKey || e.metaKey);
    const key = e.key;
    if (!isCommandKey(e, false)) {
      // Any other printable character starts a search (the shelf does the same).
      if (key.length === 1 && !ctrl && (!e.altKey || altGr) && !previewOn && !menu) {
        e.preventDefault();
        startSearch(searchOpen ? query + key : key);
      }
      return;
    }
    e.preventDefault();
    if (ctrl) {
      const k = key.toLowerCase();
      if (key === 'Enter') {
        const ids = targetsFor(null);
        if (ids.length > 0) void api.linePaste(ids).catch(fail);
      } else if (k === 'z') void undo();
      else if (k === 'c' && e.shiftKey) {
        const it = itemOf(targetsFor(null)[0] ?? null);
        if (it?.kind === 'image') void api.clipCopyOcr(it.id).then(() => showToast(t('ocrCopied'))).catch(fail);
      } else if (k === 'c') void copy(targetsFor(null));
      return;
    }
    if (key === 'Enter') {
      void (async () => {
        const ids = targetsFor(null);
        if (await copy(ids)) await hide();
      })();
    } else if (key === 'Delete') {
      if (!e.repeat) void trash(targetsFor(null));
    } else if (key === ' ') {
      if (e.repeat) return;
      if (previewOn) previewOn = false;
      else if (ensureFocus()) {
        zoom = null;
        previewOn = true;
      }
    } else if (key.startsWith('Arrow') || key === 'Home' || key === 'End') {
      // the first arrow press only shows the focus (on the hovered or first card); the next ones move it
      if (focus === null || focusedId === null) ensureFocus();
      else focus = nextFocus(order.map((r) => r.length), focus, key as NavKey);
      zoom = null;
    } else if (/^[1-9]$/.test(key)) {
      if (!ensureFocus() || !focus) return;
      const n = Number(key) - 1;
      if (n < order[focus.r].length) focus = { r: focus.r, i: n };
    } else if (key === 'p' || key === 'P') {
      const it = itemOf(targetsFor(null)[0] ?? null);
      if (it && !e.repeat) void togglePin(it);
    } else if (key === 'e' || key === 'E') {
      const it = itemOf(targetsFor(null)[0] ?? null);
      if (it && (it.kind === 'text' || it.kind === 'image')) void openItem(it);
    }
  }

  onMount(() => {
    const unlisten: Array<() => void> = [];
    let disposed = false;
    dark.addEventListener('change', applyTheme);
    applyTheme();
    document.addEventListener('visibilitychange', scheduleBreeze);
    // Pointer inside = anywhere in the window (ropes, gaps, the menu), not just on the strip.
    const root = document.documentElement;
    root.addEventListener('pointerenter', enter);
    root.addEventListener('pointerleave', leave);
    // A pointer that was already inside when the window appeared sends no enter: the first move says so.
    const moved = () => {
      if (!pointerInside) enter();
    };
    root.addEventListener('pointermove', moved, { passive: true });
    (async () => {
      // Listeners first: the window starts hidden and the first show must never be missed.
      // While hidden nothing loads, animates or times: events only mark the list stale.
      const keep = (u: () => void) => (disposed ? u() : unlisten.push(u));
      keep(await on.lineShow((p) => show(p.mode)));
      keep(await on.lineHide(hidden));
      keep(await on.clipAdded(() => markDirty()));
      keep(await on.clipUpdated(() => markDirty()));
      keep(await on.clipChanged(() => markDirty()));
      keep(await on.clipPaused((v) => (paused = v)));
      keep(await on.settingsChanged(applySettings));
      // A show may have happened before the listeners existed: ask Rust for the latest mode.
      try {
        const st = await api.lineState();
        if (st.visible && st.mode && !shown && !disposed) show(st.mode);
      } catch (e) {
        console.error(e);
      }
      try {
        const [s, info, p] = await Promise.all([api.getSettings(), api.appInfo(), api.clipGetPaused()]);
        systemLanguage = info.systemLanguage;
        paused = p;
        applySettings(s);
      } catch (e) {
        console.error(e);
      }
      if (shown) await load(); // shown while the settings were still loading
    })();
    return () => {
      disposed = true;
      dark.removeEventListener('change', applyTheme);
      document.removeEventListener('visibilitychange', scheduleBreeze);
      root.removeEventListener('pointerenter', enter);
      root.removeEventListener('pointerleave', leave);
      root.removeEventListener('pointermove', moved);
      if (raf) cancelAnimationFrame(raf);
      for (const timer of [hideTimer, peekTimer, breezeTimer, toastTimer, copiedTimer, clickTimer, reloadTimer]) clearTimeout(timer);
      unlisten.forEach((u) => u());
    };
  });
</script>

<svelte:window bind:innerWidth={width} {onkeydown} {onpointerdown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="strip" class:shown style:height="{stripH}px">
  {#each ropes as rope, i (rope.key)}
    <Rope
      label={t(`rope.${rope.key}`)}
      items={rope.items.slice(0, fit)}
      {width}
      more={overflow(rope.items.length, fit)}
      {angles}
      {falls}
      selected={new Set(selected)}
      {copiedId}
      {focusedId}
      top={tops[i]}
      band={size.band}
      card={size.card}
      scale={size.scale}
      {color}
      {thickness}
      opacity={op}
      {cardOp}
      zoomOn={settings?.lineZoomOnHover ?? false}
      onzoom={(item, rect) => (zoom = shown && item && rect ? { item, rect } : null)}
      emptyText={query.trim() ? t('search.none') : ''}
      onclick={pick}
      ondblclick={openItem}
      ondragout={dragOut}
      onremove={(item) => void trash(targetsFor(item))}
      oncontext={openMenu}
      onhover={onHover}
      onmore={() => void api.showShelf().then(hide).catch(fail)}
    />
  {/each}
  {#if paused && !empty}<span class="paused" class:shift={searchOpen}>{t('paused')}</span>{/if}
  {#if empty}
    <div class="empty" style:top="{tops[0]}px" style:height="{size.band}px" style:--op={op}>
      <span>{t('empty')}</span>
      {#if paused}<b>{t('paused')}</b>{/if}
    </div>
  {/if}

  {#if searchOpen}
    <input
      class="search"
      type="search"
      bind:this={searchEl}
      bind:value={query}
      placeholder={t('search.placeholder')}
      aria-label={t('search.placeholder')}
      spellcheck="false"
      autocomplete="off"
      onfocus={() => (searchFocused = true)}
      onblur={() => (searchFocused = false)}
    />
  {/if}

  {#if previewItem}<Preview item={previewItem} onclose={() => (previewOn = false)} />{/if}

  {#if toast}
    <div class="toast" role="status">
      <span>{toast.text}</span>
      {#if toast.undo}<button type="button" tabindex="-1" onclick={() => void undo()}>{t('undo')}</button>{/if}
    </div>
  {/if}
</div>

{#if zoom && !previewOn}
  {@const z = zoom}
  {@const file = z.item.edited ?? z.item.image ?? z.item.thumb}
  {@const zw = Math.min(z.rect.width * ZOOM_W, width - 8)}
  {@const ratio = z.item.width && z.item.height ? z.item.height / z.item.width : 0.75}
  {@const zh = Math.min(zw * ratio, Math.max(60, stripH - 8))}
  {#if file}
    <img
      class="zoom"
      src={convertFileSrc(file)}
      alt=""
      style:width="{zw}px"
      style:height="{zh}px"
      style:left="{Math.max(4, Math.min(z.rect.left + z.rect.width / 2 - zw / 2, width - zw - 4))}px"
      style:top="{Math.max(4, Math.min(z.rect.bottom + 6, stripH - zh - 4))}px"
    />
  {/if}
{/if}

{#if menu}
  {@const it = menu.item}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="menu" role="menu" tabindex="-1" bind:this={menuEl} style:left="{menu.x}px" style:top="{menu.y}px" style:width="{MENU_W}px">
    <button type="button" role="menuitem" onclick={() => menuDo(async () => { await copy(targetsFor(it)); })}>{t('menu.copy')}</button>
    <button type="button" role="menuitem" onclick={() => menuDo(() => api.linePaste(targetsFor(it)))}>{t('menu.paste')}</button>
    {#if it.kind === 'image' || it.kind === 'text'}
      <button type="button" role="menuitem" onclick={() => menuDo(() => openItem(it))}>{t('menu.edit')}</button>
    {/if}
    <button type="button" role="menuitem" onclick={() => menuDo(() => togglePin(it))}>{it.pinned ? t('menu.unpin') : t('menu.pin')}</button>
    {#if it.kind === 'image'}
      {@const file = it.edited ?? it.image}
      {#if file}
        <button type="button" role="menuitem" onclick={() => menuDo(async () => { await api.floatOpen([file]); })}>{t('menu.float')}</button>
      {/if}
      <button type="button" role="menuitem" onclick={() => menuDo(async () => { await api.clipSaveImage(it.id, 'png'); showToast(t('saved')); })}>{t('menu.save')}</button>
    {/if}
    <hr />
    <button type="button" role="menuitem" class="danger" onclick={() => menuDo(() => trash(targetsFor(it)))}>{t('menu.delete')}</button>
  </div>
{/if}

<style>
  .strip {
    position: fixed;
    inset: 0 0 auto 0;
    transform: translateY(-104%);
    transition: transform var(--dur-slow) var(--ease-out);
    font: var(--text-body);
    color: var(--text);
  }
  .strip.shown { transform: none; }
  :global([data-reduce-motion]) .strip { transition: none; }

  .paused {
    position: absolute;
    right: 16px;
    top: 8px;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--control-bg);
    color: var(--text-secondary);
    font: var(--text-caption);
    z-index: 3;
  }
  .paused.shift { right: 230px; }
  .search {
    position: absolute;
    top: 6px;
    right: 14px;
    z-index: 4;
    width: 204px;
    height: 26px;
    padding: 0 10px;
    border: 0;
    border-radius: 999px;
    background: var(--elevated-bg);
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--glass-edge), var(--shadow-sm);
    font: var(--text-callout);
    outline: none;
  }
  .search:focus { box-shadow: inset 0 0 0 2px var(--accent), var(--shadow-sm); }
  .zoom {
    position: fixed;
    z-index: 9;
    object-fit: contain;
    border-radius: var(--radius-sm);
    background: var(--elevated-bg);
    box-shadow: var(--shadow-lg);
    pointer-events: none;
  }
  .empty {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    gap: var(--space-2);
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--window-bg) calc(var(--op) * 38%), transparent);
    color: var(--text-secondary);
    font: var(--text-callout);
  }
  .empty b { color: var(--accent); font-weight: 600; }

  .toast {
    position: absolute;
    left: 50%;
    top: 10px;
    translate: -50% 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-md);
    background: var(--elevated-bg);
    box-shadow: var(--shadow-lg);
    font: var(--text-callout);
    z-index: 5;
  }
  .toast button {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-weight: 600;
    cursor: default;
  }

  .menu {
    position: fixed;
    z-index: 10;
    display: flex;
    flex-direction: column;
    padding: var(--space-1);
    border-radius: var(--radius-md);
    background: var(--elevated-bg);
    box-shadow: var(--shadow-lg), inset 0 0 0 1px var(--glass-edge);
    font: var(--text-callout);
  }
  .menu button {
    padding: 6px var(--space-3);
    border: 0;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: default;
  }
  .menu button:hover { background: var(--accent); color: var(--on-accent); }
  .menu .danger { color: var(--red); }
  .menu .danger:hover { background: var(--red); color: var(--on-accent); }
  .menu hr { width: 100%; height: 1px; margin: var(--space-1) 0; border: 0; background: var(--separator); }
</style>
