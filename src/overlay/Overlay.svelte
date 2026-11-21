<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { api, on } from '../lib/ipc';
  import type { ToastPayload, WheelFace, WheelSlots, WheelThumb } from '../lib/types';
  import { ringRadii, toLogical, type Hit } from '../lib/wheel/geometry';
  import { FAN_DELAY_MS, clickAction, dropTarget, otherFace, pointerAt } from '../lib/wheel/hover';
  import { centerTitle, commonExt } from '../lib/wheel/labels';
  import Wheel from '../lib/wheel/Wheel.svelte';
  import DragChip from './DragChip.svelte';
  import ToastCard from './ToastCard.svelte';
  import { toastView, type ToastView } from './toasts';
  import { t } from './i18n';

  interface Props {
    /** Settings.wheelFlipScroll, read when the mouse wheel reaches the page (click mode). */
    flipScroll?: () => boolean;
  }
  let { flipScroll = () => true }: Props = $props();

  /** Flips closer together than this are one (the hook debounces the same way, and in click
   *  mode the mouse wheel may arrive both from the hook and as a DOM event). */
  const FLIP_GAP_MS = 250;
  const NO_HIT: Hit = { slot: -1, child: -1 };

  type Mode = 'none' | 'wheel' | 'toast';
  let mode = $state<Mode>('none');

  // wheel: nothing is drawn until the dragged files are known (`waiting`), so Ctrl+drags of
  // text or a marquee selection leave no trace while the (needed) window is up.
  let waiting = $state(true);
  /** Click mode (context menu / "open the wheel"): pointer events instead of a drag. */
  let click = $state(false);
  let wheel = $state.raw<WheelSlots | null>(null);
  let face = $state<WheelFace>('front');
  let hovered = $state.raw<Hit>(NO_HIT);
  let fan = $state(-1);
  /** Drag mode: pointer in window coordinates (the file chip follows it). */
  let chip = $state.raw<{ x: number; y: number } | null>(null);
  let thumb = $state.raw<WheelThumb | null>(null);
  let wheelKey = $state(0);
  /** Last pointer position (window coordinates), to re-hit after a flip or a fan timer. */
  let last: { x: number; y: number } | null = null;
  /** A slot was chosen: ignore input until the next wheel. */
  let done = false;
  let lastFlip = 0;
  let armed = -1;
  let fanTimer: ReturnType<typeof setTimeout> | undefined;

  const slots = $derived(wheel ? (face === 'front' ? wheel.front : wheel.back) : []);
  const scale = $derived(wheel?.style.scale ?? 1);
  const center = $derived({
    title: wheel ? centerTitle(wheel.count, wheel.count <= 1 ? wheel.firstExt : commonExt(wheel.paths), t) : '',
    subtitle: t('overlay.escToClose'),
  });
  const thumbUrl = $derived(thumb && wheel && thumb.path === wheel.paths[0] ? thumb.dataUrl : null);

  // toast
  let toast = $state<ToastPayload | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  const view = $derived<ToastView | null>(toast ? toastView(toast, t) : null);

  function clear() {
    mode = 'none';
    wheel = null;
    face = 'front';
    hovered = NO_HIT;
    fan = -1;
    chip = null;
    thumb = null;
    click = false;
    last = null;
    done = false;
    armFan(-1);
    toast = null;
    clearTimeout(toastTimer);
  }

  /** Window coordinates -> the wheel box (centred in the window). */
  function local(x: number, y: number): { x: number; y: number } {
    const size = ringRadii(scale).size;
    return { x: x - (window.innerWidth - size) / 2, y: y - (window.innerHeight - size) / 2 };
  }

  function pointAt(x: number, y: number) {
    last = { x, y };
    if (waiting || !wheel || done) return;
    const p = local(x, y);
    const s = pointerAt(p.x, p.y, slots, fan, scale);
    fan = s.fan;
    if (s.hit.slot !== hovered.slot || s.hit.child !== hovered.child) hovered = s.hit;
    armFan(s.arm);
  }

  /** Runs the FAN_DELAY_MS timer for group slot `i` (-1 stops it); re-arming the same slot
   *  keeps the running timer. */
  function armFan(i: number) {
    if (i === armed) return;
    clearTimeout(fanTimer);
    armed = i;
    if (i < 0) return;
    fanTimer = setTimeout(() => {
      armed = -1;
      if (hovered.slot !== i || hovered.child >= 0) return;
      fan = i;
      if (last) pointAt(last.x, last.y);
    }, FAN_DELAY_MS);
  }

  function unhover() {
    hovered = NO_HIT;
    fan = -1;
    armFan(-1);
  }

  /** Once per gesture (as the hook): the first flip request flips, later ones are ignored until
   *  FLIP_GAP_MS pass without any; every request (hook flip or DOM wheel) restarts that wait. */
  function flip() {
    if (mode !== 'wheel' || waiting || !wheel || done) return;
    const now = performance.now();
    const quiet = now - lastFlip >= FLIP_GAP_MS;
    lastFlip = now;
    if (!quiet) return;
    const next = otherFace(face, wheel.front, wheel.back);
    if (next === face) return;
    face = next;
    unhover();
    if (last) pointAt(last.x, last.y);
  }

  function close() {
    clear();
    api.overlayHide().catch(console.error);
  }

  function run(paths: string[], presetId: string) {
    done = true;
    unhover();
    api.wheelDrop(paths, presetId).catch(console.error);
  }

  /** Drag mode: files dropped at (x, y) window coordinates. */
  function drop(paths: string[], x: number, y: number) {
    if (waiting || !wheel || done) {
      if (!done) close();
      return;
    }
    const p = local(x, y);
    const id = dropTarget(slots, pointerAt(p.x, p.y, slots, fan, scale).hit);
    if (id) run(paths.length ? paths : wheel.paths, id);
    else close();
  }

  // ---- click mode (the window takes clicks but never focus: Esc / Tab come from the hooks)
  const clickWheel = () => mode === 'wheel' && click;

  function onpointermove(e: PointerEvent) {
    if (clickWheel()) pointAt(e.clientX, e.clientY);
  }

  function onpointerout(e: PointerEvent) {
    if (clickWheel() && !e.relatedTarget) unhover();
  }

  function onclick(e: MouseEvent) {
    if (!clickWheel() || done) return;
    if (waiting || !wheel) {
      close();
      return;
    }
    const p = local(e.clientX, e.clientY);
    const a = clickAction(slots, pointerAt(p.x, p.y, slots, fan, scale).hit);
    if (a.kind === 'run') run(wheel.paths, a.id);
    else if (a.kind === 'close') close();
    else {
      fan = a.slot;
      armFan(-1);
      pointAt(e.clientX, e.clientY);
    }
  }

  function onwheel() {
    if (clickWheel() && flipScroll()) flip();
  }

  function oncontextmenu(e: MouseEvent) {
    if (mode !== 'wheel') return;
    e.preventDefault();
    if (click) close();
  }

  function closeToast() {
    clear();
    api.overlayHide().catch(console.error);
  }

  function toastAction(id: string) {
    if (!toast) return;
    const { kind, data } = toast;
    if (id === 'dismiss') {
      closeToast();
      return;
    }
    // Rust hides the overlay itself before acting, so a follow-up toast (e.g. "Image saved")
    // is not hidden by a late overlay_hide from here.
    clear();
    api.overlayToastAction(kind, id, data).catch(console.error);
  }

  onMount(() => {
    const unlisten: Array<Promise<() => void>> = [
      on.overlayWheel((p) => {
        clear();
        mode = 'wheel';
        waiting = true;
        click = p.click;
        wheelKey++;
      }),
      on.overlayWheelSlots((s) => {
        if (mode !== 'wheel') {
          clear();
          mode = 'wheel';
          wheelKey++;
        }
        waiting = false;
        wheel = s;
        click = s.click;
        face = s.face;
        done = false;
        unhover();
        if (last) pointAt(last.x, last.y);
      }),
      on.overlayWheelFlip(flip),
      on.overlayWheelThumb((th) => {
        if (mode === 'wheel') thumb = th;
      }),
      on.overlayToast((p) => {
        clear();
        mode = 'toast';
        toast = p;
        if (p.timeoutMs) toastTimer = setTimeout(clear, p.timeoutMs);
      }),
      on.overlayHide(clear),
      getCurrentWebview().onDragDropEvent((e) => {
        if (mode !== 'wheel' || click) return;
        const p = e.payload;
        if (p.type === 'leave') {
          chip = null;
          unhover();
          return;
        }
        const dpr = window.devicePixelRatio || 1;
        const x = toLogical(p.position.x, dpr);
        const y = toLogical(p.position.y, dpr);
        if (p.type === 'drop') {
          drop(p.paths, x, y);
        } else {
          chip = { x, y };
          pointAt(x, y);
        }
      }),
    ];
    // Tell Rust we're ready only once every listener is registered (no missed first event).
    Promise.all(unlisten)
      .then(() => api.overlayReady())
      .catch(console.error);
    return () => {
      clearTimeout(fanTimer);
      clearTimeout(toastTimer);
      unlisten.forEach((u) => u.then((f) => f()));
    };
  });
</script>

<svelte:window {onpointermove} {onpointerout} {onclick} {onwheel} {oncontextmenu} />

{#if mode === 'wheel' && !waiting && wheel}
  {#key wheelKey}
    <div class="stage">
      <Wheel
        front={wheel.front}
        back={wheel.back}
        {face}
        {hovered}
        {fan}
        style={wheel.style}
        {center}
        missingTools={wheel.missingTools}
        {t}
      />
    </div>
    {#if chip && !click}
      <DragChip x={chip.x} y={chip.y} ext={wheel.firstExt} kind={wheel.firstKind} thumb={thumbUrl} count={wheel.count} />
    {/if}
  {/key}
{:else if mode === 'toast' && toast && view}
  {#key toast.id}
    <ToastCard {view} progress={toast.progress} onaction={toastAction} onclose={closeToast} />
  {/key}
{/if}

<style>
  /* The wheel sits in the middle of the window (Rust sizes it to the wheel). */
  .stage {
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
  }
</style>
