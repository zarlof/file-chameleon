// SPIKE main entry (replaced by Task D's shell).
import { getCurrentWindow } from '@tauri-apps/api/window';

document.getElementById('app')!.innerHTML =
  '<div data-tauri-drag-region style="height:100vh;display:grid;place-items:center;font:600 20px system-ui">File Chameleon</div>';
getCurrentWindow().show();
