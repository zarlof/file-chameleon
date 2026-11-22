<!--
  CardPopover — a card's own settings, opened by the gear on a card (or "Card settings…" in its menu):
  its colour (a swatch, a custom colour, or back to the line's colour). With a selection it changes every
  selected card. It sits below the card, or above it when there is no room; Esc or a click outside closes it.
-->
<script lang="ts">
  import { CARD_COLORS, hexOrNull } from './layout';
  import { t } from './i18n';

  let {
    rect,
    winW,
    winH,
    count = 1,
    current,
    el = $bindable(null),
    onpick,
    onclose,
  }: {
    /** The card's box in the window. */
    rect: DOMRect;
    winW: number;
    winH: number;
    /** How many cards it changes (a selection). */
    count?: number;
    /** The card's own colour, null = the line's. */
    current: string | null;
    el?: HTMLElement | null;
    onpick: (color: string | null) => void;
    onclose: () => void;
  } = $props();

  const W = 236;
  const H = 150;
  const left = $derived(Math.max(6, Math.min(rect.right - W, winW - W - 6)));
  const top = $derived(rect.bottom + 6 + H <= winH ? rect.bottom + 6 : Math.max(6, rect.top - H - 6));
  const own = $derived(hexOrNull(current));
  const custom = $derived(own !== null && !(CARD_COLORS as readonly string[]).includes(own));
</script>

<div class="cardpop" bind:this={el} role="dialog" aria-label={t('card.title')} style:left="{left}px" style:top="{top}px" style:width="{W}px">
  <div class="head">
    <b>{count > 1 ? t('card.many', { count }) : t('card.title')}</b>
    <button type="button" class="close" tabindex="-1" aria-label={t('close')} title={t('close')} onclick={onclose}>
      <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true"><path d="M2 2l6 6M8 2L2 8" /></svg>
    </button>
  </div>
  <span class="label">{t('card.color')}</span>
  <div class="swatches" role="radiogroup" aria-label={t('card.color')}>
    {#each CARD_COLORS as c (c)}
      <button type="button" role="radio" tabindex="-1" aria-checked={own === c} class="sw" class:on={own === c} style:--c={c} aria-label={c} title={c} onclick={() => onpick(c)}></button>
    {/each}
  </div>
  <div class="row">
    <button type="button" class="chip" class:on={own === null} tabindex="-1" onclick={() => onpick(null)}>{t('card.colorDefault')}</button>
    <label class="custom" title={t('card.colorCustom')}>
      <input type="color" tabindex="-1" class:on={custom} value={own ?? '#ffffff'} aria-label={t('card.colorCustom')} onchange={(e) => onpick(e.currentTarget.value)} />
      <span>{t('card.colorCustom')}</span>
    </label>
  </div>
</div>

<style>
  .cardpop {
    position: fixed;
    z-index: 11;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px 12px;
    border-radius: 12px;
    background: rgba(28, 28, 32, 0.94);
    -webkit-backdrop-filter: blur(14px);
    backdrop-filter: blur(14px);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12), var(--shadow-lg);
    color: #fff;
    font: var(--text-callout);
  }
  .head { display: flex; align-items: center; justify-content: space-between; }
  .close {
    display: grid; place-items: center; width: 18px; height: 18px; padding: 0; border: 0; border-radius: 50%;
    background: rgba(255, 255, 255, 0.12); color: #fff; cursor: default;
  }
  .close svg { stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; }
  .close:hover { background: var(--red); }
  .label { color: rgba(255, 255, 255, 0.66); font: var(--text-caption); }
  .swatches { display: grid; grid-template-columns: repeat(6, 1fr); gap: 7px; }
  .sw {
    width: 26px; height: 26px; padding: 0; border: 0; border-radius: 50%; background: var(--c);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.25); cursor: default;
  }
  .sw.on { box-shadow: 0 0 0 2px rgba(28, 28, 32, 1), 0 0 0 4px var(--accent); }
  .row { display: flex; align-items: center; gap: 8px; }
  .chip {
    padding: 3px 10px; border: 0; border-radius: 999px; background: rgba(255, 255, 255, 0.12); color: #fff;
    font: var(--text-caption); cursor: default;
  }
  .chip.on { background: var(--accent); color: var(--on-accent); }
  .custom { display: inline-flex; align-items: center; gap: 6px; color: rgba(255, 255, 255, 0.8); font: var(--text-caption); }
  .custom input { width: 22px; height: 22px; padding: 0; border: 0; border-radius: 50%; background: transparent; cursor: default; }
  .custom input.on { box-shadow: 0 0 0 2px rgba(28, 28, 32, 1), 0 0 0 4px var(--accent); }
</style>
