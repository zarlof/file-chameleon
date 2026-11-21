<!--
  Line — the clothesline: up to four ropes (images, texts, links, files) across the top (or bottom) of the screen
  with the latest clips and files pinned on them. Click copies, drag leaves the window, right-click opens a menu.
  Zero work at rest: one shared rAF loop runs only while a card swings or falls, and there are no
  timers while the line is hidden. Reduce motion = no loop and no breeze.
  Never imports from src/app/.
-->
<script lang="ts">
  import { onMount, tick as afterRender } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api, errorOf, on } from '../lib/ipc';
  import type { ClipItem, ClipShelf, LineFile, Settings } from '../lib/types';
  import { fitCount, matches, overflow, ropeCards, ropeName, splitRopes, visibleRopes, type LineCard } from './logic';
  import { fileId } from './files';
  import { bandTops, cardOpacity, cardStyleAttr, ropeColor, sizeInfo, windowHeight } from './layout';
  import { isCommandKey, nextFocus, type Focus, type NavKey } from './keys';
  import { quiet, tick as hangTick } from './sound';
  import Preview from './Preview.svelte';
  import { setLang, getLang, t } from './i18n';
  import { atRest, impulse, step, swingK, type Pend } from './physics';
  import Rope from './Rope.svelte';

  const LIST_LIMIT = 200;
  const SHELF: Record<'images' | 'text' | 'links', ClipShelf> = { images: 'images', text: 'texts', links: 'links' };
  const RELOAD_DEBOUNCE = 150;
  const MENU_W = 196;
  const ZOOM_W = 2;

  let settings = $state<Settings | null>(null);
  let items = $state<ClipItem[]>([]);
  let files = $state<LineFile[]>([]);
  let paused = $state(false);
  let shown = $state(false);
  let width = $state(window.innerWidth);
  let selected = $state<number[]>([]);
  let anchor: number | null = null;
  let copiedId = $state<number | null>(null);
  let hoverId = $state<number | null>(null);
  let toast = $state<{ text: string; undo: boolean } | null>(null);
  let menu = $state<{ x: number; y: number; card: LineCard } | null>(null);
  let menuEl = $state<HTMLElement | null>(null);
  let query = $state('');
  let searchOpen = $state(false);
  let searchEl = $state<HTMLInputElement | null>(null);
  let searchFocused = $state(false);
  /** Keyboard focus (rope row, card index). Null until a key moves it; the mouse clears it. */
  let focus = $state<Focus | null>(null);
  let previewOn = $state(false);
  let zoom = $state<{ item: ClipItem; rect: DOMRect } | null>(null);
  let isDark = $state(true);
  const angles = $state<Record<number, number>>({});
  const falls = $state<Record<number, number>>({});

  const reduce = $derived(settings?.reduceMotion ?? false);
  const ropeCfg = $derived(settings?.lineRopes ?? { images: true, text: true, links: true, files: true });
  // The ropes that exist do not depend on the search (the window height does not jump while typing).
  const shownFiles = $derived.by(() => {
    const src = settings?.lineFilesSources ?? { added: true, converted: true, downloaded: true };
    return files.filter((f) => src[f.source]);
  });
  const baseRopes = $derived(visibleRopes(splitRopes(items, ropeCfg, settings?.lineRopeOrder, shownFiles)));
  const ropes = $derived(
    baseRopes.map((r) => {
      const q = query.trim();
      const filtered = q
        ? { ...r, items: r.items.filter((i) => matches(i, q)), files: r.files.filter((f) => matches(fileAsText(f), q)) }
        : r;
      return { ...filtered, cards: ropeCards(filtered, fileId) };
    }),
  );
  const size = $derived(sizeInfo(settings?.lineSize ?? 'medium'));
  const sizeName = $derived(settings?.lineSize ?? 'medium');
  const tops = $derived(bandTops(Math.max(1, ropes.length), sizeName));
  const stripH = $derived(windowHeight(Math.max(1, ropes.length), sizeName));
  const bottom = $derived(settings?.lineEdge === 'bottom');
  const op = $derived(Math.min(100, Math.max(30, settings?.lineOpacity ?? 100)) / 100);
  const cardOp = $derived(cardOpacity(op * 100));
  const thickness = $derived(Math.min(6, Math.max(1, settings?.lineThickness ?? 2)));
  const color = $derived(ropeColor(settings?.lineColor ?? 'natural', 'var(--accent)', isDark ? 'dark' : 'light'));
  const cardVarsStyle = $derived(cardStyleAttr(settings?.lineCardStyle ?? 'glassDark', settings?.lineCardRadius ?? 10, settings?.lineCardShadow ?? true));
  const maxPer = $derived(settings?.lineMaxPerRope ?? 0);
  const baseFit = $derived(fitCount(width - 72, size.card));
  const fit = $derived(maxPer >= 3 ? Math.min(baseFit, maxPer) : baseFit);
  const peekMs = $derived(settings?.linePeekMs ?? 2500);
  const hideDelay = $derived(settings?.lineHideDelayMs ?? 400);
  const k = $derived(swingK(settings?.lineSwing ?? 100, settings?.lineDamping ?? 100));
  /** The rope that shows the 1-9 badges: the keyboard-focused or hovered one, else the first. */
  const numRope = $derived.by(() => {
    if (focus) return focus.r;
    if (hoverId !== null) {
      const r = ropes.findIndex((x) => x.cards.some((c) => c.id === hoverId));
      if (r >= 0) return r;
    }
    return 0;
  });
  const order = $derived(ropes.map((r) => r.cards.slice(0, fit).map((c) => c.id)));
  const empty = $derived(ropes.length === 0);
  const focusedId = $derived(focus ? (order[focus.r]?.[focus.i] ?? null) : null);
  const byId = $derived(new Map(ropes.flatMap((r) => r.cards).map((c) => [c.id, c])));
  $effect(() => {
    if (focusedId === null) previewOn = false;
  });
  const previewItem = $derived(previewOn && focusedId !== null ? (byId.get(focusedId)?.item ?? null) : null);

  /** A file as a searchable clip-shaped record (only its name and extension count). */
  function fileAsText(f: LineFile): ClipItem {
    return { text: f.name, title: f.name, tags: [] } as unknown as ClipItem;
  }

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
    const t2 = themeSetting === 'system' ? (dark.matches ? 'dark' : 'light') : themeSetting;
    document.documentElement.dataset.theme = t2;
    isDark = t2 === 'dark';
  }

  function applySettings(s: Settings) {
    const ropesChanged =
      settings !== null &&
      (JSON.stringify(settings.lineRopes) !== JSON.stringify(s.lineRopes) || JSON.stringify(settings.lineFilesSources) !== JSON.stringify(s.lineFilesSources));
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
      const next = step(p, dt, k);
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
        if (settings?.lineSound) hangTick(settings.lineSoundVolume / 100);
      }
      else {
        falls[id] = tt;
        moving = true;
      }
    }
    raf = moving ? requestAnimationFrame(frame) : 0;
  }

  function nudge(id: number, dOmega: number) {
    if (reduce || k.impulse === 0) return;
    pend.set(id, impulse(pend.get(id) ?? { theta: 0, omega: 0 }, dOmega * k.impulse));
    kick();
  }

  function onHover(c: LineCard, dir: number) {
    hoverId = c.id;
    focus = null;
    nudge(c.id, 0.6 * dir);
  }

  function scheduleBreeze() {
    clearTimeout(breezeTimer);
    if (!shown || reduce || !settings?.lineBreeze || document.hidden) return;
    const strength = (settings.lineBreezeStrength ?? 100) / 100;
    if (strength <= 0) return;
    breezeTimer = setTimeout(
      () => {
        const ids = order.flat();
        const n = Math.min(ids.length, 1 + Math.floor(Math.random() * 3));
        for (let i = 0; i < n; i++) nudge(ids[Math.floor(Math.random() * ids.length)], (Math.random() < 0.5 ? -1 : 1) * 0.5 * strength);
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
    let nextFiles: LineFile[] = [];
    try {
      [lists, nextFiles] = await Promise.all([
        Promise.all(wanted.map((k) => api.clipList({ shelf: SHELF[k], limit: LIST_LIMIT }))),
        ropeCfg.files ? api.lineFiles() : Promise.resolve([] as LineFile[]),
      ]);
    } catch (e) {
      console.error(e);
      dirty = true;
      return;
    }
    if (my !== seq) return;
    const next = lists.flat();
    const nextIds = new Set<number>([...next.map((i) => i.id), ...nextFiles.map((f) => fileId(f.path))]);
    const fresh =
      knownReady && shown && !reduce && (settings?.lineFall ?? true)
        ? [...next.map((i) => i.id), ...nextFiles.map((f) => fileId(f.path))].filter((id) => !known.has(id)).slice(0, 4)
        : [];
    known = nextIds;
    knownReady = true;
    items = next;
    files = nextFiles;
    selected = selected.filter((id) => known.has(id));
    for (const id of fresh) falls[id] = 0;
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
      peekTimer = setTimeout(() => !pointerInside && !document.documentElement.matches(':hover') && void hide(), peekMs);
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
    }, hideDelay);
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

  function cardsOf(ids: number[]): LineCard[] {
    return ids.map((id) => byId.get(id)).filter((c): c is LineCard => !!c);
  }

  function targetsFor(card: LineCard | null): number[] {
    if (card && !selected.includes(card.id)) return [card.id];
    if (selected.length > 0) return selected;
    const id = card?.id ?? focusedId ?? hoverId;
    return id === null ? [] : [id];
  }

  /** Clip ids and file paths of a mixed target list. */
  function splitTargets(ids: number[]): { clips: number[]; paths: string[] } {
    const cs = cardsOf(ids);
    return { clips: cs.filter((c) => c.item).map((c) => c.id), paths: cs.flatMap((c) => (c.file ? [c.file.path] : [])) };
  }

  /** True when the copy happened. A group is copied by the kind of its first card (clips or files). */
  async function copy(ids: number[]): Promise<boolean> {
    if (ids.length === 0) return false;
    const { clips, paths } = splitTargets(ids);
    try {
      if (ids[0] < 0) {
        if (paths.length === 0) return false;
        await api.lineFileCopy(paths);
      } else {
        if (clips.length === 0) return false;
        await api.clipCopy(clips);
      }
      copiedId = ids[0];
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copiedId = null), 1100);
      return true;
    } catch (e) {
      fail(e);
      return false;
    }
  }

  /** The click action from the settings: copy, copy and close, or paste into the previous app. */
  async function activate(ids: number[]) {
    const action = settings?.lineClickAction ?? 'copy';
    const { clips } = splitTargets(ids);
    if (action === 'paste' && ids[0] > 0 && clips.length > 0) {
      try {
        await api.linePaste(clips);
      } catch (e) {
        fail(e);
      }
      return;
    }
    if ((await copy(ids)) && action !== 'copy') await hide();
  }

  // A double-click delivers two clicks first: the single-click action waits a moment and is cancelled by dblclick.
  let clickTimer: ReturnType<typeof setTimeout> | undefined;
  function pick(c: LineCard, e: MouseEvent) {
    clearTimeout(clickTimer);
    const mods = { shiftKey: e.shiftKey, ctrlKey: e.ctrlKey, metaKey: e.metaKey } as MouseEvent;
    clickTimer = setTimeout(() => pickNow(c, mods), 220);
  }

  function pickNow(c: LineCard, e: MouseEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    if ((e.shiftKey || ctrl) && selected.length > 0 && selected[0] < 0 !== c.id < 0) {
      // a selection holds clips or files, never both: the other kind starts a new one
      selected = [c.id];
      anchor = c.id;
      return;
    }
    if (e.shiftKey || ctrl) {
      const row = order.find((ids) => ids.includes(c.id)) ?? [];
      if (e.shiftKey && anchor !== null && row.includes(anchor)) {
        const a = row.indexOf(anchor);
        const b = row.indexOf(c.id);
        const range = row.slice(Math.min(a, b), Math.max(a, b) + 1);
        selected = ctrl ? [...new Set([...selected, ...range])] : range;
      } else {
        selected = selected.includes(c.id) ? selected.filter((x) => x !== c.id) : [...selected, c.id];
        anchor = c.id;
      }
      return;
    }
    selected = [];
    anchor = c.id;
    void activate([c.id]);
  }

  async function openCard(c: LineCard) {
    clearTimeout(clickTimer);
    const item = c.item;
    try {
      if (c.file) {
        await api.lineFileOpen(c.file.path);
        await hide();
      } else if (item && (item.kind === 'image' || item.kind === 'text')) {
        await api.lineOpenItem(item.id);
        await hide();
      } else if (item?.text) {
        await api.openUrl(item.text.trim());
        await hide();
      }
    } catch (e) {
      fail(e);
    }
  }

  async function dragOut(c: LineCard) {
    dragging = true;
    menu = null;
    zoom = null;
    try {
      if (c.file) await api.lineFileDragOut(c.file.path);
      else if (c.item) await api.lineDragOut(c.item.id);
    } catch (e) {
      console.error(e);
      showToast(t('dragFailed'));
    } finally {
      dragging = false;
      if (!pointerInside) leave();
    }
  }

  /** Delete: clips go to the trash (with undo), files only leave the list (the file stays on disk). */
  async function trash(ids: number[]) {
    if (ids.length === 0) return;
    const { clips, paths } = splitTargets(ids);
    try {
      if (clips.length > 0) {
        await api.clipTrash(clips);
        undoStack.push(clips);
      }
      if (paths.length > 0) await api.lineFileRemove(paths);
      selected = [];
      if (clips.length > 0) showToast(t('trashed', { count: clips.length }), true);
      else showToast(t('fileRemoved', { count: paths.length }));
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
  function openMenu(c: LineCard, e: MouseEvent) {
    zoom = null;
    const h = c.file ? 200 : c.item?.kind === 'image' ? 250 : 190;
    menu = { x: Math.max(4, Math.min(e.clientX, width - MENU_W - 4)), y: Math.max(4, Math.min(e.clientY, window.innerHeight - h - 4)), card: c };
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
    return id === null ? null : (byId.get(id)?.item ?? null);
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
    if (all.length === 0) {
      const f = ropes.flatMap((r) => r.files)[0];
      if (f && (await copy([fileId(f.path)]))) await hide();
      return;
    }
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
        const { clips } = splitTargets(targetsFor(null));
        if (clips.length > 0) void api.linePaste(clips).catch(fail);
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
        if (settings?.lineClickAction === 'paste') await activate(ids);
        else if (await copy(ids)) await hide();
      })();
    } else if (key === 'Delete') {
      if (!e.repeat) void trash(targetsFor(null));
    } else if (key === ' ') {
      if (e.repeat) return;
      if (previewOn) previewOn = false;
      else if (ensureFocus() && itemOf(focusedId)) {
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
      if (it && (it.kind === 'text' || it.kind === 'image')) void openCard(byId.get(it.id)!);
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
      keep(await on.filesChanged(() => markDirty()));
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
<div class="strip" class:shown class:bottom style:height="{stripH}px" style={cardVarsStyle}>
  {#each ropes as rope, i (rope.key)}
    <Rope
      label={ropeName(rope.key, settings?.lineRopeNames, t(`rope.${rope.key}`))}
      showLabel={settings?.lineShowLabels ?? true}
      cards={rope.cards.slice(0, fit)}
      {width}
      more={overflow(rope.cards.length, fit)}
      {angles}
      {falls}
      selected={new Set(selected)}
      {copiedId}
      {focusedId}
      numbers={(settings?.lineShowNumbers ?? true) && i === numRope}
      top={tops[i]}
      band={size.band}
      card={size.card}
      scale={size.scale}
      {color}
      {thickness}
      ropeStyle={settings?.lineRopeStyle ?? 'twisted'}
      sag={settings?.lineSag ?? 14}
      pinStyle={settings?.linePinStyle ?? 'plastic'}
      pinColor={settings?.linePinColor ?? 'auto'}
      {cardOp}
      zoomOn={settings?.lineZoomOnHover ?? false}
      onzoom={(item, rect) => (zoom = shown && item && rect ? { item, rect } : null)}
      emptyText={query.trim() ? t('search.none') : ''}
      onclick={pick}
      ondblclick={openCard}
      ondragout={dragOut}
      onremove={(c) => void trash(targetsFor(c))}
      oncontext={openMenu}
      onhover={onHover}
      onmore={() => void api.showMain(rope.key === 'files' ? 'recent' : 'clipboard').then(hide).catch(fail)}
    />
  {/each}
  {#if paused && !empty}<span class="paused" class:shift={searchOpen}>{t('paused')}</span>{/if}
  {#if empty}
    <div class="empty" style:top="{tops[0]}px">
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
  {@const c = menu.card}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="menu" role="menu" tabindex="-1" bind:this={menuEl} style:left="{menu.x}px" style:top="{menu.y}px" style:width="{MENU_W}px">
    {#if c.file}
      {@const f = c.file}
      <button type="button" role="menuitem" onclick={() => menuDo(() => openCard(c))}>{t('menu.open')}</button>
      <button type="button" role="menuitem" onclick={() => menuDo(() => api.lineFileReveal(f.path))}>{t('menu.reveal')}</button>
      <button type="button" role="menuitem" onclick={() => menuDo(async () => { await api.wheelOpen([f.path]); await hide(); })}>{t('menu.convert')}</button>
      <button type="button" role="menuitem" onclick={() => menuDo(async () => { await navigator.clipboard.writeText(f.path); showToast(t('pathCopied')); })}>{t('menu.copyPath')}</button>
      <hr />
      <button type="button" role="menuitem" class="danger" onclick={() => menuDo(() => trash(targetsFor(c)))}>{t('menu.removeFromList')}</button>
    {:else if c.item}
      {@const it = c.item}
      <button type="button" role="menuitem" onclick={() => menuDo(async () => { await copy(targetsFor(c)); })}>{t('menu.copy')}</button>
      <button type="button" role="menuitem" onclick={() => menuDo(() => api.linePaste(splitTargets(targetsFor(c)).clips))}>{t('menu.paste')}</button>
      {#if it.kind === 'image' || it.kind === 'text'}
        <button type="button" role="menuitem" onclick={() => menuDo(() => openCard(c))}>{t('menu.edit')}</button>
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
      <button type="button" role="menuitem" class="danger" onclick={() => menuDo(() => trash(targetsFor(c)))}>{t('menu.delete')}</button>
    {/if}
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
  .strip.bottom { inset: auto 0 0 0; transform: translateY(104%); }
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
    left: 50%;
    translate: -50% 0;
    display: flex;
    gap: var(--space-2);
    align-items: center;
    margin-top: 28px;
    padding: 6px 16px;
    border-radius: 999px;
    background: rgba(20, 20, 24, 0.78);
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.82);
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
