// Pure helpers for the presets page: what an operation does in words, the default operation of
// each action kind, duplicates, usage counts and number fields.
import type { ConvertOptions, ImageOp, Operation, Preset, WheelConfig } from '../../../lib/types';
import { presetLabel, type Translate } from '../../../lib/wheel/labels';
import { folderName, formatLabel, newPresetId, OUTPUT_FORMATS } from '../wheel/logic';

/** Actions a user preset can have (trim / crop rectangles and the editor are per file). */
export type ActionKind =
  | 'convert' | 'edit' | 'cropAspect' | 'compress' | 'speed' | 'mute' | 'mergePdf' | 'splitPdf' | 'zip' | 'extract' | 'copyTo';
export const ACTION_KINDS: ActionKind[] = [
  'convert', 'edit', 'cropAspect', 'compress', 'speed', 'mute', 'mergePdf', 'splitPdf', 'zip', 'extract', 'copyTo',
];

export const STEP_TYPES: ImageOp['type'][] = [
  'grayscale', 'rotateLeft', 'rotateRight', 'rotate180', 'flipH', 'flipV', 'trim', 'pixelate', 'blur', 'scale',
];

/** Speed factors offered (the core allows 0.25..4). */
export const SPEEDS = [0.25, 0.5, 0.75, 1.25, 1.5, 2, 3, 4];

/** Raster formats an edit can write. */
export const EDIT_TARGETS = OUTPUT_FORMATS.filter((f) => f.kind === 'image');

/** Parameter ranges, as `ImageOp::validate` / `Operation::validate` in the core. */
export const RANGES = {
  block: [2, 256],
  sigma: [0.5, 50],
  percent: [1, 400],
  aspect: [1, 1000],
  size: [1, 100000],
} as const;

export const EMPTY_OPTIONS: ConvertOptions = { quality: null, maxWidth: null, maxHeight: null, outputDir: null, outputName: null };

export function defaultStep(type: ImageOp['type']): ImageOp {
  switch (type) {
    case 'pixelate':
      return { type, block: 12 };
    case 'blur':
      return { type, sigma: 6 };
    case 'scale':
      return { type, percent: 50 };
    default:
      return { type };
  }
}

export function defaultOp(kind: ActionKind): Operation {
  switch (kind) {
    case 'convert':
      return { type: 'convert', to: 'png' };
    case 'edit':
      return { type: 'edit', ops: [{ type: 'grayscale' }], to: null };
    case 'cropAspect':
      return { type: 'cropAspect', w: 16, h: 9 };
    case 'compress':
      return { type: 'compress', level: 'balanced' };
    case 'speed':
      return { type: 'speed', factor: 2 };
    case 'copyTo':
      return { type: 'copyTo', dir: null, moveFiles: false };
    default:
      return { type: kind };
  }
}

/** Quality and max size apply to conversions and edits; the output folder to all but copy/move. */
export function usesImageOptions(op: Operation): boolean {
  return op.type === 'convert' || op.type === 'edit';
}

/** The preset with the default operation of `kind` (unchanged when it already has that kind). */
export function withOp(p: Preset, kind: ActionKind): Preset {
  if (p.op.type === kind) return p;
  const op = defaultOp(kind);
  const options: ConvertOptions = { ...EMPTY_OPTIONS, ...p.options };
  if (!usesImageOptions(op)) Object.assign(options, { quality: null, maxWidth: null, maxHeight: null });
  if (op.type === 'copyTo') options.outputDir = null;
  return { ...p, op, options };
}

/** Keeps an automatic name (the description of `before`'s operation) in step with the operation. */
export function withName(next: Preset, before: Preset, t: Translate): Preset {
  const auto = !before.name || before.name === describeOp(before.op, t);
  return auto ? { ...next, name: describeOp(next.op, t) } : next;
}

export function describeStep(s: ImageOp, t: Translate): string {
  switch (s.type) {
    case 'pixelate':
      return t('presetsPage.step.pixelate', { block: s.block });
    case 'blur':
      return t('presetsPage.step.blur', { sigma: s.sigma });
    case 'scale':
      return t('presetsPage.step.scale', { percent: s.percent });
    default:
      return t(`presetsPage.step.${s.type}`);
  }
}

/** What an operation does, in one line ("Convert to JPG", "Grayscale · Same format", "Crop 16:9"). */
export function describeOp(op: Operation, t: Translate): string {
  switch (op.type) {
    case 'convert':
      return t('presetsPage.op.convert', { to: formatLabel(op.to) });
    case 'edit': {
      const steps = op.ops.length ? op.ops.map((s) => describeStep(s, t)).join(', ') : t('presetsPage.op.noSteps');
      const to = op.to ? formatLabel(op.to) : t('presetsPage.op.sameFormat');
      return t('presetsPage.op.edit', { steps, to });
    }
    case 'cropAspect':
      return t('presetsPage.op.cropAspect', { w: op.w, h: op.h });
    case 'copyTo':
      if (!op.dir) return t(op.moveFiles ? 'presetsPage.op.moveAsk' : 'presetsPage.op.copyAsk');
      return t(op.moveFiles ? 'presetsPage.op.moveTo' : 'presetsPage.op.copyTo', { folder: folderName(op.dir) });
    case 'compress':
      return t('presetsPage.op.compress', { level: t(`presetsPage.level.${op.level}`) });
    case 'speed':
      return t('presetsPage.op.speed', { factor: op.factor });
    case 'crop':
      return t('presetsPage.op.crop', { width: op.width, height: op.height });
    default:
      return t(`presetsPage.op.${op.type}`);
  }
}

/** `describeOp` plus the quality and max size options a conversion or edit uses. */
export function describePreset(p: Preset, t: Translate): string {
  const parts = [describeOp(p.op, t)];
  if (usesImageOptions(p.op)) {
    const { quality, maxWidth: w = null, maxHeight: h = null } = p.options;
    if (quality != null) parts.push(t('presetsPage.qualityValue', { q: quality }));
    if (w !== null || h !== null) {
      parts.push(t('presetsPage.maxSizeValue', { size: w !== null && h !== null && w !== h ? `${w}×${h}` : `${w ?? h}` }));
    }
  }
  return parts.join(' · ');
}

/** An editable copy of any preset, with a fresh "u-<n>" id and a "… copy" name. */
export function duplicatePreset(p: Preset, existingIds: Iterable<string>, t: Translate): Preset {
  const copy = JSON.parse(JSON.stringify(p)) as Preset;
  return { ...copy, id: newPresetId(existingIds), name: t('presetsPage.copyName', { name: presetLabel(p, t) }), builtin: false };
}

/** How many wheel profiles and groups use a preset. */
export function presetUsage(c: WheelConfig, id: string): { profiles: number; groups: number } {
  return {
    profiles: c.profiles.filter((p) => p.front.includes(id) || p.back.includes(id)).length,
    groups: c.groups.filter((g) => g.presets.includes(id)).length,
  };
}

/** Number in [min, max] ("0,5" works too); null when empty, not a number, out of range, or not
 *  an integer when `integer`. */
export function parseNumber(s: string, min: number, max: number, integer = false): number | null {
  const v = s.trim().replace(',', '.');
  if (!/^\d+(\.\d+)?$/.test(v)) return null;
  const n = Number(v);
  if (integer && !Number.isInteger(n)) return null;
  return n >= min && n <= max ? n : null;
}
