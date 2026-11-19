<!--
  TextField — text input with optional leading icon and clear button.
  Props: value (bindable string), placeholder, type = 'text', disabled, label (aria-label), clearable = true,
         leading?: Snippet (icon), onenter?(), oninput?(value), size = 'md' | 'lg'.
-->
<script lang="ts">
  import { X } from '@lucide/svelte';
  import type { Snippet } from 'svelte';
  import { t } from '../i18n';

  interface Props {
    value: string;
    placeholder?: string;
    type?: 'text' | 'url' | 'search' | 'password';
    disabled?: boolean;
    label?: string;
    clearable?: boolean;
    leading?: Snippet;
    size?: 'md' | 'lg';
    onenter?: () => void;
    oninput?: (value: string) => void;
  }
  let { value = $bindable(), placeholder, type = 'text', disabled = false, label, clearable = true, leading, size = 'md', onenter, oninput }: Props = $props();
  let input: HTMLInputElement | undefined = $state();
</script>

<label class="field {size}" class:disabled>
  {#if leading}<span class="lead">{@render leading()}</span>{/if}
  <input bind:this={input} bind:value {type} {placeholder} {disabled} aria-label={label ?? placeholder} spellcheck="false"
    oninput={() => oninput?.(value)} onkeydown={(e) => e.key === 'Enter' && onenter?.()} />
  {#if clearable && value && !disabled}
    <button type="button" class="clear" aria-label={t('common.actions.clear')}
      onclick={() => { value = ''; oninput?.(''); input?.focus(); }}>
      <X size={11} strokeWidth={2.75} />
    </button>
  {/if}
</label>

<style>
  .field {
    display: flex; align-items: center; gap: 6px; height: var(--control-h); padding: 0 8px; border-radius: var(--radius-sm);
    background: var(--field-bg); box-shadow: inset 0 0 0 0.5px var(--hairline), var(--shadow-sm);
    transition: box-shadow var(--dur-fast) var(--ease-out);
  }
  .field.lg { height: var(--control-h-lg); padding: 0 11px; border-radius: 8px; }
  .field.disabled { opacity: 0.5; }
  .field:focus-within { box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 40%, transparent), inset 0 0 0 0.5px var(--accent); }
  .lead { display: inline-flex; color: var(--text-tertiary); }
  .lead :global(svg) { width: 14px; height: 14px; }
  input { flex: 1; min-width: 0; height: 100%; border: 0; outline: 0; background: transparent; font: var(--text-body); color: var(--text); cursor: text; user-select: text; }
  input::placeholder { color: var(--text-tertiary); }
  .clear {
    display: grid; place-items: center; flex: none; width: 15px; height: 15px; padding: 0; border: 0; border-radius: 50%;
    background: var(--text-tertiary); color: var(--content-bg); cursor: default;
  }
  .clear:hover { background: var(--text-secondary); }
</style>
