<!--
  Appearance settings — theme, accent, interface size (webview zoom, applied by the Shell),
  reduce motion, and how the drag wheel looks (colour, opacity, labels, size, detail line).
-->
<script lang="ts">
  import { Check } from '@lucide/svelte';
  import { onDestroy } from 'svelte';
  import { t } from '../../../lib/i18n';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { Accent, Settings, WheelLabels } from '../../../lib/types';
  import { Card, ListRow, SectionHeader, SegmentedControl, Select, Slider, Toggle } from '../../../lib/ui';

  const ACCENTS: Accent[] = ['blue', 'purple', 'pink', 'red', 'orange', 'yellow', 'green', 'graphite'];
  const SCALES = [80, 90, 100, 110, 120, 130, 140, 150];
  const LABELS: WheelLabels[] = ['auto', 'iconText', 'text', 'icon'];

  const s = $derived(settings.value);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  // Sliders: the thumb follows the drag at once, the value is saved after a short pause.
  type SliderKey = 'wheelOpacity' | 'wheelScale';
  let draft = $state<Partial<Record<SliderKey, number>>>({});
  const timers: Partial<Record<SliderKey, ReturnType<typeof setTimeout>>> = {};
  function slide(key: SliderKey, v: number) {
    draft[key] = v;
    clearTimeout(timers[key]);
    timers[key] = setTimeout(() => {
      set({ [key]: v });
      delete draft[key];
    }, 250);
  }
  onDestroy(() => Object.values(timers).forEach((id) => clearTimeout(id)));
  const opacity = $derived(draft.wheelOpacity ?? s.wheelOpacity);
  const wheelScale = $derived(draft.wheelScale ?? s.wheelScale);

  const pct = (value: number) => t('settings.appearance.percent', { value });
  const themes = $derived([
    { value: 'system' as const, label: t('settings.appearance.themeSystem') },
    { value: 'light' as const, label: t('settings.appearance.themeLight') },
    { value: 'dark' as const, label: t('settings.appearance.themeDark') },
  ]);
  /** Keeps an unusual stored value selectable. */
  const scaleOptions = $derived(
    (SCALES.includes(s.uiScale) ? SCALES : [...SCALES, s.uiScale].sort((a, b) => a - b)).map((n) => ({ value: String(n), label: pct(n) })),
  );
  const labelOptions = $derived(LABELS.map((l) => ({ value: l, label: t(`settings.appearance.labels.${l}`) })));
</script>

{#snippet swatch(color: string, name: string, on: boolean, pick: () => void)}
  <button type="button" role="radio" aria-checked={on} class="swatch" class:on style:--c={color} title={name} aria-label={name} onclick={pick}>
    {#if on}<Check size={12} strokeWidth={3} />{/if}
  </button>
{/snippet}

<section>
  <h1>{t('common.nav.appearance')}</h1>

  <Card>
    <ListRow title={t('settings.appearance.theme')}>
      {#snippet trailing()}<SegmentedControl options={themes} value={s.theme} size="sm" onchange={(v) => set({ theme: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.appearance.accent')}>
      {#snippet trailing()}
        <span class="swatches" role="radiogroup" aria-label={t('settings.appearance.accent')}>
          {#each ACCENTS as a (a)}{@render swatch(`var(--${a})`, t(`settings.accents.${a}`), s.accent === a, () => set({ accent: a }))}{/each}
        </span>
      {/snippet}
    </ListRow>
    <ListRow title={t('settings.appearance.uiScale')} subtitle={t('settings.appearance.uiScaleHint')}>
      {#snippet trailing()}<Select value={String(s.uiScale)} options={scaleOptions} label={t('settings.appearance.uiScale')} onchange={(v) => set({ uiScale: Number(v) })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.appearance.reduceMotion')} subtitle={t('settings.appearance.reduceMotionHint')}>
      {#snippet trailing()}<Toggle checked={s.reduceMotion} label={t('settings.appearance.reduceMotion')} onchange={(v) => set({ reduceMotion: v })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.appearance.wheel')} />
  <Card>
    <ListRow title={t('settings.appearance.wheelColor')}>
      {#snippet trailing()}
        <span class="swatches" role="radiogroup" aria-label={t('settings.appearance.wheelColor')}>
          {@render swatch('var(--accent)', t('settings.appearance.wheelColorAccent'), s.wheelColor === 'accent', () => set({ wheelColor: 'accent' }))}
          <span class="divider" aria-hidden="true"></span>
          {#each ACCENTS as a (a)}{@render swatch(`var(--${a})`, t(`settings.accents.${a}`), s.wheelColor === a, () => set({ wheelColor: a }))}{/each}
        </span>
      {/snippet}
    </ListRow>
    <ListRow title={t('settings.appearance.wheelOpacity')}>
      {#snippet trailing()}
        <span class="slider"><Slider value={opacity} min={40} max={100} step={5} label={t('settings.appearance.wheelOpacity')} oninput={(v) => slide('wheelOpacity', v)} /><output>{pct(opacity)}</output></span>
      {/snippet}
    </ListRow>
    <ListRow title={t('settings.appearance.wheelLabels')}>
      {#snippet trailing()}<Select value={s.wheelLabels} options={labelOptions} label={t('settings.appearance.wheelLabels')} onchange={(v) => set({ wheelLabels: v as WheelLabels })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.appearance.wheelSize')}>
      {#snippet trailing()}
        <span class="slider"><Slider value={wheelScale} min={80} max={130} step={5} label={t('settings.appearance.wheelSize')} oninput={(v) => slide('wheelScale', v)} /><output>{pct(wheelScale)}</output></span>
      {/snippet}
    </ListRow>
    <ListRow title={t('settings.appearance.wheelShowDetail')} subtitle={t('settings.appearance.wheelShowDetailHint')}>
      {#snippet trailing()}<Toggle checked={s.wheelShowDetail} label={t('settings.appearance.wheelShowDetail')} onchange={(v) => set({ wheelShowDetail: v })} />{/snippet}
    </ListRow>
  </Card>
</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  section > :global(.sh) { margin-top: var(--space-2); }

  .swatches { display: inline-flex; align-items: center; gap: 8px; }
  .swatch {
    display: grid; place-items: center; width: 20px; height: 20px; padding: 0; border: 0; border-radius: 50%;
    background: var(--c); color: #fff; cursor: default; transition: transform var(--dur-fast) var(--ease-spring), box-shadow var(--dur-fast);
  }
  .swatch:hover { transform: scale(1.12); }
  .swatch.on { box-shadow: 0 0 0 2px var(--elevated-bg), 0 0 0 4px var(--c); }
  .swatch:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .divider { width: 1px; height: 16px; margin: 0 2px; background: var(--separator); }

  .slider { display: inline-flex; align-items: center; gap: var(--space-3); width: 240px; }
  .slider output { min-width: 40px; text-align: right; font: var(--text-body); color: var(--text-secondary); font-variant-numeric: tabular-nums; }
</style>
