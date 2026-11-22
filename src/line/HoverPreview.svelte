<!--
  HoverPreview — a card shown 2.5 times larger while the pointer rests on it: the whole picture, all of
  a text (Markdown, scrolls when it is longer than the screen allows), a link's title and full address,
  a file's thumbnail, full name and folder. It opens below the card, or above it when there is more room
  there, and stays while the pointer is on the card or on the preview itself (so a long text can be
  scrolled). Clicking it opens the card. {@html} only ever receives renderMd output, which escapes first.
-->
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { LineCard } from './logic';
  import { cardText } from './logic';
  import { fileCard } from './files';
  import { renderMd } from './markdown';
  import { getLang, t } from './i18n';

  /** How much larger than the card the preview is. */
  const SCALE = 2.5;

  let {
    card,
    rect,
    cardW,
    winW,
    winH,
    vars = '',
    onenter,
    onleave,
    onopen,
  }: {
    card: LineCard;
    /** The card's box in the window. */
    rect: DOMRect;
    /** The card width on the rope (px). */
    cardW: number;
    winW: number;
    winH: number;
    /** Card look variables (style string) so the preview wears the card's colours. */
    vars?: string;
    onenter: () => void;
    onleave: () => void;
    onopen: () => void;
  } = $props();

  const GAP = 8;
  const item = $derived(card.item);
  const file = $derived(card.file);
  const w = $derived(Math.min(Math.max(260, cardW * SCALE), winW - 16));
  const below = $derived.by(() => {
    const down = winH - rect.bottom - GAP * 2;
    const up = rect.top - GAP * 2;
    return down >= 220 || down >= up;
  });
  const maxH = $derived(Math.max(80, below ? winH - rect.bottom - GAP * 2 : rect.top - GAP * 2));
  const left = $derived(Math.max(8, Math.min(rect.left + rect.width / 2 - w / 2, winW - w - 8)));

  const image = $derived(item?.kind === 'image' ? (item.edited ?? item.image ?? item.thumb) : null);
  const ratio = $derived(item?.width && item?.height ? item.height / item.width : 0.75);
  const imgH = $derived(Math.min(w * ratio, maxH));
  const html = $derived(item?.kind === 'text' ? renderMd(item.text ?? '') : '');
  const words = $derived(item ? cardText(item, 1) : { title: '', lines: [] as string[] });
  const url = $derived((item?.text ?? '').trim());
  const fc = $derived(file ? fileCard(file, getLang()) : null);
  const folder = $derived(file ? file.path.replace(/[\\/][^\\/]*$/, '') : '');
  let failed = $state(false);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="hoverpv"
  class:pic={!!image}
  role="button"
  tabindex="-1"
  aria-label={t('preview')}
  style={vars}
  style:width="{w}px"
  style:left="{left}px"
  style:top={below ? `${rect.bottom + GAP}px` : null}
  style:bottom={below ? null : `${winH - rect.top + GAP}px`}
  style:max-height="{maxH}px"
  onpointerenter={onenter}
  onpointerleave={onleave}
  onclick={onopen}
>
  {#if item?.kind === 'image'}
    {#if image && !failed}
      <img src={convertFileSrc(image)} alt="" draggable="false" style:height="{imgH}px" onerror={() => (failed = true)} />
    {:else}
      <span class="none">{t('noPreview')}</span>
    {/if}
  {:else if item?.kind === 'text'}
    <div class="body">
      {#if item.title}<b class="title">{item.title}</b>{/if}
      <div class="md">{@html html}</div>
    </div>
  {:else if item}
    <div class="body">
      {#if item.thumb}<img class="lt" src={convertFileSrc(item.thumb)} alt="" draggable="false" />{/if}
      <b class="title">{item.linkTitle ?? item.title ?? words.lines[0] ?? url}</b>
      {#if words.lines[0]}<span class="sub">{words.lines[0]}</span>{/if}
      <span class="url">{url}</span>
    </div>
  {:else if file && fc}
    <div class="body">
      {#if file.thumb && !failed}
        <img class="ft" src={`${convertFileSrc(file.thumb)}?v=${file.mtime ?? 0}`} alt="" draggable="false" onerror={() => (failed = true)} />
      {/if}
      <b class="title">{fc.name}</b>
      <span class="sub">{[fc.ext.toUpperCase(), fc.sizeText, t(`source.${fc.sourceIcon}`)].filter(Boolean).join(' · ')}</span>
      <span class="url"><i>{t('preview.path')}:</i> {folder}</span>
    </div>
  {/if}
</div>

<style>
  .hoverpv {
    position: fixed;
    z-index: 9;
    overflow: auto;
    border-radius: calc(var(--c-radius, 10px) + 4px);
    background: var(--c-bg, rgba(20, 20, 24, 0.92));
    -webkit-backdrop-filter: var(--c-blur, none);
    backdrop-filter: var(--c-blur, none);
    box-shadow: inset 0 0 0 1px var(--c-edge, rgba(255, 255, 255, 0.1)), var(--shadow-lg);
    color: var(--c-fg, #fff);
    font: var(--text-body);
    overscroll-behavior: contain;
    cursor: default;
    animation: pop var(--dur-fast) var(--ease-out);
  }
  @keyframes pop { from { opacity: 0; transform: scale(0.96); } }
  :global([data-reduce-motion]) .hoverpv { animation: none; }
  .hoverpv.pic { padding: 6px; }
  .hoverpv > img { display: block; width: 100%; object-fit: contain; border-radius: var(--c-radius, 10px); pointer-events: none; }
  .none { display: grid; place-items: center; height: 120px; color: var(--c-fg2); }
  .body { display: flex; flex-direction: column; gap: 6px; padding: 14px 16px; }
  .title { font: var(--text-headline); font-weight: 650; color: var(--c-fg); word-break: break-word; }
  .sub { color: var(--c-fg2); font: var(--text-callout); }
  .url { color: var(--c-fg2); font: var(--text-callout); word-break: break-all; user-select: text; }
  .url i { font-style: normal; font-weight: 600; }
  .lt, .ft { display: block; width: 100%; max-height: 220px; object-fit: contain; border-radius: var(--c-radius, 10px); pointer-events: none; }
  .md { color: var(--c-fg); line-height: 1.45; word-break: break-word; user-select: text; }
  .md :global(p) { margin: 0 0 0.5em; white-space: pre-wrap; }
  .md :global(p:last-child) { margin-bottom: 0; }
  .md :global(h1), .md :global(h2), .md :global(h3) { margin: 0.3em 0; font-size: 1.05em; font-weight: 650; }
  .md :global(ul) { margin: 0 0 0.5em; padding-left: 1.2em; }
  .md :global(code) { font-family: var(--font-mono, monospace); background: color-mix(in srgb, var(--c-fg) 12%, transparent); border-radius: 3px; padding: 0 3px; }
</style>
