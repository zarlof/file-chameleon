<!--
  Peg — one card hanging from the rope by a little clothespin (Pin.svelte). The card swings around the pin
  (`theta`, driven by the shared loop in Line.svelte). Dark glass by default (see cardVars in layout.ts):
  image = rounded thumbnail with a thin border, text = translucent note, link = preview image, title and
  domain, file = thumbnail or type glyph, name, extension chip, size and source icon. A small 1-9 badge
  sits at the bottom left. Gestures: click, Ctrl/Shift-click, double-click, drag out (6 px), right-click, hover x.
-->
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { cardText, type LineCard } from './logic';
  import { fileCard } from './files';
  import Pin from './Pin.svelte';
  import Spider from './Spider.svelte';
  import { renderMd } from './markdown';
  import { getLang, t } from './i18n';

  const ZOOM_DELAY = 400;

  let {
    card: model,
    theta,
    drop,
    selected,
    copied,
    focused = false,
    scale = 1,
    width = 112,
    cardOp = 1,
    pinStyle = 'plastic',
    pinColor = 'auto',
    num = null,
    zoomOn = false,
    onzoom,
    onclick,
    ondblclick,
    ondragout,
    onremove,
    oncontext,
    onhover,
    onpin,
  }: {
    card: LineCard;
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
    pinStyle?: 'spider' | 'plastic' | 'wood' | 'metal';
    pinColor?: string;
    /** 1..9 badge (the key that picks this card), or null. */
    num?: number | null;
    /** After a 400 ms rest on an image card, report its box (null when the pointer leaves). */
    zoomOn?: boolean;
    onzoom?: (item: NonNullable<LineCard['item']> | null, rect: DOMRect | null) => void;
    onclick: (e: MouseEvent) => void;
    ondblclick: () => void;
    ondragout: () => void;
    onremove: () => void;
    oncontext: (e: MouseEvent) => void;
    onhover: (dir: number) => void;
    /** The pin button at the top left (pinned cards stay first). */
    onpin?: () => void;
  } = $props();

  const item = $derived(model.item);
  const file = $derived(model.file);
  const thumb = $derived(item ? (item.editedThumb ?? item.thumb) : null);
  const words = $derived(item ? cardText(item) : { title: '', lines: [] as string[] });
  const noteHtml = $derived(item?.kind === 'text' ? renderMd((item.text ?? '').split('\n').slice(0, 7).join('\n')) : '');
  const fc = $derived(file ? fileCard(file, getLang()) : null);
  const app = $derived(model.app ?? null);
  const kindClass = $derived(app ? 'app' : file ? 'file' : item?.kind === 'image' ? 'image' : item?.kind === 'text' ? 'text' : 'link');
  const pinned = $derived(item?.pinned ?? file?.pinned ?? false);
  const spider = $derived(pinStyle === 'spider');
  const label = $derived(app ? app.name : file ? file.name : words.title || t(item?.kind === 'image' ? 'image' : 'rope.text'));
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
    if (zoomOn && item?.kind === 'image' && onzoom) zoomTimer = setTimeout(() => cardEl && item && onzoom(item, cardEl.getBoundingClientRect()), ZOOM_DELAY);
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
  class:spider
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
  {#if spider}
    <span class="hanger"><Spider {scale} /></span>
  {:else}
    <span class="pin"><Pin pinStyle={pinStyle === 'spider' ? 'plastic' : pinStyle} color={pinColor} {pinned} {scale} /></span>
  {/if}

  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    bind:this={cardEl}
    class="card kind-{kindClass}"
    role="button"
    tabindex="-1"
    aria-label={label}
    onpointerdown={down}
    onclick={click}
    ondblclick={ondblclick}
    oncontextmenu={(e) => {
      e.preventDefault();
      oncontext(e);
    }}
  >
    {#if app}
      <div class="app">
        {#if app.icon && !thumbFailed}
          <img src={convertFileSrc(app.icon)} alt="" draggable="false" onerror={() => (thumbFailed = true)} />
        {:else}
          <span class="glyph" aria-hidden="true">{app.name.slice(0, 1).toUpperCase()}</span>
        {/if}
        <b title={app.name}>{app.name}</b>
      </div>
    {:else if file && fc}
      <div class="file">
        <div class="fthumb">
          {#if file.thumb && !thumbFailed}
            <img src={`${convertFileSrc(file.thumb)}?v=${file.mtime ?? 0}`} alt="" draggable="false" onerror={() => (thumbFailed = true)} />
          {:else}
            <svg viewBox="0 0 24 28" width={22 * scale} height={26 * scale} aria-hidden="true">
              <path d="M3 1.5h12l6 6v19a1.5 1.5 0 0 1-1.5 1.5h-16.5a1.5 1.5 0 0 1-1.5-1.5v-23.5a1.5 1.5 0 0 1 1.5-1.5Z" class="doc" />
              <path d="M15 1.5v6h6" class="fold" />
            </svg>
          {/if}
        </div>
        <b title={fc.name}>{fc.name}</b>
        <div class="meta">
          {#if fc.ext}<i class="chip">{fc.ext}</i>{/if}
          <span>{fc.sizeText}</span>
          <svg class="src" viewBox="0 0 12 12" width="11" height="11" role="img" aria-label={t(`source.${fc.sourceIcon}`)}>
            <title>{t(`source.${fc.sourceIcon}`)}</title>
            {#if fc.sourceIcon === 'downloaded'}
              <path d="M6 1.5v7M3 6l3 3 3-3M2 10.5h8" />
            {:else if fc.sourceIcon === 'converted'}
              <path d="M2 6a4 4 0 0 1 7-2.6M10 6a4 4 0 0 1-7 2.6M9.4 1.2v2.4H7M2.6 10.8V8.4H5" />
            {:else}
              <path d="M6 2v8M2 6h8" />
            {/if}
          </svg>
        </div>
      </div>
    {:else if item?.kind === 'image'}
      <div class="print">
        {#if thumb && !thumbFailed}
          <img src={convertFileSrc(thumb)} alt="" draggable="false" onerror={() => (thumbFailed = true)} />
        {:else}
          <span class="ph">{t('noPreview')}</span>
        {/if}
      </div>
    {:else if item?.kind === 'text'}
      <div class="note">
        {#if item.title}<b>{item.title}</b>{/if}
        <div class="md">{@html noteHtml}</div>
      </div>
    {:else if item}
      <div class="tag">
        {#if item.thumb}<img class="lt" src={convertFileSrc(item.thumb)} alt="" draggable="false" />{:else}<span class="lt fav" aria-hidden="true">{(words.lines[0] ?? '?').slice(0, 1).toUpperCase()}</span>{/if}
        {#if words.title}<b>{words.title}</b>{/if}
        {#each words.lines as l, i (i)}<span class="host">{l}</span>{/each}
      </div>
    {/if}
    {#if num !== null}<span class="num" aria-hidden="true">{num}</span>{/if}
    {#if onpin && !app}
      <button type="button" class="pinbtn" class:on={pinned} tabindex="-1" aria-pressed={pinned}
        aria-label={t(pinned ? 'menu.unpin' : 'menu.pin')} title={t(pinned ? 'menu.unpin' : 'menu.pin')}
        onpointerdown={(e) => e.stopPropagation()}
        ondblclick={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation();
          onpin();
        }}>
        <svg viewBox="0 0 24 24" width="12" height="12" aria-hidden="true">
          <g transform="rotate(40 12 12)">
            <path d="M12 17v5" />
            <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z" class="head" />
          </g>
        </svg>
      </button>
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
    top: calc(var(--s) * -14px);
    left: 50%;
    margin-left: calc(var(--s) * -5px);
    z-index: 2;
    line-height: 0;
    pointer-events: none;
  }

  .hanger {
    position: absolute;
    top: calc(var(--s) * -8px);
    left: 50%;
    margin-left: calc(var(--s) * -13px);
    z-index: 2;
    line-height: 0;
    pointer-events: none;
  }
  .card {
    position: relative;
    margin-top: calc(var(--s) * 8px);
    border-radius: var(--c-radius, 10px);
    background: var(--c-bg, rgba(20, 20, 24, 0.78));
    -webkit-backdrop-filter: var(--c-blur, none);
    backdrop-filter: var(--c-blur, none);
    box-shadow: inset 0 0 0 1px var(--c-edge, rgba(255, 255, 255, 0.1)), var(--c-shadow, none);
    color: var(--c-fg, #fff);
    font: var(--text-caption);
    font-size: calc(var(--s) * 11px);
    opacity: var(--card-op);
    overflow: hidden;
    transition: box-shadow var(--dur-fast), transform var(--dur) var(--ease-out);
  }
  /* padding, not a card margin: a margin would collapse through the peg and push the spider down */
  .peg.spider { padding-top: calc(var(--s) * 26px); }
  .spider .card { margin-top: 0; }
  .peg:hover .card { transform: translateY(-1px); }
  .selected .card { box-shadow: 0 0 0 2px var(--accent), var(--c-shadow, none); }
  .focused .card { outline: 2px solid var(--c-fg, #fff); outline-offset: 2px; }
  .copied .card { transform: translateY(-6px); }
  :global([data-reduce-motion]) .card { transition: none; }

  .card.kind-image { box-shadow: inset 0 0 0 1px var(--c-edge, rgba(255, 255, 255, 0.18)), var(--c-shadow, none); background: transparent; -webkit-backdrop-filter: none; backdrop-filter: none; }
  .print { height: calc(var(--s) * 92px); border-radius: var(--c-radius, 10px); overflow: hidden; }
  .print img, .ph { display: block; width: 100%; height: 100%; object-fit: cover; pointer-events: none; }
  .ph { display: grid; place-items: center; background: var(--c-bg); color: var(--c-fg2); font: var(--text-caption); }

  .note, .tag, .file {
    display: flex;
    flex-direction: column;
    gap: 2px;
    height: calc(var(--s) * 92px);
    padding: calc(var(--s) * 8px);
    overflow: hidden;
  }
  .note b, .tag b, .file b {
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
  .file b { -webkit-line-clamp: 1; line-clamp: 1; }
  .tag { padding: calc(var(--s) * 4px) calc(var(--s) * 4px) calc(var(--s) * 6px); }
  .tag b, .tag span:not(.lt) { padding: 0 calc(var(--s) * 4px); }
  .tag span:not(.lt) { color: var(--c-fg2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tag .host { color: var(--c-fg2); font-weight: 500; }
  .tag .lt { display: block; width: 100%; height: calc(var(--s) * 40px); object-fit: cover; border-radius: calc(var(--c-radius, 10px) - 3px); flex: none; pointer-events: none; }
  .tag .fav { display: grid; place-items: center; background: color-mix(in srgb, var(--c-fg) 10%, transparent); color: var(--c-fg2); font-weight: 650; font-size: calc(var(--s) * 18px); }
  .md { flex: 1; min-height: 0; overflow: hidden; color: var(--c-fg2); line-height: 1.3; word-break: break-word; }
  .md :global(p), .md :global(ul), .md :global(h1), .md :global(h2), .md :global(h3) { margin: 0; font-size: inherit; font-weight: inherit; }
  .md :global(h1), .md :global(h2), .md :global(h3) { color: var(--c-fg); font-weight: 600; }
  .md :global(strong) { color: var(--c-fg); }
  .md :global(ul) { padding-left: 1.1em; }
  .md :global(p) { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .md :global(code) { font-family: var(--font-mono, monospace); background: color-mix(in srgb, var(--c-fg) 12%, transparent); border-radius: 3px; padding: 0 2px; }
  .note b { color: var(--c-fg); }

  .app {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: calc(var(--s) * 8px);
    height: calc(var(--s) * 92px);
    padding: calc(var(--s) * 8px);
    overflow: hidden;
  }
  .app img, .app .glyph { width: calc(var(--s) * 40px); height: calc(var(--s) * 40px); flex: none; pointer-events: none; }
  .app img { object-fit: contain; }
  .app .glyph { display: grid; place-items: center; border-radius: 10px; background: color-mix(in srgb, var(--c-fg) 14%, transparent); font-weight: 650; font-size: calc(var(--s) * 20px); }
  .app b {
    max-width: 100%;
    font: var(--text-callout);
    font-size: calc(var(--s) * 11.5px);
    font-weight: 600;
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fthumb { flex: 1; min-height: 0; display: grid; place-items: center; border-radius: calc(var(--c-radius, 10px) - 4px); background: color-mix(in srgb, var(--c-fg) 8%, transparent); overflow: hidden; }
  .fthumb img { display: block; width: 100%; height: 100%; object-fit: cover; pointer-events: none; }
  .doc { fill: color-mix(in srgb, var(--c-fg) 16%, transparent); stroke: var(--c-fg2); stroke-width: 1.3; stroke-linejoin: round; }
  .fold { fill: none; stroke: var(--c-fg2); stroke-width: 1.3; stroke-linejoin: round; }
  .meta { display: flex; align-items: center; gap: 4px; color: var(--c-fg2); font-size: calc(var(--s) * 10px); white-space: nowrap; }
  .chip { padding: 0 4px; border-radius: 4px; background: color-mix(in srgb, var(--c-fg) 14%, transparent); color: var(--c-fg); font-style: normal; font-weight: 600; text-transform: uppercase; }
  .src { margin-left: auto; flex: none; fill: none; stroke: var(--c-fg2); stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; }

  .num {
    position: absolute;
    left: 5px;
    bottom: 5px;
    display: grid;
    place-items: center;
    min-width: 15px;
    height: 15px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.55);
    -webkit-backdrop-filter: blur(4px);
    backdrop-filter: blur(4px);
    color: #fff;
    font: var(--text-caption);
    font-size: 10px;
    font-weight: 600;
    pointer-events: none;
  }
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
    top: 4px;
    right: 4px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    opacity: 0;
    cursor: default;
    transition: opacity var(--dur-fast);
  }
  .card:hover .x { opacity: 1; }
  .pinbtn {
    position: absolute;
    top: 4px;
    left: 4px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    opacity: 0;
    cursor: default;
    transition: opacity var(--dur-fast);
    z-index: 1;
  }
  .pinbtn svg { fill: none; stroke: currentColor; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }
  .card:hover .pinbtn, .pinbtn.on { opacity: 1; }
  .pinbtn.on { background: var(--accent); color: var(--on-accent); }
  .pinbtn.on .head { fill: currentColor; }
  .pinbtn:hover { background: var(--accent); color: var(--on-accent); }
  .x:hover { background: var(--red); }
</style>
