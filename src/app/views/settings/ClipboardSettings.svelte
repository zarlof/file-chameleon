<!--
  Clipboard settings ("Pano ayarları") — history on/off and pause, what is captured, suggestion toasts,
  retention per kind and the item cap, ignored apps, OCR, link titles, watched image folders and storage (used space, clear unsaved, empty trash).
  Every control writes through the settings store immediately; pause goes through `clipSetPaused`.
-->
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { Folder, FolderPlus, Plus, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { api, errorOf, on } from '../../../lib/ipc';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { ClipStats, Settings } from '../../../lib/types';
  import { Button, Card, formatBytes, IconButton, ListRow, SectionHeader, Select, Sheet, TextField, Toggle, toast } from '../../../lib/ui';
  import { addFolderPath } from '../clipboard/clips';
  import { addExe } from './logic';

  const MAX_FOLDERS = 3;
  const s = $derived(settings.value);
  const off = $derived(!s.clipboardEnabled);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  /** Select options for a numeric setting; keeps an unusual stored value selectable. */
  function numOptions(values: number[], current: number, label: (n: number) => string) {
    const all = values.includes(current) ? values : [...values, current].sort((a, b) => a - b);
    return all.map((n) => ({ value: String(n), label: label(n) }));
  }
  const days = (n: number) => (n === 0 ? t('settings.clipboard.forever') : t('settings.clipboard.days', { count: n }));
  /** 0 = forever, listed last. */
  const dayOptions = (current: number) => {
    const opts = numOptions([1, 3, 7, 15, 30], current === 0 ? 1 : current, days);
    return [...opts, { value: '0', label: days(0) }];
  };
  const itemOptions = $derived(numOptions([100, 300, 500, 1000, 2000], s.clipboardMaxItems, (n) => t('settings.clipboard.items', { count: n })));

  // ---- pause (not a plain setting: the backend owns it, the tray can change it too)
  let paused = $state(settings.value.clipboardPaused);
  async function setPaused(v: boolean) {
    try {
      await api.clipSetPaused(v);
    } catch (e) {
      paused = !v; // roll the switch back: the backend state did not change
      toast.show(errorOf(e).message, 'error');
    }
  }

  // ---- ignored apps
  let appInput = $state('');
  function addApp() {
    const next = addExe(s.clipboardIgnoreApps, appInput);
    if (next !== s.clipboardIgnoreApps) set({ clipboardIgnoreApps: next });
    appInput = '';
  }
  function removeApp(name: string) {
    set({ clipboardIgnoreApps: s.clipboardIgnoreApps.filter((a) => a !== name) });
  }

  // ---- watched image folders
  async function addFolder() {
    try {
      const r = await open({ directory: true });
      if (typeof r !== 'string') return;
      const next = addFolderPath(s.clipboardWatchFolders, r, MAX_FOLDERS);
      if (next !== s.clipboardWatchFolders) set({ clipboardWatchFolders: next });
    } catch {
      /* dialog unavailable */
    }
  }
  function removeFolder(path: string) {
    set({ clipboardWatchFolders: s.clipboardWatchFolders.filter((p) => p !== path) });
  }

  // ---- storage
  let stats = $state<ClipStats | null>(null);
  let clearOpen = $state(false);
  let emptyOpen = $state(false);
  let busy = $state(false);

  async function loadStats() {
    try {
      stats = await api.clipStats();
    } catch {
      stats = null;
    }
  }

  async function clearUnsaved() {
    clearOpen = false;
    busy = true;
    try {
      const n = await api.clipClearUnsaved();
      toast.show(t('settings.clipboard.cleared', { count: n }), 'success');
    } catch (e) {
      toast.show(errorOf(e).message, 'error');
    } finally {
      busy = false;
      void loadStats();
    }
  }

  async function emptyTrash() {
    emptyOpen = false;
    busy = true;
    try {
      await api.clipEmptyTrash();
      toast.show(t('settings.clipboard.emptied'), 'success');
    } catch (e) {
      toast.show(errorOf(e).message, 'error');
    } finally {
      busy = false;
      void loadStats();
    }
  }

  onMount(() => {
    void loadStats();
    api.clipGetPaused().then((p) => (paused = p)).catch(() => {});
    const unlisten: (() => void)[] = [];
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const keep = (p: Promise<() => void>) => p.then((u) => (disposed ? u() : unlisten.push(u))).catch(() => {});
    keep(on.clipPaused((p) => (paused = p)));
    keep(on.clipChanged(() => {
      clearTimeout(timer);
      timer = setTimeout(() => void loadStats(), 300);
    }));
    return () => {
      disposed = true;
      clearTimeout(timer);
      unlisten.forEach((u) => u());
    };
  });
</script>

<section>
  <h1>{t('common.nav.clipboardSettings')}</h1>

  <Card>
    <ListRow title={t('settings.clipboard.enable')} subtitle={t('settings.clipboard.enableHint')}>
      {#snippet trailing()}<Toggle checked={s.clipboardEnabled} label={t('settings.clipboard.enable')} onchange={(v) => set({ clipboardEnabled: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.pause')} subtitle={t('settings.clipboard.pauseHint')}>
      {#snippet trailing()}<Toggle bind:checked={paused} disabled={off} label={t('settings.clipboard.pause')} onchange={setPaused} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.clipboard.capture')} />
  <Card>
    <ListRow title={t('settings.clipboard.captureText')}>
      {#snippet trailing()}<Toggle checked={s.clipboardCaptureText} disabled={off} label={t('settings.clipboard.captureText')} onchange={(v) => set({ clipboardCaptureText: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.captureLinks')}>
      {#snippet trailing()}<Toggle checked={s.clipboardCaptureLinks} disabled={off} label={t('settings.clipboard.captureLinks')} onchange={(v) => set({ clipboardCaptureLinks: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.captureImages')}>
      {#snippet trailing()}<Toggle checked={s.clipboardImages} disabled={off} label={t('settings.clipboard.captureImages')} onchange={(v) => set({ clipboardImages: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.suggestLinks')} subtitle={t('settings.clipboard.suggestHint')}>
      {#snippet trailing()}<Toggle checked={s.clipboardSuggestLinks} disabled={off} label={t('settings.clipboard.suggestLinks')} onchange={(v) => set({ clipboardSuggestLinks: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.suggestImages')}>
      {#snippet trailing()}<Toggle checked={s.clipboardSuggestImages} disabled={off} label={t('settings.clipboard.suggestImages')} onchange={(v) => set({ clipboardSuggestImages: v })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.clipboard.retention')} subtitle={t('settings.clipboard.retentionHint')} />
  <Card>
    <ListRow title={t('settings.clipboard.retentionText')}>
      {#snippet trailing()}<Select value={String(s.clipboardRetentionText)} options={dayOptions(s.clipboardRetentionText)} label={t('settings.clipboard.retentionText')}
        onchange={(v) => set({ clipboardRetentionText: Number(v) })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.retentionLinks')}>
      {#snippet trailing()}<Select value={String(s.clipboardRetentionLinks)} options={dayOptions(s.clipboardRetentionLinks)} label={t('settings.clipboard.retentionLinks')}
        onchange={(v) => set({ clipboardRetentionLinks: Number(v) })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.retentionImages')}>
      {#snippet trailing()}<Select value={String(s.clipboardRetentionImages)} options={dayOptions(s.clipboardRetentionImages)} label={t('settings.clipboard.retentionImages')}
        onchange={(v) => set({ clipboardRetentionImages: Number(v) })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.maxItems')} subtitle={t('settings.clipboard.maxItemsHint')}>
      {#snippet trailing()}<Select value={String(s.clipboardMaxItems)} options={itemOptions} label={t('settings.clipboard.maxItems')}
        onchange={(v) => set({ clipboardMaxItems: Number(v) })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.clipboard.privacy')} />
  <Card>
    <ListRow title={t('settings.clipboard.ignoreApps')} subtitle={t('settings.clipboard.ignoreAppsHint')}>
      <div class="chips">
        {#each s.clipboardIgnoreApps as name (name)}
          <span class="chip">
            <span class="chip-name">{name}</span>
            <button type="button" class="chip-x" aria-label={t('settings.clipboard.removeApp', { name })} title={t('settings.clipboard.removeApp', { name })}
              onclick={() => removeApp(name)}><X size={11} strokeWidth={2.5} /></button>
          </span>
        {:else}
          <span class="empty">{t('settings.clipboard.ignoreEmpty')}</span>
        {/each}
      </div>
      <div class="add">
        <span class="field"><TextField bind:value={appInput} placeholder={t('settings.clipboard.ignorePlaceholder')} label={t('settings.clipboard.ignoreApps')} onenter={addApp} /></span>
        <Button size="sm" disabled={!appInput.trim()} onclick={addApp}>
          {#snippet icon()}<Plus size={13} />{/snippet}
          {t('settings.clipboard.add')}
        </Button>
      </div>
    </ListRow>
    <ListRow title={t('settings.clipboard.ocr')}>
      <span class="note">{t('settings.clipboard.ocrHint')}</span>
      {#snippet trailing()}<Toggle checked={s.clipboardOcr} label={t('settings.clipboard.ocr')} onchange={(v) => set({ clipboardOcr: v })} />{/snippet}
    </ListRow>
    <!-- The privacy note wraps: it must be read in full. -->
    <ListRow title={t('settings.clipboard.linkTitles')}>
      <span class="note">{t('settings.clipboard.linkTitlesHint')}</span>
      {#snippet trailing()}<Toggle checked={s.clipboardLinkTitles} label={t('settings.clipboard.linkTitles')} onchange={(v) => set({ clipboardLinkTitles: v })} />{/snippet}
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.clipboard.folders')} subtitle={t('settings.clipboard.foldersHint')}>
    {#snippet action()}
      <Button size="sm" disabled={s.clipboardWatchFolders.length >= MAX_FOLDERS} onclick={addFolder}
        title={s.clipboardWatchFolders.length >= MAX_FOLDERS ? t('settings.clipboard.foldersFull') : undefined}>
        {#snippet icon()}<FolderPlus size={13} />{/snippet}
        {t('settings.clipboard.addFolder')}
      </Button>
    {/snippet}
  </SectionHeader>
  <Card>
    {#each s.clipboardWatchFolders as path (path)}
      <ListRow title={path.split(/[\\/]/).filter(Boolean).pop() ?? path} subtitle={path}>
        {#snippet leading()}<Folder size={16} />{/snippet}
        {#snippet trailing()}
          <IconButton label={t('settings.clipboard.removeFolder')} onclick={() => removeFolder(path)}><X size={14} /></IconButton>
        {/snippet}
      </ListRow>
    {:else}
      <ListRow title={t('settings.clipboard.foldersEmpty')} />
    {/each}
  </Card>

  <SectionHeader title={t('settings.clipboard.storage')} />
  <Card>
    <ListRow title={t('settings.clipboard.used')}
      subtitle={stats ? t('settings.clipboard.inTrash', { count: stats.trash }) : undefined}>
      {#snippet trailing()}
        <span class="value">{stats ? t('settings.clipboard.usedValue', { items: stats.items, size: formatBytes(stats.bytes) }) : '—'}</span>
      {/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.clearUnsaved')} subtitle={t('settings.clipboard.clearUnsavedHint')}>
      {#snippet trailing()}<Button size="sm" disabled={busy} onclick={() => (clearOpen = true)}>{t('settings.clipboard.clearButton')}</Button>{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.emptyTrash')} subtitle={t('settings.clipboard.emptyTrashHint')}>
      {#snippet trailing()}<Button size="sm" variant="destructive" disabled={busy || stats?.trash === 0} onclick={() => (emptyOpen = true)}>{t('settings.clipboard.emptyButton')}</Button>{/snippet}
    </ListRow>
  </Card>
</section>

<Sheet bind:open={clearOpen} title={t('settings.clipboard.clearTitle')}>
  <p class="confirm">{t('settings.clipboard.clearMessage')}</p>
  {#snippet footer()}
    <Button onclick={() => (clearOpen = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={clearUnsaved}>{t('settings.clipboard.clearButton')}</Button>
  {/snippet}
</Sheet>

<Sheet bind:open={emptyOpen} title={t('settings.clipboard.emptyTitle')}>
  <p class="confirm">{t('settings.clipboard.emptyMessage')}</p>
  {#snippet footer()}
    <Button onclick={() => (emptyOpen = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={emptyTrash}>{t('settings.clipboard.emptyButton')}</Button>
  {/snippet}
</Sheet>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  section > :global(.sh) { margin-top: var(--space-2); }

  .chips { display: flex; flex-wrap: wrap; gap: 6px; margin-top: var(--space-1); }
  .chip {
    display: inline-flex; align-items: center; gap: 4px; height: 24px; padding: 0 4px 0 10px; border-radius: 999px;
    background: var(--control-bg); font: var(--text-callout); color: var(--text);
  }
  .chip-name { max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chip-x {
    display: grid; place-items: center; width: 16px; height: 16px; padding: 0; border: 0; border-radius: 50%;
    background: transparent; color: var(--text-secondary); cursor: default; transition: background var(--dur-fast);
  }
  .chip-x:hover { background: var(--control-bg-hover); color: var(--text); }
  .empty { font: var(--text-callout); color: var(--text-tertiary); }
  .add { display: flex; align-items: center; gap: var(--space-2); margin-top: var(--space-2); }
  .field { width: 220px; }
  .note { display: block; margin-top: -2px; font: var(--text-callout); color: var(--text-secondary); white-space: normal; }
  .value { font: var(--text-body); color: var(--text); font-variant-numeric: tabular-nums; }
  .confirm { margin: 0; color: var(--text-secondary); }
</style>
