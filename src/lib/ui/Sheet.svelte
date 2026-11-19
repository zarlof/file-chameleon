<!--
  Sheet — modal panel over a dimmed backdrop (Esc / backdrop click closes).
  Props: open (bindable), title?, width = 420 (px), onclose?(), children: Snippet, footer?: Snippet (button row).
-->
<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import { modalClosed, modalOpened } from './modal.svelte';
  interface Props { open: boolean; title?: string; width?: number; onclose?: () => void; children?: Snippet; footer?: Snippet }
  let { open = $bindable(), title, width = 420, onclose, children, footer }: Props = $props();

  $effect(() => {
    if (!open) return;
    // untrack: the effect must depend only on `open`, not on the shared counter it mutates.
    untrack(modalOpened);
    return () => untrack(modalClosed);
  });

  function close() {
    open = false;
    onclose?.();
  }
  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onkeydown={(e) => open && e.key === 'Escape' && close()} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={(e) => e.target === e.currentTarget && close()}>
    <div class="panel" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" style:width="min({width}px, calc(100% - 48px))" use:focusOnMount>
      {#if title}<h2>{title}</h2>{/if}
      <div class="body">{@render children?.()}</div>
      {#if footer}<div class="foot">{@render footer()}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 100; display: grid; place-items: center; background: rgba(0, 0, 0, 0.28); animation: fade var(--dur) var(--ease-out); }
  .panel {
    display: flex; flex-direction: column; gap: var(--space-3); max-height: calc(100% - 48px); padding: var(--space-5);
    border-radius: var(--radius-lg); background: var(--elevated-bg); box-shadow: var(--shadow-lg), var(--glass-edge);
    backdrop-filter: blur(30px) saturate(1.6); outline: 0; animation: pop var(--dur-slow) var(--ease-spring);
  }
  h2 { margin: 0; font: var(--text-title); letter-spacing: var(--tracking-tight); }
  .body { overflow: auto; font: var(--text-body); }
  .foot { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-1); }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: scale(0.94) translateY(8px); } }
</style>
