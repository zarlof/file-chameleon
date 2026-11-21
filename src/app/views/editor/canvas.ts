// Image editor model: shapes in image pixels, pure geometry, undo history and drawing.
// The document never copies pixels: a crop is a rect over the original image (`Doc.view`), so
// undo is cheap and only the final save renders at full resolution.

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Tool = 'crop' | 'pen' | 'marker' | 'rect' | 'ellipse' | 'arrow' | 'line' | 'text';

/** Every shape is in image pixels; widths were converted from screen pixels when drawn. */
export type Shape =
  | { type: 'pen' | 'marker'; points: Point[]; color: string; width: number }
  | { type: 'rect' | 'ellipse' | 'line' | 'arrow'; a: Point; b: Point; color: string; width: number }
  | { type: 'text'; at: Point; text: string; color: string; size: number };

export interface Doc {
  shapes: Shape[];
  /** Shown (cropped) part of the original image. */
  view: Rect;
}

/** Ink colours (image data, so fixed hex values rather than theme variables). */
export const SWATCHES = [
  { id: 'red', ink: '#ff3b30' },
  { id: 'orange', ink: '#ff9500' },
  { id: 'yellow', ink: '#ffcc00' },
  { id: 'green', ink: '#34c759' },
  { id: 'blue', ink: '#007aff' },
  { id: 'purple', ink: '#af52de' },
  { id: 'black', ink: '#1c1c1e' },
  { id: 'white', ink: '#ffffff' },
] as const;
/** Stroke widths in screen pixels. */
export const WIDTHS = [2, 4, 6, 10] as const;
/** The marker is wide and translucent. */
export const MARKER_ALPHA = 0.4;
export const MARKER_FACTOR = 4;
/** Text size in screen pixels for a stroke width. */
export const textSize = (width: number) => 10 + width * 4;
export const TEXT_FONT = "600 {size}px 'Inter Variable', 'Segoe UI', sans-serif";

export type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'move';

/** Longest side asked of `readImagePreview` for editing files (the backend downscales larger images to it). */
export const PREVIEW_CAP = 8192;

/** A preview whose longest side is exactly the cap was most likely downscaled: the saved file will be that size. */
export function wasReduced(w: number, h: number, cap = PREVIEW_CAP): boolean {
  return Math.max(w, h) === cap;
}

/** Rect spanned by two corners (positive size). */
export function normRect(a: Point, b: Point): Rect {
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(b.x - a.x), h: Math.abs(b.y - a.y) };
}

/** Ends of the two arrowhead barbs: `size` long, 30° off the shaft, starting at `to`. */
export function arrowHead(from: Point, to: Point, size: number): [Point, Point] {
  const angle = Math.atan2(to.y - from.y, to.x - from.x) || 0;
  const barb = (d: number): Point => ({ x: to.x - size * Math.cos(angle + d), y: to.y - size * Math.sin(angle + d) });
  return [barb(Math.PI / 6), barb(-Math.PI / 6)];
}

