<!--
  EmptyState — centered placeholder.
  Props: title, message?, icon?: Snippet (rendered large in a soft tile), action?: Snippet (e.g. a Button).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props { title: string; message?: string; icon?: Snippet; action?: Snippet }
  let { title, message, icon, action }: Props = $props();
</script>

<div class="empty">
  {#if icon}<div class="tile">{@render icon()}</div>{/if}
  <h2>{title}</h2>
  {#if message}<p>{message}</p>{/if}
  {#if action}<div class="act">{@render action()}</div>{/if}
</div>

<style>
  .empty { display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 6px; padding: var(--space-8); min-height: 100%; animation: rise var(--dur-slow) var(--ease-out); }
  .tile { display: grid; place-items: center; width: 56px; height: 56px; margin-bottom: 8px; border-radius: 16px; color: var(--accent); background: var(--accent-soft); }
  .tile :global(svg) { width: 28px; height: 28px; }
  h2 { margin: 0; font: var(--text-title); letter-spacing: var(--tracking-tight); }
  p { margin: 0; max-width: 340px; font: var(--text-body); color: var(--text-secondary); }
  .act { margin-top: 12px; }
  @keyframes rise { from { opacity: 0; transform: translateY(6px); } }
</style>
