<!--
  Shortcuts settings — the Ctrl+drag wheel trigger: on/off, any app vs Explorer only, apps where it
  never opens, and how the open wheel flips between its faces; then the global hotkeys (clipboard
  shelf, clipboard library, download the copied link) with a recorder and conflict reporting.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { Plus, TriangleAlert, X } from '@lucide/svelte';
  import { formatHotkey, sameHotkey } from '../../../lib/hotkeys';
  import { currentLang, t } from '../../../lib/i18n';
  import { api } from '../../../lib/ipc';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { HotkeyStatus, Settings } from '../../../lib/types';
  import { Button, Card, HotkeyRecorder, ListRow, SectionHeader, TextField, Toggle, toast } from '../../../lib/ui';
  import { addExe } from './logic';

  const s = $derived(settings.value);
  const off = $derived(!s.wheelEnabled);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  let appInput = $state('');
  function addApp() {
    const next = addExe(s.wheelExcludedApps, appInput);
    if (next !== s.wheelExcludedApps) set({ wheelExcludedApps: next });
    appInput = '';
  }
  function removeApp(name: string) {
    set({ wheelExcludedApps: s.wheelExcludedApps.filter((a) => a !== name) });
  }

  // ---- global hotkeys
  type HotkeyKey = 'hotkeyShelf' | 'hotkeyLibrary' | 'hotkeyDownloadLink' | 'hotkeyLine';
  const HOTKEYS: { key: HotkeyKey; name: HotkeyStatus['name'] }[] = [
    { key: 'hotkeyShelf', name: 'shelf' },
    { key: 'hotkeyLibrary', name: 'library' },
    { key: 'hotkeyDownloadLink', name: 'downloadLink' },
    { key: 'hotkeyLine', name: 'line' },
  ];
  let statuses = $state<HotkeyStatus[]>([]);
  let statusTimer: ReturnType<typeof setTimeout> | undefined;

  async function loadStatus() {
    try {
      statuses = await api.hotkeyStatus();
    } catch {
      statuses = []; // unknown: show no conflict
    }
  }
  onMount(() => {
    void loadStatus();
    return () => clearTimeout(statusTimer);
  });

  /** The backend could not register the hotkey that is set now (another app owns it, or Windows reserves it). */
  function conflict(h: (typeof HOTKEYS)[number]): boolean {
    const value = s[h.key];
    const st = statuses.find((x) => x.name === h.name);
    return !!value && !!st && !st.ok && (st.keys === value || sameHotkey(st.keys, value));
  }

  async function setHotkey(key: HotkeyKey, value: string) {
    const clash = HOTKEYS.find((h) => h.key !== key && sameHotkey(s[h.key], value));
    if (clash) {
      toast.show(t('settings.hotkeys.duplicate', { keys: formatHotkey(value, currentLang()), name: t(`settings.hotkeys.${clash.name}`) }), 'error');
      return;
    }
    if (await settings.update({ [key]: value } as Partial<Settings>)) {
      // The hotkey thread re-registers after the save; ask again once it had a moment.
      clearTimeout(statusTimer);
      statusTimer = setTimeout(() => void loadStatus(), 300);
    }
  }
</script>

