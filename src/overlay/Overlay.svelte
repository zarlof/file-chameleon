<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { api, on } from '../lib/ipc';
  import type { Kind, TargetOption, ToastPayload } from '../lib/types';
  import { orderSlices, sliceAt, toLogical } from '../lib/wheel/geometry';
  import Wheel from './Wheel.svelte';
  import ToastCard from './ToastCard.svelte';
  import { toastView, type ToastView } from './toasts';
  import { t } from './i18n';

  type Mode = 'none' | 'wheel' | 'toast';
  let mode = $state<Mode>('none');

  // wheel
  let waiting = $state(true);
  let slices = $state<TargetOption[]>([]);
  let paths = $state<string[]>([]);
  let hovered = $state(-1);
  let firstKind = $state<Kind | null>(null);
  let wheelKey = $state(0);

  // toast
  let toast = $state<ToastPayload | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  const view = $derived<ToastView | null>(toast ? toastView(toast, t) : null);

  function clear() {
    mode = 'none';
    slices = [];
    paths = [];
    hovered = -1;
    firstKind = null;
    toast = null;
    clearTimeout(toastTimer);
  }

  function hit(p: { x: number; y: number }): number {
    const dpr = window.devicePixelRatio || 1;
    return sliceAt(toLogical(p.x, dpr), toLogical(p.y, dpr), slices.length);
  }

  function closeToast() {
    clear();
    api.overlayHide().catch(console.error);
  }

  function toastAction(id: string) {
    if (!toast) return;
    const { kind, data } = toast;
    if (id !== 'dismiss') api.overlayToastAction(kind, id, data).catch(console.error);
    closeToast();
  }

  onMount(() => {
    const unlisten: Array<Promise<() => void>> = [
      on.overlayWheel(() => {
        clear();
        mode = 'wheel';
        waiting = true;
        wheelKey++;
      }),
      on.overlayWheelTargets((w) => {
        mode = 'wheel';
        waiting = false;
        paths = w.paths;
        slices = orderSlices(w.targets, w.recent);
        firstKind = w.firstKind ?? w.targets[0]?.kind ?? null;
      }),
      on.overlayToast((p) => {
        clear();
        mode = 'toast';
        toast = p;
        if (p.timeoutMs) toastTimer = setTimeout(clear, p.timeoutMs);
      }),
      on.overlayHide(clear),
      getCurrentWebview().onDragDropEvent((e) => {
        if (mode !== 'wheel') return;
        const p = e.payload;
        if (p.type === 'enter' || p.type === 'over') {
          hovered = hit(p.position);
        } else if (p.type === 'leave') {
          hovered = -1;
        } else if (p.type === 'drop') {
          const i = hit(p.position);
          const dropped = p.paths.length ? p.paths : paths;
          hovered = -1;
          if (i >= 0) api.wheelDrop(dropped, slices[i].id).catch(console.error);
          else api.overlayHide().catch(console.error);
        }
      }),
    ];
    // Tell Rust we're ready only once every listener is registered (no missed first event).
    Promise.all(unlisten)
      .then(() => api.overlayReady())
      .catch(console.error);
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });
</script>

{#if mode === 'wheel'}
  {#key wheelKey}
    <Wheel {slices} {hovered} {waiting} fileCount={paths.length} {firstKind} />
  {/key}
{:else if mode === 'toast' && toast && view}
  {#key toast.id}
    <ToastCard {view} progress={toast.progress} onaction={toastAction} onclose={closeToast} />
  {/key}
{/if}
