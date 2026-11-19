<!--
  FileChip — compact file pill: kind icon, name, size, optional remove button.
  Props: name, size?: number (bytes), kind?: Kind | null ('image' | 'vector' | 'video' | 'audio' | 'pdf' | 'data'),
         onremove?() (shows the remove button).
-->
<script lang="ts">
  import { X } from '@lucide/svelte';
  import type { Kind } from '../types';
  import { t } from '../i18n';
  import Icon from './Icon.svelte';
  import { formatBytes } from './format';

  interface Props { name: string; size?: number; kind?: Kind | null; onremove?: () => void }
  let { name, size, kind = null, onremove }: Props = $props();
</script>

<span class="chip" title={name}>
  <span class="ico"><Icon name={kind ?? 'file'} size={14} /></span>
  <span class="name">{name}</span>
  {#if size !== undefined}<span class="size">{formatBytes(size)}</span>{/if}
  {#if onremove}
    <button type="button" class="rm" aria-label={t('common.actions.remove')} onclick={onremove}><X size={12} strokeWidth={2.5} /></button>
  {/if}
</span>

<style>
  .chip { display: inline-flex; align-items: center; gap: 6px; max-width: 260px; height: 28px; padding: 0 6px 0 9px; border-radius: 8px; background: var(--card-bg); box-shadow: var(--shadow-sm); font: var(--text-callout); }
  .ico { display: inline-flex; color: var(--accent); }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .size { flex: none; color: var(--text-tertiary); }
  .rm { display: grid; place-items: center; flex: none; width: 18px; height: 18px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-secondary); cursor: default; }
  .rm:hover { background: var(--control-bg-hover); color: var(--text); }
  .chip:not(:has(.rm)) { padding-right: 9px; }
</style>
