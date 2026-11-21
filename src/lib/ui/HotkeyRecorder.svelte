<!--
  HotkeyRecorder — click, then press a combination to record a global hotkey ("Ctrl+Alt+V").
  Esc cancels, Backspace / Delete clears (= off). Needs Ctrl, Alt or Win (F1–F24 alone are fine).
  Props: value ("" = off), onchange(value), status? (false = the backend could not register it: conflict),
         disabled, label (aria-label).
-->
<script lang="ts">
  import { currentLang, t } from '../i18n';
  import { formatHotkey, fromKeyboardEvent, heldModifiers } from '../hotkeys';

  interface Props { value: string; onchange: (v: string) => void; status?: boolean; disabled?: boolean; label?: string }
  let { value, onchange, status, disabled = false, label }: Props = $props();

  let recording = $state(false);
  let held = $state<string[]>([]);

  const parts = $derived(formatHotkey(value, 'en').split('+').filter(Boolean));
  const spoken = $derived(value ? formatHotkey(value, currentLang()) : t('settings.hotkeys.recorder.off'));

  function stop() {
    recording = false;
    held = [];
  }

  function click(e: MouseEvent) {
    // Keyboard activation (Enter / Space) while recording is a key to record, not a toggle.
    if (recording && e.detail === 0) return;
    if (recording) stop();
    else recording = true;
  }

  function keydown(e: KeyboardEvent) {
    const plain = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
    if (!recording) {
      if (plain && (e.key === 'Backspace' || e.key === 'Delete') && value) {
        e.preventDefault();
        onchange('');
      }
      return;
    }
    if (plain && e.key === 'Tab') {
      stop(); // let focus move on
      return;
    }
    // Keep the keys away from the page (Ctrl+1..4 navigation, the library's shortcuts).
    e.preventDefault();
    e.stopPropagation();
    if (plain && e.key === 'Escape') return stop();
    if (plain && (e.key === 'Backspace' || e.key === 'Delete')) {
      onchange('');
      return stop();
    }
    const combo = fromKeyboardEvent(e);
    if (combo) {
      stop();
      if (combo !== value) onchange(combo);
    } else {
      held = heldModifiers(e);
    }
  }

  function keyup(e: KeyboardEvent) {
    if (recording) held = heldModifiers(e);
  }
</script>

<button type="button" class="rec" class:recording class:conflict={status === false && !recording} {disabled}
  aria-label={label ? `${label}: ${spoken}` : spoken} aria-pressed={recording}
  onclick={click} onkeydown={keydown} onkeyup={keyup} onblur={stop}>
  {#if recording}
    {#if held.length}
      {#each held as k (k)}<kbd>{k}</kbd>{/each}<span class="more">…</span>
    {:else}
      <span class="prompt">{t('settings.hotkeys.recorder.recording')}</span>
    {/if}
  {:else if parts.length}
    {#each parts as k, i (i)}<kbd>{k}</kbd>{/each}
  {:else}
    <span class="off">{t('settings.hotkeys.recorder.off')}</span>
  {/if}
</button>

<style>
  .rec {
    display: inline-flex; align-items: center; justify-content: center; gap: 4px; min-width: 132px; height: var(--control-h); padding: 0 8px;
    border: 0; border-radius: var(--radius-sm); font: var(--text-body); color: var(--text); background: var(--card-bg-hover);
    box-shadow: var(--shadow-sm); cursor: default; transition: box-shadow var(--dur-fast), background var(--dur-fast);
  }
  .rec:hover:not(:disabled) { background: var(--elevated-bg); }
  .rec:disabled { opacity: 0.45; }
  .rec.recording { background: var(--accent-soft); box-shadow: 0 0 0 2px var(--accent); }
  .rec.conflict { box-shadow: 0 0 0 1.5px var(--red); }
  kbd {
    padding: 1px 6px; border-radius: var(--radius-xs); background: var(--control-bg); box-shadow: inset 0 -1px 0 var(--hairline);
    font: var(--text-caption); font-family: var(--font); color: var(--text);
  }
  .prompt { color: var(--accent); font: var(--text-callout); }
  .more { color: var(--text-tertiary); }
  .off { color: var(--text-tertiary); }
</style>
