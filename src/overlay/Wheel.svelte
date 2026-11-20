<script lang="ts">
  import type { Component } from 'svelte';
  import ImageIcon from '@lucide/svelte/icons/image';
  import PenTool from '@lucide/svelte/icons/pen-tool';
  import Film from '@lucide/svelte/icons/film';
  import Music from '@lucide/svelte/icons/music';
  import FileText from '@lucide/svelte/icons/file-text';
  import Table from '@lucide/svelte/icons/table';
  import Download from '@lucide/svelte/icons/download';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Ban from '@lucide/svelte/icons/ban';
  import type { Kind, TargetOption } from '../lib/types';
  import { INNER_R, MORE_ID, OUTER_R, WHEEL_SIZE, needsDownload, polar, sliceCenterAngle, slicePath } from '../lib/wheel/geometry';
  import { t } from './i18n';

  // Mounted only once the dragged files are known (see Overlay.svelte), so non-file drags never
  // show anything.
  interface Props {
    slices: TargetOption[];
    hovered: number;
    fileCount: number;
    firstKind: Kind | null;
    /** Tools that are not installed: their slices show a small download glyph. */
    missingTools: string[];
  }
  let { slices, hovered, fileCount, firstKind, missingTools }: Props = $props();

  const icons: Record<Kind, Component<{ size?: number; strokeWidth?: number }>> = {
    image: ImageIcon,
    vector: PenTool,
    video: Film,
    audio: Music,
    pdf: FileText,
    data: Table,
  };

  const n = $derived(slices.length);
  const labelR = (INNER_R + OUTER_R) / 2;
  const CenterIcon = $derived(firstKind ? icons[firstKind] : null);
</script>

<div class="wheel" style:width="{WHEEL_SIZE}px" style:height="{WHEEL_SIZE}px">
  <div class="disc"></div>

  <svg width={WHEEL_SIZE} height={WHEEL_SIZE} viewBox="0 0 {WHEEL_SIZE} {WHEEL_SIZE}" aria-hidden="true">
    {#each slices as _, i (i)}
      <path d={slicePath(i, n)} class="slice" class:hot={i === hovered} />
    {/each}
  </svg>

  {#each slices as s, i (s.id)}
    {@const pos = polar(sliceCenterAngle(i, n), labelR)}
    {@const Icon = s.id === MORE_ID ? Ellipsis : icons[s.kind]}
    <div class="label" class:hot={i === hovered} style:left="{pos.x}px" style:top="{pos.y}px">
      <Icon size={18} strokeWidth={1.8} />
      <span class="fmt">{s.id === MORE_ID ? t('more') : s.label}</span>
      {#if needsDownload(s, missingTools)}
        <span class="tool"><Download size={10} strokeWidth={2.4} /></span>
      {/if}
    </div>
  {/each}

  <div class="center" class:empty={n === 0}>
    {#if n === 0}
      <Ban size={22} strokeWidth={1.6} />
      <span class="count">{t('notSupported')}</span>
    {:else}
      {#if CenterIcon}<CenterIcon size={22} strokeWidth={1.6} />{/if}
      <span class="count">{t('files', { n: fileCount, count: fileCount })}</span>
    {/if}
  </div>
</div>

<style>
  .wheel {
    position: relative;
    font: var(--text-caption);
    color: var(--text);
    animation: appear 180ms var(--ease-spring) both;
  }
  .disc {
    position: absolute;
    left: 20px;
    top: 20px;
    right: 20px;
    bottom: 20px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--elevated-bg) 92%, transparent);
    border: 0.5px solid var(--hairline);
    box-shadow: var(--shadow-lg), var(--glass-edge);
  }
  svg {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .slice {
    fill: var(--control-bg);
    transform-origin: 220px 220px;
    transition:
      fill var(--dur-fast) var(--ease-out),
      transform var(--dur) var(--ease-spring);
  }
  .slice.hot {
    fill: var(--accent);
    transform: scale(1.045);
  }
  .label {
    position: absolute;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    pointer-events: none;
    transition:
      transform var(--dur) var(--ease-spring),
      color var(--dur-fast);
  }
  .label.hot {
    color: var(--on-accent);
    transform: translate(-50%, -50%) scale(1.14);
  }
  .fmt {
    font-weight: 600;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    max-width: 70px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tool {
    position: absolute;
    right: -10px;
    top: -4px;
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--control-bg-active);
    color: var(--text-secondary);
  }
  .center {
    position: absolute;
    left: calc(50% - 56px);
    top: calc(50% - 56px);
    width: 112px;
    height: 112px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    text-align: center;
    color: var(--text-secondary);
    pointer-events: none;
  }
  .center.empty {
    color: var(--text-tertiary);
  }
  .count {
    font: var(--text-caption);
    color: var(--text);
  }
  .empty .count {
    color: var(--text-secondary);
  }
  @keyframes appear {
    from {
      opacity: 0;
      transform: scale(0.85);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
</style>
