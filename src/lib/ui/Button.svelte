<!--
  Button
  Props: variant = 'secondary' ('primary' | 'secondary' | 'plain' | 'destructive'), size = 'md' ('sm' | 'md' | 'lg'),
         loading = false (shows a spinner, blocks clicks), disabled, type = 'button', title, onclick,
         icon?: Snippet (leading icon), children: Snippet (label).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Spinner from './Spinner.svelte';

  interface Props {
    variant?: 'primary' | 'secondary' | 'plain' | 'destructive';
    size?: 'sm' | 'md' | 'lg';
    loading?: boolean;
    disabled?: boolean;
    type?: 'button' | 'submit';
    title?: string;
    onclick?: (e: MouseEvent) => void;
    icon?: Snippet;
    children?: Snippet;
  }
  let { variant = 'secondary', size = 'md', loading = false, disabled = false, type = 'button', title, onclick, icon, children }: Props = $props();
</script>

<button class="btn {variant} {size}" {type} {title} disabled={disabled || loading} aria-busy={loading || undefined}
  onclick={(e) => !loading && onclick?.(e)}>
  {#if loading}
    <Spinner size={size === 'lg' ? 16 : 13} />
  {:else if icon}
    <span class="icon">{@render icon()}</span>
  {/if}
  {#if children}<span class="label">{@render children()}</span>{/if}
</button>

<style>
  .btn {
    display: inline-flex; align-items: center; justify-content: center; gap: 6px;
    height: var(--control-h); padding: 0 12px; border: 0; border-radius: var(--radius-sm);
    font: var(--text-body); font-weight: 500; white-space: nowrap; cursor: default;
    color: var(--text); background: var(--control-bg);
    transition: background var(--dur-fast) var(--ease-out), transform var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out), opacity var(--dur-fast);
  }
  .btn.sm { height: 24px; padding: 0 9px; font: var(--text-callout); font-weight: 500; }
  .btn.lg { height: var(--control-h-lg); padding: 0 18px; border-radius: 8px; font-size: 14px; }
  .btn:active:not(:disabled) { transform: scale(0.97); }
  .btn:disabled { opacity: 0.45; }
  .icon { display: inline-flex; }
  .icon :global(svg) { width: 15px; height: 15px; }

  .secondary { background: var(--card-bg-hover); box-shadow: var(--shadow-sm); }
  .secondary:hover:not(:disabled) { background: var(--elevated-bg); }
  .secondary:active:not(:disabled) { background: var(--control-bg-active); }

  .primary {
    color: var(--on-accent);
    background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 92%, white), var(--accent));
    box-shadow: 0 1px 2px color-mix(in srgb, var(--accent) 40%, transparent), inset 0 0.5px 0 rgba(255, 255, 255, 0.35);
  }
  .primary:hover:not(:disabled) { background: var(--accent-hover); }

  .plain { background: transparent; color: var(--accent); }
  .plain:hover:not(:disabled) { background: var(--control-bg); }
  .plain:active:not(:disabled) { background: var(--control-bg-hover); }

  .destructive { color: var(--red); background: color-mix(in srgb, var(--red) 13%, transparent); }
  .destructive:hover:not(:disabled) { background: color-mix(in srgb, var(--red) 22%, transparent); }
</style>
