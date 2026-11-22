<!--
  About — app name and version, the GitHub repository and third-party licences.
-->
<script lang="ts">
  import appIcon from '../../../../src-tauri/icons/128x128.png';
  import { t } from '../../../lib/i18n';
  import { api } from '../../../lib/ipc';
  import { settings } from '../../../lib/stores/settings.svelte';
  import { Button, Card, guard, ListRow } from '../../../lib/ui';

  const REPO_URL = 'https://github.com/zarlof31/file-chameleon';
  const NOTICES_URL = `${REPO_URL}/blob/main/THIRD_PARTY_NOTICES.md`;

  function openUrl(url: string) {
    void guard(() => api.openPath(url));
  }
</script>

<section>
  <h1>{t('common.nav.about')}</h1>

  <Card padded>
    <div class="about">
      <img class="logo" src={appIcon} alt="" width="64" height="64" draggable="false" />
      <div class="about-name">{t('common.appName')}</div>
      {#if settings.appInfo}<div class="about-ver">{t('settings.about.version')} {settings.appInfo.version}</div>{/if}
    </div>
  </Card>

  <Card>
    <ListRow title={t('settings.about.repo')} subtitle={t('settings.about.repoValue')}>
      {#snippet trailing()}<Button size="sm" onclick={() => openUrl(REPO_URL)}>{t('settings.about.open')}</Button>{/snippet}
    </ListRow>
    <ListRow title={t('settings.about.licenses')}>
      {#snippet trailing()}<Button size="sm" onclick={() => openUrl(NOTICES_URL)}>{t('settings.about.licensesOpen')}</Button>{/snippet}
      <span class="body">{t('settings.about.licensesValue')}</span>
    </ListRow>
  </Card>

</section>

<style>
  section { display: flex; flex-direction: column; gap: var(--space-3); max-width: 860px; padding: 0 var(--space-6) var(--space-6); }
  h1 { margin: 0 0 var(--space-1); font: var(--text-large-title); letter-spacing: var(--tracking-tight); }
  .about { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: var(--space-3) 0; }
  .logo { width: 64px; height: 64px; margin-bottom: var(--space-2); object-fit: contain; }
  .about-name { font: var(--text-title); letter-spacing: var(--tracking-tight); }
  .about-ver { font: var(--text-callout); color: var(--text-secondary); }
  .body { display: block; font: var(--text-callout); color: var(--text-secondary); white-space: normal; }
</style>
