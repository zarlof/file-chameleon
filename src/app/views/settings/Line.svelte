<!--
  Line settings ("İp") — the clothesline: on/off, look (rope style, colour, thickness, sag, clothespins, cards,
  labels, number badges, card colour) with a live preview drawn by the real Rope/Peg components, the four
  ropes (order, names, switches, files sources, cards per rope), screen edge, behaviour, motion and sound,
  the hotkeys (with conflict status), "show now" and a reset to defaults. Texts live in line.json.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { ArrowDown, ArrowUp, Check, TriangleAlert } from '@lucide/svelte';
  import { formatHotkey, sameHotkey } from '../../../lib/hotkeys';
  import { currentLang, t } from '../../../lib/i18n';
  import { api, errorOf } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import { DEFAULT_SETTINGS, settings } from '../../../lib/stores/settings.svelte';
  import type { ClipItem, HotkeyStatus, LineFile, Settings } from '../../../lib/types';
  import { Button, Card, HotkeyRecorder, IconButton, ListRow, SectionHeader, SegmentedControl, Select, Slider, TextField, Toggle, toast } from '../../../lib/ui';
  import { CARD_COLORS, cardOpacity, cardStyleAttr, ropeColor, sizeInfo } from '../../../line/layout';
  import { setLang, t as lt } from '../../../line/i18n';
  import { lineDefaults, moveRope, ropeName, sanitizeOrder, type LineCard, type RopeKey } from '../../../line/logic';
  import Rope from '../../../line/Rope.svelte';

  const s = $derived(settings.value);
  const off = $derived(!s.lineEnabled);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  // The preview components read their texts through the line's own tiny i18n.
  $effect(() => {
    setLang(currentLang(), 'en');
  });

  // ---- sliders (debounced writes; the value shown follows the thumb at once)
  type SliderKey =
    | 'lineThickness' | 'lineOpacity' | 'lineSag' | 'lineCardRadius' | 'linePeekMs' | 'lineRevealDwellMs'
    | 'lineHideDelayMs' | 'lineSwing' | 'lineDamping' | 'lineBreezeStrength' | 'lineSoundVolume';
  const SLIDERS: SliderKey[] = [
    'lineThickness', 'lineOpacity', 'lineSag', 'lineCardRadius', 'linePeekMs', 'lineRevealDwellMs',
    'lineHideDelayMs', 'lineSwing', 'lineDamping', 'lineBreezeStrength', 'lineSoundVolume',
  ];
  const vals = $state<Record<SliderKey, number>>(Object.fromEntries(SLIDERS.map((k) => [k, settings.value[k]])) as Record<SliderKey, number>);
  const slideTimers: Partial<Record<SliderKey, ReturnType<typeof setTimeout>>> = {};
  function slide(key: SliderKey, v: number) {
    vals[key] = v;
    clearTimeout(slideTimers[key]);
    slideTimers[key] = setTimeout(() => {
      delete slideTimers[key];
      set({ [key]: v } as Partial<Settings>);
    }, 120);
  }
  // Follow changes that come from elsewhere (a reset, another window) unless the thumb is being dragged.
  $effect(() => {
    const cur = s;
    for (const k of SLIDERS) if (!slideTimers[k]) vals[k] = cur[k];
  });

  // ---- look
  const SWATCHES = [
    { id: 'natural', name: 'natural' },
    { id: '#3a3a3c', name: 'charcoal' },
    { id: '#ffffff', name: 'white' },
    { id: 'accent', name: 'accent' },
    { id: '#ff453a', name: 'red' },
    { id: '#30d158', name: 'green' },
    { id: '#0a84ff', name: 'blue' },
    { id: '#ff6bb0', name: 'pink' },
  ] as const;
  const choice = <T extends string>(keys: readonly T[], prefix: string) => keys.map((k) => ({ value: k, label: t(`${prefix}.${k}`) }));
  const cardStyleOptions = $derived(choice(['glassDark', 'glassLight', 'paper'] as const, 'line.settings.cardStyle'));
  const sizeOptions = $derived([
    { value: 'small' as const, label: t('line.settings.sizeSmall') },
    { value: 'medium' as const, label: t('line.settings.sizeMedium') },
    { value: 'large' as const, label: t('line.settings.sizeLarge') },
  ]);
  const edgeOptions = $derived([
    { value: 'top' as const, label: t('line.settings.edgeTop') },
    { value: 'bottom' as const, label: t('line.settings.edgeBottom') },
  ]);
  const clickOptions = $derived([
    { value: 'open', label: t('line.settings.clickOpen') },
    { value: 'copy', label: t('line.settings.clickCopy') },
    { value: 'copyClose', label: t('line.settings.clickCopyClose') },
    { value: 'paste', label: t('line.settings.clickPaste') },
  ]);
  const maxOptions = $derived([
    { value: '0', label: t('line.settings.maxAuto') },
    ...[3, 5, 8, 10, 15, 20, 30].map((n) => ({ value: String(n), label: String(n) })),
  ]);
  const isHex = (v: string) => /^#[0-9a-f]{6}$/i.test(v);
  const custom = $derived(isHex(s.lineColor) && !SWATCHES.some((w) => w.id === s.lineColor));
  const customCard = $derived(isHex(s.lineCardColor) && !(CARD_COLORS as readonly string[]).includes(s.lineCardColor.toLowerCase()));

  async function resetCardColors() {
    try {
      await api.lineClearCardColors();
      toast.show(t('line.settings.cardColorsResetDone'), 'info');
    } catch (e) {
      toast.show(errorOf(e).message, 'error');
    }
  }

  // ---- ropes
  const order = $derived(sanitizeOrder(s.lineRopeOrder));
  const ropeTitle = (k: RopeKey) => t(`line.settings.ropes${k === 'images' ? 'Images' : k === 'text' ? 'Text' : k === 'links' ? 'Links' : 'Files'}`);
  function setRope(key: RopeKey, v: boolean) {
    set({ lineRopes: { ...s.lineRopes, [key]: v } });
  }
  function move(key: RopeKey, delta: number) {
    set({ lineRopeOrder: moveRope(order, key, delta) });
  }
  let nameTimer: ReturnType<typeof setTimeout> | undefined;
  const pendingNames: Partial<Record<RopeKey, string>> = {};
  function setName(key: RopeKey, v: string) {
    pendingNames[key] = v.trim().slice(0, 24);
    clearTimeout(nameTimer);
    nameTimer = setTimeout(() => {
      const names = { ...s.lineRopeNames, ...pendingNames };
      for (const k of Object.keys(pendingNames) as RopeKey[]) delete pendingNames[k];
      set({ lineRopeNames: names });
    }, 300);
  }
  function setSource(key: keyof Settings['lineFilesSources'], v: boolean) {
    set({ lineFilesSources: { ...s.lineFilesSources, [key]: v } });
  }

  async function resetLine() {
    const patch = lineDefaults(DEFAULT_SETTINGS as unknown as Record<string, unknown>) as Partial<Settings>;
    for (const k of SLIDERS) {
      clearTimeout(slideTimers[k]);
      delete slideTimers[k];
    }
    if (await settings.update(patch)) toast.show(t('line.settings.resetDone'), 'info');
  }

  // ---- live preview: the real Rope with three sample cards
  const size = $derived(sizeInfo(s.lineSize));
  const PW = $derived(48 + 3 * size.card + 2 * 16);
  let wrapW = $state(0);
  const fitScale = $derived(wrapW > 0 ? Math.min(1, wrapW / PW) : 1);
  const dark = $derived(s.theme === 'dark' || (s.theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches));
  const pcolor = $derived(ropeColor(s.lineColor, 'var(--accent)', dark ? 'dark' : 'light'));
  const pcardVars = $derived(cardStyleAttr(s.lineCardStyle, vals.lineCardRadius, s.lineCardShadow, s.lineCardColor));
  const base = { image: null, thumb: null, width: null, height: null, site: null, hash: '', createdAt: 0, pinned: false, note: null, tags: [], folder: null, ocr: null, trashedAt: null, sourceApp: null, edited: null, editedThumb: null, saved: false, size: 0, rank: null, color: null };
  const sampleCards: LineCard[] = $derived([
    { id: 1, item: { ...base, id: 1, kind: 'text', text: lt('settings.sampleNote'), title: null, linkTitle: null } as unknown as ClipItem, file: null },
    { id: 2, item: { ...base, id: 2, kind: 'link', text: 'https://example.com/page', title: null, linkTitle: lt('settings.sampleLinkTitle'), pinned: true } as unknown as ClipItem, file: null },
    { id: 3, item: null, file: { path: 'C:\\sample', name: lt('settings.sampleFile'), source: 'converted', at: 0, size: 1258291, kind: 'pdf', thumb: null } as LineFile },
  ]);
  const noop = () => {};
  const none: Record<number, number> = {};
  const noSel = new Set<number>();

  // ---- hotkeys
  const OTHERS = [
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
      for (const k of SLIDERS) clearTimeout(slideTimers[k]);
      clearTimeout(nameTimer);
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

{#snippet slideRow(key: SliderKey, title: string, min: number, max: number, step: number, unit = '', hint?: string, disabled = false)}
  <ListRow {title} subtitle={hint}>
    {#snippet trailing()}
      <span class="slider"><Slider value={vals[key]} {min} {max} {step} disabled={off || disabled} label={title} oninput={(v) => slide(key, v)} /><output>{vals[key]}{unit}</output></span>
    {/snippet}
  </ListRow>
{/snippet}

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
  </Card>

  <SectionHeader title={t('line.settings.section.look')} />
  <Card>
    <div class="pvwrap" bind:clientWidth={wrapW} style:height="{(size.band - 30) * fitScale}px" aria-hidden="true">
      <div class="pv" style:width="{PW}px" style:height="{size.band - 30}px" style:transform="scale({fitScale})" style={pcardVars}>
        <Rope
          label={ropeName('text', s.lineRopeNames, lt('rope.text'))}
          showLabel={s.lineShowLabels}
          cards={sampleCards}
          width={PW}
          more={0}
          angles={none}
          falls={none}
          selected={noSel}
          copiedId={null}
          numbers={s.lineShowNumbers}
          top={-6}
          band={size.band}
          card={size.card}
          scale={size.scale}
          color={pcolor}
          thickness={vals.lineThickness}
          sag={vals.lineSag}
          cardOp={cardOpacity(vals.lineOpacity)}
          ropeOp={Math.min(100, Math.max(30, vals.lineOpacity)) / 100}
          onclick={noop}
          ondblclick={noop}
          ondragstart={noop}
          onremove={noop}
          oncontext={noop}
          onhover={noop}
          onmore={noop}
        />
      </div>
    </div>
    <ListRow title={t('line.settings.color')}>
      {#snippet trailing()}
        <span class="swatches" role="radiogroup" aria-label={t('line.settings.color')}>
          {#each SWATCHES as w (w.id)}
            <button type="button" role="radio" aria-checked={s.lineColor === w.id} class="swatch" class:on={s.lineColor === w.id} disabled={off}
              style:--c={w.id === 'accent' ? 'var(--accent)' : w.id === 'natural' ? '#d9d4cc' : w.id} title={t(`line.settings.color.${w.name}`)} aria-label={t(`line.settings.color.${w.name}`)}
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
    {@render slideRow('lineThickness', t('line.settings.thickness'), 1, 6, 1)}
    {@render slideRow('lineSag', t('line.settings.sag'), 0, 40, 1, '', t('line.settings.sagHint'))}
    <ListRow title={t('line.settings.cardStyle')}>
      {#snippet trailing()}<SegmentedControl options={cardStyleOptions} value={s.lineCardStyle} size="sm" disabled={off} onchange={(v) => set({ lineCardStyle: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.cardColor')} subtitle={t('line.settings.cardColorHint')}>
      {#snippet trailing()}
        <span class="swatches cardsw" role="radiogroup" aria-label={t('line.settings.cardColor')}>
          <button type="button" class="chip" class:on={!s.lineCardColor} disabled={off} onclick={() => set({ lineCardColor: '' })}>{t('line.settings.cardColorStyle')}</button>
          {#each CARD_COLORS as c (c)}
            <button type="button" role="radio" aria-checked={s.lineCardColor.toLowerCase() === c} class="swatch" class:on={s.lineCardColor.toLowerCase() === c} disabled={off}
              style:--c={c} title={c} aria-label={c} onclick={() => set({ lineCardColor: c })}>
              {#if s.lineCardColor.toLowerCase() === c}<Check size={12} strokeWidth={3} color={['#2b2d31', '#1e3a5f', '#4b2a6e', '#7a1f2b'].includes(c) ? '#fff' : '#16161a'} />{/if}
            </button>
          {/each}
          <input type="color" class="custom" class:on={customCard} disabled={off} value={isHex(s.lineCardColor) ? s.lineCardColor : '#ffffff'}
            title={t('line.settings.cardColorCustom')} aria-label={t('line.settings.cardColorCustom')}
            onchange={(e) => set({ lineCardColor: e.currentTarget.value })} />
        </span>
      {/snippet}
    </ListRow>
    <ListRow title={t('line.settings.cardColorsReset')} subtitle={t('line.settings.cardColorsResetHint')}>
      {#snippet trailing()}<Button size="sm" disabled={off} onclick={resetCardColors}>{t('line.settings.cardColorsReset')}</Button>{/snippet}
    </ListRow>
    {@render slideRow('lineCardRadius', t('line.settings.cardRadius'), 0, 16, 1, ' px')}
    <ListRow title={t('line.settings.cardShadow')} subtitle={t('line.settings.cardShadowHint')}>
      {#snippet trailing()}<Toggle checked={s.lineCardShadow} disabled={off} label={t('line.settings.cardShadow')} onchange={(v) => set({ lineCardShadow: v })} />{/snippet}
    </ListRow>
    {@render slideRow('lineOpacity', t('line.settings.opacity'), 30, 100, 5, '%')}
    <ListRow title={t('line.settings.size')}>
      {#snippet trailing()}<SegmentedControl options={sizeOptions} value={s.lineSize} size="sm" disabled={off} onchange={(v) => set({ lineSize: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.labels')} subtitle={t('line.settings.labelsHint')}>
      {#snippet trailing()}<Toggle checked={s.lineShowLabels} disabled={off} label={t('line.settings.labels')} onchange={(v) => set({ lineShowLabels: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.numbers')} subtitle={t('line.settings.numbersHint')}>
      {#snippet trailing()}<Toggle checked={s.lineShowNumbers} disabled={off} label={t('line.settings.numbers')} onchange={(v) => set({ lineShowNumbers: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.zoom')} subtitle={t('line.settings.zoomHint')}>
      {#snippet trailing()}<Toggle checked={s.lineZoomOnHover} disabled={off} label={t('line.settings.zoom')} onchange={(v) => set({ lineZoomOnHover: v })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('line.settings.section.ropes')} subtitle={t('line.settings.dragHint')} />
  <Card>
    {#each order as key, i (key)}
      <ListRow title={ropeTitle(key)}>
        {#snippet trailing()}
          <span class="ropecfg">
            <span class="nameField">
              <TextField value={s.lineRopeNames[key] ?? ''} placeholder={t('line.settings.ropeNamePlaceholder')} disabled={off} clearable={false}
                label={t('line.settings.ropeName', { rope: ropeTitle(key) })} oninput={(v) => setName(key, v)} />
            </span>
            <IconButton label={t('line.settings.moveUp')} size={26} disabled={off || i === 0} onclick={() => move(key, -1)}><ArrowUp size={14} /></IconButton>
            <IconButton label={t('line.settings.moveDown')} size={26} disabled={off || i >= order.length - 1} onclick={() => move(key, 1)}><ArrowDown size={14} /></IconButton>
            <Toggle checked={s.lineRopes[key]} disabled={off} label={ropeTitle(key)} onchange={(v) => setRope(key, v)} />
          </span>
        {/snippet}
      </ListRow>
      {#if key === 'files'}
        <ListRow title={t('line.settings.filesSources')} />
      <ListRow title={t('line.settings.filesAdded')}>
        {#snippet trailing()}<Toggle checked={s.lineFilesSources.added} disabled={off || !s.lineRopes.files} label={t('line.settings.filesAdded')} onchange={(v) => setSource('added', v)} />{/snippet}
      </ListRow>
      <ListRow title={t('line.settings.filesConverted')}>
        {#snippet trailing()}<Toggle checked={s.lineFilesSources.converted} disabled={off || !s.lineRopes.files} label={t('line.settings.filesConverted')} onchange={(v) => setSource('converted', v)} />{/snippet}
      </ListRow>
      <ListRow title={t('line.settings.filesDownloaded')}>
        {#snippet trailing()}<Toggle checked={s.lineFilesSources.downloaded} disabled={off || !s.lineRopes.files} label={t('line.settings.filesDownloaded')} onchange={(v) => setSource('downloaded', v)} />{/snippet}
      </ListRow>
      {/if}
    {/each}
    <ListRow title={t('line.settings.maxPerRope')} subtitle={t('line.settings.maxPerRopeHint')}>
      {#snippet trailing()}<Select value={String(s.lineMaxPerRope)} options={maxOptions} disabled={off} label={t('line.settings.maxPerRope')} onchange={(v) => set({ lineMaxPerRope: Number(v) })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('line.settings.section.position')} />
  <Card>
    <ListRow title={t('line.settings.edge')} subtitle={t('line.settings.edgeHint')}>
      {#snippet trailing()}<SegmentedControl options={edgeOptions} value={s.lineEdge} size="sm" disabled={off} onchange={(v) => set({ lineEdge: v })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('line.settings.section.behaviour')} />
  <Card>
    <ListRow title={t('line.settings.peekOnCopy')} subtitle={t('line.settings.peekOnCopyHint')}>
      {#snippet trailing()}<Toggle checked={s.linePeekOnCopy} disabled={off} label={t('line.settings.peekOnCopy')} onchange={(v) => set({ linePeekOnCopy: v })} />{/snippet}
    </ListRow>
    {@render slideRow('linePeekMs', t('line.settings.peekMs'), 1000, 6000, 250, ' ms')}
    <ListRow title={t('line.settings.edgeReveal')} subtitle={t('line.settings.edgeRevealHint')}>
      {#snippet trailing()}<Toggle checked={s.lineEdgeReveal} disabled={off} label={t('line.settings.edgeReveal')} onchange={(v) => set({ lineEdgeReveal: v })} />{/snippet}
    </ListRow>
    {@render slideRow('lineRevealDwellMs', t('line.settings.revealDwell'), 150, 1000, 50, ' ms', t('line.settings.revealDwellHint'))}
    {@render slideRow('lineHideDelayMs', t('line.settings.hideDelay'), 200, 2000, 50, ' ms', t('line.settings.hideDelayHint'))}
    <ListRow title={t('line.settings.clickAction')}>
      {#snippet trailing()}<Select value={s.lineClickAction} options={clickOptions} disabled={off} label={t('line.settings.clickAction')} onchange={(v) => set({ lineClickAction: v as Settings['lineClickAction'] })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('line.settings.section.motion')} subtitle={s.reduceMotion ? t('line.settings.reduceNote') : undefined} />
  <Card>
    {@render slideRow('lineSwing', t('line.settings.swing'), 0, 200, 10, '%', undefined, s.reduceMotion)}
    {@render slideRow('lineDamping', t('line.settings.damping'), 50, 200, 10, '%', undefined, s.reduceMotion)}
    <ListRow title={t('line.settings.breeze')} subtitle={t('line.settings.breezeHint')}>
      {#snippet trailing()}<Toggle checked={s.lineBreeze} disabled={off || s.reduceMotion} label={t('line.settings.breeze')} onchange={(v) => set({ lineBreeze: v })} />{/snippet}
    </ListRow>
    {@render slideRow('lineBreezeStrength', t('line.settings.breezeStrength'), 0, 200, 10, '%', undefined, s.reduceMotion || !s.lineBreeze)}
    <ListRow title={t('line.settings.fall')} subtitle={t('line.settings.fallHint')}>
      {#snippet trailing()}<Toggle checked={s.lineFall} disabled={off || s.reduceMotion} label={t('line.settings.fall')} onchange={(v) => set({ lineFall: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('line.settings.sound')} subtitle={t('line.settings.soundHint')}>
      {#snippet trailing()}<Toggle checked={s.lineSound} disabled={off || s.reduceMotion} label={t('line.settings.sound')} onchange={(v) => set({ lineSound: v })} />{/snippet}
    </ListRow>
    {@render slideRow('lineSoundVolume', t('line.settings.soundVolume'), 0, 100, 5, '%', undefined, s.reduceMotion || !s.lineSound)}
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

  <SectionHeader title={t('line.settings.reset')} />
  <Card>
    <ListRow title={t('line.settings.resetButton')} subtitle={t('line.settings.resetHint')}>
      {#snippet trailing()}<Button size="sm" onclick={resetLine}>{t('line.settings.resetButton')}</Button>{/snippet}
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
  .cardsw { flex-wrap: wrap; justify-content: flex-end; max-width: 330px; row-gap: 6px; }
  .custom { width: 24px; height: 24px; padding: 0; border: 0; border-radius: 50%; background: transparent; cursor: default; }
  .custom.on { box-shadow: 0 0 0 2px var(--window-bg), 0 0 0 4px var(--accent); }
  .chip { padding: 2px 10px; border: 0; border-radius: 999px; background: var(--control-bg); color: var(--text); font: var(--text-callout); cursor: default; }
  .chip.on { background: var(--accent); color: var(--on-accent); }
  .chip:disabled { opacity: 0.45; }
  .slider { display: inline-flex; align-items: center; gap: var(--space-2); width: 240px; }
  .slider output { min-width: 56px; text-align: right; font: var(--text-callout); color: var(--text-secondary); }
  .ropecfg { display: inline-flex; align-items: center; gap: var(--space-1); }
  .nameField { width: 150px; margin-right: var(--space-2); }
  .pvwrap { margin: var(--space-3); overflow: hidden; border-radius: var(--radius-lg); background: linear-gradient(135deg, #2b3a55, #5b3f6e 55%, #8a5a4a); }
  .pv { position: relative; transform-origin: 0 0; }
  .needs { display: inline-flex; align-items: center; gap: 5px; font: var(--text-callout); color: var(--text-2, inherit); white-space: normal; }
  .conflict { display: inline-flex; align-items: center; gap: 5px; font: var(--text-callout); color: var(--red); white-space: normal; }
</style>
