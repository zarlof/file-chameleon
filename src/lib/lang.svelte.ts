// Reactive holder for the active UI language (kept separate so i18n.ts stays importable everywhere).
export type Lang = 'tr' | 'en';
export const langState = $state<{ current: Lang }>({ current: 'en' });
