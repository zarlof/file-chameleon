<!--
  Peg — one clip hanging from the rope by a little clothespin. The card swings around the pin
  (`theta`, driven by the shared loop in Line.svelte). Image = photo print, text = note sheet,
  link = tag card. Gestures: click, Ctrl/Shift-click, double-click, drag out (6 px), right-click, hover x.
-->
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { ClipItem } from '../lib/types';
  import { cardText } from './logic';
  import { t } from './i18n';

  let {
    item,
    theta,
    drop,
    selected,
    copied,
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
    onclick: (e: MouseEvent) => void;
    ondblclick: () => void;
    ondragout: () => void;
    onremove: () => void;
    oncontext: (e: MouseEvent) => void;
    onhover: (dir: number) => void;
  } = $props();

  const thumb = $derived(item.editedThumb ?? item.thumb);
  const words = $derived(cardText(item));
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
  }
  $effect(() => cleanup);
</script>

<div
  class="peg"
  class:selected
  class:copied
  style:--theta="{theta}rad"
  style:--lift="{(1 - drop) * -72}px"
  style:opacity={drop < 0.15 ? drop / 0.15 : 1}
  role="presentation"
  onpointerenter={enter}
>
  <svg class="pin" viewBox="0 0 16 38" width="14" height="34" aria-hidden="true" class:pinned={item.pinned}>
    <rect x="3.2" y="1" width="4.4" height="35" rx="2" class="leg" />
    <rect x="8.4" y="1" width="4.4" height="35" rx="2" class="leg" />
    <circle cx="8" cy="9" r="3.2" class="coil" />
    <path d="M3.4 24h9.2" class="grip" />
  </svg>

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
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
        {#if words.title}<b>{words.title}</b>{/if}
        {#each words.lines as l, i (i)}<span>{l}</span>{/each}
      </div>
    {:else}
      <div class="tag">
        <i class="hole"></i>
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
    width: 112px;
    transform-origin: 50% 0;
    transform: translateY(var(--lift)) rotate(var(--theta));
    will-change: transform;
  }
  .pin {
    position: absolute;
    top: -17px;
    left: 50%;
    margin-left: -7px;
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
    margin-top: 8px;
    border-radius: var(--radius-xs);
    box-shadow: var(--shadow-md);
    color: var(--text);
    font: var(--text-caption);
    overflow: hidden;
    transition: box-shadow var(--dur-fast), transform var(--dur) var(--ease-out);
  }
  .peg:hover .card { box-shadow: var(--shadow-lg); }
  .selected .card { box-shadow: 0 0 0 2px var(--accent), var(--shadow-md); }
  .copied .card { transform: translateY(-6px); }
  :global([data-reduce-motion]) .card { transition: none; }

  .print { padding: 5px 5px 14px; background: var(--card-bg); }
  .print img, .ph { display: block; width: 100%; height: 76px; object-fit: cover; border-radius: 1px; background: var(--control-bg); pointer-events: none; }
  .ph { display: grid; place-items: center; color: var(--text-tertiary); font: var(--text-caption); }

  .note, .tag {
    display: flex;
    flex-direction: column;
    gap: 2px;
    height: 92px;
    padding: var(--space-2);
    background: var(--elevated-bg);
    overflow: hidden;
  }
  .note { background: linear-gradient(135deg, var(--elevated-bg) 90%, var(--control-bg-hover) 100%); }
  .note b, .tag b {
    font: var(--text-callout);
    font-weight: 600;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    word-break: break-word;
  }
  .note span, .tag span {
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tag { padding-top: 14px; border-radius: var(--radius-sm); border-left: 3px solid var(--accent); }
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
</style>
