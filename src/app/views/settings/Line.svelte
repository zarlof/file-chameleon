<!--
  Line settings ("İp") — the clothesline: on/off, edge reveal, peek on copy, breeze, look (colour,
  thickness, opacity, size with a live preview), zoom and sound, which ropes to show, the hotkeys
  (with conflict status) and a "show now" button. Texts live in line.json.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { Check, TriangleAlert } from '@lucide/svelte';
  import { formatHotkey, sameHotkey } from '../../../lib/hotkeys';
  import { currentLang, t } from '../../../lib/i18n';
  import { api, errorOf } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { HotkeyStatus, Settings } from '../../../lib/types';
  import { Button, Card, HotkeyRecorder, ListRow, SectionHeader, SegmentedControl, Slider, Toggle, toast } from '../../../lib/ui';
  import { cardOpacity, ropeColor, sizeInfo } from '../../../line/layout';
  import { ropePath, ropeY } from '../../../line/physics';

  const s = $derived(settings.value);
  const off = $derived(!s.lineEnabled);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  function setRope(key: 'images' | 'text' | 'links', v: boolean) {
    set({ lineRopes: { ...s.lineRopes, [key]: v } });
  }

  // ---- look
  const SWATCHES = [
    { id: '#b89b6a', name: 'natural' },
    { id: '#3a3a3c', name: 'charcoal' },
    { id: '#f2f2f7', name: 'white' },
    { id: 'accent', name: 'accent' },
    { id: '#ff453a', name: 'red' },
    { id: '#30d158', name: 'green' },
    { id: '#0a84ff', name: 'blue' },
    { id: '#ff6bb0', name: 'pink' },
  ] as const;
  const sizeOptions = $derived([
    { value: 'small' as const, label: t('line.settings.sizeSmall') },
    { value: 'medium' as const, label: t('line.settings.sizeMedium') },
    { value: 'large' as const, label: t('line.settings.sizeLarge') },
  ]);
  const isHex = (v: string) => /^#[0-9a-f]{6}$/i.test(v);
  const custom = $derived(isHex(s.lineColor) && !SWATCHES.some((w) => w.id === s.lineColor));
  let thickness = $state(settings.value.lineThickness);
  let opacity = $state(settings.value.lineOpacity);
  const slideTimers: Partial<Record<'lineThickness' | 'lineOpacity', ReturnType<typeof setTimeout>>> = {};
  function slide(key: 'lineThickness' | 'lineOpacity', v: number) {
    if (key === 'lineThickness') thickness = v;
    else opacity = v;
    clearTimeout(slideTimers[key]);
    slideTimers[key] = setTimeout(() => set({ [key]: v } as Partial<Settings>), 120);
  }

  // live preview: one rope, three sample cards, drawn with the same maths as the line
  const size = $derived(sizeInfo(s.lineSize));
  const PW = $derived(48 + 3 * size.card + 2 * 16);
  let wrapW = $state(0);
  const fitScale = $derived(wrapW > 0 ? Math.min(1, wrapW / PW) : 1);
  const pcolor = $derived(ropeColor(s.lineColor, 'var(--accent)'));
  const pop = $derived(Math.min(100, Math.max(30, opacity)) / 100);
  const pcenters = $derived([0, 1, 2].map((i) => 24 + i * (size.card + 16) + size.card / 2));
  const sag = $derived(18 * size.scale);
  const pd = $derived(ropePath(PW, pcenters, sag));
  const samples = $derived([t('line.settings.sampleText'), t('line.settings.sampleLink'), t('line.settings.previewLabel')]);

  // ---- hotkeys
  const OTHERS = [
    { key: 'hotkeyShelf', name: 'settings.hotkeys.shelf' },
    { key: 'hotkeyLibrary', name: 'settings.hotkeys.library' },
    { key: 'hotkeyDownloadLink', name: 'settings.hotkeys.downloadLink' },
  ] as const;
  const EXTRA = [
    { key: 'hotkeyLineSearch', name: 'lineSearch', label: 'hotkeySearch' },
    { key: 'hotkeyCapture', name: 'capture', label: 'hotkeyCapture' },
    { key: 'hotkeyCopyLatestImage', name: 'copyLatestImage', label: 'hotkeyCopyLatest' },
  ] as const;
  let statuses = $state<HotkeyStatus[]>([]);
  let statusTimer: ReturnType<typeof setTimeout> | undefined;

  async function loadStatus() {
    try {
      statuses = await api.hotkeyStatus();
    } catch {
      statuses = [];
    }
  }
  onMount(() => {
    void loadStatus();
    return () => {
      clearTimeout(statusTimer);
      clearTimeout(slideTimers.lineThickness);
      clearTimeout(slideTimers.lineOpacity);
    };
  });

  function failed(name: HotkeyStatus['name'], value: string): boolean {
    const st = statuses.find((x) => x.name === name);
    return !!value && !!st && !st.ok && (st.keys === value || sameHotkey(st.keys, value));
  }
  const conflict = $derived(failed('line', s.hotkeyLine));

  function reloadStatusSoon() {
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => void loadStatus(), 300);
  }

  async function setHotkey(value: string) {
    const clash = [
      ...OTHERS.map((h) => ({ k: h.key as keyof Settings, n: t(h.name) })),
      ...EXTRA.map((h) => ({ k: h.key as keyof Settings, n: t(`line.settings.${h.label}`) })),
    ].find((h) => value && sameHotkey(String(s[h.k]), value));
    if (clash) {
      toast.show(t('line.settings.duplicate', { keys: formatHotkey(value, currentLang()), name: clash.n }), 'error');
      return;
    }
    if (await settings.update({ hotkeyLine: value })) reloadStatusSoon();
  }

  async function setExtra(key: (typeof EXTRA)[number]['key'], value: string) {
    const all: { k: keyof Settings; n: string }[] = [
      ...OTHERS.map((h) => ({ k: h.key, n: t(h.name) })),
      { k: 'hotkeyLine', n: t('line.settings.hotkey') },
      ...EXTRA.map((h) => ({ k: h.key, n: t(`line.settings.${h.label}`) })),
    ];
    const clash = value ? all.find((h) => h.k !== key && sameHotkey(String(s[h.k]), value)) : undefined;
    if (clash) {
      toast.show(t('line.settings.duplicate', { keys: formatHotkey(value, currentLang()), name: clash.n }), 'error');
      return;
    }
    if (await settings.update({ [key]: value } as Partial<Settings>)) reloadStatusSoon();
  }

  async function showNow() {
    try {
      await api.lineShow('hotkey');
    } catch (e) {
      toast.show(errorOf(e).message, 'error');
    }
  }
