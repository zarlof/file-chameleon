import { errorOf } from '../ipc';
import { toast } from './toaster.svelte';

/** Run `action`; a failure becomes an error toast instead of vanishing. Resolves to false on
 *  failure. Use it for anything the user clicked (open, reveal, cancel...). */
export async function guard(action: () => Promise<unknown>): Promise<boolean> {
  try {
    await action();
    return true;
  } catch (e) {
    toast.show(errorOf(e).message, 'error');
    return false;
  }
}
