<!--
  Peg — one clip hanging from the rope by a little clothespin. The card swings around the pin
  (`theta`, driven by the shared loop in Line.svelte). Image = photo print, text = note sheet,
  link = tag card. Gestures: click, Ctrl/Shift-click, double-click, drag out (6 px), right-click, hover x.
-->
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { ClipItem } from '../lib/types';
  import { cardText } from './logic';
  import { renderMd } from './markdown';
  import { t } from './i18n';

  const ZOOM_DELAY = 400;

  let {
    item,
    theta,
    drop,
    selected,
    copied,
    focused = false,
    scale = 1,
    width = 112,
    cardOp = 1,
    zoomOn = false,
    onzoom,
    onclick,
    ondblclick,
    ondragout,
    onremove,
    oncontext,
    onhover,
  }: {
    item: ClipItem;
    theta: number;
    /** 0 = just arrived (high above), 1 = hanging. */
    drop: number;
    selected: boolean;
    copied: boolean;
    /** The keyboard focus is on this card. */
    focused?: boolean;
    /** Size factor (0.8 / 1 / 1.25) and the card width in px, from layout.ts. */
    scale?: number;
    width?: number;
    /** Card opacity, 0.9 to 1. */
    cardOp?: number;
    /** After a 400 ms rest on an image card, report its box (null when the pointer leaves). */
    zoomOn?: boolean;
    onzoom?: (item: ClipItem | null, rect: DOMRect | null) => void;
    onclick: (e: MouseEvent) => void;
    ondblclick: () => void;
    ondragout: () => void;
    onremove: () => void;
    oncontext: (e: MouseEvent) => void;
    onhover: (dir: number) => void;
  } = $props();

  const thumb = $derived(item.editedThumb ?? item.thumb);
  const words = $derived(cardText(item));
  const noteHtml = $derived(item.kind === 'text' ? renderMd((item.text ?? '').split('\n').slice(0, 7).join('\n')) : '');
  let cardEl = $state<HTMLElement | null>(null);
  let zoomTimer: ReturnType<typeof setTimeout> | undefined;
  let thumbFailed = $state(false);

  const DRAG_PX = 6;
  let start: { x: number; y: number } | null = null;
  let dragged = false;

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    start = { x: e.clientX, y: e.clientY };
    dragged = false;
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', up);
  }
  function move(e: PointerEvent) {
    if (!start || dragged) return;
    if (Math.hypot(e.clientX - start.x, e.clientY - start.y) >= DRAG_PX) {
      dragged = true;
      cleanup();
      ondragout();
    }
  }
  function up() {
    cleanup();
  }
  function cleanup() {
    start = null;
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
    window.removeEventListener('pointercancel', up);
  }
  function click(e: MouseEvent) {
    if (dragged) {
      dragged = false;
      return;
    }
    onclick(e);
  }
  function enter(e: PointerEvent) {
    onhover(e.movementX === 0 ? 1 : Math.sign(e.movementX));
    clearTimeout(zoomTimer);
    if (zoomOn && item.kind === 'image' && onzoom) zoomTimer = setTimeout(() => cardEl && onzoom(item, cardEl.getBoundingClientRect()), ZOOM_DELAY);
  }
  function leave() {
    clearTimeout(zoomTimer);
    onzoom?.(null, null);
  }
  $effect(() => () => {
    cleanup();
    clearTimeout(zoomTimer);
  });
</script>

<div
  class="peg"
  class:selected
  class:copied
  class:focused
  style:--s={scale}
  style:width="{width}px"
  style:--card-op={cardOp}
  style:--theta="{theta}rad"
  style:--lift="{(1 - drop) * -72}px"
  style:opacity={drop < 0.15 ? drop / 0.15 : 1}
  role="presentation"
  onpointerenter={enter}
  onpointerleave={leave}
