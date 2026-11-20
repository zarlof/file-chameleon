<!--
  Clipboard settings — history on/off, what is kept, suggestions, history size and image retention.
  (Extended with the full clipboard options by a later task.)
-->
<script lang="ts">
  import { t } from '../../../lib/i18n';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { Settings } from '../../../lib/types';
  import { Card, ListRow, Select, Toggle } from '../../../lib/ui';

  const s = $derived(settings.value);
  const off = $derived(!s.clipboardEnabled);

  function set(patch: Partial<Settings>) {
    void settings.update(patch);
  }

  /** Select options for a numeric setting; keeps an unusual stored value selectable. */
  function numOptions(values: number[], current: number, label: (n: number) => string) {
    const all = values.includes(current) ? values : [...values, current].sort((a, b) => a - b);
    return all.map((n) => ({ value: String(n), label: label(n) }));
  }
  const itemOptions = $derived(numOptions([50, 100, 200, 500, 1000], s.clipboardMaxItems, (n) => t('settings.clipboard.items', { count: n })));
  const dayOptions = $derived(numOptions([1, 7, 14, 30, 90, 365], s.clipboardRetentionImages, (n) => t('settings.clipboard.days', { count: n })));
</script>

<section>
  <h1>{t('common.nav.clipboardSettings')}</h1>

  <Card>
    <ListRow title={t('settings.clipboard.enable')} subtitle={t('settings.clipboard.enableHint')}>
      {#snippet trailing()}<Toggle checked={s.clipboardEnabled} label={t('settings.clipboard.enable')} onchange={(v) => set({ clipboardEnabled: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.images')}>
      {#snippet trailing()}<Toggle checked={s.clipboardImages} disabled={off} label={t('settings.clipboard.images')} onchange={(v) => set({ clipboardImages: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.suggestLinks')}>
      {#snippet trailing()}<Toggle checked={s.clipboardSuggestLinks} disabled={off} label={t('settings.clipboard.suggestLinks')} onchange={(v) => set({ clipboardSuggestLinks: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.suggestImages')}>
      {#snippet trailing()}<Toggle checked={s.clipboardSuggestImages} disabled={off} label={t('settings.clipboard.suggestImages')} onchange={(v) => set({ clipboardSuggestImages: v })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.maxItems')}>
      {#snippet trailing()}<Select value={String(s.clipboardMaxItems)} options={itemOptions} disabled={off} label={t('settings.clipboard.maxItems')} onchange={(v) => set({ clipboardMaxItems: Number(v) })} />{/snippet}
    </ListRow>
    <ListRow title={t('settings.clipboard.retention')}>
      {#snippet trailing()}<Select value={String(s.clipboardRetentionImages)} options={dayOptions} disabled={off} label={t('settings.clipboard.retention')} onchange={(v) => set({ clipboardRetentionImages: Number(v) })} />{/snippet}
    </ListRow>
  </Card>
</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
</style>
