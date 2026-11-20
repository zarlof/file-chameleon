<script lang="ts">
  import { untrack } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { Button, Card, EmptyState, Icon, ListRow, ProgressBar, SectionHeader, SegmentedControl, Select, Spinner, TextField, toast } from '../../../lib/ui';
  import { t } from '../../../lib/i18n';
  import { api, errorOf, on } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import { settings } from '../../../lib/stores/settings.svelte';
  import { tools } from '../../../lib/stores/tools.svelte';
  import type { DlMode, DlQuality, DownloadRequest, DownloadSnapshot, MediaInfo } from '../../../lib/types';
  import DownloadRow from './DownloadRow.svelte';
  import PreviewCard from './PreviewCard.svelte';
  import { missingToolOf, sortedDownloads, upsert } from './helpers';

  type Failure = { code: string; message: string };
  /** What to run again once a missing tool has been installed. */
  type Retry = { kind: 'probe' } | { kind: 'download'; req: DownloadRequest } | { kind: 'none' };

  let url = $state('');
  let checking = $state(false);
  let info = $state<MediaInfo | null>(null);
  let failure = $state<Failure | null>(null);
  let downloads = $state<DownloadSnapshot[]>([]);

  // The user's picks; until they pick, the saved defaults apply (and follow settings once loaded).
  let pickedMode = $state<DlMode | null>(null);
  let pickedQuality = $state<DlQuality | null>(null);
  let pickedDir = $state<string | null>(null);
  const mode = $derived(pickedMode ?? settings.value.downloadMode);
  const quality = $derived(pickedQuality ?? settings.value.downloadQuality);
  const outDir = $derived(pickedDir ?? settings.value.downloadDir ?? '');

  /** A tool the last action needs, with the action to repeat afterwards. */
  let needTool = $state<{ id: string; retry: Retry } | null>(null);
  /** An install this view started and is waiting for. */
  let waiting = $state<{ id: string; retry: Retry } | null>(null);
  let installError = $state<string | null>(null);

  let probeSeq = 0;
  const requests = new Map<number, DownloadRequest>();

  // ---- what the checked link allows
  const plain = $derived(!!info && (info.direct || info.kind !== 'video'));
  const effectiveMode = $derived<DlMode>(plain ? 'original' : mode);
  const effectiveQuality = $derived<DlQuality>(plain || mode === 'audio' || mode === 'gif' ? 'best' : quality);
  const showQuality = $derived(!plain && (mode === 'video' || mode === 'original'));
  const modeOptions = $derived([
    { value: 'video' as DlMode, label: t('download.mode.video') },
    { value: 'audio' as DlMode, label: t('download.mode.audio') },
    { value: 'gif' as DlMode, label: t('download.mode.gif') },
    { value: 'original' as DlMode, label: t('download.mode.original') },
  ]);
  const qualityOptions = $derived([
    { value: 'best', label: t('download.quality.best') },
    { value: 'p1080', label: t('download.quality.p1080') },
    { value: 'p720', label: t('download.quality.p720') },
  ]);
  const sorted = $derived(sortedDownloads(downloads));

  function toolName(id: string): string {
    return tools.get(id)?.name ?? id;
  }

  function fail(err: Failure, retry: Retry) {
    const id = missingToolOf(err.code, err.message);
    if (id) {
      needTool = { id, retry };
      failure = null;
    } else {
      needTool = null;
      failure = err;
    }
  }

  function failureText(f: Failure): string {
    const key = `download.error.${f.code}`;
    const text = t(key);
    return text === key ? t('download.error.failed') : text;
  }

  async function check() {
    const link = url.trim();
    if (!link) return;
    const seq = ++probeSeq;
    checking = true;
    failure = null;
    needTool = null;
    installError = null;
    info = null;
    try {
      const result = await api.probeUrl(link);
      if (seq === probeSeq) info = result;
    } catch (e) {
      const err = errorOf(e);
      if (seq === probeSeq && err.code !== 'cancelled') fail(err, { kind: 'probe' });
    } finally {
      if (seq === probeSeq) checking = false;
    }
  }

  async function paste() {
    let text = '';
    try {
      text = (await navigator.clipboard.readText()).trim();
    } catch {
      /* clipboard blocked: treated like an empty one */
    }
    if (!text) {
      toast.show(t('download.url.clipboardEmpty'), 'info');
      return;
    }
    url = text;
    await check();
  }

  async function chooseFolder() {
    try {
      const picked = await open({ directory: true, multiple: false, title: t('download.folder.choose'), defaultPath: outDir || undefined });
      if (typeof picked === 'string') pickedDir = picked;
    } catch {
      /* dialog unavailable */
    }
  }

  async function begin(req: DownloadRequest) {
    try {
      const id = await api.startDownload(req);
      requests.set(id, req);
      return true;
    } catch (e) {
      fail(errorOf(e), { kind: 'download', req });
      return false;
    }
  }

  async function start() {
    if (!info) return;
    failure = null;
    needTool = null;
    installError = null;
    const req: DownloadRequest = { url: info.url, mode: effectiveMode, quality: effectiveQuality, outDir, title: info.title };
    if (await begin(req)) {
      // Ready for the next link; the list below shows this one.
      info = null;
      url = '';
    }
  }

  // ---- installing a missing tool, then trying again
  function install(id: string, retry: Retry) {
    if (waiting || tools.isInstalling(id)) return; // one install at a time
    installError = null;
    void tools.install(id); // fire and forget: completion arrives through `tools.progress`
    waiting = { id, retry };
  }

  $effect(() => {
    const w = waiting;
    if (!w || w.id in tools.progress) return;
    const err = tools.errors[w.id];
    untrack(() => void finishInstall(w, err));
  });

  async function finishInstall(w: { id: string; retry: Retry }, err: string | undefined) {
    waiting = null;
    await tools.refresh();
    if (err || !tools.isInstalled(w.id)) {
      installError = err ?? 'failed';
      toast.show(t('download.tool.failed', { name: toolName(w.id) }), 'error');
      return;
    }
    needTool = null;
    if (w.retry.kind === 'probe') await check();
    else if (w.retry.kind === 'download') await begin(w.retry.req);
  }

  const installing = $derived(!!needTool && (waiting?.id === needTool.id || tools.isInstalling(needTool.id)));
  const installPercent = $derived(needTool ? Math.round((tools.progress[needTool.id] ?? 0) * 100) : 0);

  function installFromRow(d: DownloadSnapshot, id: string) {
    const req = requests.get(d.id);
    install(id, req ? { kind: 'download', req } : { kind: 'none' });
  }

  // ---- list
  function cancel(id: number) {
    void api.cancelDownload(id);
  }
  function reveal(path: string) {
    void api.revealPath(path).catch(() => {});
  }
  function convert(path: string) {
    nav.goto({ view: 'convert', paths: [path], url: null });
  }

  $effect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    untrack(() => {
      void on
        .downloadUpdate((s) => (downloads = upsert(downloads, s)))
        .then((un) => (gone ? un() : (off = un)))
        .catch(() => {});
      void api
        .listDownloads()
        .then((list) => {
          const known = new Set(downloads.map((d) => d.id));
          downloads = [...downloads, ...list.filter((d) => !known.has(d.id))];
        })
        .catch(() => {});
    });
    return () => {
      gone = true;
      off?.();
    };
  });

  // Links handed over by the Shell, the tray or a toast.
  $effect(() => {
    const p = nav.pending && nav.consume('download');
    if (p?.url) {
      const link = p.url;
      untrack(() => {
        url = link;
        void check();
      });
    }
  });
