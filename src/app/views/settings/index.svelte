<!--
  Settings — System-Settings-style: a section list on the left, grouped cards on the right.
  Every control writes through the settings store immediately (no Save button).
-->
<script lang="ts">
  import { Check, CircleDot, Clipboard, FolderOutput as OutputIcon, Info, Settings as Gear, Wrench } from '@lucide/svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onDestroy } from 'svelte';
  import { t } from '../../../lib/i18n';
  import { errorOf } from '../../../lib/ipc';
  import { settings } from '../../../lib/stores/settings.svelte';
  import { tools } from '../../../lib/stores/tools.svelte';
  import type { Accent, Settings, ToolStatus } from '../../../lib/types';
  import { Badge, Button, Card, ListRow, ProgressBar, SectionHeader, SegmentedControl, Select, Slider, Toggle, formatBytes, toast } from '../../../lib/ui';

  type Section = 'general' | 'wheel' | 'clipboard' | 'output' | 'engines' | 'about';
  const sections: { id: Section; icon: typeof Gear }[] = [
    { id: 'general', icon: Gear },
    { id: 'wheel', icon: CircleDot },
    { id: 'clipboard', icon: Clipboard },
    { id: 'output', icon: OutputIcon },
    { id: 'engines', icon: Wrench },
    { id: 'about', icon: Info },
  ];
  const ACCENTS: Accent[] = ['blue', 'purple', 'pink', 'red', 'orange', 'yellow', 'green', 'graphite'];

  let section = $state<Section>('general');
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

  /** Select options for a numeric setting; keeps an unusual stored value selectable. */
  function numOptions(values: number[], current: number, label: (n: number) => string) {
    const all = values.includes(current) ? values : [...values, current].sort((a, b) => a - b);
    return all.map((n) => ({ value: String(n), label: label(n) }));
  }

  const languages = $derived([
    { value: 'system', label: t('settings.general.languageSystem') },
    { value: 'tr', label: 'Türkçe' },
    { value: 'en', label: 'English' },
  ]);
  const themes = $derived([
    { value: 'system' as const, label: t('settings.general.themeSystem') },
    { value: 'light' as const, label: t('settings.general.themeLight') },
    { value: 'dark' as const, label: t('settings.general.themeDark') },
  ]);
  const browsers = $derived([
    { value: '', label: t('settings.clipboard.cookiesNone') },
    { value: 'edge', label: 'Microsoft Edge' },
    { value: 'chrome', label: 'Google Chrome' },
    { value: 'firefox', label: 'Firefox' },
    { value: 'brave', label: 'Brave' },
  ]);
  const itemOptions = $derived(numOptions([50, 100, 200, 500, 1000], s.clipboardMaxItems, (n) => t('settings.clipboard.items', { count: n })));
  const dayOptions = $derived(numOptions([1, 7, 14, 30, 90, 365], s.clipboardRetentionDays, (n) => t('settings.clipboard.days', { count: n })));

  function purpose(tool: ToolStatus): string {
    const key = `settings.engines.purpose.${tool.id}`;
    const text = t(key);
    return text === key ? tool.purpose : text;
  }
  const badgeTone = { managed: 'accent', system: 'green', missing: 'orange' } as const;

  // yt-dlp breaks when sites change: let the user fetch the newest build right away.
  let updating = $state(false);
  async function updateYtdlp() {
    updating = true;
    try {
      const updated = await tools.updateYtdlp();
      toast.show(t(updated ? 'settings.engines.updated' : 'settings.engines.upToDate', { name: 'yt-dlp' }), 'success');
    } catch (e) {
      toast.show(t('settings.engines.updateFailed', { message: errorOf(e).message }), 'error', 6000);
    } finally {
      updating = false;
    }
  }
</script>

