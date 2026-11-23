<!--
  Translate — the live screen translation: each translated paragraph sits on a dark card over the
  original text, sized to its box; a small pill at the top says what is going on. The window is
  click-through and never focused; the hotkey or Esc (handled in Rust) ends it.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, tick } from 'svelte';
  import { api } from '../lib/ipc';
  import { setLang, t } from '../line/i18n';
  import { fitText } from './fit';

  type Shown = { x: number; y: number; w: number; h: number; lineH: number; text: string };
  type View = { status: 'working' | 'live' | 'empty' | 'error'; blocks: Shown[]; target: string | null };

  let view = $state<View | null>(null);
  let boxes: HTMLElement[] = $state([]);

  const pill = $derived(
    !view
      ? ''
      : view.status === 'working'
        ? t('translate.working')
        : view.status === 'empty'
          ? t('translate.empty')
          : view.status === 'error'
            ? t('translate.error')
            : t('translate.live', { lang: t(view.target === 'en' ? 'translate.toEn' : view.target === 'tr' ? 'translate.toTr' : 'translate.both') }),
  );

  async function show(v: View | null) {
    view = v;
    await tick();
    view?.blocks.forEach((b, i) => {
      const el = boxes[i];
      if (el) fitText(el, b.lineH, Math.max(b.h * 1.25, b.lineH * 1.3));
    });
  }

  onMount(() => {
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const [s, info] = await Promise.all([api.getSettings(), api.appInfo()]);
        setLang(s.language, info.systemLanguage);
      } catch {
        /* English */
      }
      stop = await listen<View>('translate://view', (e) => void show(e.payload));
      await show(await invoke<View | null>('translate_view'));
    })();
    return () => stop?.();
  });
</script>

{#if view}
  {#each view.blocks as b, i (i)}
    <div class="box" bind:this={boxes[i]} style:left="{b.x - 3}px" style:top="{b.y - 2}px" style:width="{b.w + 6}px" style:min-height="{b.h + 4}px">{b.text}</div>
  {/each}
  {#if pill}<div class="pill" class:bad={view.status === 'error'}>{pill}</div>{/if}
{/if}

<style>
  .box {
    position: absolute;
    box-sizing: border-box;
    padding: 1px 3px;
    border-radius: 4px;
    background: rgba(18, 18, 22, 0.9);
    color: #fff;
    font-family: 'Segoe UI', system-ui, sans-serif;
    line-height: 1.18;
    overflow-wrap: anywhere;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
  }
  .pill {
    position: absolute;
    top: 10px;
    left: 50%;
    transform: translateX(-50%);
    padding: 5px 14px;
    border-radius: 999px;
    background: rgba(18, 18, 22, 0.88);
    color: #fff;
    font: 600 12px 'Segoe UI', system-ui, sans-serif;
    white-space: nowrap;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.35);
  }
  .pill.bad { background: rgba(170, 30, 40, 0.92); }
</style>
