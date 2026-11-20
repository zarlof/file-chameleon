<!--
  Convert — drop files, pick a format or tool, convert. Dropped paths arrive through `nav.pending`
  (the Shell owns the only drag-drop listener); the drop card itself opens the file dialog.
-->
<script lang="ts" module>
  /** Only jobs started since the app launched show in the queue below the form. */
  const sessionStart = Date.now();
</script>

<script lang="ts">
  import { ChevronRight, CircleCheck, FileUp, FolderPlus } from '@lucide/svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { api, errorOf } from '../../../lib/ipc';
  import { t } from '../../../lib/i18n';
  import { jobs } from '../../../lib/stores/jobs.svelte';
  import { nav } from '../../../lib/stores/nav.svelte';
  import { settings } from '../../../lib/stores/settings.svelte';
  import { tools } from '../../../lib/stores/tools.svelte';
  import type { CompressLevel, JobRequest, Operation, TargetsResponse, ToolOp } from '../../../lib/types';
  import {
    Badge, Button, Card, FileChip, Icon, ListRow, ProgressBar, SectionHeader, SegmentedControl, Select, Slider, TextField,
  } from '../../../lib/ui';
  import JobRow from '../recent/JobRow.svelte';
  import { jobTitle } from '../recent/logic';
  import { buildOptions, buildRequest, buildToolOp, groupTargets, missingTools, opNeeds, SPEEDS } from './logic';

  const TOOL_ORDER: ToolOp[] = ['compress', 'trim', 'speed', 'mute', 'mergePdf', 'splitPdf', 'crop'];

  let paths = $state<string[]>([]);
  let resp = $state<TargetsResponse | null>(null);
  let loading = $state(false);
  let loadError = $state(false);
  let over = $state(false);
  let choice = $state<string | null>(null); // "target:<id>" | "tool:<op>"
  let busy = $state(false);
  let submitError = $state<string | null>(null);

  let level = $state<CompressLevel>('balanced');
  let start = $state('0:00');
  let end = $state('');
  let speed = $state<number>(1.5);
  let cx = $state('0');
  let cy = $state('0');
  let cw = $state('');
  let ch = $state('');

  let optionsOpen = $state(false);
  let quality = $state(settings.value.imageQuality);
  let maxWidth = $state('');
  let maxHeight = $state('');
  let outDir = $state<string | null>(settings.value.outputDir);

  let token = 0;

  // Dropped / handed-over files (re-runs whenever nav.pending changes, even while mounted).
  $effect(() => {
    const p = nav.pending && nav.consume('convert');
    if (p) void addFiles(p.paths);
  });

  async function refresh(next: string[]) {
    const mine = ++token;
    if (next.length === 0) {
      paths = [];
      resp = null;
      choice = null;
      loading = false;
      return;
    }
    loading = true;
    loadError = false;
    try {
      const r = await api.listTargets(next);
      if (mine !== token) return;
      resp = r;
      paths = r.inputs.map((i) => i.path);
      const stillValid =
        (choice?.startsWith('target:') && r.targets.some((x) => `target:${x.id}` === choice)) ||
        (choice?.startsWith('tool:') && r.tools.some((x) => `tool:${x}` === choice));
      if (!stillValid) choice = null;
    } catch {
      if (mine !== token) return;
      loadError = true;
    } finally {
      if (mine === token) loading = false;
    }
  }

  async function addFiles(more: string[]) {
    submitError = null;
    await refresh([...new Set([...paths, ...more])]);
  }

  async function pickFiles() {
    try {
      const r = await open({ multiple: true });
      if (r) await addFiles(Array.isArray(r) ? r : [r]);
    } catch {
      /* dialog unavailable */
    }
  }

  async function pickFolder() {
    try {
      const r = await open({ directory: true });
      if (typeof r === 'string') await addFiles([r]);
    } catch {
      /* dialog unavailable */
    }
  }

  async function pickOutDir() {
    try {
      const r = await open({ directory: true });
      if (typeof r === 'string') outDir = r;
    } catch {
      /* dialog unavailable */
    }
  }

  function remove(path: string) {
    void refresh(paths.filter((p) => p !== path));
  }

  const supported = $derived((resp?.inputs ?? []).filter((i) => i.kind !== null));
  const target = $derived(choice?.startsWith('target:') ? resp?.targets.find((x) => `target:${x.id}` === choice) : undefined);
  const toolOp = $derived(choice?.startsWith('tool:') ? (choice.slice(5) as ToolOp) : undefined);
  const groups = $derived(groupTargets(resp?.targets ?? []));
  const toolList = $derived(TOOL_ORDER.filter((o) => resp?.tools.includes(o)));

  const operation = $derived<Operation | null>(
    target
      ? { type: 'convert', to: target.id }
      : toolOp
        ? buildToolOp(toolOp, { level, start, end, speed, x: cx, y: cy, width: cw, height: ch })
        : null,
  );
  const needs = $derived(target ? target.needsTools : toolOp ? opNeeds(toolOp, supported) : []);
  const missing = $derived(tools.list.length > 0 ? missingTools(needs, tools.list) : []);
  const useQuality = $derived(target?.kind === 'image' || target?.kind === 'video');
  const showSize = $derived(useQuality || toolOp === 'crop');
  const installing = $derived(needs.find((id) => tools.isInstalling(id)));
  const canGo = $derived(!busy && !installing && operation !== null && supported.length > 0);

  function mb(bytes: number): number {
    const m = bytes / 1048576;
    return m >= 50 ? Math.round(m / 10) * 10 : Math.max(1, Math.round(m));
  }
  const firstMissing = $derived(missing[0] ? tools.get(missing[0]) : undefined);
  const actionLabel = $derived(
    installing
      ? t('convert.action.downloading', { name: tools.get(installing)?.name ?? installing })
      : missing.length > 0
        ? t('convert.action.download', { name: firstMissing?.name ?? missing[0], size: firstMissing ? mb(firstMissing.size) : '?' })
        : target
          ? t('convert.action.convertCount', { count: supported.length })
          : t('convert.action.apply'),
  );

  /**
   * Submit, or (when tools are missing) let the backend download them and submit afterwards: that
   * runs in Rust, so the conversion still starts if this window is closed during the download.
   */
  async function submit(req: JobRequest, needed: string[]) {
    try {
      if (needed.length === 0) await jobs.submit(req);
      else jobs.remember(await tools.installAndSubmit(needed, req), req);
      await refresh([]);
    } catch (e) {
      const err = errorOf(e);
      submitError =
        err.code === 'installFailed'
          ? t('settings.engines.error', { message: err.message })
          : err.message || t('convert.action.submitFailed');
    } finally {
      busy = false;
    }
  }

  function run() {
    if (!operation || busy || installing) return;
    busy = true;
    submitError = null;
    const options = buildOptions({ useQuality, quality, maxWidth: showSize ? maxWidth : '', maxHeight: showSize ? maxHeight : '', outputDir: outDir });
    const req = buildRequest(supported.map((i) => i.path), operation, options);
    void submit(req, [...missing]);
  }

  const fileCount = $derived(resp?.inputs.length ?? 0);
  // Home (no files loaded): finished jobs are listed under "Recent conversions", so the queue skips them.
  const recentDone = $derived(fileCount === 0 ? jobs.list.filter((j) => j.state === 'done').slice(0, 6) : []);
  const queue = $derived(
    jobs.list.filter((j) => j.createdAt >= sessionStart && (fileCount > 0 || j.state !== 'done')).slice(0, 5),
  );
  const levels = $derived([
    { value: 'light' as const, label: t('convert.params.light') },
    { value: 'balanced' as const, label: t('convert.params.balanced') },
    { value: 'strong' as const, label: t('convert.params.strong') },
  ]);
  const speedOptions = SPEEDS.map((s) => ({ value: String(s), label: `${s}×` }));
