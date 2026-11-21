<!--
  Line — the clothesline: up to three ropes (images, texts, links) across the top of the screen with the
  latest clips pinned on them. Click copies, drag leaves the window, right-click opens a menu.
  Zero work at rest: one shared rAF loop runs only while a card swings or falls, and there are no
  timers while the line is hidden. Reduce motion = no loop and no breeze.
  Never imports from src/app/.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorOf, on } from '../lib/ipc';
  import type { ClipItem, ClipShelf, Settings } from '../lib/types';
  import { fitCount, overflow, parseMode, splitRopes, visibleRopes, type RopeKey } from './logic';
  import { setLang, getLang, t } from './i18n';
  import { atRest, impulse, step, type Pend } from './physics';
  import Rope from './Rope.svelte';

  const LIST_LIMIT = 200;
  const SHELF: Record<RopeKey, ClipShelf> = { images: 'images', text: 'texts', links: 'links' };
  const HIDE_DELAY = 400;
  const PEEK_MS = 2500;
  const MENU_W = 196;

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
  const angles = $state<Record<number, number>>({});
  const falls = $state<Record<number, number>>({});

  const reduce = $derived(settings?.reduceMotion ?? false);
  const ropeCfg = $derived(settings?.lineRopes ?? { images: true, text: true, links: true });
  const ropes = $derived(visibleRopes(splitRopes(items, ropeCfg)));
  const fit = $derived(fitCount(width - 72));
  const order = $derived(ropes.map((r) => r.items.slice(0, fit).map((i) => i.id)));
  const empty = $derived(ropes.length === 0);

  const pend = new Map<number, Pend>();
  let raf = 0;
  let lastFrame = 0;
  let dirty = true;
  let seq = 0;
  let known = new Set<number>();
  /** false until the first list is in (that one never plays the fall). */
  let knownReady = false;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
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
      if (tt >= 1) delete falls[id];
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
  function markDirty() {
    dirty = true;
    if (shown) void load();
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
  function show(mode: 'hotkey' | 'peek' | 'reveal') {
    clearTimeout(hideTimer);
    clearTimeout(peekTimer);
    shown = true;
    if (dirty) void load();
    // A little ripple on arrival.
    if (!reduce) {
      order.flat().slice(0, 14).forEach((id, i) => setTimeout(() => shown && nudge(id, (i % 2 ? 1 : -1) * 0.35), i * 25));
    }
    scheduleBreeze();
    if (mode === 'peek' && !pointerInside) peekTimer = setTimeout(() => !pointerInside && void hide(), PEEK_MS);
  }

  function hidden() {
    shown = false;
    menu = null;
    selected = [];
    anchor = null;
    toast = null;
    clearTimeout(hideTimer);
    clearTimeout(peekTimer);
    clearTimeout(breezeTimer);
    clearTimeout(clickTimer);
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
    if (menu) return; // the menu lives outside the strip: the pointer going onto it is not leaving
    hideTimer = setTimeout(() => {
      if (!dragging && !menu) void hide();
    }, HIDE_DELAY);
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
    const id = item?.id ?? hoverId;
    return id === null ? [] : [id];
  }

  async function copy(ids: number[]) {
    if (ids.length === 0) return;
    try {
      await api.clipCopy(ids);
      copiedId = ids[0];
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedId = null), 1100);
    } catch (e) {
      fail(e);
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
    const h = item.kind === 'image' ? 250 : 190;
    menu = { x: Math.max(4, Math.min(e.clientX, width - MENU_W - 4)), y: Math.max(4, Math.min(e.clientY, window.innerHeight - h - 4)), item };
  }

  async function menuDo(fn: () => Promise<void> | void) {
    menu = null;
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
    if (menu && !menuEl?.contains(e.target as Node)) menu = null;
  }

  function onkeydown(e: KeyboardEvent) {
    const altGr = e.getModifierState('AltGraph');
    const ctrl = !altGr && (e.ctrlKey || e.metaKey);
    if (e.key === 'Escape') {
      e.preventDefault();
      if (menu) menu = null;
      else void hide();
    } else if (e.key === 'Delete' && !ctrl) {
      e.preventDefault();
      if (!e.repeat) void trash(targetsFor(null));
    } else if (ctrl && e.key.toLowerCase() === 'z') {
      e.preventDefault();
      void undo();
    } else if (ctrl && e.key.toLowerCase() === 'c') {
      e.preventDefault();
      void copy(targetsFor(null));
    }
  }

  onMount(() => {
    const unlisten: Array<() => void> = [];
    let disposed = false;
    dark.addEventListener('change', applyTheme);
    applyTheme();
    document.addEventListener('visibilitychange', scheduleBreeze);
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
      // The first show may have happened before the listeners existed: Rust passes its mode in the URL.
      const first = parseMode(window.location.search);
      if (first && !shown) show(first);
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
      if (raf) cancelAnimationFrame(raf);
      for (const timer of [hideTimer, peekTimer, breezeTimer, toastTimer, copiedTimer, clickTimer]) clearTimeout(timer);
      unlisten.forEach((u) => u());
    };
  });
</script>

<svelte:window bind:innerWidth={width} {onkeydown} {onpointerdown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="strip" class:shown onpointerenter={enter} onpointerleave={leave}>
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
      onclick={pick}
      ondblclick={openItem}
      ondragout={dragOut}
      onremove={(item) => void trash(targetsFor(item))}
      oncontext={openMenu}
      onhover={onHover}
      onmore={() => void api.showShelf().then(hide).catch(fail)}
    />
    {#if i === 0 && paused}<span class="paused">{t('paused')}</span>{/if}
  {/each}
  {#if empty}
    <div class="empty">
      <span>{t('empty')}</span>
      {#if paused}<b>{t('paused')}</b>{/if}
    </div>
  {:else if paused && ropes.length === 0}
    <span class="paused">{t('paused')}</span>
  {/if}

  {#if toast}
    <div class="toast" role="status">
      <span>{toast.text}</span>
      {#if toast.undo}<button type="button" tabindex="-1" onclick={() => void undo()}>{t('undo')}</button>{/if}
    </div>
  {/if}
</div>

{#if menu}
  {@const it = menu.item}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="menu" role="menu" tabindex="-1" bind:this={menuEl} onpointerenter={enter} onpointerleave={leave} style:left="{menu.x}px" style:top="{menu.y}px" style:width="{MENU_W}px">
    <button type="button" role="menuitem" onclick={() => menuDo(() => copy(targetsFor(it)))}>{t('menu.copy')}</button>
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
    padding: 4px 0;
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
  .empty {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    justify-content: center;
    width: fit-content;
    margin: var(--space-4) auto;
    padding: var(--space-2) var(--space-4);
    border-radius: 999px;
    background: var(--elevated-bg);
    box-shadow: var(--shadow-md);
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
