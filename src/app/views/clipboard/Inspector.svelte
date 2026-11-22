<!--
  Inspector — right panel for the one selected item: preview, title, note, tags, folder, editable text,
  OCR text, link title / open, source app, dates, size, and the item actions.
  Props: item (null = none or several selected), count (selected), folders, trash (shown in the trash),
         onupdated(item) (a field was saved), onrefresh() (tags/folder/edit changed: refetch),
         onquick(action), ondelete() (delete forever), onedit() (open the image editor), onpreview().
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import {
    Copy, Download, ExternalLink, Image as ImageIcon, MousePointerClick, Pencil, Pin, PinOff, RefreshCw, RotateCcw, Trash2, Undo2, X,
  } from '@lucide/svelte';
  import { currentLang, t } from '../../../lib/i18n';
  import { api } from '../../../lib/ipc';
  import { nav } from '../../../lib/stores/nav.svelte';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { ClipField, ClipFolder, ClipItem } from '../../../lib/types';
  import { Button, formatBytes, IconButton, Select, TextField, toast } from '../../../lib/ui';
  import { baseName, folderTree, type QuickAction } from './clips';
  import { guard, imageUrl, isWebUrl } from './ui';

  interface Props {
    item: ClipItem | null;
    count: number;
    folders: ClipFolder[];
    trash: boolean;
    onupdated: (item: ClipItem) => void;
    onrefresh: () => void;
    onquick: (action: QuickAction) => void;
    ondelete: () => void;
    onedit: () => void;
    onpreview: () => void;
  }
  let { item, count, folders, trash, onupdated, onrefresh, onquick, ondelete, onedit, onpreview }: Props = $props();

  const FORMATS = [
    { id: 'png', label: 'PNG' },
    { id: 'jpg', label: 'JPG' },
    { id: 'webp', label: 'WebP' },
  ] as const;

  const isLink = $derived(item?.kind === 'link' || item?.kind === 'mediaLink');
  const src = $derived(item?.kind === 'image' ? imageUrl(item) : null);
  const folderOptions = $derived([
    { value: '', label: t('clipboard.folder.none') },
    ...folderTree(folders).map((n) => ({ value: String(n.folder.id), label: `${' '.repeat(n.depth)}${n.folder.name}` })),
  ]);
  const dateFmt = $derived(new Intl.DateTimeFormat(currentLang(), { dateStyle: 'medium', timeStyle: 'short' }));

  // Drafts follow the item, except the field being typed in (an OCR or title update must not wipe typing).
  let editing = $state<ClipField | null>(null);
  let title = $state('');
  let note = $state('');
  let text = $state('');
  let newTag = $state('');
  $effect(() => {
    const v = item?.title ?? '';
    if (untrack(() => editing) !== 'title') title = v;
  });
  $effect(() => {
    const v = item?.note ?? '';
    if (untrack(() => editing) !== 'note') note = v;
  });
  $effect(() => {
    const v = item?.text ?? '';
    if (untrack(() => editing) !== 'text') text = v;
  });

  async function commit(field: ClipField) {
    editing = null;
    const it = item;
    if (!it) return;
    const value = field === 'title' ? title : field === 'note' ? note : text;
    const stored = (field === 'title' ? it.title : field === 'note' ? it.note : it.text) ?? '';
    if (field === 'text') {
      if (value === stored) return;
      if (!value.trim()) {
        text = stored; // a text item cannot become empty
        return;
      }
    } else if (value.trim() === stored) {
      return;
    }
    await guard(async () => onupdated(await api.clipSetField(it.id, field, value)));
  }

  async function addTag() {
    const tag = newTag.trim();
    const it = item;
    if (!tag || !it) return;
    newTag = '';
    if (await guard(() => api.clipTag([it.id], [tag], []))) onrefresh();
  }

  async function removeTag(tag: string) {
    const it = item;
    if (it && (await guard(() => api.clipTag([it.id], [], [tag])))) onrefresh();
  }

  async function moveTo(value: string) {
    const it = item;
    if (it && (await guard(() => api.clipMove([it.id], value === '' ? null : Number(value))))) onrefresh();
  }

  async function revert() {
    const it = item;
    if (it) await guard(async () => onupdated(await api.clipRevertEdit(it.id)));
  }

  async function saveAs(format: 'png' | 'jpg' | 'webp') {
    const it = item;
    if (!it) return;
    await guard(async () => {
      const path = await api.clipSaveImage(it.id, format);
      toast.show(t('clipboard.toast.savedImage', { name: baseName(path) }), 'success');
    });
  }

  async function copyOcr() {
    const it = item;
    if (it && (await guard(() => api.clipCopyOcr(it.id)))) toast.show(t('clipboard.toast.copied', { count: 1 }), 'success', 1500);
  }

  function openLink() {
    if (item && isWebUrl(item.text)) void guard(() => api.openPath(item.text!.trim()));
  }
