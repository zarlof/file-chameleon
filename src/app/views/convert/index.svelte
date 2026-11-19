<!-- Placeholder: replaced by the convert view task. Models the nav.pending consumption contract. -->
<script lang="ts">
  import { t } from '../../../lib/i18n';
  import { nav } from '../../../lib/stores/nav.svelte';

  let paths = $state<string[]>([]);

  // Re-runs whenever nav.pending changes, even if this view is already mounted.
  $effect(() => {
    const p = nav.pending && nav.consume('convert');
    if (p) paths = p.paths;
  });
</script>

<section>
  <h1>{t('common.nav.convert')}</h1>
  {#if paths.length > 0}<p>{t('common.files', { count: paths.length })}</p>{/if}
</section>

<style>
  section { padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0; font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  p { color: var(--text-secondary); }
</style>
