// In-window transient messages. Usage: `toast.show('Saved', 'success')`; render <Toast /> once (the Shell does).
export type ToastKind = 'info' | 'success' | 'error';
export interface ToastItem { id: number; message: string; kind: ToastKind }

class ToastStore {
  items = $state<ToastItem[]>([]);
  private next = 1;

  show(message: string, kind: ToastKind = 'info', timeoutMs = 3500): number {
    const id = this.next++;
    this.items = [...this.items.slice(-3), { id, message, kind }];
    if (timeoutMs > 0) setTimeout(() => this.dismiss(id), timeoutMs);
    return id;
  }

  dismiss(id: number) {
    this.items = this.items.filter((i) => i.id !== id);
  }
}

export const toast = new ToastStore();
