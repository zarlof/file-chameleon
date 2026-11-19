// Overlay entry: wheel + toasts in one tiny transparent always-on-top window.
import { mount } from 'svelte';
import './lib/ui/theme.css';
import { api, on } from './lib/ipc';
import type { Settings } from './lib/types';
import Overlay from './overlay/Overlay.svelte';
import { setLang } from './overlay/i18n';

const root = document.documentElement;
const dark = window.matchMedia('(prefers-color-scheme: dark)');
let theme: Settings['theme'] = 'system';

function applyTheme() {
  root.dataset.theme = theme === 'system' ? (dark.matches ? 'dark' : 'light') : theme;
}

function apply(s: Settings, systemLanguage: 'tr' | 'en') {
  theme = s.theme;
  root.dataset.accent = s.accent;
  setLang(s.language, systemLanguage);
  applyTheme();
}

dark.addEventListener('change', applyTheme);
applyTheme();

let systemLanguage: 'tr' | 'en' = 'en';

async function init() {
  try {
    const [s, info] = await Promise.all([api.getSettings(), api.appInfo()]);
    systemLanguage = info.systemLanguage;
    apply(s, systemLanguage);
  } catch (e) {
    console.error(e);
  }
  on.settingsChanged((s) => apply(s, systemLanguage));
  mount(Overlay, { target: document.getElementById('overlay')! });
}

void init();
