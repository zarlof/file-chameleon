<!--
  PreviewSheet — Space preview of one item: the full image, the whole text, or a link with its title.
  Props: open (bindable), item (null closes nothing; shows nothing).
-->
<script lang="ts">
  import { t } from '../../../lib/i18n';
  import type { ClipItem } from '../../../lib/types';
  import { Sheet } from '../../../lib/ui';
  import { clipLabel } from './clips';
  import { imageUrl } from './ui';

  interface Props { open: boolean; item: ClipItem | null }
  let { open = $bindable(), item }: Props = $props();

  const isLink = $derived(item?.kind === 'link' || item?.kind === 'mediaLink');
  const src = $derived(item?.kind === 'image' ? imageUrl(item) : null);
  const heading = $derived(item ? (item.title ?? (isLink ? item.linkTitle : null) ?? t('clipboard.preview.title')) : '');
</script>

<Sheet bind:open title={heading} width={760}>
  {#if item}
    {#if item.kind === 'image'}
      <div class="img">{#if src}<img src={src} alt={clipLabel(item) ?? ''} draggable="false" />{/if}</div>
    {:else}
      <pre class="text" class:url={isLink}>{item.text}</pre>
    {/if}
    {#if item.note}<p class="note">{item.note}</p>{/if}
    <p class="hint">{t('clipboard.preview.hint')}</p>
  {/if}
</Sheet>

<style>
  .img { display: grid; place-items: center; max-height: 64vh; overflow: hidden; border-radius: var(--radius-sm); background: var(--control-bg); }
  .img img { max-width: 100%; max-height: 64vh; object-fit: contain; }
  .text {
    margin: 0; max-height: 60vh; overflow: auto; padding: var(--space-3); border-radius: var(--radius-sm); background: var(--control-bg);
    font: var(--text-body); font-family: var(--font-mono); white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; cursor: text;
  }
  .text.url { color: var(--accent); }
  .note { margin: var(--space-3) 0 0; color: var(--text-secondary); white-space: pre-wrap; }
  .hint { margin: var(--space-2) 0 0; font: var(--text-caption); color: var(--text-tertiary); }
</style>
