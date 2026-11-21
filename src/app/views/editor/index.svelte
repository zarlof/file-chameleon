<!--
  Image editor (`editor` view, not in the sidebar). Opened with NavPayload.paths[0] = an image path
  (the wheel's "annotate" preset, the Convert view) or `clip:<id>` (the clipboard library).
  Tools: crop, pen, marker, rectangle, ellipse, arrow, line, text; colours, stroke width, undo/redo.
  Clipboard images save as the item's edited copy (original kept); files as "<name> (düzenlendi).png".
  Keys: Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z), Ctrl+S, Enter applies a crop, Esc drops it, C P M R E A L T pick tools.
-->
<script module lang="ts">
  import { nav } from '../../../lib/stores/nav.svelte';
  import type { View } from '../../../lib/types';

  // "Cancel" returns to the view the user came from: remember the last view that was not the editor.
  // (This module is imported by the Shell at start-up, so the root effect sees every navigation.)
  let previousView: View = 'convert';
  $effect.root(() => {
    $effect(() => {
      if (nav.view !== 'editor') previousView = nav.view;
    });
  });
</script>

<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import {
    ArrowUpRight, Check, ChevronLeft, Circle, Crop, FolderOpen, Highlighter, ImageOff, Info, Minus, Pencil, Redo2, Square, Type, Undo2, X,
  } from '@lucide/svelte';
  import { onDestroy } from 'svelte';
  import { t } from '../../../lib/i18n';
  import { api, errorOf } from '../../../lib/ipc';
  import { Button, EmptyState, IconButton, modal, Sheet, Spinner, toast } from '../../../lib/ui';
  import * as C from './canvas';

  type Target = { kind: 'clip'; id: number } | { kind: 'file'; path: string };
  type Drag =
    | { kind: 'shape' }
    | { kind: 'cropNew'; start: C.Point }
    | { kind: 'cropHandle'; handle: C.Handle; start: C.Rect; from: C.Point };
  interface TextEdit { id: number; at: C.Point; x: number; y: number; value: string }

  const TOOLS: { id: C.Tool; icon: typeof Crop; key: string }[] = [
    { id: 'crop', icon: Crop, key: 'c' },
    { id: 'pen', icon: Pencil, key: 'p' },
    { id: 'marker', icon: Highlighter, key: 'm' },
    { id: 'rect', icon: Square, key: 'r' },
    { id: 'ellipse', icon: Circle, key: 'e' },
    { id: 'arrow', icon: ArrowUpRight, key: 'a' },
    { id: 'line', icon: Minus, key: 'l' },
    { id: 'text', icon: Type, key: 't' },
  ];
  /** Room around the image inside the stage (px). */
  const PAD = 48;
  /** Crop handle reach in screen pixels. */
  const GRAB = 8;

  let target = $state<Target | null>(null);
  let name = $state('');
  let image = $state.raw<HTMLImageElement | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let hist = $state.raw<C.History<C.Doc>>(C.history({ shapes: [], view: { x: 0, y: 0, w: 0, h: 0 } }));
  let tool = $state<C.Tool>('pen');
  let color = $state<string>(C.SWATCHES[0].ink);
  let width = $state<number>(C.WIDTHS[1]);
  let draft = $state.raw<C.Shape | null>(null);
  let crop = $state.raw<C.Rect | null>(null);
  let textEdit = $state<TextEdit | null>(null);
  let cursor = $state('crosshair');
  let stageW = $state(0);
  let stageH = $state(0);
  let stage: HTMLElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  let saving = $state(false);
  let saved = $state<string | null>(null);
  let leaveOpen = $state(false);
  /** The image was downscaled for editing: the saved file will have that size. */
  let reduced = $state(false);
  let drag: Drag | null = null;
  let textSeq = 0;
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  const doc = $derived(hist.stack[hist.index]);
  const dirty = $derived(hist.index > 0);
  const scale = $derived(C.fitScale(doc.view.w, doc.view.h, stageW - PAD, stageH - PAD));
  const cssW = $derived(Math.max(1, Math.round(doc.view.w * scale)));
  const cssH = $derived(Math.max(1, Math.round(doc.view.h * scale)));

  // ------------------------------------------------------------ loading

  $effect(() => {
    const p = nav.pending && nav.consume('editor');
    if (p?.paths[0]) void open(p.paths[0]);
  });

  const message = (e: unknown) => (e instanceof Error ? e.message : errorOf(e).message);
  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  function loadImage(src: string, cors = false): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      if (cors) img.crossOrigin = 'anonymous';
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error(t('editor.loadFailed')));
      img.src = src;
    });
  }

  /** A backend preview (PNG data URL, longest side <= PREVIEW_CAP); flags `reduced` when it was likely downscaled. */
  async function loadPreview(path: string): Promise<HTMLImageElement> {
    const img = await loadImage(await api.readImagePreview(path, C.PREVIEW_CAP));
    reduced = C.wasReduced(img.naturalWidth, img.naturalHeight);
    return img;
  }

  /** Clipboard images load through the asset protocol (CORS, so the canvas stays exportable, full size); fallback: a preview. */
  async function loadStored(path: string): Promise<HTMLImageElement> {
    let url: string | null = null;
    try {
      url = convertFileSrc(path);
    } catch {
      /* not in Tauri */
    }
    if (url) {
      try {
        return await loadImage(url, true);
      } catch {
        /* fall back to the backend */
      }
    }
    return loadPreview(path);
  }

  async function open(source: string) {
    loading = true;
    error = null;
    saved = null;
    crop = null;
    draft = null;
    textEdit = null;
    reduced = false;
    try {
      let img: HTMLImageElement;
      if (source.startsWith('clip:')) {
        const id = Number(source.slice(5));
        const item = await api.clipGet(id);
        const path = item.edited ?? item.image;
        if (item.kind !== 'image' || !path) throw new Error(t('editor.notImage'));
        img = await loadStored(path);
        target = { kind: 'clip', id };
        name = item.title ?? t('editor.clipName');
      } else {
        img = await loadPreview(source);
        target = { kind: 'file', path: source };
        name = fileName(source);
      }
      image = img;
      hist = C.history({ shapes: [], view: { x: 0, y: 0, w: img.naturalWidth, h: img.naturalHeight } });
    } catch (e) {
      target = null;
      image = null;
      error = message(e);
    } finally {
      loading = false;
    }
  }

  // ------------------------------------------------------------ drawing

  let raf = 0;
  $effect(() => {
    const args = { img: image, d: doc, dr: draft, cr: tool === 'crop' ? crop : null, w: cssW, h: cssH };
    if (!canvas || !args.img) return;
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => paint(args.img!, args.d, args.dr, args.cr, args.w, args.h));
  });
  onDestroy(() => {
    cancelAnimationFrame(raf);
    clearTimeout(savedTimer);
  });

  function paint(img: HTMLImageElement, d: C.Doc, dr: C.Shape | null, cr: C.Rect | null, w: number, h: number) {
    if (!canvas || d.view.w <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    const pw = Math.round(w * dpr);
    const ph = Math.round(h * dpr);
    if (canvas.width !== pw || canvas.height !== ph) {
      canvas.width = pw;
      canvas.height = ph;
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, pw, ph);
    const k = (w / d.view.w) * dpr;
    ctx.setTransform(k, 0, 0, k, -d.view.x * k, -d.view.y * k);
    ctx.imageSmoothingQuality = 'high';
    ctx.drawImage(img, 0, 0);
    for (const s of d.shapes) C.drawShape(ctx, s);
    if (dr) C.drawShape(ctx, dr);
    if (cr) C.drawCropOverlay(ctx, d.view, cr, d.view.w / w);
  }

  /** One screen pixel in image pixels. */
  const px = () => (scale > 0 ? 1 / scale : 1);

  function imagePoint(e: PointerEvent): C.Point {
    return C.clientToImage({ x: e.clientX, y: e.clientY }, canvas!.getBoundingClientRect(), doc.view);
  }

  function commitDoc(next: C.Doc) {
    hist = C.commit(hist, next);
    saved = null;
  }

  /** `r` clamped (and rounded) inside the shown area. */
  function clampToView(r: C.Rect): C.Rect {
    const v = doc.view;
    const c = C.cropToRect(v.w, v.h, { ...r, x: r.x - v.x, y: r.y - v.y });
    return { ...c, x: c.x + v.x, y: c.y + v.y };
  }

  function down(e: PointerEvent) {
    if (e.button !== 0 || !image || !canvas) return;
    const p = imagePoint(e);
    if (tool === 'text') {
      startText(e, p);
      return;
    }
    canvas.setPointerCapture(e.pointerId);
    if (tool === 'crop') {
      const handle = crop ? C.hitHandle(crop, p, GRAB * px()) : null;
      if (crop && handle) drag = { kind: 'cropHandle', handle, start: crop, from: p };
      else {
        drag = { kind: 'cropNew', start: p };
        crop = { x: p.x, y: p.y, w: 0, h: 0 };
      }
      return;
    }
    const w = (tool === 'marker' ? width * C.MARKER_FACTOR : width) * px();
    draft = tool === 'pen' || tool === 'marker' ? { type: tool, points: [p], color, width: w } : { type: tool, a: p, b: p, color, width: w };
    drag = { kind: 'shape' };
  }

  function move(e: PointerEvent) {
    if (!image || !canvas) return;
    const p = imagePoint(e);
    if (!drag) {
      cursor = tool === 'crop' ? C.handleCursor(crop ? C.hitHandle(crop, p, GRAB * px()) : null) : tool === 'text' ? 'text' : 'crosshair';
      return;
    }
    if (drag.kind === 'cropNew') crop = C.normRect(drag.start, p);
    else if (drag.kind === 'cropHandle') crop = C.dragHandle(drag.start, drag.handle, p.x - drag.from.x, p.y - drag.from.y);
    else if (draft) {
      if ('points' in draft) {
        // Appending in place keeps long strokes linear; the new object only triggers a repaint.
        draft.points.push(p);
        draft = { ...draft };
      } else if ('b' in draft) draft = { ...draft, b: p };
    }
  }

  function up() {
    const d = drag;
    drag = null;
    if (!d) return;
    if (d.kind !== 'shape') {
      const c = crop ? clampToView(crop) : null;
      crop = c && c.w >= 2 && c.h >= 2 ? c : null;
      return;
    }
    const s = draft;
    draft = null;
    if (!s) return;
    if ('points' in s) commitDoc({ ...doc, shapes: [...doc.shapes, { ...s, points: C.smoothPath(s.points, px()) }] });
    else if ('b' in s && Math.hypot(s.b.x - s.a.x, s.b.y - s.a.y) >= 3 * px()) commitDoc({ ...doc, shapes: [...doc.shapes, s] });
  }

  function applyCrop() {
    if (!crop) return;
    const c = clampToView(crop);
    crop = null;
    const v = doc.view;
    if (c.w < 1 || c.h < 1 || (c.x === v.x && c.y === v.y && c.w === v.w && c.h === v.h)) return;
    commitDoc({ ...doc, view: c });
  }

  // ------------------------------------------------------------ text

  function startText(e: PointerEvent, p: C.Point) {
    commitText();
    const box = stage?.getBoundingClientRect();
    if (!box) return;
    textEdit = { id: ++textSeq, at: p, x: e.clientX - box.left, y: e.clientY - box.top, value: '' };
  }

  /** Commit the open text box (only the one named by `id`, when given: a late blur must not commit a newer box). */
  function commitText(id?: number) {
    const te = textEdit;
    if (!te || (id !== undefined && te.id !== id)) return;
    textEdit = null;
    if (te.value.trim()) commitDoc({ ...doc, shapes: [...doc.shapes, { type: 'text', at: te.at, text: te.value, color, size: C.textSize(width) * px() }] });
  }

  /** Focus after the click that created the box has finished moving the focus (mousedown runs first). */
  function focusLater(node: HTMLInputElement) {
    const id = setTimeout(() => node.focus());
    return { destroy: () => clearTimeout(id) };
  }

  function textKey(e: KeyboardEvent, id: number) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitText(id);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      textEdit = null;
    }
  }

  // ------------------------------------------------------------ commands

  function pickTool(next: C.Tool) {
    if (next === tool) return;
    commitText();
    crop = null;
    tool = next;
    cursor = next === 'text' ? 'text' : 'crosshair';
  }

  function undoAct() {
    commitText();
    if (crop || draft) {
      crop = null;
      draft = null;
      return;
    }
    hist = C.undo(hist);
  }

  function redoAct() {
    hist = C.redo(hist);
  }

  async function save() {
    if (!image || !target || saving) return;
    commitText();
    // A crop frame that was drawn but not applied (Enter / Crop) is not part of the image.
    saving = true;
    try {
      const url = C.exportPng(image, doc);
      if (target.kind === 'clip') {
        await api.clipSaveEdited(target.id, url);
        toast.show(t('editor.savedClip'), 'success');
        leave();
      } else {
        const out = await api.saveAnnotated(target.path, url);
        hist = C.history(doc);
        saved = out;
        clearTimeout(savedTimer);
        savedTimer = setTimeout(() => (saved = null), 10_000);
      }
    } catch (e) {
      toast.show(t('editor.saveFailed', { message: message(e) }), 'error');
    } finally {
      saving = false;
    }
  }

  function cancel() {
    if (dirty) leaveOpen = true;
    else leave();
  }

  function leave() {
    leaveOpen = false;
    nav.go(previousView);
  }

  function editable(target: EventTarget | null) {
    const el = target as HTMLElement | null;
    return !!el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName));
  }

  function onKey(e: KeyboardEvent) {
    if (modal.open > 0 || editable(e.target) || e.altKey) return;
    const ctrl = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (ctrl) {
      if (key === 'z' && !e.shiftKey) undoAct();
      else if (key === 'y' || (key === 'z' && e.shiftKey)) redoAct();
      else if (key === 's') void save();
      else return;
      e.preventDefault();
      return;
    }
    if (e.key === 'Enter' && tool === 'crop' && crop) {
      e.preventDefault();
      applyCrop();
    } else if (e.key === 'Escape') {
      crop = null;
    } else {
      const next = TOOLS.find((x) => x.key === key);
      if (next && image) pickTool(next.id);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<section class="editor">
  <header>
    <Button variant="plain" size="sm" onclick={cancel}>
      {#snippet icon()}<ChevronLeft size={14} />{/snippet}{t('editor.back')}
    </Button>
    <div class="title">
      <h1>{t('common.nav.editor')}</h1>
      {#if name}<span class="name" title={target?.kind === 'file' ? target.path : name}>{name}</span>{/if}
    </div>
    <div class="actions">
      <IconButton label={t('editor.undo')} disabled={!image || (hist.index === 0 && !crop)} onclick={undoAct}><Undo2 size={16} /></IconButton>
      <IconButton label={t('editor.redo')} disabled={!image || hist.index >= hist.stack.length - 1} onclick={redoAct}><Redo2 size={16} /></IconButton>
      <Button variant="primary" size="sm" loading={saving} disabled={!image} onclick={save}>
        {#snippet icon()}<Check size={13} />{/snippet}{t('editor.save')}
      </Button>
    </div>
  </header>

  <div class="toolbar" class:off={!image}>
    <div class="group" role="radiogroup" aria-label={t('editor.tools')}>
      {#each TOOLS as tl (tl.id)}
        <IconButton label={`${t(`editor.tool.${tl.id}`)} (${tl.key.toUpperCase()})`} active={tool === tl.id} disabled={!image} onclick={() => pickTool(tl.id)}>
          <tl.icon size={16} />
        </IconButton>
      {/each}
    </div>
    {#if tool === 'crop'}
      <div class="group">
        <span class="hint">{crop ? t('editor.cropApplyHint') : t('editor.cropHint')}</span>
        <Button size="sm" variant="primary" disabled={!crop} onclick={applyCrop}>{t('editor.cropApply')}</Button>
        <Button size="sm" variant="plain" disabled={!crop} onclick={() => (crop = null)}>{t('common.actions.cancel')}</Button>
      </div>
    {:else}
      <div class="group" role="radiogroup" aria-label={t('editor.color')}>
        {#each C.SWATCHES as c (c.id)}
          <button type="button" class="swatch" class:on={color === c.ink} style:--ink={c.ink} role="radio" aria-checked={color === c.ink}
            aria-label={t(`editor.colors.${c.id}`)} title={t(`editor.colors.${c.id}`)} disabled={!image} onclick={() => (color = c.ink)}></button>
        {/each}
      </div>
      <div class="group" role="radiogroup" aria-label={t('editor.width')}>
        {#each C.WIDTHS as w (w)}
          <button type="button" class="width" class:on={width === w} role="radio" aria-checked={width === w}
            aria-label={t('editor.widthValue', { value: w })} title={t('editor.widthValue', { value: w })} disabled={!image} onclick={() => (width = w)}>
            <i style:width="{w + 2}px" style:height="{w + 2}px"></i>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#if reduced && image}
    <p class="notice" role="note"><Info size={14} />{t('editor.reduced', { size: C.PREVIEW_CAP })}</p>
  {/if}

  <div class="stage" bind:this={stage} bind:clientWidth={stageW} bind:clientHeight={stageH}>
    {#if loading}
      <Spinner size={22} />
    {:else if error}
      <EmptyState title={t('editor.errorTitle')} message={error}>
        {#snippet icon()}<ImageOff />{/snippet}
        {#snippet action()}<Button onclick={leave}>{t('editor.back')}</Button>{/snippet}
      </EmptyState>
    {:else if !image}
      <EmptyState title={t('editor.emptyTitle')} message={t('editor.emptyMessage')}>
        {#snippet icon()}<ImageOff />{/snippet}
        {#snippet action()}<Button onclick={leave}>{t('editor.back')}</Button>{/snippet}
      </EmptyState>
    {:else}
      <canvas bind:this={canvas} style:width="{cssW}px" style:height="{cssH}px" style:cursor={cursor}
        onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up}></canvas>
      {#if textEdit}
        {@const id = textEdit.id}
        {#key id}
          <input class="text-in" bind:value={textEdit.value} use:focusLater aria-label={t('editor.textInput')} placeholder={t('editor.textPlaceholder')}
            size={Math.max(4, textEdit.value.length + 1)} style:left="{textEdit.x}px" style:top="{textEdit.y}px"
            style:font-size="{C.textSize(width)}px" style:color={color}
            onkeydown={(e) => textKey(e, id)} onblur={() => commitText(id)} />
        {/key}
      {/if}
    {/if}

    {#if saved}
      <div class="saved" role="status">
        <Check size={15} />
        <span class="saved-name">{t('editor.savedFile', { name: fileName(saved) })}</span>
        <Button size="sm" onclick={() => saved && api.revealPath(saved).catch(() => {})}>
          {#snippet icon()}<FolderOpen size={13} />{/snippet}{t('editor.reveal')}
        </Button>
        <IconButton label={t('common.actions.close')} size={24} onclick={() => (saved = null)}><X size={13} /></IconButton>
      </div>
    {/if}
  </div>
</section>

<Sheet bind:open={leaveOpen} title={t('editor.leave.title')}>
  <p class="confirm">{t('editor.leave.message')}</p>
  {#snippet footer()}
    <Button onclick={() => (leaveOpen = false)}>{t('editor.leave.stay')}</Button>
    <Button variant="destructive" onclick={leave}>{t('editor.leave.discard')}</Button>
  {/snippet}
</Sheet>

<style>
  .editor { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--space-5) var(--space-4); }
  header { display: flex; align-items: center; gap: var(--space-3); flex: none; padding-bottom: var(--space-3); }
  .title { display: flex; align-items: baseline; gap: var(--space-3); flex: 1; min-width: 0; }
  h1 { margin: 0; font: var(--text-title); letter-spacing: var(--tracking-tight); white-space: nowrap; }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: var(--text-callout); color: var(--text-secondary); }
  .actions { display: flex; align-items: center; gap: 4px; flex: none; }

  .toolbar {
    display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-4); flex: none; margin-bottom: var(--space-3);
    padding: 4px 8px; border-radius: var(--radius-md); background: var(--card-bg); box-shadow: var(--shadow-sm);
  }
  .toolbar.off { opacity: 0.6; }
  .group { display: flex; align-items: center; gap: 4px; }
  .hint { font: var(--text-callout); color: var(--text-secondary); margin-right: 4px; }
  .swatch {
    width: 20px; height: 20px; padding: 0; border: 0; border-radius: 50%; cursor: default; background: var(--ink);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 25%, transparent); transition: transform var(--dur-fast) var(--ease-out);
  }
  .swatch:hover:not(:disabled) { transform: scale(1.12); }
  .swatch.on { box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 25%, transparent), 0 0 0 2px var(--content-bg), 0 0 0 4px var(--accent); }
  .width {
    display: grid; place-items: center; width: 28px; height: 28px; padding: 0; border: 0; border-radius: var(--radius-sm);
    background: transparent; cursor: default; color: var(--text);
  }
  .width:hover:not(:disabled) { background: var(--control-bg); }
  .width.on { background: var(--accent-soft); color: var(--accent); }
  .width i { display: block; border-radius: 50%; background: currentColor; }

  .notice {
    display: flex; align-items: center; gap: 6px; flex: none; margin: calc(-1 * var(--space-1)) 0 var(--space-3); padding: 6px 10px;
    border-radius: var(--radius-sm); font: var(--text-callout); color: var(--text);
    background: color-mix(in srgb, var(--orange) 14%, transparent);
  }
  .notice > :global(svg) { flex: none; color: var(--orange); }
  .stage {
    position: relative; flex: 1; min-height: 0; display: grid; place-items: center; overflow: hidden; border-radius: var(--radius-md);
    background: var(--control-bg);
  }
  canvas { display: block; box-shadow: var(--shadow-md); touch-action: none; }
  .text-in {
    position: absolute; z-index: 5; margin: 0; padding: 0; border: 0; outline: 1px dashed var(--accent); outline-offset: 3px;
    background: transparent; font-family: 'Inter Variable', 'Segoe UI', sans-serif; font-weight: 600; line-height: 1;
    cursor: text; user-select: text;
  }
  .text-in::placeholder { color: var(--text-tertiary); }
  .saved {
    position: absolute; left: 50%; bottom: var(--space-4); z-index: 6; display: flex; align-items: center; gap: var(--space-2);
    max-width: calc(100% - 32px); padding: 5px 5px 5px 14px; border-radius: 99px; transform: translateX(-50%);
    background: var(--elevated-bg); box-shadow: var(--shadow-lg), var(--glass-edge); backdrop-filter: blur(24px) saturate(1.6);
    font: var(--text-body); animation: rise var(--dur-slow) var(--ease-out);
  }
  .saved > :global(svg) { flex: none; color: var(--green); }
  .saved-name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .confirm { margin: 0; color: var(--text-secondary); }
  @keyframes rise { from { opacity: 0; transform: translate(-50%, 10px); } }
</style>