</script>

<section>
  <h1>{t('convert.title')}</h1>

  {#if fileCount === 0}
    <button
      type="button"
      class="drop"
      class:over
      onclick={pickFiles}
      ondragover={(e) => { e.preventDefault(); over = true; }}
      ondragleave={() => (over = false)}
      ondrop={() => (over = false)}
    >
      <span class="drop-icon"><FileUp size={30} strokeWidth={1.5} /></span>
      <span class="drop-title">{over ? t('convert.drop.release') : t('convert.drop.title')}</span>
      <span class="drop-sub">{t('convert.drop.subtitle')}</span>
    </button>
    {#if settings.value.wheelEnabled}<p class="home-hint">{t('convert.home.hint')}</p>{/if}
    <div class="center">
      <Button variant="plain" size="sm" onclick={pickFolder}>
        {#snippet icon()}<FolderPlus size={14} />{/snippet}
        {t('convert.drop.addFolder')}
      </Button>
    </div>
    {#if loadError}<p class="error">{t('convert.loadError')}</p>{/if}
  {:else}
    <div class="files">
      <div class="chips">
        {#each resp?.inputs ?? [] as f (f.path)}
          <span class="chipwrap" class:unsupported={f.kind === null}>
            <FileChip name={f.name} size={f.size} kind={f.kind} onremove={() => remove(f.path)} />
            {#if f.kind === null}<Badge tone="orange">{t('convert.unsupported')}</Badge>{/if}
          </span>
        {/each}
      </div>
      <div class="file-actions">
        <Button size="sm" onclick={pickFiles}>{t('convert.drop.addFiles')}</Button>
        <Button size="sm" onclick={pickFolder}>{t('convert.drop.addFolder')}</Button>
        <Button size="sm" variant="plain" onclick={() => refresh([])}>{t('convert.drop.clear')}</Button>
      </div>
    </div>
    {#if loadError}<p class="error">{t('convert.loadError')}</p>{/if}

    {#if resp && (groups.length > 0 || toolList.length > 0)}
      {#if groups.length > 0}
        <SectionHeader title={t('convert.formats.title')} />
        <Card padded>
          <div class="groups">
            {#each groups as g (g.kind)}
              <div class="group">
                <span class="group-label"><Icon name={g.kind} size={13} />{t(`convert.kind.${g.kind}`)}</span>
                <div class="pills">
                  {#each g.targets as tg (tg.id)}
                    <button type="button" class="pill" class:selected={choice === `target:${tg.id}`} aria-pressed={choice === `target:${tg.id}`}
                      onclick={() => (choice = `target:${tg.id}`)}>{tg.label}</button>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        </Card>
      {:else}
        <p class="muted">{t('convert.formats.empty')}</p>
      {/if}

      {#if toolList.length > 0}
        <SectionHeader title={t('convert.tools.title')} />
        <Card padded>
          <div class="pills">
            {#each toolList as op (op)}
              <button type="button" class="pill" class:selected={choice === `tool:${op}`} aria-pressed={choice === `tool:${op}`}
                onclick={() => (choice = `tool:${op}`)}>{t(`convert.tools.${op}`)}</button>
            {/each}
          </div>

          {#if toolOp}
            <div class="params">
              {#if toolOp === 'compress'}
                <SegmentedControl options={levels} bind:value={level} />
              {:else if toolOp === 'trim'}
                <label class="field"><span>{t('convert.params.start')}</span>
                  <TextField bind:value={start} placeholder={t('convert.params.timeHint')} label={t('convert.params.start')} clearable={false} /></label>
                <label class="field"><span>{t('convert.params.end')}</span>
                  <TextField bind:value={end} placeholder={t('convert.params.timeHint')} label={t('convert.params.end')} clearable={false} /></label>
              {:else if toolOp === 'speed'}
                <label class="field"><span>{t('convert.params.speed')}</span>
                  <Select value={String(speed)} options={speedOptions} label={t('convert.params.speed')} onchange={(v) => (speed = Number(v))} /></label>
              {:else if toolOp === 'crop'}
                <label class="field num"><span>{t('convert.params.x')}</span><TextField bind:value={cx} label={t('convert.params.x')} clearable={false} /></label>
                <label class="field num"><span>{t('convert.params.y')}</span><TextField bind:value={cy} label={t('convert.params.y')} clearable={false} /></label>
                <label class="field num"><span>{t('convert.params.width')}</span><TextField bind:value={cw} label={t('convert.params.width')} clearable={false} /></label>
                <label class="field num"><span>{t('convert.params.height')}</span><TextField bind:value={ch} label={t('convert.params.height')} clearable={false} /></label>
              {:else}
                <p class="muted">{t(`convert.params.${toolOp === 'mute' ? 'muteInfo' : toolOp === 'mergePdf' ? 'mergeInfo' : 'splitInfo'}`)}</p>
              {/if}
            </div>
            {#if operation === null}<p class="hint">{t('convert.params.invalid')}</p>{/if}
          {/if}
        </Card>
      {/if}

      <button type="button" class="disclosure" class:open={optionsOpen} aria-expanded={optionsOpen} onclick={() => (optionsOpen = !optionsOpen)}>
        <ChevronRight size={14} /> {t('convert.options.title')}
      </button>
      {#if optionsOpen}
        <Card padded>
          <div class="opts">
            {#if useQuality}
              <div class="opt">
                <span class="opt-label">{t('convert.options.quality')}</span>
                <span class="opt-ctl"><Slider bind:value={quality} min={1} max={100} label={t('convert.options.quality')} /><output>{quality}</output></span>
              </div>
            {/if}
            {#if showSize}
              <div class="opt">
                <span class="opt-label">{t('convert.options.maxWidth')}</span>
                <span class="opt-ctl small"><TextField bind:value={maxWidth} placeholder={t('convert.options.pixels')} label={t('convert.options.maxWidth')} clearable={false} /></span>
              </div>
              <div class="opt">
                <span class="opt-label">{t('convert.options.maxHeight')}</span>
                <span class="opt-ctl small"><TextField bind:value={maxHeight} placeholder={t('convert.options.pixels')} label={t('convert.options.maxHeight')} clearable={false} /></span>
              </div>
            {/if}
            <div class="opt">
              <span class="opt-label">{t('convert.options.outputDir')}</span>
              <span class="opt-ctl dir">
                <span class="path" title={outDir ?? ''}>{outDir ?? t('convert.options.sameAsSource')}</span>
                <Button size="sm" onclick={pickOutDir}>{t('convert.options.choose')}</Button>
                {#if outDir}<Button size="sm" variant="plain" onclick={() => (outDir = null)}>{t('convert.options.reset')}</Button>{/if}
              </span>
            </div>
          </div>
        </Card>
      {/if}

      <div class="action">
        {#if installing}<div class="dl"><ProgressBar value={tools.progress[installing] ?? null} /></div>{/if}
        {#if submitError}<p class="error">{submitError}</p>{/if}
        <Button variant="primary" size="lg" disabled={!canGo} loading={busy && !installing} onclick={run}>{actionLabel}</Button>
        {#if !choice}<span class="hint">{t('convert.action.pick')}</span>{/if}
      </div>
    {:else if loading}
      <p class="muted">{t('convert.analyzing')}</p>
    {:else if resp}
      <p class="muted">{t('convert.formats.empty')}</p>
    {/if}
  {/if}

  {#if queue.length > 0}
    <SectionHeader title={t('convert.queue.title')}>
      {#snippet action()}<Button variant="plain" size="sm" onclick={() => nav.go('recent')}>{t('convert.queue.all')}</Button>{/snippet}
    </SectionHeader>
    <Card>
      {#each queue as job (job.id)}<JobRow {job} />{/each}
    </Card>
  {/if}

  {#if recentDone.length > 0}
    <SectionHeader title={t('convert.home.recent')}>
      {#snippet action()}<Button variant="plain" size="sm" onclick={() => jobs.clear()}>{t('common.actions.clear')}</Button>{/snippet}
    </SectionHeader>
    <Card>
      {#each recentDone as job (job.id)}
        {@const output = job.outputs[0]}
        <ListRow title={jobTitle(job.outputs.length > 0 ? job.outputs : job.inputs)} subtitle={jobTitle(job.inputs)}>
          {#snippet leading()}<span class="done"><CircleCheck size={18} /></span>{/snippet}
          {#snippet trailing()}
            {#if output}
              <Button size="sm" onclick={() => api.revealPath(output).catch(() => {})}>{t('recent.reveal')}</Button>
              <Button size="sm" onclick={() => api.openPath(output).catch(() => {})}>{t('recent.open')}</Button>
            {/if}
          {/snippet}
        </ListRow>
      {/each}
    </Card>
  {/if}
</section>

<style>
  section { padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-4); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  section > :global(.sh) { margin-top: var(--space-5); }

  .drop {
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; width: 100%; min-height: 210px;
    padding: var(--space-6); border: 1.5px dashed var(--hairline); border-radius: var(--radius-lg); background: var(--card-bg);
    color: var(--text); font: inherit; cursor: default; box-shadow: var(--shadow-sm), var(--glass-edge);
    transition: transform var(--dur) var(--ease-spring), border-color var(--dur-fast), background var(--dur-fast), box-shadow var(--dur);
  }
  .drop:hover, .drop.over { border-color: var(--accent); background: var(--accent-soft); box-shadow: var(--shadow-md), var(--glass-edge); }
  .drop.over { transform: scale(1.015); }
  .drop:active { transform: scale(0.99); }
  .drop-icon {
    display: grid; place-items: center; width: 60px; height: 60px; margin-bottom: 6px; border-radius: 18px;
    color: var(--accent); background: var(--accent-soft); transition: transform var(--dur) var(--ease-spring);
  }
  .drop:hover .drop-icon, .drop.over .drop-icon { transform: translateY(-3px) scale(1.06); }
  .drop-title { font: var(--text-title); letter-spacing: var(--tracking-tight); }
  .drop-sub { font: var(--text-body); color: var(--text-secondary); }
  .center { display: flex; justify-content: center; margin-top: var(--space-3); }
  .home-hint { margin: var(--space-3) 0 0; font: var(--text-callout); color: var(--text-secondary); text-align: center; }
  .done { display: inline-flex; justify-content: center; width: 24px; color: var(--green); }

  .files { display: flex; flex-direction: column; gap: var(--space-3); }
  .chips { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .chipwrap { display: inline-flex; align-items: center; gap: 6px; }
  .chipwrap.unsupported { opacity: 0.8; }
  .file-actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }

  .groups { display: flex; flex-direction: column; gap: var(--space-4); }
  .group { display: flex; flex-direction: column; gap: var(--space-2); }
  .group-label { display: inline-flex; align-items: center; gap: 6px; font: var(--text-caption); text-transform: uppercase; letter-spacing: 0.02em; color: var(--text-secondary); }
  .pills { display: flex; flex-wrap: wrap; gap: 6px; }
  .pill {
    height: var(--control-h); padding: 0 14px; border: 0; border-radius: 999px; background: var(--control-bg); color: var(--text);
    font: var(--text-body); font-weight: 500; cursor: default;
    transition: background var(--dur-fast), color var(--dur-fast), transform var(--dur-fast) var(--ease-out);
  }
  .pill:hover { background: var(--control-bg-hover); }
  .pill:active { transform: scale(0.96); }
  .pill.selected { background: var(--accent); color: var(--on-accent); }
  .pill:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .params { display: flex; flex-wrap: wrap; align-items: flex-end; gap: var(--space-3); margin-top: var(--space-4); }
  .field { display: flex; flex-direction: column; gap: 4px; width: 120px; font: var(--text-caption); color: var(--text-secondary); }
  .field.num { width: 90px; }

  .disclosure {
    display: inline-flex; align-items: center; gap: 4px; margin: var(--space-4) 0 var(--space-2); padding: 2px 4px; border: 0;
    background: none; color: var(--text-secondary); font: var(--text-headline); cursor: default;
  }
  .disclosure :global(svg) { transition: transform var(--dur) var(--ease-out); }
  .disclosure.open :global(svg) { transform: rotate(90deg); }
  .opts { display: flex; flex-direction: column; gap: var(--space-3); }
  .opt { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
  .opt-label { font: var(--text-body); }
  .opt-ctl { display: inline-flex; align-items: center; gap: var(--space-3); min-width: 0; }
  .opt-ctl.small { width: 120px; }
  .opt-ctl output { min-width: 28px; text-align: right; font: var(--text-body); color: var(--text-secondary); font-variant-numeric: tabular-nums; }
  .opt-ctl.dir { flex: 1; justify-content: flex-end; }
  .path { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: var(--text-callout); color: var(--text-secondary); }

  .action { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-2); margin-top: var(--space-5); }
  .dl { width: 100%; max-width: 360px; }
  .muted, .hint { margin: var(--space-3) 0 0; font: var(--text-callout); color: var(--text-secondary); }
  .hint { margin-top: 0; }
  .error { margin: var(--space-2) 0 0; font: var(--text-callout); color: var(--red); }
</style>
