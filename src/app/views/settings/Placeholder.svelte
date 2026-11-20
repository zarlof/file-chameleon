<!--
  Placeholder — stands in for a page that a later task builds. Props: title.
-->
<script lang="ts">
  import { Construction } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';

  let { title }: { title: string } = $props();
</script>

<section>
  <h1>{title}</h1>
  <div class="soon">
    <Construction size={28} strokeWidth={1.5} />
    <p>{t('common.placeholder')}</p>
  </div>
</section>

<style>
  section { padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-4); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .soon { display: flex; flex-direction: column; align-items: center; gap: var(--space-3); padding: var(--space-8) 0; color: var(--text-tertiary); }
  p { margin: 0; font: var(--text-body); color: var(--text-secondary); }
</style>
