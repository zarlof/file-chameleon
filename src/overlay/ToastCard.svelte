<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import X from '@lucide/svelte/icons/x';
  import type { ToastView } from './toasts';
  import { t } from './i18n';

  interface Props {
    view: ToastView;
    progress: number | null;
    onaction: (id: string) => void;
    onclose: () => void;
  }
  let { view, progress, onaction, onclose }: Props = $props();
</script>

<div class="card">
  <div class="icon"><Sparkles size={22} strokeWidth={1.8} /></div>
  <div class="text">
    <div class="title">{view.title}</div>
    {#if view.body}<div class="body">{view.body}</div>{/if}
    {#if view.actions.length > 0}
      <div class="actions">
        {#each view.actions.slice(0, 2) as a (a.id)}
          <button type="button" onclick={() => onaction(a.id)}>{a.label}</button>
        {/each}
      </div>
    {/if}
  </div>
  {#if view.thumb}
    <img class="thumb" src={convertFileSrc(view.thumb)} alt="" />
  {/if}
  <button type="button" class="close" aria-label={t('close')} onclick={onclose}>
    <X size={12} strokeWidth={2.4} />
  </button>
  {#if progress !== null}
    <div class="bar"><div class="fill" style:width="{Math.round(Math.min(1, Math.max(0, progress)) * 100)}%"></div></div>
  {/if}
</div>

<style>
  /* The window sits at the bottom right of the work area: the card hugs its bottom edge. */
  .card {
    position: absolute;
    left: 12px;
    bottom: 12px;
    width: 380px;
    box-sizing: border-box;
    min-height: 84px;
    max-height: 126px;
    padding: 14px;
    display: flex;
    gap: 12px;
    align-items: flex-start;
    border-radius: 18px;
    overflow: hidden;
    background: color-mix(in srgb, var(--elevated-bg) 92%, transparent);
    border: 0.5px solid var(--hairline);
    box-shadow: var(--shadow-lg), var(--glass-edge);
    color: var(--text);
    font: var(--text-body);
    animation: slide 220ms var(--ease-spring) both;
  }
  .icon {
    flex: none;
    width: 38px;
    height: 38px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--on-accent);
    background: linear-gradient(160deg, color-mix(in srgb, var(--accent) 80%, white), var(--accent));
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font: var(--text-headline);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .body {
    font: var(--text-callout);
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
  .actions button {
    font: var(--text-caption);
    color: var(--text);
    background: var(--control-bg);
    border: 0;
    border-radius: var(--radius-sm);
    padding: 4px 10px;
    cursor: default;
  }
  .actions button:hover {
    background: var(--control-bg-hover);
  }
  .actions button:active {
    background: var(--control-bg-active);
  }
  .thumb {
    flex: none;
    width: 48px;
    height: 48px;
    border-radius: var(--radius-sm);
    object-fit: cover;
  }
  .close {
    position: absolute;
    left: 6px;
    top: 6px;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border: 0;
    padding: 0;
    border-radius: 50%;
    color: var(--text-secondary);
    background: var(--elevated-bg);
    box-shadow: var(--shadow-sm);
    opacity: 0;
    transition: opacity var(--dur-fast);
    cursor: default;
  }
  .card:hover .close {
    opacity: 1;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: var(--control-bg);
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width var(--dur) var(--ease-out);
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(110%);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }
</style>
