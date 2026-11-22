<!--
  LineShortcuts — Settings > Line: the shortcuts rope's apps. A list in the user's order (move up /
  down, remove) and "Add shortcut", which opens a searchable picker of the apps on the taskbar, in
  the Start menu and on the desktop, plus "Choose a file…" for anything else (an .exe, a folder…).
-->
<script lang="ts">
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { ArrowDown, ArrowUp, FolderOpen, Plus, Search, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { api, errorOf } from '../../../lib/ipc';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { AppEntry, LineShortcut } from '../../../lib/types';
  import { Button, Card, EmptyState, IconButton, ListRow, Sheet, Spinner, TextField, toast } from '../../../lib/ui';

  const MAX = 40;
  let { disabled = false }: { disabled?: boolean } = $props();

  const list = $derived(settings.value.lineShortcuts ?? []);
  let icons = $state<Record<string, string | null>>({});
  let pickerOpen = $state(false);
  let apps = $state<AppEntry[] | null>(null);
  let query = $state('');

  const key = (p: string) => p.trim().toLowerCase().replaceAll('/', '\\');
  const has = (p: string) => list.some((s) => key(s.path) === key(p));

  /** Icons for paths we have not asked about yet. */
  async function fetchIcons(paths: string[]) {
    const missing = [...new Set(paths)].filter((p) => !(p in icons));
    if (missing.length === 0) return;
    for (const p of missing) icons[p] = null; // asked once
    try {
      const got = await api.appIcons(missing);
      missing.forEach((p, i) => (icons[p] = got[i] ?? null));
    } catch (e) {
      console.error(e);
    }
  }

  $effect(() => {
    void fetchIcons(list.map((s) => s.path));
  });

  function save(next: LineShortcut[]) {
    void settings.update({ lineShortcuts: next });
  }

  function add(name: string, path: string) {
    if (has(path)) return;
    if (list.length >= MAX) {
      toast.show(t('line.settings.shortcutsFull'), 'error');
      return;
    }
    save([...list, { name, path }]);
    toast.show(t('line.settings.shortcutsAdded', { name }));
  }

  function remove(path: string) {
    save(list.filter((s) => s.path !== path));
  }

  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= list.length) return;
    const next = [...list];
    [next[i], next[j]] = [next[j], next[i]];
    save(next);
  }

  async function openPicker() {
    pickerOpen = true;
    query = '';
    if (apps) return;
    try {
      apps = await api.appsList();
    } catch (e) {
      apps = [];
      toast.show(errorOf(e).message, 'error');
    }
  }

  async function pickFile() {
    const picked = await openDialog({ multiple: false, directory: false });
    if (typeof picked !== 'string') return;
    const base = picked.split(/[\\/]/).pop() ?? picked;
    add(base.replace(/\.(lnk|exe|url|appref-ms)$/i, ''), picked);
    pickerOpen = false;
  }

  function fold(s: string) {
    return s.toLocaleLowerCase('tr').normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ı/g, 'i');
  }
  const shown = $derived.by(() => {
    const q = fold(query.trim());
    const all = apps ?? [];
    return (q ? all.filter((a) => fold(a.name).includes(q)) : all).slice(0, 300);
  });
  $effect(() => {
    if (pickerOpen) void fetchIcons(shown.slice(0, 80).map((a) => a.path));
  });
</script>

<Card>
  {#if list.length === 0}
    <ListRow title={t('line.settings.shortcutsEmpty')} subtitle={t('line.settings.shortcutsHint')} />
  {/if}
  {#each list as sc, i (sc.path)}
    <ListRow title={sc.name} subtitle={sc.path}>
      {#snippet leading()}
        {#if icons[sc.path]}<img class="ico" src={convertFileSrc(icons[sc.path]!)} alt="" />{:else}<span class="ico ph">{sc.name.slice(0, 1).toUpperCase()}</span>{/if}
      {/snippet}
      {#snippet trailing()}
        <span class="row">
          <IconButton label={t('line.settings.moveUp')} size={26} disabled={disabled || i === 0} onclick={() => move(i, -1)}><ArrowUp size={14} /></IconButton>
          <IconButton label={t('line.settings.moveDown')} size={26} disabled={disabled || i >= list.length - 1} onclick={() => move(i, 1)}><ArrowDown size={14} /></IconButton>
          <IconButton label={t('line.settings.shortcutsRemove')} size={26} {disabled} onclick={() => remove(sc.path)}><X size={14} /></IconButton>
        </span>
      {/snippet}
    </ListRow>
  {/each}
  <ListRow title="">
    {#snippet trailing()}
      <Button variant="secondary" size="sm" {disabled} onclick={openPicker}><Plus size={14} />{t('line.settings.shortcutsAdd')}</Button>
    {/snippet}
  </ListRow>
</Card>

<Sheet bind:open={pickerOpen} title={t('line.settings.shortcutsPicker')} width={520}>
  <div class="picker">
    <div class="search">
      <TextField bind:value={query} placeholder={t('line.settings.shortcutsSearch')} label={t('line.settings.shortcutsSearch')}>
        {#snippet leading()}<Search size={14} />{/snippet}
      </TextField>
    </div>
    <div class="apps">
      {#if apps === null}
        <div class="center"><Spinner /> {t('line.settings.shortcutsLoading')}</div>
      {:else if shown.length === 0}
        <EmptyState title={t('line.settings.shortcutsNone')} />
      {:else}
        {#each shown as a (a.path)}
          {@const added = has(a.path)}
          <button type="button" class="app" disabled={added} onclick={() => add(a.name, a.path)}>
            {#if icons[a.path]}<img class="ico" src={convertFileSrc(icons[a.path]!)} alt="" />{:else}<span class="ico ph">{a.name.slice(0, 1).toUpperCase()}</span>{/if}
            <span class="name">{a.name}</span>
            <span class="place">{t(`line.settings.place.${a.place}`)}</span>
            {#if added}<span class="tick">✓</span>{:else}<Plus size={14} />{/if}
          </button>
        {/each}
      {/if}
    </div>
  </div>
  {#snippet footer()}
    <Button variant="secondary" onclick={pickFile}><FolderOpen size={14} />{t('line.settings.shortcutsFile')}</Button>
    <Button variant="primary" onclick={() => (pickerOpen = false)}>{t('common.actions.close')}</Button>
  {/snippet}
</Sheet>

<style>
  .row { display: inline-flex; align-items: center; gap: var(--space-1); }
  .ico { width: 24px; height: 24px; flex: none; object-fit: contain; }
  .ph { display: grid; place-items: center; border-radius: 6px; background: var(--control-bg); color: var(--text-secondary); font: var(--text-callout); font-weight: 600; }
  .picker { display: flex; flex-direction: column; gap: var(--space-2); }
  .apps { display: flex; flex-direction: column; max-height: 360px; overflow-x: hidden; overflow-y: auto; margin: 0 calc(var(--space-2) * -1); }
  .center { display: flex; align-items: center; justify-content: center; gap: var(--space-2); padding: var(--space-6); color: var(--text-secondary); font: var(--text-callout); }
  .app {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 6px var(--space-2);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text);
    font: var(--text-body);
    text-align: left;
    cursor: default;
  }
  .app:hover:not(:disabled) { background: var(--control-bg); }
  .app:disabled { opacity: 0.6; }
  .app .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .app .place { font: var(--text-caption); color: var(--text-secondary); }
  .tick { color: var(--accent); font-weight: 700; }
</style>
