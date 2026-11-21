<!--
  Wheel editor ("Çark") — three columns: (1) profiles, (2) the selected profile's Front/Back faces
  as a live donut + ordered slot list, and its options (name, mixed selections, extensions,
  reset / delete), (3) presets and groups to add. Every change saves through the wheelConfig
  store (debounced; rolled back with a toast when the backend refuses it).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { open as pickDir } from '@tauri-apps/plugin-dialog';
  import { CircleDot, RotateCcw, Trash2, TriangleAlert, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { api } from '../../../lib/ipc';
  import { wheelConfig } from '../../../lib/stores/wheelConfig.svelte';
  import type { Preset, PresetGroup, Profile, WheelConfig, WheelFace } from '../../../lib/types';
  import { Button, Card, EmptyState, ListRow, Sheet, TextField, toast, Toggle } from '../../../lib/ui';
  import { profileLabel } from '../../../lib/wheel/labels';
  import PresetEditor from '../presets/PresetEditor.svelte';
  import { defaultOp, describeOp, EMPTY_OPTIONS } from '../presets/logic';
  import GroupSheet from './GroupSheet.svelte';
  import PresetPicker from './PresetPicker.svelte';
  import ProfileList from './ProfileList.svelte';
  import SlotEditor from './SlotEditor.svelte';
  import {
    addSlot, deleteGroup, deleteProfile, FACE_LIMIT, folderName, GROUP_PREFIX, newPresetId, normalizeExt, replaceProfile,
    shadowedBy, upsertGroup, upsertPreset,
  } from './logic';

  let profileId = $state('images');
  let face = $state<WheelFace>('front');
  let selectedId = $state<string | null>(null);
  let nameDraft = $state<string | null>(null);
  let extDraft = $state('');
  let extError = $state(false);
  /** Preset ids that fit the profile's files (from the core); null = do not dim. */
  let fit = $state.raw<Set<string> | null>(null);
  let newPreset = $state.raw<Preset | null>(null);
  let newPresetOpen = $state(false);
  let groupEdit = $state.raw<{ group: PresetGroup | null } | null>(null);
  let confirm = $state.raw<{ title: string; message: string; label: string; run: () => void } | null>(null);
  let confirmOpen = $state(false);
  let mid: HTMLElement | undefined = $state();

  const config = $derived(wheelConfig.value);
  const profile = $derived(config ? (config.profiles.find((p) => p.id === profileId) ?? config.profiles[0] ?? null) : null);
  /** The mixed profile takes any selection, so nothing is dimmed there. */
  const fitKey = $derived(profile && !profile.mixed ? profile.extensions.join(',') : null);

  onMount(() => void wheelConfig.load());

  $effect(() => {
    const key = fitKey;
    void wheelConfig.revision; // ask again once a save reached the backend
    if (key === null) {
      fit = null;
      return;
    }
    let live = true;
    api
      .presetsForExtensions(key ? key.split(',') : [])
      .then((ids) => live && (fit = new Set(ids)))
      .catch(() => live && (fit = null));
    return () => {
      live = false;
    };
  });

  function selectProfile(id: string) {
    profileId = id;
    face = 'front';
    selectedId = null;
    nameDraft = null;
    extDraft = '';
    extError = false;
    if (mid) mid.scrollTop = 0;
  }

  function saveProfile(next: Profile) {
    if (wheelConfig.value) wheelConfig.save(replaceProfile(wheelConfig.value, next));
  }

  function full() {
    toast.show(t('wheel.picker.full', { max: FACE_LIMIT[face] }), 'error');
  }

  /** Put a slot on the current face, after the selected slot (or at the end). */
  function add(slotId: string) {
    if (!profile) return;
    const list = profile[face];
    if (list.includes(slotId)) {
      selectedId = slotId;
      return;
    }
    const at = selectedId ? list.indexOf(selectedId) : -1;
    const next = addSlot(profile, face, slotId, at >= 0 ? at + 1 : undefined);
    if (next === profile) return full();
    saveProfile(next);
    selectedId = slotId;
  }

  /** Apply `edit` (which adds a preset or group) and put `slotId` on the current face. */
  function addNew(edit: (c: WheelConfig) => WheelConfig, slotId: string) {
    const c = wheelConfig.value;
    if (!c || !profile) return;
    const p = c.profiles.find((x) => x.id === profile.id) ?? profile;
    const at = selectedId ? p[face].indexOf(selectedId) : -1;
    const placed = addSlot(p, face, slotId, at >= 0 ? at + 1 : undefined);
    wheelConfig.save(replaceProfile(edit(c), placed));
    if (placed === p) full();
    else selectedId = slotId;
  }

  function openNewPreset() {
    if (!config) return;
    const op = defaultOp('convert');
    newPreset = { id: newPresetId(config.presets.map((p) => p.id)), name: describeOp(op, t), builtin: false, op, options: { ...EMPTY_OPTIONS } };
    newPresetOpen = true;
  }

  function createPreset() {
    const p = newPreset;
    newPresetOpen = false;
    newPreset = null;
    if (p) addNew((c) => upsertPreset(c, p), p.id);
  }

  async function copyToFolder() {
    if (!profile) return;
    if (profile[face].length >= FACE_LIMIT[face]) return full();
    let dir: string | string[] | null = null;
    try {
      dir = await pickDir({ directory: true });
    } catch {
      return; // dialog unavailable
    }
    const c = wheelConfig.value;
    if (typeof dir !== 'string' || !c) return;
    const p: Preset = {
      id: newPresetId(c.presets.map((x) => x.id)),
      name: folderName(dir),
      builtin: false,
      op: { type: 'copyTo', dir, moveFiles: false },
      options: { ...EMPTY_OPTIONS },
    };
    addNew((cfg) => upsertPreset(cfg, p), p.id);
  }

  function saveGroup(g: PresetGroup) {
    const c = wheelConfig.value;
    if (!c) return;
    if (g.id) {
      wheelConfig.save(upsertGroup(c, g));
      return;
    }
    const group = { ...g, id: newPresetId(c.groups.map((x) => x.id)) };
    addNew((cfg) => upsertGroup(cfg, group), GROUP_PREFIX + group.id);
  }

  function removeGroup(id: string) {
    if (wheelConfig.value) wheelConfig.save(deleteGroup(wheelConfig.value, id));
    if (selectedId === GROUP_PREFIX + id) selectedId = null;
  }

  function createProfile() {
    const c = wheelConfig.value;
    if (!c) return;
    const p: Profile = {
      id: newPresetId(c.profiles.map((x) => x.id)),
      name: t('wheel.profiles.newName'),
      builtin: false,
      extensions: [],
      mixed: false,
      front: [],
      back: [],
    };
    // First in the list: the core opens the first profile that has a file's extension.
    wheelConfig.save({ ...c, profiles: [p, ...c.profiles] });
    selectProfile(p.id);
  }

  function setName(v: string) {
    if (!profile) return;
    nameDraft = v;
    // Built-ins fall back to their localized name ("").
    saveProfile({ ...profile, name: v.trim() || (profile.builtin ? '' : t('wheel.profiles.newName')) });
  }

  function addExtension() {
    if (!profile) return;
    const e = normalizeExt(extDraft);
    extError = e === null;
    if (e === null) return;
    extDraft = '';
    if (!profile.extensions.includes(e)) saveProfile({ ...profile, extensions: [...profile.extensions, e] });
  }

  function ask(title: string, message: string, label: string, run: () => void) {
    confirm = { title, message, label, run };
    confirmOpen = true;
  }

  function resetProfile() {
    if (!profile) return;
    const id = profile.id;
    ask(t('wheel.profile.resetTitle', { name: profileLabel(profile, t) }), t('wheel.profile.resetMessage'), t('wheel.profile.reset'), async () => {
      if (await wheelConfig.resetProfile(id)) selectedId = null;
    });
  }

  function removeProfile() {
    if (!profile) return;
    const p = profile;
    ask(t('wheel.profile.deleteTitle', { name: profileLabel(p, t) }), t('wheel.profile.deleteMessage'), t('wheel.profile.delete'), () => {
      if (wheelConfig.value) wheelConfig.save(deleteProfile(wheelConfig.value, p.id));
      selectProfile(wheelConfig.value?.profiles[0]?.id ?? 'images');
    });
  }

  function resetAll() {
    ask(t('wheel.resetAllTitle'), t('wheel.resetAllMessage'), t('wheel.resetAll'), async () => {
      if (await wheelConfig.resetAll()) selectProfile(profileId);
    });
  }

  function runConfirm() {
    const c = confirm;
    confirmOpen = false;
    c?.run();
  }
</script>

<section>
  <header>
    <h1>{t('common.nav.wheel')}</h1>
    {#if config}
      <Button size="sm" onclick={resetAll}>
        {#snippet icon()}<RotateCcw size={13} />{/snippet}
        {t('wheel.resetAll')}
      </Button>
    {/if}
  </header>

  {#if !config || !profile}
    {#if wheelConfig.failed}
      <EmptyState title={t('wheel.loadFailed')}>
        {#snippet icon()}<CircleDot />{/snippet}
        {#snippet action()}<Button onclick={() => wheelConfig.load()}>{t('wheel.retry')}</Button>{/snippet}
      </EmptyState>
    {/if}
  {:else}
    <div class="cols">
      <div class="col side">
        <ProfileList profiles={config.profiles} selected={profile.id} onselect={selectProfile} oncreate={createProfile} />
      </div>

      <div class="col mid" bind:this={mid}>
        <h2>{t('wheel.arrange.title')}</h2>
        <SlotEditor {config} {profile} bind:face bind:selectedId onchange={saveProfile}
          oneditgroup={(id) => (groupEdit = { group: config.groups.find((g) => g.id === id) ?? null })} />

        <Card>
          <ListRow title={t('wheel.profile.name')}>
            {#snippet trailing()}
              <span class="name-field">
                <TextField value={nameDraft ?? profile.name} placeholder={profile.builtin ? profileLabel({ id: profile.id, name: '' }, t) : t('wheel.profiles.newName')}
                  label={t('wheel.profile.name')} clearable={false} oninput={setName} />
              </span>
            {/snippet}
          </ListRow>
          <ListRow title={t('wheel.profile.mixed')} subtitle={t('wheel.profile.mixedHint')}>
            {#snippet trailing()}
              <Toggle checked={profile.mixed} label={t('wheel.profile.mixed')} onchange={(mixed) => saveProfile({ ...profile, mixed })} />
            {/snippet}
          </ListRow>
          {#if profile.id === 'other'}
            <ListRow title={t('wheel.profile.extensions')} subtitle={t('wheel.profile.otherHint')} />
          {:else}
            <div class="exts">
              <span class="exts-title">{t('wheel.profile.extensions')}</span>
              <div class="chips">
                {#each profile.extensions as ext (ext)}
                  {@const owner = shadowedBy(config.profiles, profile.id, ext)}
                  <span class="ext" class:warn={owner} title={owner ? t('wheel.profile.shadowed', { ext, profile: profileLabel(owner, t) }) : undefined}>
                    {#if owner}<TriangleAlert size={11} strokeWidth={2.2} />{/if}
                    {ext}
                    <button type="button" class="x" aria-label={t('wheel.profile.removeExtension', { ext })}
                      onclick={() => saveProfile({ ...profile, extensions: profile.extensions.filter((e) => e !== ext) })}>
                      <X size={10} strokeWidth={2.6} />
                    </button>
                  </span>
                {:else}
                  <span class="muted">{t('wheel.profile.noExtensions')}</span>
                {/each}
              </div>
              <div class="add-ext">
                <span class="ext-field" class:bad={extError}>
                  <TextField bind:value={extDraft} placeholder={t('wheel.profile.extensionPlaceholder')} label={t('wheel.profile.extensions')}
                    onenter={addExtension} oninput={() => (extError = false)} />
                </span>
                <Button size="sm" disabled={!extDraft.trim()} onclick={addExtension}>{t('wheel.profile.addExtension')}</Button>
              </div>
              {#if extError}<p class="error">{t('wheel.profile.badExtension')}</p>{/if}
            </div>
          {/if}
        </Card>
        <div class="profile-acts">
          {#if profile.builtin}
            <Button size="sm" onclick={resetProfile}>
              {#snippet icon()}<RotateCcw size={13} />{/snippet}
              {t('wheel.profile.reset')}
            </Button>
          {:else}
            <Button size="sm" variant="destructive" onclick={removeProfile}>
              {#snippet icon()}<Trash2 size={13} />{/snippet}
              {t('wheel.profile.delete')}
            </Button>
          {/if}
        </div>
      </div>

      <div class="col side">
        <PresetPicker {config} {profile} {face} {fit} onadd={add} onnewpreset={openNewPreset}
          onnewgroup={() => (groupEdit = { group: null })} oncopyfolder={copyToFolder} />
      </div>
    </div>
  {/if}
</section>

{#if groupEdit && config}
  {@const g = groupEdit.group}
  <GroupSheet group={g} presets={config.presets} onsave={saveGroup} ondelete={g ? () => removeGroup(g.id) : undefined}
    onclose={() => (groupEdit = null)} />
{/if}

<Sheet bind:open={newPresetOpen} title={t('wheel.newPreset.title')} width={520} onclose={() => (newPreset = null)}>
  {#if newPreset}
    <PresetEditor preset={newPreset} onchange={(p) => (newPreset = p)} />
  {/if}
  {#snippet footer()}
    <Button onclick={() => { newPresetOpen = false; newPreset = null; }}>{t('common.actions.cancel')}</Button>
    <Button variant="primary" onclick={createPreset}>{t('wheel.newPreset.create')}</Button>
  {/snippet}
</Sheet>

<Sheet bind:open={confirmOpen} title={confirm?.title}>
  <p class="confirm">{confirm?.message}</p>
  {#snippet footer()}
    <Button onclick={() => (confirmOpen = false)}>{t('common.actions.cancel')}</Button>
    <Button variant="destructive" onclick={runConfirm}>{confirm?.label}</Button>
  {/snippet}
</Sheet>

<style>
  section { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--space-6); }
  header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex: none; margin-bottom: var(--space-3); }
  h1 { margin: 0; font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  h2 { margin: 0 0 var(--space-1); font: var(--text-headline); }
  .cols {
    flex: 1; min-height: 0; display: grid; gap: var(--space-4);
    grid-template-columns: minmax(140px, 0.7fr) minmax(280px, 1.5fr) minmax(210px, 1fr);
  }
  .col { min-height: 0; overflow: auto; display: flex; flex-direction: column; gap: var(--space-3); padding: 2px 2px var(--space-5); }
  /* Side columns: the list inside scrolls, the column does not. */
  .side { overflow: hidden; }
  .side > :global(*) { flex: 1; }
  /* Middle column: scrolls as a whole; its parts keep their height (cards clip their overflow). */
  .mid > :global(*) { flex: none; }
  .name-field { display: block; width: 180px; }
  .exts { display: flex; flex-direction: column; gap: var(--space-2); padding: 10px var(--space-4) var(--space-3); border-top: 0.5px solid var(--separator); }
  .exts-title { font: var(--text-body); }
  .chips { display: flex; flex-wrap: wrap; gap: 5px; }
  .ext {
    display: inline-flex; align-items: center; gap: 4px; height: 22px; padding: 0 3px 0 8px; border-radius: 99px;
    background: var(--control-bg-hover); font: var(--text-callout); font-variant-numeric: tabular-nums;
  }
  .ext.warn { background: color-mix(in srgb, var(--orange) 18%, transparent); color: var(--orange); }
  .x {
    display: grid; place-items: center; width: 16px; height: 16px; padding: 0; border: 0; border-radius: 50%;
    background: transparent; color: var(--text-secondary); cursor: default;
  }
  .x:hover { background: var(--control-bg-active); color: var(--text); }
  .muted { font: var(--text-callout); color: var(--text-tertiary); }
  .add-ext { display: flex; align-items: center; gap: var(--space-2); }
  .ext-field { display: block; width: 140px; }
  .ext-field.bad :global(.field) { box-shadow: 0 0 0 2px color-mix(in srgb, var(--red) 55%, transparent); }
  .error { margin: 0; font: var(--text-callout); color: var(--red); }
  .profile-acts { display: flex; justify-content: flex-end; }
  .confirm { margin: 0; color: var(--text-secondary); }
</style>
