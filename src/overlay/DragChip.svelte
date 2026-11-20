<!--
  DragChip — small file card that follows the pointer while files are dragged over the wheel:
  kind icon (or the thumbnail once it arrives) and the extension badge. Window coordinates.
-->
<script lang="ts">
  import type { Kind } from '../lib/types';
  import { slotIcon } from '../lib/wheel/icons';

  interface Props {
    x: number;
    y: number;
    /** Lowercase extension of the first file. */
    ext: string | null;
    kind: Kind | null;
    /** Data URL of the first image's thumbnail. */
    thumb: string | null;
    count: number;
  }
  let { x, y, ext, kind, thumb, count }: Props = $props();

  const tones: Record<Kind, string> = {
    image: 'var(--orange)',
    vector: 'var(--teal)',
    video: 'var(--indigo)',
    audio: 'var(--pink)',
    pdf: 'var(--red)',
    data: 'var(--green)',
    subtitle: 'var(--blue)',
    archive: 'var(--graphite)',
  };
  const Icon = $derived(slotIcon(kind));
</script>

<div class="chip" class:stack={count > 1} style:left="{x + 16}px" style:top="{y + 18}px" style:--tone={kind ? tones[kind] : 'var(--graphite)'} aria-hidden="true">
  <div class="card">
    {#if thumb}
      <img src={thumb} alt="" />
    {:else}
      <Icon size={20} strokeWidth={1.7} />
    {/if}
  </div>
  {#if ext}<span class="badge">{ext.toUpperCase()}</span>{/if}
</div>

<style>
  .chip {
    --chip-ink: #ffffff;
    position: fixed;
    width: 44px;
    height: 52px;
    pointer-events: none;
  }
  .card {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding-bottom: 8px;
    overflow: hidden;
    border-radius: 8px;
    background: var(--elevated-bg);
    color: var(--tone);
    box-shadow: var(--shadow-md);
  }
  /* Several files: a second card peeks out behind. */
  .stack::before {
    content: '';
    position: absolute;
    inset: 0;
    transform: translate(4px, 4px) rotate(4deg);
    border-radius: 8px;
    background: var(--elevated-bg);
    box-shadow: var(--shadow-sm);
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .badge {
    position: absolute;
    left: -5px;
    bottom: 6px;
    max-width: 52px;
    padding: 1px 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-radius: var(--radius-xs);
    background: var(--tone);
    color: var(--chip-ink);
    font: 700 9px/12px var(--font);
    letter-spacing: 0.03em;
  }
</style>
