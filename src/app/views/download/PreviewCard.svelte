<!--
  PreviewCard — what the checked link contains: thumbnail, title, site, duration, resolution, size.
  Props: info (MediaInfo).
-->
<script lang="ts">
  import { Badge, Card, Icon, formatBytes } from '../../../lib/ui';
  import { t } from '../../../lib/i18n';
  import type { MediaInfo } from '../../../lib/types';
  import { formatDuration, resolutionLabel, safeThumbnail } from './helpers';

  let { info }: { info: MediaInfo } = $props();

  let thumbFailed = $state(false);
  const thumb = $derived(thumbFailed ? null : safeThumbnail(info.thumbnail));
  const icon = $derived(info.kind === 'audio' ? 'music' : info.kind === 'image' ? 'image' : 'film');
  const facts = $derived(
    [
      formatDuration(info.durationSecs),
      resolutionLabel(info.width, info.height),
      info.sizeBytes ? t('download.preview.size', { size: formatBytes(info.sizeBytes) }) : '',
    ].filter(Boolean),
  );

  // A new link gets a fresh chance to load its thumbnail.
  $effect(() => {
    void info.thumbnail;
    thumbFailed = false;
  });
</script>

<Card padded>
  <div class="preview">
    <div class="thumb">
      {#if thumb}
        <img src={thumb} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={() => (thumbFailed = true)} />
      {:else}
        <Icon name={icon} size={26} />
      {/if}
    </div>
    <div class="meta">
      <div class="badges">
        {#if info.site}<Badge tone="accent">{info.site}</Badge>{/if}
        <Badge>{t(`download.preview.kind.${info.kind}`)}</Badge>
      </div>
      <h3 title={info.title}>{info.title || t('download.preview.untitled')}</h3>
      {#if info.uploader}<p class="by">{t('download.preview.by', { name: info.uploader })}</p>{/if}
      {#if facts.length > 0}<p class="facts">{facts.join(' · ')}</p>{/if}
    </div>
  </div>
</Card>

<style>
  .preview { display: flex; gap: var(--space-4); align-items: center; }
  .thumb {
    flex: none; display: grid; place-items: center; width: 144px; aspect-ratio: 16 / 9; overflow: hidden;
    border-radius: var(--radius-sm); background: var(--control-bg); color: var(--text-tertiary);
  }
  .thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .meta { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .badges { display: flex; gap: 6px; flex-wrap: wrap; }
  h3 {
    margin: 0; font: var(--text-headline); display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2;
    -webkit-box-orient: vertical; overflow: hidden; overflow-wrap: anywhere;
  }
  p { margin: 0; font: var(--text-callout); color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