<section>
  <h1>{t('settings.title')}</h1>
  <div class="layout">
    <nav aria-label={t('settings.title')}>
      {#each sections as { id, icon: Ico } (id)}
        <button type="button" class="item" class:selected={section === id} aria-current={section === id ? 'page' : undefined} onclick={() => (section = id)}>
          <Ico size={16} strokeWidth={1.8} />
          <span>{t(`settings.sections.${id}`)}</span>
        </button>
      {/each}
    </nav>

    <div class="pane">
      {#key section}
        <div class="page">
          {#if section === 'general'}
            <Card>
              <ListRow title={t('settings.general.language')}>
                {#snippet trailing()}<Select value={s.language} options={languages} label={t('settings.general.language')} onchange={(v) => set({ language: v as Settings['language'] })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.general.theme')}>
                {#snippet trailing()}<SegmentedControl options={themes} value={s.theme} size="sm" onchange={(v) => set({ theme: v })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.general.accent')}>
                {#snippet trailing()}
                  <span class="swatches" role="radiogroup" aria-label={t('settings.general.accent')}>
                    {#each ACCENTS as a (a)}
                      <button type="button" role="radio" aria-checked={s.accent === a} class="swatch" class:on={s.accent === a}
                        style:--c={`var(--${a})`} title={t(`settings.accents.${a}`)} aria-label={t(`settings.accents.${a}`)} onclick={() => set({ accent: a })}>
                        {#if s.accent === a}<Check size={12} strokeWidth={3} />{/if}
                      </button>
                    {/each}
                  </span>
                {/snippet}
              </ListRow>
              <ListRow title={t('settings.general.launchAtLogin')} subtitle={t('settings.general.launchAtLoginHint')}>
                {#snippet trailing()}<Toggle checked={s.launchAtLogin} label={t('settings.general.launchAtLogin')} onchange={(v) => set({ launchAtLogin: v })} />{/snippet}
              </ListRow>
            </Card>
          {:else if section === 'wheel'}
            <Card>
              <ListRow title={t('settings.wheel.enable')} subtitle={t('settings.wheel.enableHint')}>
                {#snippet trailing()}<Toggle checked={s.wheelEnabled} label={t('settings.wheel.enable')} onchange={(v) => set({ wheelEnabled: v })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.wheel.anyApp')} subtitle={t('settings.wheel.anyAppHint')}>
                {#snippet trailing()}<Toggle checked={s.wheelAnyApp} disabled={!s.wheelEnabled} label={t('settings.wheel.anyApp')} onchange={(v) => set({ wheelAnyApp: v })} />{/snippet}
              </ListRow>
            </Card>
            <SectionHeader title={t('settings.wheel.howTitle')} />
            <Card padded>
              <div class="how">
                <div class="illus" aria-hidden="true">
                  <span class="file">PNG</span>
                  <span class="arrow">→</span>
                  <span class="keys"><kbd>Ctrl</kbd></span>
                  <span class="wheel">
                    {#each [0, 1, 2, 3, 4, 5] as i (i)}<i style:--a={`${i * 60}deg`}></i>{/each}
                  </span>
                </div>
                <p>{t('settings.wheel.howBody')}</p>
              </div>
            </Card>
          {:else if section === 'clipboard'}
            <Card>
              <ListRow title={t('settings.clipboard.enable')} subtitle={t('settings.clipboard.enableHint')}>
                {#snippet trailing()}<Toggle checked={s.clipboardEnabled} label={t('settings.clipboard.enable')} onchange={(v) => set({ clipboardEnabled: v })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.clipboard.images')}>
                {#snippet trailing()}<Toggle checked={s.clipboardImages} disabled={!s.clipboardEnabled} label={t('settings.clipboard.images')} onchange={(v) => set({ clipboardImages: v })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.clipboard.suggestLinks')}>
                {#snippet trailing()}<Toggle checked={s.clipboardSuggestLinks} disabled={!s.clipboardEnabled} label={t('settings.clipboard.suggestLinks')} onchange={(v) => set({ clipboardSuggestLinks: v })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.clipboard.suggestImages')}>
                {#snippet trailing()}<Toggle checked={s.clipboardSuggestImages} disabled={!s.clipboardEnabled} label={t('settings.clipboard.suggestImages')} onchange={(v) => set({ clipboardSuggestImages: v })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.clipboard.maxItems')}>
                {#snippet trailing()}<Select value={String(s.clipboardMaxItems)} options={itemOptions} disabled={!s.clipboardEnabled} label={t('settings.clipboard.maxItems')} onchange={(v) => set({ clipboardMaxItems: Number(v) })} />{/snippet}
              </ListRow>
              <ListRow title={t('settings.clipboard.retention')}>
                {#snippet trailing()}<Select value={String(s.clipboardRetentionDays)} options={dayOptions} disabled={!s.clipboardEnabled} label={t('settings.clipboard.retention')} onchange={(v) => set({ clipboardRetentionDays: Number(v) })} />{/snippet}
              </ListRow>
            </Card>
            <SectionHeader title={t('settings.clipboard.cookies')} />
            <Card>
              <ListRow title={t('settings.clipboard.cookies')} subtitle={t('settings.clipboard.cookiesHint')}>
                {#snippet trailing()}<Select value={s.cookiesBrowser ?? ''} options={browsers} label={t('settings.clipboard.cookies')} onchange={(v) => set({ cookiesBrowser: v || null })} />{/snippet}
              </ListRow>
            </Card>
          {:else if section === 'output'}
            <Card>
              <ListRow title={t('settings.output.convertDir')} subtitle={s.outputDir ?? t('settings.output.sameAsSource')}>
                {#snippet trailing()}
                  {#if s.outputDir}<Button size="sm" variant="plain" onclick={() => set({ outputDir: null })}>{t('settings.output.reset')}</Button>{/if}
                  <Button size="sm" onclick={() => pickDir('outputDir')}>{t('settings.output.choose')}</Button>
                {/snippet}
              </ListRow>
              <ListRow title={t('settings.output.downloadDir')} subtitle={s.downloadDir ?? t('settings.output.defaultDownloads')}>
                {#snippet trailing()}
                  {#if s.downloadDir}<Button size="sm" variant="plain" onclick={() => set({ downloadDir: null })}>{t('settings.output.reset')}</Button>{/if}
                  <Button size="sm" onclick={() => pickDir('downloadDir')}>{t('settings.output.choose')}</Button>
                {/snippet}
              </ListRow>
              <ListRow title={t('settings.output.quality')}>
                {#snippet trailing()}
                  <span class="quality"><Slider value={quality} min={1} max={100} label={t('settings.output.quality')} oninput={onQuality} /><output>{quality}</output></span>
                {/snippet}
              </ListRow>
            </Card>
          {:else if section === 'engines'}
            <p class="note">{t('settings.engines.note')}</p>
            <Card>
              {#each tools.list as tool (tool.id)}
                {@const installing = tools.isInstalling(tool.id)}
                <ListRow title={tool.name} subtitle={purpose(tool)}>
                  {#snippet trailing()}
                    <Badge tone={badgeTone[tool.source]}>{t(`settings.engines.source.${tool.source}`)}</Badge>
                    {#if tool.source === 'missing'}
                      <Button size="sm" variant="primary" loading={installing} disabled={installing} onclick={() => tools.install(tool.id)}>
                        {installing ? t('settings.engines.installing') : `${t('settings.engines.install')} (${formatBytes(tool.size)})`}
                      </Button>
                    {:else if tool.source === 'system'}
                      <!-- A managed copy replaces a broken or outdated one on PATH (managed is used first). -->
                      <Button size="sm" loading={installing} disabled={installing} onclick={() => tools.install(tool.id)}>
                        {installing ? t('settings.engines.installing') : `${t('settings.engines.installManaged')} (${formatBytes(tool.size)})`}
                      </Button>
                    {:else if tool.source === 'managed'}
                      {#if tool.id === 'yt-dlp'}
                        <Button size="sm" loading={updating} disabled={updating || installing} onclick={updateYtdlp}>
                          {updating ? t('settings.engines.updating') : t('settings.engines.update')}
                        </Button>
                      {/if}
                      <Button size="sm" disabled={installing || (tool.id === 'yt-dlp' && updating)} onclick={() => tools.uninstall(tool.id)}>{t('settings.engines.remove')}</Button>
                    {/if}
                  {/snippet}
                  <span class="meta">
                    {#if tool.version}{t('settings.engines.version', { version: tool.version })} · {/if}{formatBytes(tool.size)} · {t('settings.engines.license', { license: tool.license })}
                  </span>
                  {#if installing}<div class="bar"><ProgressBar value={tools.progress[tool.id] ?? null} /></div>{/if}
                  {#if tools.errors[tool.id]}<span class="err">{t('settings.engines.error', { message: tools.errors[tool.id] })}</span>{/if}
                </ListRow>
              {/each}
            </Card>
          {:else}
            <Card padded>
              <div class="about">
                <div class="about-name">{t('common.appName')}</div>
                {#if settings.appInfo}<div class="about-ver">{t('settings.about.version')} {settings.appInfo.version}</div>{/if}
              </div>
            </Card>
            <Card>
              <ListRow title={t('settings.about.repo')} subtitle={t('settings.about.repoValue')} />
            </Card>
            <p class="note">{t('settings.about.licensesValue')}</p>
            <p class="note">{t('settings.about.credits')}</p>
          {/if}
        </div>
      {/key}
    </div>
  </div>
</section>

<style>
  section { padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-4); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .layout { display: flex; gap: var(--space-5); align-items: flex-start; }
  nav { position: sticky; top: 0; display: flex; flex-direction: column; gap: 2px; flex: none; width: 168px; }
  .item {
    display: flex; align-items: center; gap: 9px; height: 30px; padding: 0 10px; border: 0; border-radius: var(--radius-sm);
    background: transparent; color: var(--text); font: var(--text-body); text-align: left; cursor: default;
    transition: background var(--dur-fast);
  }
  .item:hover { background: var(--control-bg); }
  .item.selected { background: var(--accent); color: var(--on-accent); }
  .item:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .pane { flex: 1; min-width: 0; }
  .page { display: flex; flex-direction: column; gap: var(--space-3); animation: fade var(--dur) var(--ease-out); }
  .page > :global(.sh) { margin-top: var(--space-2); }
  @keyframes fade { from { opacity: 0; transform: translateY(4px); } }

  .swatches { display: inline-flex; gap: 8px; }
  .swatch {
    display: grid; place-items: center; width: 20px; height: 20px; padding: 0; border: 0; border-radius: 50%;
    background: var(--c); color: #fff; cursor: default; transition: transform var(--dur-fast) var(--ease-spring), box-shadow var(--dur-fast);
  }
  .swatch:hover { transform: scale(1.12); }
  .swatch.on { box-shadow: 0 0 0 2px var(--elevated-bg), 0 0 0 4px var(--c); }
  .swatch:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }

  .how { display: flex; align-items: center; gap: var(--space-5); }
  .how p { margin: 0; font: var(--text-body); color: var(--text-secondary); }
  .illus { display: flex; align-items: center; flex: none; gap: 10px; padding: 14px 18px; border-radius: var(--radius-md); background: var(--control-bg); }
  .file { padding: 6px 9px; border-radius: var(--radius-sm); background: var(--card-bg-hover); box-shadow: var(--shadow-sm); font: var(--text-caption); }
  .arrow { color: var(--text-tertiary); }
  kbd {
    padding: 3px 8px; border-radius: var(--radius-xs); background: var(--card-bg-hover); box-shadow: var(--shadow-sm);
    font: var(--text-caption); font-family: var(--font);
  }
  .wheel { position: relative; width: 52px; height: 52px; border-radius: 50%; background: var(--accent-soft); box-shadow: inset 0 0 0 1px var(--accent); }
  .wheel i {
    position: absolute; top: 50%; left: 50%; width: 8px; height: 8px; margin: -4px; border-radius: 50%; background: var(--accent);
    transform: rotate(var(--a)) translateY(-18px);
  }

  .quality { display: inline-flex; align-items: center; gap: var(--space-3); width: 220px; }
  .quality output { min-width: 28px; text-align: right; font: var(--text-body); color: var(--text-secondary); font-variant-numeric: tabular-nums; }

  .note { margin: 0 var(--space-1); font: var(--text-callout); color: var(--text-secondary); }
  .meta { display: block; font: var(--text-callout); color: var(--text-tertiary); }
  .bar { margin-top: 6px; }
  .err { display: block; margin-top: 4px; font: var(--text-callout); color: var(--red); }

  .about { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: var(--space-3) 0; }
  .about-name { font: var(--text-title); letter-spacing: var(--tracking-tight); }
  .about-ver { font: var(--text-callout); color: var(--text-secondary); }
</style>
