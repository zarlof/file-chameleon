<!--
  General settings — language, start-up and close behaviour, Explorer right-click entry,
  conversion output and download defaults. Every control writes through the settings store
  immediately (no Save button).
-->
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { onDestroy } from 'svelte';
  import { t } from '../../../lib/i18n';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { DlMode, DlQuality, Settings } from '../../../lib/types';
  import { Button, Card, ListRow, SectionHeader, Select, Slider, Toggle } from '../../../lib/ui';

  const s = $derived(settings.value);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  // Slider: keep the thumb responsive, persist after a short pause.
  let quality = $state(settings.value.imageQuality);
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    quality = settings.value.imageQuality;
  });
  function onQuality(v: number) {
    quality = v;
    clearTimeout(timer);
    timer = setTimeout(() => set({ imageQuality: v }), 250);
  }
  onDestroy(() => clearTimeout(timer));

  async function pickDir(key: 'outputDir' | 'downloadDir') {
    try {
      const r = await open({ directory: true });
      if (typeof r === 'string') set({ [key]: r } as Partial<Settings>);
    } catch {
      /* dialog unavailable */
    }
  }

  const languages = $derived([
    { value: 'system', label: t('settings.general.languageSystem') },
    { value: 'tr', label: 'Türkçe' },
    { value: 'en', label: 'English' },
  ]);
  const modes = $derived([
    { value: 'original', label: t('download.mode.original') },
    { value: 'video', label: t('download.mode.video') },
    { value: 'audio', label: t('download.mode.audio') },
    { value: 'gif', label: t('download.mode.gif') },
  ]);
  const qualities = $derived([
    { value: 'best', label: t('download.quality.best') },
    { value: 'p1080', label: t('download.quality.p1080') },
    { value: 'p720', label: t('download.quality.p720') },
  ]);
  const browsers = $derived([
    { value: '', label: t('settings.general.cookiesNone') },
    { value: 'edge', label: 'Microsoft Edge' },
    { value: 'chrome', label: 'Google Chrome' },
    { value: 'firefox', label: 'Firefox' },
    { value: 'brave', label: 'Brave' },
  ]);
  /** Audio and GIF downloads ignore the quality setting. */
  const qualityApplies = $derived(s.downloadMode === 'original' || s.downloadMode === 'video');
</script>

<section>
  <h1>{t('common.nav.general')}</h1>

  <SectionHeader title={t('settings.general.app')} />
  <Card>
    <ListRow title={t('settings.general.language')}>
      {#snippet trailing()}<Select value={s.language} options={languages} label={t('settings.general.language')} onchange={(v) => set({ language: v as Settings['language'] })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.general.launchAtLogin')} subtitle={t('settings.general.launchAtLoginHint')}>
      {#snippet trailing()}<Toggle checked={s.launchAtLogin} label={t('settings.general.launchAtLogin')} onchange={(v) => set({ launchAtLogin: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.general.closeToTray')} subtitle={t('settings.general.closeToTrayHint')}>
      {#snippet trailing()}<Toggle checked={s.closeToTray} label={t('settings.general.closeToTray')} onchange={(v) => set({ closeToTray: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.general.contextMenu')} subtitle={t('settings.general.contextMenuHint')}>
      {#snippet trailing()}<Toggle checked={s.contextMenu} label={t('settings.general.contextMenu')} onchange={(v) => set({ contextMenu: v })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.general.convert')} />
  <Card>
    <ListRow title={t('settings.general.outputDir')} subtitle={s.outputDir ?? t('settings.general.sameAsSource')}>
      {#snippet trailing()}
        {#if s.outputDir}<Button size="sm" variant="plain" onclick={() => set({ outputDir: null })}>{t('settings.general.reset')}</Button>{/if}
        <Button size="sm" onclick={() => pickDir('outputDir')}>{t('settings.general.choose')}</Button>
      {/snippet}
    </ListRow>
    <ListRow title={t('settings.general.quality')}>
      {#snippet trailing()}
        <span class="slider"><Slider value={quality} min={1} max={100} label={t('settings.general.quality')} oninput={onQuality} /><output>{quality}</output></span>
      {/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.general.downloads')} />
  <Card>
    <ListRow title={t('settings.general.downloadDir')} subtitle={s.downloadDir ?? t('settings.general.defaultDownloads')}>
      {#snippet trailing()}
        {#if s.downloadDir}<Button size="sm" variant="plain" onclick={() => set({ downloadDir: null })}>{t('settings.general.reset')}</Button>{/if}
        <Button size="sm" onclick={() => pickDir('downloadDir')}>{t('settings.general.choose')}</Button>
      {/snippet}
    </ListRow>
    <ListRow title={t('settings.general.downloadMode')}>
      {#snippet trailing()}<Select value={s.downloadMode} options={modes} label={t('settings.general.downloadMode')} onchange={(v) => set({ downloadMode: v as DlMode })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.general.downloadQuality')} subtitle={qualityApplies ? undefined : t('settings.general.downloadQualityHint')}>
      {#snippet trailing()}<Select value={s.downloadQuality} options={qualities} disabled={!qualityApplies} label={t('settings.general.downloadQuality')} onchange={(v) => set({ downloadQuality: v as DlQuality })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.general.cookies')} subtitle={t('settings.general.cookiesHint')}>
      {#snippet trailing()}<Select value={s.cookiesBrowser ?? ''} options={browsers} label={t('settings.general.cookies')} onchange={(v) => set({ cookiesBrowser: v || null })} />{/snippet}
    </ListRow>
  </Card>
</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  section > :global(.sh) { margin-top: var(--space-2); }
  .slider { display: inline-flex; align-items: center; gap: var(--space-3); width: 220px; }
  .slider output { min-width: 28px; text-align: right; font: var(--text-body); color: var(--text-secondary); font-variant-numeric: tabular-nums; }
</style>
