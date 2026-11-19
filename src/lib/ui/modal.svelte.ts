// Counts open modal Sheets so global handlers (e.g. the Shell's file drop) can stand down.
export const modal = $state({ open: 0 });

export function modalOpened() {
  modal.open++;
}

export function modalClosed() {
  modal.open = Math.max(0, modal.open - 1);
}