/** `rect` (any sign) clamped inside a `w`×`h` image, edges rounded to whole pixels. Size may be 0. */
export function cropToRect(w: number, h: number, rect: Rect): Rect {
  const r = normRect({ x: rect.x, y: rect.y }, { x: rect.x + rect.w, y: rect.y + rect.h });
  const clamp = (v: number, max: number) => Math.min(max, Math.max(0, Math.round(v)));
  const x0 = clamp(r.x, w);
  const y0 = clamp(r.y, h);
  const x1 = clamp(r.x + r.w, w);
  const y1 = clamp(r.y + r.h, h);
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

/** Which crop handle `p` grabs (`tol` = reach in the same units): corners, then edges, then the inside. */
export function hitHandle(rect: Rect, p: Point, tol: number): Handle | null {
  const near = (v: number, edge: number) => Math.abs(v - edge) <= tol;
  const inX = p.x >= rect.x - tol && p.x <= rect.x + rect.w + tol;
  const inY = p.y >= rect.y - tol && p.y <= rect.y + rect.h + tol;
  if (!inX || !inY) return null;
  const l = near(p.x, rect.x);
  const r = near(p.x, rect.x + rect.w);
  const t = near(p.y, rect.y);
  const b = near(p.y, rect.y + rect.h);
  if (t && l) return 'nw';
  if (t && r) return 'ne';
  if (b && r) return 'se';
  if (b && l) return 'sw';
  if (t) return 'n';
  if (b) return 's';
  if (l) return 'w';
  if (r) return 'e';
  return 'move';
}

/** `start` with `handle` dragged by (dx, dy); normalized when an edge passes the opposite one. */
export function dragHandle(start: Rect, handle: Handle, dx: number, dy: number): Rect {
  if (handle === 'move') return { ...start, x: start.x + dx, y: start.y + dy };
  let x0 = start.x;
  let y0 = start.y;
  let x1 = start.x + start.w;
  let y1 = start.y + start.h;
  if (handle.includes('w')) x0 += dx;
  if (handle.includes('e')) x1 += dx;
  if (handle.includes('n')) y0 += dy;
  if (handle.includes('s')) y1 += dy;
  return normRect({ x: x0, y: y0 }, { x: x1, y: y1 });
}

/** CSS resize cursor for a handle. */
export function handleCursor(h: Handle | null): string {
  if (!h) return 'crosshair';
  if (h === 'move') return 'move';
  return h === 'n' || h === 's' ? 'ns-resize' : h === 'e' || h === 'w' ? 'ew-resize' : h === 'nw' || h === 'se' ? 'nwse-resize' : 'nesw-resize';
}

/** Client point -> image pixel, for a canvas showing `view` inside the client `box`. */
export function clientToImage(client: Point, box: { left: number; top: number; width: number; height: number }, view: Rect): Point {
  const sx = box.width > 0 ? view.w / box.width : 1;
  const sy = box.height > 0 ? view.h / box.height : 1;
  return { x: view.x + (client.x - box.left) * sx, y: view.y + (client.y - box.top) * sy };
}

/** Scale that fits `w`×`h` into the box, never above 1. */
export function fitScale(w: number, h: number, boxW: number, boxH: number): number {
  if (w <= 0 || h <= 0 || boxW <= 0 || boxH <= 0) return 1;
  return Math.min(1, boxW / w, boxH / h);
}

/** Drop points closer than `step` to the previous kept one; the last point is always kept. */
export function smoothPath(points: Point[], step: number): Point[] {
  if (points.length < 2) return points.slice();
  const out = [points[0]];
  for (let i = 1; i < points.length - 1; i++) {
    const last = out[out.length - 1];
    if (Math.hypot(points[i].x - last.x, points[i].y - last.y) >= step) out.push(points[i]);
  }
  const end = points[points.length - 1];
  const last = out[out.length - 1];
  if (Math.hypot(end.x - last.x, end.y - last.y) >= step || out.length === 1) out.push(end);
  else out[out.length - 1] = end;
  return out;
}

// ------------------------------------------------------------------ history

export interface History<T> {
  stack: T[];
  index: number;
}

export const history = <T>(first: T): History<T> => ({ stack: [first], index: 0 });

/** Add a state after the current one (drops the redo branch), keeping at most `limit`. */
export function commit<T>(h: History<T>, next: T, limit = 100): History<T> {
  const stack = [...h.stack.slice(0, h.index + 1), next];
  const drop = Math.max(0, stack.length - limit);
  return { stack: stack.slice(drop), index: stack.length - 1 - drop };
}

export const undo = <T>(h: History<T>): History<T> => (h.index > 0 ? { ...h, index: h.index - 1 } : h);
export const redo = <T>(h: History<T>): History<T> => (h.index < h.stack.length - 1 ? { ...h, index: h.index + 1 } : h);

// ------------------------------------------------------------------ drawing

/** Draw one shape (image coordinates: the caller sets the transform). */
export function drawShape(ctx: CanvasRenderingContext2D, s: Shape): void {
  ctx.save();
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  if (s.type === 'text') {
    ctx.fillStyle = s.color;
    ctx.font = TEXT_FONT.replace('{size}', String(s.size));
    ctx.textBaseline = 'top';
    ctx.fillText(s.text, s.at.x, s.at.y);
    ctx.restore();
    return;
  }
  ctx.strokeStyle = s.color;
  ctx.fillStyle = s.color;
  ctx.lineWidth = s.width;
  ctx.beginPath();
  switch (s.type) {
    case 'pen':
    case 'marker': {
      if (s.type === 'marker') ctx.globalAlpha = MARKER_ALPHA;
      const p = s.points;
      if (p.length === 0) break;
      ctx.moveTo(p[0].x, p[0].y);
      if (p.length === 1) ctx.lineTo(p[0].x + 0.01, p[0].y);
      // Quadratic curves through the midpoints: smooth without extra points.
      for (let i = 1; i < p.length - 1; i++) ctx.quadraticCurveTo(p[i].x, p[i].y, (p[i].x + p[i + 1].x) / 2, (p[i].y + p[i + 1].y) / 2);
      if (p.length > 1) ctx.lineTo(p[p.length - 1].x, p[p.length - 1].y);
      ctx.stroke();
      break;
    }
    case 'rect': {
      const r = normRect(s.a, s.b);
      ctx.strokeRect(r.x, r.y, r.w, r.h);
      break;
    }
    case 'ellipse': {
      const r = normRect(s.a, s.b);
      ctx.ellipse(r.x + r.w / 2, r.y + r.h / 2, r.w / 2, r.h / 2, 0, 0, Math.PI * 2);
      ctx.stroke();
      break;
    }
    case 'line':
    case 'arrow': {
      const head = Math.max(10, s.width * 3.5);
      const len = Math.hypot(s.b.x - s.a.x, s.b.y - s.a.y);
      // The shaft stops inside the head so its round cap does not poke through the tip.
      const k = s.type === 'arrow' && len > 0 ? Math.max(0, len - head * 0.6) / len : 1;
      ctx.moveTo(s.a.x, s.a.y);
      ctx.lineTo(s.a.x + (s.b.x - s.a.x) * k, s.a.y + (s.b.y - s.a.y) * k);
      ctx.stroke();
      if (s.type === 'arrow') {
        const [p1, p2] = arrowHead(s.a, s.b, head);
        ctx.beginPath();
        ctx.moveTo(s.b.x, s.b.y);
        ctx.lineTo(p1.x, p1.y);
        ctx.lineTo(p2.x, p2.y);
        ctx.closePath();
        ctx.fill();
      }
      break;
    }
  }
  ctx.restore();
}

/**
 * Crop overlay in image coordinates: dims outside `rect` (within `view`), draws the frame and
 * corner handles. `px` = one screen pixel in image units.
 */
export function drawCropOverlay(ctx: CanvasRenderingContext2D, view: Rect, rect: Rect, px: number): void {
  ctx.save();
  ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
  ctx.beginPath();
  ctx.rect(view.x, view.y, view.w, view.h);
  ctx.rect(rect.x, rect.y, rect.w, rect.h);
  ctx.fill('evenodd');
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 1.5 * px;
  ctx.strokeRect(rect.x, rect.y, rect.w, rect.h);
  // Rule-of-thirds guides.
  ctx.globalAlpha = 0.45;
  ctx.lineWidth = px;
  ctx.beginPath();
  for (const f of [1 / 3, 2 / 3]) {
    ctx.moveTo(rect.x + rect.w * f, rect.y);
    ctx.lineTo(rect.x + rect.w * f, rect.y + rect.h);
    ctx.moveTo(rect.x, rect.y + rect.h * f);
    ctx.lineTo(rect.x + rect.w, rect.y + rect.h * f);
  }
  ctx.stroke();
  ctx.globalAlpha = 1;
  ctx.fillStyle = '#ffffff';
  const s = 8 * px;
  for (const [x, y] of [[rect.x, rect.y], [rect.x + rect.w, rect.y], [rect.x, rect.y + rect.h], [rect.x + rect.w, rect.y + rect.h]]) {
    ctx.fillRect(x - s / 2, y - s / 2, s, s);
  }
  ctx.restore();
}

/** Render the document at full resolution (the shown part only) and return a PNG data URL. */
export function exportPng(image: CanvasImageSource, doc: Doc): string {
  const canvas = document.createElement('canvas');
  canvas.width = doc.view.w;
  canvas.height = doc.view.h;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('canvas');
  ctx.translate(-doc.view.x, -doc.view.y);
  ctx.drawImage(image, 0, 0);
  for (const s of doc.shapes) drawShape(ctx, s);
  const url = canvas.toDataURL('image/png');
  // Release the pixel buffer right away (large images).
  canvas.width = canvas.height = 0;
  return url;
}
