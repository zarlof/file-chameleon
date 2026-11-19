<!--
  Toast — host for in-window transient messages. Mount once near the app root; fire with
  `import { toast } from 'src/lib/ui'; toast.show(message, 'info' | 'success' | 'error')`.
  No props.
-->
<script lang="ts">
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import Icon from './Icon.svelte';
  import { toast } from './toaster.svelte';

  const icon = { info: 'info', success: 'circle-check', error: 'circle-alert' } as const;
</script>

<div class="host" aria-live="polite">
  {#each toast.items as item (item.id)}
    <button type="button" class="toast {item.kind}" onclick={() => toast.dismiss(item.id)}
      transition:fly={{ y: 16, duration: 260, easing: cubicOut }}>
      <Icon name={icon[item.kind]} size={16} />
      <span>{item.message}</span>
    </button>
  {/each}
</div>

<style>
  .host { position: fixed; left: 50%; bottom: 20px; z-index: 200; display: flex; flex-direction: column; gap: 8px; align-items: center; transform: translateX(-50%); pointer-events: none; }
  .toast {
    display: inline-flex; align-items: center; gap: 8px; max-width: 420px; padding: 9px 14px 9px 12px; border: 0; border-radius: 99px;
    font: var(--text-body); color: var(--text); text-align: left; background: var(--elevated-bg);
    backdrop-filter: blur(24px) saturate(1.6); box-shadow: var(--shadow-lg), var(--glass-edge); pointer-events: auto; cursor: default;
  }
  .info :global(svg) { color: var(--accent); }
  .success :global(svg) { color: var(--green); }
  .error :global(svg) { color: var(--red); }
</style>
