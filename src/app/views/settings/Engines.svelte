<!--
  Engines — optional external tools (FFmpeg, yt-dlp, Deno, PDFium): where each comes from,
  install / remove a managed copy, and update yt-dlp on demand.
-->
<script lang="ts">
  import { t } from '../../../lib/i18n';
  import { errorOf } from '../../../lib/ipc';
  import { tools } from '../../../lib/stores/tools.svelte';
  import type { ToolStatus } from '../../../lib/types';
  import { Badge, Button, Card, ListRow, ProgressBar, formatBytes, toast } from '../../../lib/ui';

  function purpose(tool: ToolStatus): string {
    const key = `settings.engines.purpose.${tool.id}`;
    const text = t(key);
    return text === key ? tool.purpose : text;
  }
  const badgeTone = { managed: 'accent', system: 'green', missing: 'orange' } as const;

  // yt-dlp breaks when sites change: let the user fetch the newest build right away.
  let updating = $state(false);
  async function updateYtdlp() {
    updating = true;
    try {
      const updated = await tools.updateYtdlp();
      toast.show(t(updated ? 'settings.engines.updated' : 'settings.engines.upToDate', { name: 'yt-dlp' }), 'success');
    } catch (e) {
      toast.show(t('settings.engines.updateFailed', { message: errorOf(e).message }), 'error', 6000);
    } finally {
      updating = false;
    }
  }
</script>

<section>
  <h1>{t('common.nav.engines')}</h1>
  <p class="note">{t('settings.engines.note')}</p>
  <Card>
    {#each tools.list as tool (tool.id)}
      {@const installing = tools.isInstalling(tool.id)}
      <ListRow title={tool.name} subtitle={purpose(tool)}>
        {#snippet trailing()}
          <Badge tone={badgeTone[tool.source]}>{t(`settings.engines.source.${tool.source}`)}</Badge>
          {#if tool.source === 'missing'}
            <Button size="sm" variant="primary" loading={installing} disabled={installing} onclick={() => tools.install(tool.id)}>
              {installing ? t('settings.engines.installing') : `${t('settings.engines.install')} (${formatBytes(tool.size)})`}
            </Button>
          {:else if tool.source === 'system'}
            <!-- A managed copy replaces a broken or outdated one on PATH (managed is used first). -->
            <Button size="sm" loading={installing} disabled={installing} onclick={() => tools.install(tool.id)}>
              {installing ? t('settings.engines.installing') : `${t('settings.engines.installManaged')} (${formatBytes(tool.size)})`}
            </Button>
          {:else if tool.source === 'managed'}
            {#if tool.id === 'yt-dlp'}
              <Button size="sm" loading={updating} disabled={updating || installing} onclick={updateYtdlp}>
                {updating ? t('settings.engines.updating') : t('settings.engines.update')}
              </Button>
            {/if}
            <Button size="sm" disabled={installing || (tool.id === 'yt-dlp' && updating)} onclick={() => tools.uninstall(tool.id)}>{t('settings.engines.remove')}</Button>
          {/if}
        {/snippet}
        <span class="meta">
          {#if tool.version}{t('settings.engines.version', { version: tool.version })} · {/if}{formatBytes(tool.size)} · {t('settings.engines.license', { license: tool.license })}
        </span>
        {#if installing}<div class="bar"><ProgressBar value={tools.progress[tool.id] ?? null} /></div>{/if}
        {#if tools.errors[tool.id]}<span class="err">{t('settings.engines.error', { message: tools.errors[tool.id] })}</span>{/if}
      </ListRow>
    {/each}
  </Card>
</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .note { margin: 0 var(--space-1); font: var(--text-callout); color: var(--text-secondary); }
  .meta { display: block; font: var(--text-callout); color: var(--text-tertiary); }
  .bar { margin-top: 6px; }
  .err { display: block; margin-top: 4px; font: var(--text-callout); color: var(--red); }
</style>