</script>

<section class="view">
  <h1>{t('download.title')}</h1>

  <div class="bar">
    <div class="field">
      <TextField
        size="lg"
        type="url"
        bind:value={url}
        placeholder={t('download.url.placeholder')}
        label={t('download.url.label')}
        onenter={check}
      >
        {#snippet leading()}<Icon name="link" size={14} />{/snippet}
      </TextField>
    </div>
    <Button size="lg" onclick={paste}>{t('download.url.paste')}</Button>
    <Button size="lg" variant="primary" loading={checking} disabled={!url.trim()} onclick={check}>{t('download.url.check')}</Button>
  </div>

  {#if checking}
    <div class="status"><Spinner size={14} /><span>{t('download.preview.checking')}</span></div>
  {/if}

  {#if failure}
    <div class="notice bad" role="alert"><Icon name="circle-alert" size={16} /><span>{failureText(failure)}</span></div>
  {/if}

  {#if needTool}
    <Card padded>
      <div class="tool">
        <div class="tool-text">
          <strong>{t(`download.tool.reason.${needTool.id}`, { name: toolName(needTool.id) })}</strong>
          {#if installing}
            <ProgressBar value={installPercent > 0 ? installPercent / 100 : null} />
            <span class="muted">{t('download.tool.installing', { name: toolName(needTool.id), percent: installPercent })}</span>
          {:else if installError}
            <span class="bad-text">{t('download.tool.failed', { name: toolName(needTool.id) })}</span>
          {/if}
        </div>
        <Button variant="primary" disabled={installing} onclick={() => needTool && install(needTool.id, needTool.retry)}>
          {t('download.tool.install', { name: toolName(needTool.id) })}
        </Button>
      </div>
    </Card>
  {/if}

  {#if info}
    <PreviewCard {info} />

    <Card>
      {#if !plain}
        <ListRow title={t('download.mode.label')} subtitle={t(`download.mode.hint.${mode}`)}>
          {#snippet trailing()}
            <SegmentedControl options={modeOptions} value={mode} onchange={(v) => (pickedMode = v)} />
          {/snippet}
        </ListRow>
        {#if showQuality}
          <ListRow title={t('download.quality.label')}>
            {#snippet trailing()}
              <Select
                label={t('download.quality.label')}
                value={quality}
                options={qualityOptions}
                onchange={(v) => (pickedQuality = v as DlQuality)}
              />
            {/snippet}
          </ListRow>
        {/if}
      {/if}
      <ListRow title={t('download.folder.title')} subtitle={outDir || t('download.folder.default')}>
        {#snippet leading()}<Icon name="folder-open" size={18} />{/snippet}
        {#snippet trailing()}
          <Button size="sm" onclick={chooseFolder}>{t('download.folder.change')}</Button>
        {/snippet}
      </ListRow>
    </Card>

    <div class="go">
      <Button variant="primary" size="lg" onclick={start}>
        {#snippet icon()}<Icon name="download" size={15} />{/snippet}
        {t('download.action.download')}
      </Button>
    </div>
  {/if}

  {#if sorted.length > 0}
    <div class="list">
      <SectionHeader title={t('download.list.title')} />
      <Card>
        {#each sorted as d (d.id)}
          {@const needed = d.state === 'failed' ? missingToolOf(d.errorCode, d.error) : null}
          <DownloadRow
            {d}
            tool={needed}
            toolName={needed ? toolName(needed) : ''}
            toolBusy={!!needed && (waiting?.id === needed || tools.isInstalling(needed))}
            oncancel={cancel}
            onreveal={reveal}
            onconvert={convert}
            oninstall={(id) => installFromRow(d, id)}
          />
        {/each}
      </Card>
    </div>
  {:else if !info && !checking && !failure && !needTool}
    <div class="empty">
      <EmptyState title={t('download.empty.title')} message={t('download.empty.message')}>
        {#snippet icon()}<Icon name="download" size={28} />{/snippet}
        {#snippet action()}
          <p class="sites">{t('download.empty.sites')}</p>
          <p class="note">{t('download.empty.note')}</p>
        {/snippet}
      </EmptyState>
    </div>
  {/if}
</section>

<style>
  .view { display: flex; flex-direction: column; gap: var(--space-4); padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0; font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .bar { display: flex; gap: var(--space-2); align-items: center; }
  .field { flex: 1; min-width: 0; }
  .status { display: flex; align-items: center; gap: var(--space-2); color: var(--text-secondary); font: var(--text-callout); }
  .notice { display: flex; align-items: flex-start; gap: var(--space-2); padding: var(--space-3) var(--space-4); border-radius: var(--radius-md); font: var(--text-body); }
  .notice.bad { color: var(--red); background: color-mix(in srgb, var(--red) 10%, transparent); }
  .tool { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
  .tool-text { display: flex; flex-direction: column; gap: 6px; min-width: 0; flex: 1; font: var(--text-body); }
  .muted { color: var(--text-secondary); font: var(--text-callout); }
  .bad-text { color: var(--red); font: var(--text-callout); }
  .go { display: flex; justify-content: flex-end; }
  .list { display: flex; flex-direction: column; }
  .empty { min-height: 280px; }
  .sites { margin: 0; max-width: 360px; font: var(--text-callout); color: var(--text-secondary); }
  .note { margin: 6px 0 0; max-width: 360px; font: var(--text-caption); color: var(--text-tertiary); }
</style>