>
  <svg class="pin" viewBox="0 0 16 38" width={14 * scale} height={34 * scale} aria-hidden="true" class:pinned={item.pinned}>
    <rect x="3.2" y="1" width="4.4" height="35" rx="2" class="leg" />
    <rect x="8.4" y="1" width="4.4" height="35" rx="2" class="leg" />
    <circle cx="8" cy="9" r="3.2" class="coil" />
    <path d="M3.4 24h9.2" class="grip" />
  </svg>

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    bind:this={cardEl}
    class="card kind-{item.kind === 'image' ? 'image' : item.kind === 'text' ? 'text' : 'link'}"
    role="button"
    tabindex="-1"
    aria-label={words.title || t(item.kind === 'image' ? 'image' : 'rope.text')}
    onpointerdown={down}
    onclick={click}
    ondblclick={ondblclick}
    oncontextmenu={(e) => {
      e.preventDefault();
      oncontext(e);
    }}
  >
    {#if item.kind === 'image'}
      <div class="print">
        {#if thumb && !thumbFailed}
          <img src={convertFileSrc(thumb)} alt="" draggable="false" onerror={() => (thumbFailed = true)} />
        {:else}
          <span class="ph">{t('noPreview')}</span>
        {/if}
      </div>
    {:else if item.kind === 'text'}
      <div class="note">
        {#if item.title}<b>{item.title}</b>{/if}
        <div class="md">{@html noteHtml}</div>
      </div>
    {:else}
      <div class="tag">
        <i class="hole"></i>
        {#if item.thumb}<img class="lt" src={convertFileSrc(item.thumb)} alt="" draggable="false" />{/if}
        {#if words.title}<b>{words.title}</b>{/if}
        {#each words.lines as l, i (i)}<span class="host">{l}</span>{/each}
      </div>
    {/if}
    {#if copied}<span class="tick">{t('copied')}</span>{/if}
    <button type="button" class="x" tabindex="-1" aria-label={t('remove')} title={t('remove')}
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        onremove();
      }}>
      <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true"><path d="M2 2l6 6M8 2L2 8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
    </button>
  </div>
</div>

<style>
  .peg {
    position: relative;
    transform-origin: 50% 0;
    transform: translateY(var(--lift)) rotate(var(--theta));
    will-change: transform;
  }
  .pin {
    position: absolute;
    top: calc(var(--s) * -17px);
    left: 50%;
    margin-left: calc(var(--s) * -7px);
    z-index: 2;
    filter: drop-shadow(0 1px 1.5px rgba(0, 0, 0, 0.35));
    pointer-events: none;
  }
  .leg { fill: var(--text-tertiary); }
  .coil { fill: var(--text-secondary); }
  .grip { stroke: var(--card-bg); stroke-width: 1.2; stroke-linecap: round; opacity: 0.6; }
  .pinned .leg { fill: var(--accent); }
  .pinned .coil { fill: var(--accent-hover); }

  .card {
    position: relative;
    margin-top: calc(var(--s) * 8px);
    border-radius: var(--radius-xs);
    box-shadow: var(--shadow-md);
    color: var(--text);
    font: var(--text-caption);
    font-size: calc(var(--s) * 11px);
    opacity: var(--card-op);
    overflow: hidden;
    transition: box-shadow var(--dur-fast), transform var(--dur) var(--ease-out);
  }
  .peg:hover .card { box-shadow: var(--shadow-lg); }
  .selected .card { box-shadow: 0 0 0 2px var(--accent), var(--shadow-md); }
  .focused .card { outline: 2px solid var(--text); outline-offset: 2px; }
  .copied .card { transform: translateY(-6px); }
  :global([data-reduce-motion]) .card { transition: none; }

  .print { padding: calc(var(--s) * 5px) calc(var(--s) * 5px) calc(var(--s) * 14px); background: var(--card-bg); }
  .print img, .ph { display: block; width: 100%; height: calc(var(--s) * 76px); object-fit: cover; border-radius: 1px; background: var(--control-bg); pointer-events: none; }
  .ph { display: grid; place-items: center; color: var(--text-tertiary); font: var(--text-caption); }

  .note, .tag {
    display: flex;
    flex-direction: column;
    gap: 2px;
    height: calc(var(--s) * 92px);
    padding: calc(var(--s) * 8px);
    background: var(--elevated-bg);
    overflow: hidden;
  }
  .note { background: linear-gradient(135deg, var(--elevated-bg) 90%, var(--control-bg-hover) 100%); }
  .note b, .tag b {
    font: var(--text-callout);
    font-size: calc(var(--s) * 12px);
    font-weight: 600;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    word-break: break-word;
  }
  .tag span {
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tag { padding-top: calc(var(--s) * 14px); border-radius: var(--radius-sm); border-left: 3px solid var(--accent); }
  .hole {
    position: absolute;
    top: 5px;
    left: 50%;
    width: 7px;
    height: 7px;
    margin-left: -3.5px;
    border-radius: 50%;
    background: var(--window-bg);
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  .tag .host { color: var(--accent); font-weight: 600; }

  .tick {
    position: absolute;
    inset: auto 6px 6px auto;
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--on-accent);
    font: var(--text-caption);
    pointer-events: none;
  }
  .x {
    position: absolute;
    top: 3px;
    right: 3px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--elevated-bg);
    color: var(--text-secondary);
    box-shadow: var(--shadow-sm);
    opacity: 0;
    cursor: default;
    transition: opacity var(--dur-fast);
  }
  .card:hover .x { opacity: 1; }
  .x:hover { background: var(--red); color: var(--on-accent); }
  .tag .lt { display: block; width: 100%; height: calc(var(--s) * 40px); object-fit: cover; border-radius: 2px; flex: none; pointer-events: none; }
  .md { flex: 1; min-height: 0; overflow: hidden; color: var(--text-secondary); line-height: 1.3; word-break: break-word; }
  .md :global(p), .md :global(ul), .md :global(h1), .md :global(h2), .md :global(h3) { margin: 0; font-size: inherit; font-weight: inherit; }
  .md :global(h1), .md :global(h2), .md :global(h3) { color: var(--text); font-weight: 600; }
  .md :global(strong) { color: var(--text); }
  .md :global(ul) { padding-left: 1.1em; }
  .md :global(p) { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .md :global(code) { font-family: var(--font-mono, monospace); background: var(--control-bg); border-radius: 3px; padding: 0 2px; }
</style>