</script>

<section>
  <h1>{t('line.settings.title')}</h1>
  <p class="intro">{t('line.settings.intro')}</p>

  {#if !s.clipboardEnabled}
    <button type="button" class="hint" onclick={() => nav.go('clipboardSettings')}>
      <TriangleAlert size={14} />
      <span>{t('line.settings.clipboardOff')}</span>
    </button>
  {/if}

  <SectionHeader title={t('line.settings.section.general')} />
  <Card>
    <ListRow title={t('line.settings.enable')} subtitle={t('line.settings.enableHint')}>
      {#snippet trailing()}<Toggle checked={s.lineEnabled} label={t('line.settings.enable')} onchange={(v) => set({ lineEnabled: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.edgeReveal')} subtitle={t('line.settings.edgeRevealHint')}>
      {#snippet trailing()}<Toggle checked={s.lineEdgeReveal} disabled={off} label={t('line.settings.edgeReveal')} onchange={(v) => set({ lineEdgeReveal: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.peekOnCopy')} subtitle={t('line.settings.peekOnCopyHint')}>
      {#snippet trailing()}<Toggle checked={s.linePeekOnCopy} disabled={off} label={t('line.settings.peekOnCopy')} onchange={(v) => set({ linePeekOnCopy: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.breeze')} subtitle={t('line.settings.breezeHint')}>
      {#snippet trailing()}<Toggle checked={s.lineBreeze} disabled={off || s.reduceMotion} label={t('line.settings.breeze')} onchange={(v) => set({ lineBreeze: v })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('line.settings.section.look')} />
  <Card>
    <ListRow title={t('line.settings.color')}>
      {#snippet trailing()}
        <span class="swatches" role="radiogroup" aria-label={t('line.settings.color')}>
          {#each SWATCHES as w (w.id)}
            <button type="button" role="radio" aria-checked={s.lineColor === w.id} class="swatch" class:on={s.lineColor === w.id} disabled={off}
              style:--c={w.id === 'accent' ? 'var(--accent)' : w.id} title={t(`line.settings.color.${w.name}`)} aria-label={t(`line.settings.color.${w.name}`)}
              onclick={() => set({ lineColor: w.id })}>
              {#if s.lineColor === w.id}<Check size={12} strokeWidth={3} />{/if}
            </button>
          {/each}
          <input type="color" class="custom" class:on={custom} disabled={off} value={isHex(s.lineColor) ? s.lineColor : '#b89b6a'}
            title={t('line.settings.colorCustom')} aria-label={t('line.settings.colorCustom')}
            onchange={(e) => set({ lineColor: e.currentTarget.value })} />
        </span>
      {/snippet}
    </ListRow>
    <ListRow title={t('line.settings.thickness')}>
      {#snippet trailing()}<span class="slider"><Slider value={thickness} min={1} max={6} step={1} disabled={off} label={t('line.settings.thickness')} oninput={(v) => slide('lineThickness', v)} /><output>{thickness}</output></span>{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.opacity')}>
      {#snippet trailing()}<span class="slider"><Slider value={opacity} min={30} max={100} step={5} disabled={off} label={t('line.settings.opacity')} oninput={(v) => slide('lineOpacity', v)} /><output>{opacity}%</output></span>{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.size')}>
      {#snippet trailing()}<SegmentedControl options={sizeOptions} value={s.lineSize} size="sm" disabled={off} onchange={(v) => set({ lineSize: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.zoom')} subtitle={t('line.settings.zoomHint')}>
      {#snippet trailing()}<Toggle checked={s.lineZoomOnHover} disabled={off} label={t('line.settings.zoom')} onchange={(v) => set({ lineZoomOnHover: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.sound')} subtitle={t('line.settings.soundHint')}>
      {#snippet trailing()}<Toggle checked={s.lineSound} disabled={off || s.reduceMotion} label={t('line.settings.sound')} onchange={(v) => set({ lineSound: v })} />{/snippet}
    </ListRow>
    <div class="pvwrap" bind:clientWidth={wrapW} style:height="{size.band * fitScale}px">
    <div class="pv" aria-hidden="true" style:width="{PW}px" style:transform="scale({fitScale})" style:--op={pop} style:--rope={pcolor} style:--w="{Math.min(6, Math.max(1, thickness))}px" style:height="{size.band}px" style:--s={size.scale}>
      <svg width={PW} height={sag + 40} viewBox="0 0 {PW} {sag + 40}">
        <g transform="translate(0 {16 * size.scale})">
          <path d={pd} class="strand" />
          <circle cx="3" cy="0" r={3 + thickness * 0.9} class="knot" />
          <circle cx={PW - 3} cy="0" r={3 + thickness * 0.9} class="knot" />
        </g>
      </svg>
      {#each pcenters as c, i (i)}
        <span class="pcard" style:left="{c - size.card / 2}px" style:width="{size.card}px" style:top="{16 * size.scale + ropeY(PW, c, pcenters, sag) + 8 * size.scale}px"
          style:opacity={cardOpacity(opacity)}>{samples[i]}</span>
      {/each}
    </div>
    </div>
  </Card>

  <SectionHeader title={t('line.settings.section.ropes')} />
  <Card>
    <ListRow title={t('line.settings.ropesImages')}>
      {#snippet trailing()}<Toggle checked={s.lineRopes.images} disabled={off} label={t('line.settings.ropesImages')} onchange={(v) => setRope('images', v)} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.ropesText')}>
      {#snippet trailing()}<Toggle checked={s.lineRopes.text} disabled={off} label={t('line.settings.ropesText')} onchange={(v) => setRope('text', v)} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.ropesLinks')}>
      {#snippet trailing()}<Toggle checked={s.lineRopes.links} disabled={off} label={t('line.settings.ropesLinks')} onchange={(v) => setRope('links', v)} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('line.settings.section.hotkey')} />
  <Card>
    <ListRow title={t('line.settings.hotkey')} subtitle={conflict ? undefined : t('line.settings.hotkeyHint')}>
      {#if conflict}<span class="conflict"><TriangleAlert size={13} />{t('line.settings.conflict')}</span>{/if}
      {#snippet trailing()}
        <HotkeyRecorder value={s.hotkeyLine} disabled={off} status={conflict ? false : undefined} label={t('line.settings.hotkey')} onchange={setHotkey} />
      {/snippet}
    </ListRow>
    {#each EXTRA as h (h.key)}
      {@const clash = failed(h.name, s[h.key])}
      <ListRow title={t(`line.settings.${h.label}`)} subtitle={clash ? undefined : t(`line.settings.${h.label}Hint`)}>
        {#if clash}<span class="conflict"><TriangleAlert size={13} />{t('line.settings.conflict')}</span>{:else if !s.clipboardEnabled && (h.key === 'hotkeyCapture' || h.key === 'hotkeyCopyLatestImage')}<span class="needs"><TriangleAlert size={13} />{t('line.settings.needsClipboard')}</span>{/if}
        {#snippet trailing()}
          <HotkeyRecorder value={s[h.key]} disabled={off} status={clash ? false : undefined} label={t(`line.settings.${h.label}`)} onchange={(v) => setExtra(h.key, v)} />
        {/snippet}
      </ListRow>
    {/each}
    <ListRow title={t('line.settings.showNow')} subtitle={t('line.settings.showNowHint')}>
      {#snippet trailing()}<Button size="sm" disabled={off} onclick={showNow}>{t('line.settings.showNow')}</Button>{/snippet}
    </ListRow>
  </Card>
</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .intro { margin: 0; font: var(--text-body); color: var(--text-secondary); }
  section > :global(.sh) { margin-top: var(--space-2); }
  .hint {
    display: flex; align-items: center; gap: var(--space-2); padding: var(--space-2) var(--space-3); border: 0;
    border-radius: var(--radius-md); background: var(--accent-soft); color: var(--text); font: var(--text-callout);
    text-align: left; cursor: default;
  }
  .swatches { display: inline-flex; align-items: center; gap: 6px; }
  .swatch {
    display: grid; place-items: center; width: 20px; height: 20px; padding: 0; border: 0; border-radius: 50%; background: var(--c);
    color: #fff; box-shadow: inset 0 0 0 1px var(--hairline); cursor: default;
  }
  .swatch.on { box-shadow: 0 0 0 2px var(--window-bg), 0 0 0 4px var(--accent); }
  .swatch:disabled { opacity: 0.45; }
  .custom { width: 24px; height: 24px; padding: 0; border: 0; border-radius: 50%; background: transparent; cursor: default; }
  .custom.on { box-shadow: 0 0 0 2px var(--window-bg), 0 0 0 4px var(--accent); }
  .slider { display: inline-flex; align-items: center; gap: var(--space-2); width: 200px; }
  .slider output { min-width: 36px; text-align: right; font: var(--text-callout); color: var(--text-secondary); }
  .pv {
    position: relative; transform-origin: 0 0;
    border-radius: var(--radius-lg); overflow: hidden; background: color-mix(in srgb, var(--text) calc(var(--op) * 6%), transparent);
  }
  .pvwrap { margin: var(--space-2) var(--space-3) var(--space-3); }
  .pv svg { position: absolute; left: 0; top: 0; overflow: visible; }
  .pv .strand { fill: none; stroke: var(--rope); stroke-width: var(--w); stroke-linecap: round; opacity: calc(0.35 + 0.65 * var(--op)); }
  .pv .knot { fill: var(--rope); opacity: calc(0.35 + 0.65 * var(--op)); }
  .pcard {
    position: absolute; display: grid; place-items: center; height: calc(var(--s) * 80px); border-radius: var(--radius-xs);
    background: var(--elevated-bg); box-shadow: var(--shadow-md); font: var(--text-callout); font-size: calc(var(--s) * 12px); color: var(--text-secondary);
  }
  .needs { display: inline-flex; align-items: center; gap: 5px; font: var(--text-callout); color: var(--text-2, inherit); white-space: normal; }
  .conflict { display: inline-flex; align-items: center; gap: 5px; font: var(--text-callout); color: var(--red); white-space: normal; }
</style>
