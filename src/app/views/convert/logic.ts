// Pure helpers for the Convert view.
import type {
  CompressLevel, ConvertOptions, InputInfo, JobRequest, Kind, Operation, TargetOption, ToolOp, ToolStatus,
} from '../../../lib/types';

/** Parse "ss", "m:ss", "h:mm:ss" (seconds may have decimals) into milliseconds; null when invalid. */
export function parseTime(text: string): number | null {
  const s = text.trim();
  if (!s) return null;
  const parts = s.split(':');
  if (parts.length > 3) return null;
  let total = 0;
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i];
    const last = i === parts.length - 1;
    if (!(last ? /^\d+(\.\d+)?$/ : /^\d+$/).test(p)) return null;
    // Minutes / seconds fields are below 60 when they are not the leading field.
    if (i > 0 && Number(p) >= 60) return null;
    total = total * 60 + Number(p);
  }
  return Math.round(total * 1000);
}

export function formatTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const ss = String(total % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** Targets grouped by kind, keeping the backend's order. */
export function groupTargets(targets: TargetOption[]): { kind: Kind; targets: TargetOption[] }[] {
  const out: { kind: Kind; targets: TargetOption[] }[] = [];
  for (const t of targets) {
    let g = out.find((x) => x.kind === t.kind);
    if (!g) out.push((g = { kind: t.kind, targets: [] }));
    g.targets.push(t);
  }
  return out;
}

/** Tool ids from `needs` that are currently missing. Unknown tools count as missing. */
export function missingTools(needs: string[], status: ToolStatus[]): string[] {
  return needs.filter((id) => {
    const s = status.find((x) => x.id === id);
    return !s || s.source === 'missing';
  });
}

/** Tools an operation needs beyond the in-process engines. */
export function opNeeds(op: ToolOp, inputs: InputInfo[]): string[] {
  switch (op) {
    case 'trim':
    case 'mute':
    case 'speed':
      return ['ffmpeg'];
    case 'compress':
      return inputs.some((i) => i.kind === 'video' || i.kind === 'audio') ? ['ffmpeg'] : [];
    default:
      return [];
  }
}

export const SPEEDS = [0.5, 1.5, 2] as const;

export interface ToolParams {
  level: CompressLevel;
  start: string;
  end: string;
  speed: number;
  x: string;
  y: string;
  width: string;
  height: string;
}

function nonNeg(s: string): number | null {
  const t = s.trim();
  return /^\d+$/.test(t) ? Number(t) : null;
}

/** Build the operation for a tool, or null when its parameters are invalid / incomplete. */
export function buildToolOp(op: ToolOp, p: ToolParams): Operation | null {
  switch (op) {
    case 'compress':
      return { type: 'compress', level: p.level };
    case 'trim': {
      const startMs = parseTime(p.start);
      const endMs = parseTime(p.end);
      if (startMs === null || endMs === null || endMs <= startMs) return null;
      return { type: 'trim', startMs, endMs };
    }
    case 'speed':
      return { type: 'speed', factor: p.speed };
    case 'mute':
      return { type: 'mute' };
    case 'mergePdf':
      return { type: 'mergePdf' };
    case 'splitPdf':
      return { type: 'splitPdf' };
    case 'crop': {
      const x = nonNeg(p.x);
      const y = nonNeg(p.y);
      const width = nonNeg(p.width);
      const height = nonNeg(p.height);
      if (x === null || y === null || !width || !height) return null;
      return { type: 'crop', x, y, width, height };
    }
  }
}

export interface OptionInputs {
  /** Quality is only sent for image/video targets. */
  useQuality: boolean;
  quality: number;
  maxWidth: string;
  maxHeight: string;
  outputDir: string | null;
}

export function buildOptions(o: OptionInputs): ConvertOptions {
  const dim = (s: string) => nonNeg(s) || null;
  return {
    quality: o.useQuality ? o.quality : null,
    maxWidth: dim(o.maxWidth),
    maxHeight: dim(o.maxHeight),
    outputDir: o.outputDir,
  };
}

export function buildRequest(inputs: string[], op: Operation, options: ConvertOptions): JobRequest {
  return { inputs, op, options };
}
