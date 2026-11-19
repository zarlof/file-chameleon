// Counts open modal Sheets so global handlers (e.g. the Shell's file drop) can stand down.
export const modal = $state({ open: 0 });