<section>
  <h1>{t('common.nav.shortcuts')}</h1>

  <SectionHeader title={t('settings.shortcuts.wheel')} />
  <Card>
    <ListRow title={t('settings.shortcuts.enable')} subtitle={t('settings.shortcuts.enableHint')}>
      {#snippet trailing()}<Toggle checked={s.wheelEnabled} label={t('settings.shortcuts.enable')} onchange={(v) => set({ wheelEnabled: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.shortcuts.anyApp')} subtitle={t('settings.shortcuts.anyAppHint')}>
      {#snippet trailing()}<Toggle checked={s.wheelAnyApp} disabled={off} label={t('settings.shortcuts.anyApp')} onchange={(v) => set({ wheelAnyApp: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.shortcuts.flipScroll')} subtitle={t('settings.shortcuts.flipScrollHint')}>
      {#snippet trailing()}<Toggle checked={s.wheelFlipScroll} disabled={off} label={t('settings.shortcuts.flipScroll')} onchange={(v) => set({ wheelFlipScroll: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.shortcuts.flipTab')}>
      {#snippet trailing()}<Toggle checked={s.wheelFlipTab} disabled={off} label={t('settings.shortcuts.flipTab')} onchange={(v) => set({ wheelFlipTab: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.shortcuts.shiftBack')} subtitle={t('settings.shortcuts.shiftBackHint')}>
      {#snippet trailing()}<Toggle checked={s.wheelShiftBack} disabled={off} label={t('settings.shortcuts.shiftBack')} onchange={(v) => set({ wheelShiftBack: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.shortcuts.excluded')} subtitle={t('settings.shortcuts.excludedHint')}>
      <div class="apps" class:off>
        {#each s.wheelExcludedApps as name (name)}
          <span class="chip">
            <span class="chip-name">{name}</span>
            <button type="button" class="chip-x" disabled={off} aria-label={t('settings.shortcuts.removeApp', { name })} title={t('settings.shortcuts.removeApp', { name })}
              onclick={() => removeApp(name)}><X size={11} strokeWidth={2.5} /></button>
          </span>
        {:else}
          <span class="empty">{t('settings.shortcuts.excludedEmpty')}</span>
        {/each}
      </div>
      <div class="add">
        <span class="field"><TextField bind:value={appInput} placeholder={t('settings.shortcuts.excludedPlaceholder')} label={t('settings.shortcuts.excluded')} disabled={off} onenter={addApp} /></span>
        <Button size="sm" disabled={off || !appInput.trim()} onclick={addApp}>
          {#snippet icon()}<Plus size={13} />{/snippet}
          {t('settings.shortcuts.add')}
        </Button>
      </div>
    </ListRow>
  </Card>

  <SectionHeader title={t('settings.shortcuts.howTitle')} />
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
      <p>{t('settings.shortcuts.howBody')}</p>
    </div>
  </Card>

  <SectionHeader title={t('settings.hotkeys.title')} subtitle={t('settings.hotkeys.hint')} />
  <Card>
    {#each HOTKEYS as h (h.key)}
      {@const clash = conflict(h)}
      <ListRow title={t(`settings.hotkeys.${h.name}`)} subtitle={clash ? undefined : t(`settings.hotkeys.${h.name}Hint`)}>
        {#if clash}<span class="conflict"><TriangleAlert size={13} />{t('settings.hotkeys.conflict')}</span>{/if}
        {#snippet trailing()}
          <HotkeyRecorder value={s[h.key]} status={clash ? false : undefined} label={t(`settings.hotkeys.${h.name}`)} onchange={(v) => setHotkey(h.key, v)} />
        {/snippet}
      </ListRow>
    {/each}
  </Card>
</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  section > :global(.sh) { margin-top: var(--space-2); }

  .apps { display: flex; flex-wrap: wrap; gap: 6px; margin-top: var(--space-1); }
  .apps.off { opacity: 0.5; }
  .chip {
    display: inline-flex; align-items: center; gap: 4px; height: 24px; padding: 0 4px 0 10px; border-radius: 999px;
    background: var(--control-bg); font: var(--text-callout); color: var(--text);
  }
  .chip-name { max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chip-x {
    display: grid; place-items: center; width: 16px; height: 16px; padding: 0; border: 0; border-radius: 50%;
    background: transparent; color: var(--text-secondary); cursor: default; transition: background var(--dur-fast);
  }
  .chip-x:hover:not(:disabled) { background: var(--control-bg-hover); color: var(--text); }
  .empty { font: var(--text-callout); color: var(--text-tertiary); }
  .add { display: flex; align-items: center; gap: var(--space-2); margin-top: var(--space-2); }
  .field { width: 220px; }

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
  .conflict { display: inline-flex; align-items: center; gap: 5px; font: var(--text-callout); color: var(--red); white-space: normal; }
  .wheel i {
    position: absolute; top: 50%; left: 50%; width: 8px; height: 8px; margin: -4px; border-radius: 50%; background: var(--accent);
    transform: rotate(var(--a)) translateY(-18px);
  }
</style>
