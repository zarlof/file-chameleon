<!--
  Line settings ("İp") — the clothesline: on/off, edge reveal, peek on copy, breeze, which ropes to
  show, the open hotkey (with conflict status) and a "show now" button. Texts live in line.json.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { TriangleAlert } from '@lucide/svelte';
  import { formatHotkey, sameHotkey } from '../../../lib/hotkeys';
  import { currentLang, t } from '../../../lib/i18n';
  import { api, errorOf } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { HotkeyStatus, Settings } from '../../../lib/types';
  import { Button, Card, HotkeyRecorder, ListRow, SectionHeader, Toggle, toast } from '../../../lib/ui';

  const s = $derived(settings.value);
  const off = $derived(!s.lineEnabled);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  function setRope(key: 'images' | 'text' | 'links', v: boolean) {
    set({ lineRopes: { ...s.lineRopes, [key]: v } });
  }

  // ---- hotkey
  const OTHERS = [
    { key: 'hotkeyShelf', name: 'settings.hotkeys.shelf' },
    { key: 'hotkeyLibrary', name: 'settings.hotkeys.library' },
    { key: 'hotkeyDownloadLink', name: 'settings.hotkeys.downloadLink' },
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
    return () => clearTimeout(statusTimer);
  });

  const conflict = $derived.by(() => {
    const st = statuses.find((x) => x.name === 'line');
    return !!s.hotkeyLine && !!st && !st.ok && (st.keys === s.hotkeyLine || sameHotkey(st.keys, s.hotkeyLine));
  });

  async function setHotkey(value: string) {
    const clash = OTHERS.find((h) => sameHotkey(s[h.key], value));
    if (clash) {
      toast.show(t('line.settings.duplicate', { keys: formatHotkey(value, currentLang()), name: t(clash.name) }), 'error');
      return;
    }
    if (await settings.update({ hotkeyLine: value })) {
      clearTimeout(statusTimer);
      statusTimer = setTimeout(() => void loadStatus(), 300);
    }
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
  .conflict { display: inline-flex; align-items: center; gap: 5px; font: var(--text-callout); color: var(--red); white-space: normal; }
</style>