</script>

<aside class="insp" aria-label={t('clipboard.inspector.label')}>
  {#if item}
    {@const it = item}
    <div class="scroll">
      {#if it.kind === 'image'}
        <button type="button" class="preview" onclick={onpreview} title={t('clipboard.inspector.preview')}>
          {#if src}<img src={src} alt="" draggable="false" />{:else}<ImageIcon size={28} />{/if}
          {#if it.edited}<span class="badge">{t('clipboard.inspector.edited')}</span>{/if}
        </button>
        {#if !trash}
          <div class="row-actions">
            <Button size="sm" onclick={onedit}>{#snippet icon()}<Pencil size={13} />{/snippet}{t('clipboard.action.edit')}</Button>
            {#if it.edited}
              <Button size="sm" variant="plain" onclick={revert}>{#snippet icon()}<Undo2 size={13} />{/snippet}{t('clipboard.action.revert')}</Button>
            {/if}
            <Button size="sm" variant="plain" onclick={() => nav.goto({ view: 'convert', paths: [it.edited ?? it.image ?? ''], url: null })}>
              {#snippet icon()}<RefreshCw size={13} />{/snippet}{t('clipboard.action.convert')}
            </Button>
          </div>
          <div class="row-actions">
            <span class="cap-inline">{t('clipboard.inspector.saveAs')}</span>
            {#each FORMATS as f (f.id)}
              <Button size="sm" variant="plain" onclick={() => saveAs(f.id)}>{f.label}</Button>
            {/each}
          </div>
        {/if}
      {:else}
        <label class="block">
          <span class="cap">{isLink ? t('clipboard.inspector.link') : t('clipboard.inspector.text')}</span>
          <textarea class="area" class:mono={isLink} rows={isLink ? 3 : 7} bind:value={text} disabled={trash} spellcheck="false"
            onfocus={() => (editing = 'text')} onblur={() => commit('text')}></textarea>
        </label>
        {#if isLink}
          {#if it.linkTitle}<p class="link-title">{it.linkTitle}</p>{/if}
          <div class="row-actions">
            {#if isWebUrl(it.text)}
              <Button size="sm" onclick={openLink}>{#snippet icon()}<ExternalLink size={13} />{/snippet}{t('clipboard.action.open')}</Button>
            {/if}
            {#if it.kind === 'mediaLink' && !trash}
              <Button size="sm" variant="primary" onclick={() => nav.goto({ view: 'download', paths: [], url: it.text?.trim() ?? null })}>
                {#snippet icon()}<Download size={13} />{/snippet}{t('clipboard.action.download')}
              </Button>
            {/if}
          </div>
        {/if}
      {/if}

      <div class="block">
        <span class="cap">{t('clipboard.inspector.title')}</span>
        <div class="field" onfocusin={() => (editing = 'title')} onfocusout={() => commit('title')}>
          <TextField bind:value={title} placeholder={t('clipboard.inspector.titlePlaceholder')} disabled={trash} label={t('clipboard.inspector.title')} />
        </div>
      </div>

      <label class="block">
        <span class="cap">{t('clipboard.inspector.note')}</span>
        <textarea class="area" rows="3" bind:value={note} disabled={trash} placeholder={t('clipboard.inspector.notePlaceholder')}
          onfocus={() => (editing = 'note')} onblur={() => commit('note')}></textarea>
      </label>

      <div class="block">
        <span class="cap">{t('clipboard.inspector.tags')}</span>
        <div class="chips">
          {#each it.tags as tag (tag)}
            <span class="chip">
              <span class="chip-name">#{tag}</span>
              {#if !trash}
                <button type="button" class="chip-x" aria-label={t('clipboard.tags.remove', { tag })} title={t('clipboard.tags.remove', { tag })}
                  onclick={() => removeTag(tag)}><X size={11} strokeWidth={2.5} /></button>
              {/if}
            </span>
          {/each}
        </div>
        {#if !trash}
          <TextField bind:value={newTag} placeholder={t('clipboard.tags.placeholder')} label={t('clipboard.tags.add')} onenter={addTag} />
        {/if}
      </div>

      <div class="block">
        <span class="cap">{t('clipboard.inspector.folder')}</span>
        <Select value={it.folder !== null ? String(it.folder) : ''} options={folderOptions} disabled={trash} label={t('clipboard.inspector.folder')}
          onchange={moveTo} />
      </div>

      {#if it.kind === 'image'}
        <div class="block">
          <span class="cap row">
            {t('clipboard.inspector.ocr')}
            {#if it.ocr}<IconButton label={t('clipboard.action.copyOcr')} size={22} onclick={copyOcr}><Copy size={12} /></IconButton>{/if}
          </span>
          {#if it.ocr}
            <p class="ocr">{it.ocr}</p>
          {:else}
            <p class="muted">
              {it.ocr === '' ? t('clipboard.inspector.ocrNone') : settings.value.clipboardOcr ? t('clipboard.inspector.ocrPending') : t('clipboard.inspector.ocrOff')}
            </p>
          {/if}
        </div>
      {/if}

      <dl class="meta">
        <dt>{t('clipboard.inspector.source')}</dt><dd>{it.sourceApp ?? '—'}</dd>
        <dt>{t('clipboard.inspector.created')}</dt><dd>{dateFmt.format(it.createdAt)}</dd>
        {#if it.trashedAt !== null}<dt>{t('clipboard.inspector.trashed')}</dt><dd>{dateFmt.format(it.trashedAt)}</dd>{/if}
        {#if it.kind === 'image'}<dt>{t('clipboard.inspector.dimensions')}</dt><dd>{t('clipboard.imageSize', { width: it.width ?? 0, height: it.height ?? 0 })}</dd>{/if}
        <dt>{t('clipboard.inspector.size')}</dt><dd>{formatBytes(it.size)}</dd>
        <dt>{t('clipboard.inspector.kept')}</dt><dd>{it.saved ? t('clipboard.inspector.keptSaved') : t('clipboard.inspector.keptRetention')}</dd>
      </dl>
    </div>

    <div class="foot">
      {#if trash}
        <Button size="sm" variant="primary" onclick={() => onquick('restore')}>{#snippet icon()}<RotateCcw size={13} />{/snippet}{t('clipboard.action.restore')}</Button>
        <span class="grow"></span>
        <Button size="sm" variant="destructive" onclick={ondelete}>{#snippet icon()}<Trash2 size={13} />{/snippet}{t('clipboard.action.deleteForever')}</Button>
      {:else}
        <Button size="sm" variant="primary" onclick={() => onquick('copy')}>{#snippet icon()}<Copy size={13} />{/snippet}{t('clipboard.action.copy')}</Button>
        <span class="grow"></span>
        <IconButton label={it.pinned ? t('clipboard.action.unpin') : t('clipboard.action.pin')} active={it.pinned} onclick={() => onquick('pin')}>
          {#if it.pinned}<PinOff size={15} />{:else}<Pin size={15} />{/if}
        </IconButton>
        <IconButton label={t('clipboard.action.trash')} onclick={() => onquick('trash')}><Trash2 size={15} /></IconButton>
      {/if}
    </div>
  {:else}
    <div class="none">
      <MousePointerClick size={22} />
      <p>{count > 1 ? t('clipboard.inspector.many', { count }) : t('clipboard.inspector.empty')}</p>
    </div>
  {/if}
</aside>

<style>
  .insp {
    display: flex; flex-direction: column; width: 272px; flex: none; min-height: 0; overflow: hidden;
    border-radius: var(--radius-md); background: var(--card-bg); box-shadow: var(--shadow-sm);
  }
  .scroll { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: var(--space-3); padding: var(--space-3); }
  /* Scroll instead of squeezing the blocks. */
  .scroll > * { flex: none; }
  .preview {
    position: relative; display: grid; place-items: center; width: 100%; height: 180px; padding: 0; border: 0; border-radius: var(--radius-sm);
    background: var(--control-bg); color: var(--text-tertiary); overflow: hidden; cursor: zoom-in;
  }
  .preview img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .badge {
    position: absolute; left: 6px; bottom: 6px; padding: 2px 7px; border-radius: 99px; font: var(--text-caption);
    background: var(--accent); color: var(--on-accent);
  }
  .row-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; margin-top: calc(-1 * var(--space-1)); }
  .cap-inline { font: var(--text-caption); color: var(--text-secondary); margin-right: 2px; }
  .block { display: flex; flex-direction: column; gap: 6px; }
  .cap { font: var(--text-caption); color: var(--text-secondary); }
  .cap.row { display: flex; align-items: center; justify-content: space-between; min-height: 22px; }
  .area {
    width: 100%; resize: vertical; min-height: 56px; padding: 6px 8px; border: 0; border-radius: var(--radius-sm); outline: 0;
    font: var(--text-body); color: var(--text); background: var(--field-bg); cursor: text; user-select: text;
    box-shadow: inset 0 0 0 0.5px var(--hairline), var(--shadow-sm); transition: box-shadow var(--dur-fast);
  }
  .area.mono { font-family: var(--font-mono); font-size: 12px; overflow-wrap: anywhere; }
  .area:focus { box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 40%, transparent), inset 0 0 0 0.5px var(--accent); }
  .area:disabled { opacity: 0.6; }
  .area::placeholder { color: var(--text-tertiary); }
  .link-title { margin: calc(-1 * var(--space-1)) 0 0; font: var(--text-headline); overflow-wrap: anywhere; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chips:empty { display: none; }
  .chip {
    display: inline-flex; align-items: center; gap: 2px; height: 22px; padding: 0 3px 0 8px; border-radius: 999px;
    background: var(--control-bg); font: var(--text-callout);
  }
  .chip-name { max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chip-x {
    display: grid; place-items: center; width: 16px; height: 16px; padding: 0; border: 0; border-radius: 50%;
    background: transparent; color: var(--text-secondary); cursor: default;
  }
  .chip-x:hover { background: var(--control-bg-hover); color: var(--text); }
  .ocr {
    margin: 0; max-height: 140px; overflow-y: auto; padding: 6px 8px; border-radius: var(--radius-sm); background: var(--control-bg);
    font: var(--text-callout); white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; cursor: text;
  }
  .muted { margin: 0; font: var(--text-callout); color: var(--text-tertiary); }
  .meta { display: grid; grid-template-columns: auto 1fr; gap: 4px var(--space-3); margin: 0; font: var(--text-callout); }
  .meta dt { color: var(--text-secondary); }
  .meta dd { margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: right; }
  .foot { flex: none; display: flex; align-items: center; gap: 4px; padding: var(--space-2) var(--space-3); border-top: 0.5px solid var(--separator); }
  .grow { flex: 1; }
  .none { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-2); padding: var(--space-5); color: var(--text-tertiary); text-align: center; }
  .none p { margin: 0; font: var(--text-callout); color: var(--text-secondary); }
</style>
