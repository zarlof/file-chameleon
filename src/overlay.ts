// SPIKE overlay (replaced by Task H): proves drag-enter/over/drop reach the wheel window.
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { api, on } from './lib/ipc';
import type { TargetOption } from './lib/types';

const root = document.getElementById('overlay')!;
let paths: string[] = [];
let slices: TargetOption[] = [];
let hovered = -1;

function render(msg = '') {
  const n = slices.length;
  root.innerHTML = `
    <div style="position:fixed;inset:20px;border-radius:50%;background:rgba(30,30,32,.82);
      color:#fff;font:600 13px system-ui;display:grid;place-items:center;box-shadow:0 10px 40px rgba(0,0,0,.35)">
      <div style="position:absolute;inset:0">
        ${slices
          .map((s, i) => {
            const a = (i / n) * 2 * Math.PI - Math.PI / 2;
            const x = 50 + Math.cos(a) * 36, y = 50 + Math.sin(a) * 36;
            return `<div style="position:absolute;left:${x}%;top:${y}%;transform:translate(-50%,-50%);
              padding:6px 10px;border-radius:10px;background:${i === hovered ? '#0a84ff' : 'rgba(255,255,255,.12)'}">${s.label}</div>`;
          })
          .join('')}
      </div>
      <div>${msg || `${paths.length} file(s)`}</div>
    </div>`;
}

function hit(x: number, y: number): number {
  const cx = window.innerWidth / 2, cy = window.innerHeight / 2;
  const dx = x - cx, dy = y - cy;
  if (Math.hypot(dx, dy) < 60 || slices.length === 0) return -1;
  let a = Math.atan2(dy, dx) + Math.PI / 2;
  if (a < 0) a += 2 * Math.PI;
  return Math.round((a / (2 * Math.PI)) * slices.length) % slices.length;
}

on.overlayWheel(() => {
  paths = [];
  slices = [];
  hovered = -1;
  render('…');
});
on.overlayWheelTargets((t) => {
  paths = t.paths;
  slices = t.targets.slice(0, 8);
  console.log('targets', t);
  render();
});
on.overlayHide(() => (root.innerHTML = ''));

getCurrentWebview().onDragDropEvent((e) => {
  const p = e.payload;
  console.log('dd', p.type, 'position' in p ? p.position : '');
  if (p.type === 'over' || p.type === 'enter') {
    const r = window.devicePixelRatio || 1;
    hovered = hit(p.position.x / r, p.position.y / r);
    render();
  } else if (p.type === 'drop') {
    const r = window.devicePixelRatio || 1;
    const i = hit(p.position.x / r, p.position.y / r);
    if (i >= 0) api.wheelDrop(p.paths, slices[i].id).catch(console.error);
    else api.overlayHide();
  }
});

api.overlayReady();
