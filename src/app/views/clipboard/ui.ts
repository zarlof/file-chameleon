// UI glue shared by the library components: asset URLs for stored images, error toasts.
import { convertFileSrc } from '@tauri-apps/api/core';
import { errorOf } from '../../../lib/ipc';
import type { ClipItem } from '../../../lib/types';
import { toast } from '../../../lib/ui';

/** `asset:` URL of a local file (the scope covers the clipboard store); null outside Tauri. */
export function assetUrl(path: string | null | undefined): string | null {
  if (!path) return null;
  try {
    return convertFileSrc(path);
  } catch {
    return null;
  }
}

/** Thumbnail to show (edited copy first). */
export const thumbUrl = (c: ClipItem) => assetUrl(c.editedThumb ?? c.thumb ?? c.edited ?? c.image);
/** Full image to show (edited copy first). */
export const imageUrl = (c: ClipItem) => assetUrl(c.edited ?? c.image);

/** Run `action`; a failure becomes an error toast. Resolves to false on failure. */
export async function guard(action: () => Promise<unknown>): Promise<boolean> {
  try {
    await action();
    return true;
  } catch (e) {
    toast.show(errorOf(e).message, 'error');
    return false;
  }
}

/** Only web links are opened in the browser (copied text could name anything). */
export const isWebUrl = (s: string | null | undefined) => !!s && /^https?:\/\/\S+$/i.test(s.trim());
