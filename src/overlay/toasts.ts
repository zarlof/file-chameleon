// Maps a ToastPayload (data, not text) to localized title/body/actions.
import type { ToastPayload } from '../lib/types';
import type { T } from './i18n';

export interface ToastAction {
  id: string;
  label: string;
}

export interface ToastView {
  title: string;
  body: string;
  actions: ToastAction[];
  /** Local file path of an optional thumbnail. */
  thumb: string | null;
}

export function shortenUrl(url: string, max = 44): string {
  const s = url.replace(/^[a-z]+:\/\//i, '').replace(/^www\./i, '').replace(/\/$/, '');
  return s.length > max ? s.slice(0, max - 1) + '…' : s;
}

function baseName(p: unknown): string {
  return typeof p === 'string' ? (p.split(/[\\/]/).pop() ?? p) : '';
}

export function toastView(p: ToastPayload, t: T): ToastView {
  const d = p.data ?? {};
  const a = (id: string): ToastAction => ({ id, label: t(`action.${id}`) });
  switch (p.kind) {
    case 'jobDone': {
      const out = Array.isArray(d.outputs) ? d.outputs[0] : undefined;
      return {
        title: t('toast.jobDone.title'),
        body: baseName(out) || String(d.inputName ?? ''),
        actions: [a('reveal'), a('open')],
        thumb: null,
      };
    }
    case 'jobFailed': {
      const code = String(d.errorCode ?? 'failed');
      const key = `error.${code}`;
      const translated = t(key);
      const reason = translated === key ? t('error.failed') : translated;
      const name = String(d.inputName ?? '');
      return {
        title: t('toast.jobFailed.title'),
        body: name ? `${name} · ${reason}` : reason,
        actions: [a('details')],
        thumb: null,
      };
    }
    case 'jobsBatch': {
      const done = Number(d.done ?? 0);
      const failed = Number(d.failed ?? 0);
      return {
        title: t('toast.jobsBatch.title', { done, count: done }),
        body: failed > 0 ? t('toast.jobsBatch.failed', { failed }) : t('toast.jobsBatch.ok'),
        actions: [a('reveal')],
        thumb: null,
      };
    }
    case 'toolNeeded': {
      const size = Number(d.size ?? 0);
      const mb = size > 0 ? Math.max(1, Math.round(size / 1_000_000)) : 0;
      return {
        title: t('toast.toolNeeded.title', { name: String(d.name ?? d.tool ?? '') }),
        body: mb > 0 ? t('toast.toolNeeded.body', { size: mb }) : t('toast.toolNeeded.bodyUnknown'),
        actions: [a('install'), a('dismiss')],
        thumb: null,
      };
    }
    case 'toolInstalling':
      return {
        title: t('toast.toolInstalling.title', { name: String(d.name ?? d.tool ?? '') }),
        body: t('toast.toolInstalling.body'),
        actions: [],
        thumb: null,
      };
    case 'toolFailed': {
      const code = String(d.errorCode ?? 'failed');
      const translated = t(`error.${code}`);
      return {
        title: t('toast.toolFailed.title', { name: String(d.name ?? d.tool ?? '') }),
        body: translated === `error.${code}` ? t('error.failed') : translated,
        actions: [],
        thumb: null,
      };
    }
    case 'clipLink': {
      const site = String(d.site ?? '');
      return {
        title: site ? t('toast.clipLink.title', { site }) : t('toast.clipLink.titleNoSite'),
        body: shortenUrl(String(d.url ?? '')),
        actions: [a('download'), a('dismiss')],
        thumb: null,
      };
    }
    case 'clipImage':
      return {
        title: t('toast.clipImage.title'),
        body: `${d.width ?? '?'}×${d.height ?? '?'}`,
        actions: [a('savePng'), a('convert')],
        thumb: typeof d.thumb === 'string' ? d.thumb : null,
      };
    case 'imageSaved':
      return {
        title: t('toast.imageSaved.title'),
        body: baseName(d.output),
        actions: [a('reveal'), a('open')],
        thumb: null,
      };
    case 'downloadDone':
      return {
        title: t('toast.downloadDone.title'),
        body: baseName(d.output) || String(d.title ?? ''),
        actions: [a('reveal'), a('open')],
        thumb: null,
      };
    case 'noLink':
      return { title: t('toast.noLink.title'), body: '', actions: [], thumb: null };
    case 'noPinned':
      return { title: t('toast.noPinned.title', { n: String(d.n ?? '') }), body: t('toast.noPinned.body'), actions: [], thumb: null };
    case 'textCopied':
      return { title: t('toast.textCopied.title'), body: t('toast.textCopied.body', { chars: String(d.chars ?? 0) }), actions: [], thumb: null };
    case 'noText':
      return { title: t('toast.noText.title'), body: '', actions: [], thumb: null };
    case 'updateAvailable':
      return { title: t('toast.updateAvailable.title', { version: String(d.version ?? '') }), body: t('toast.updateAvailable.body'), actions: [a('install'), a('dismiss')], thumb: null };
    case 'updateInstalling':
      return { title: t('toast.updateInstalling.title', { version: String(d.version ?? '') }), body: t('toast.updateInstalling.body'), actions: [], thumb: null };
    case 'updateFailed':
      return { title: t('toast.updateFailed.title'), body: t('toast.updateFailed.body'), actions: [], thumb: null };
    default:
      return { title: String(p.kind), body: '', actions: [], thumb: null };
  }
}
