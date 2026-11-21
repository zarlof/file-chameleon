<!--
  PresetEditor — form for one preset: name, action, then the action's settings (target format,
  edit steps with parameters, aspect, strength, speed, copy/move folder), quality and max size
  for conversions and edits, and the output folder. Controlled: every valid change calls
  onchange(next); number fields keep what is typed and apply it only when it is in range.
  Remount it (`{#key preset.id}`) to edit another preset.
  Props: preset, readonly = false (built-ins: everything disabled), onchange?(preset).
-->
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { Plus, X } from '@lucide/svelte';
  import { t } from '../../../lib/i18n';
  import { settings } from '../../../lib/stores/settings.svelte';
  import type { CompressLevel, ConvertOptions, ImageOp, Operation, Preset } from '../../../lib/types';
  import { Button, Card, IconButton, ListRow, SegmentedControl, Select, Slider, TextField, Toggle } from '../../../lib/ui';
  import { presetLabel } from '../../../lib/wheel/labels';
  import { formatLabel, OUTPUT_FORMATS } from '../wheel/logic';
  import {
    ACTION_KINDS, defaultStep, describeOp, EDIT_TARGETS, parseNumber, RANGES, SPEEDS, STEP_TYPES, usesImageOptions, withName,
    withOp, type ActionKind,
  } from './logic';

  interface Props {
    preset: Preset;
    readonly?: boolean;
    onchange?: (p: Preset) => void;
  }
  let { preset, readonly = false, onchange }: Props = $props();

  type Range = readonly [number, number];

  /** Text typed into number fields, by field key (shown as typed; applied only when valid). */
  let drafts = $state<Record<string, string>>({});
  let nameDraft = $state<string | null>(null);

  const op = $derived(preset.op);
  const o = $derived(preset.options);

  const kinds = $derived(
    [...ACTION_KINDS, ...((ACTION_KINDS as string[]).includes(op.type) ? [] : [op.type])].map((k) => ({
      value: k,
      label: t(`presetsPage.kind.${k}`),
    })),
  );
  const targets = $derived.by(() => {
    const list = OUTPUT_FORMATS.map((f) => ({ value: f.id, label: f.label }));
    if (op.type === 'convert' && !list.some((f) => f.value === op.to)) list.push({ value: op.to, label: formatLabel(op.to) });
    return list;
  });
  const editTargets = $derived([
    { value: '', label: t('presetsPage.field.sameFormat') },
    ...EDIT_TARGETS.map((f) => ({ value: f.id, label: f.label })),
  ]);
  const stepTypes = $derived(STEP_TYPES.map((s) => ({ value: s, label: t(`presetsPage.stepType.${s}`) })));
  const levels = $derived(
    (['light', 'balanced', 'strong'] as const).map((l) => ({ value: l, label: t(`presetsPage.level.${l}`) })),
  );
  const speeds = $derived.by(() => {
    const list = [...SPEEDS];
    if (op.type === 'speed' && !list.includes(op.factor)) list.push(op.factor);
    return list.sort((a, b) => a - b).map((f) => ({ value: String(f), label: `${f}×` }));
  });

  function emit(next: Preset) {
    if (!readonly) onchange?.(next);
  }
  function setOp(next: Operation) {
    emit(withName({ ...preset, op: next }, preset, t));
  }
  function setOptions(patch: Partial<ConvertOptions>) {
    emit({ ...preset, options: { ...preset.options, ...patch } });
  }
  function setKind(kind: string) {
    drafts = {};
    nameDraft = null;
    emit(withName(withOp(preset, kind as ActionKind), preset, t));
  }
  function setName(v: string) {
    nameDraft = v;
    emit({ ...preset, name: v.trim() || describeOp(preset.op, t) });
  }

  function onNum(key: string, s: string, range: Range, integer: boolean, optional: boolean, apply: (n: number | null) => void) {
    drafts[key] = s;
    if (optional && !s.trim()) return apply(null);
    const n = parseNumber(s, range[0], range[1], integer);
    if (n !== null) apply(n);
  }

  function editOps(): ImageOp[] {
    return op.type === 'edit' ? op.ops : [];
  }
  function setSteps(ops: ImageOp[]) {
    if (op.type === 'edit') setOp({ ...op, ops });
  }
  function setStep(i: number, step: ImageOp) {
    setSteps(editOps().map((s, j) => (j === i ? step : s)));
  }
  function clearStepDrafts() {
    for (const k of Object.keys(drafts)) if (k.startsWith('step-')) delete drafts[k];
  }
  function setStepType(i: number, type: string) {
    clearStepDrafts();
    setStep(i, defaultStep(type as ImageOp['type']));
  }
  function removeStep(i: number) {
    clearStepDrafts();
    setSteps(editOps().filter((_, j) => j !== i));
  }

  async function pickFolder(apply: (dir: string) => void) {
    try {
      const r = await open({ directory: true });
      if (typeof r === 'string') apply(r);
    } catch {
      /* dialog unavailable */
    }
  }
  function setCopyDir(dir: string | null) {
    if (preset.op.type === 'copyTo') setOp({ ...preset.op, dir });
  }
</script>

{#snippet num(key: string, value: number | null | undefined, range: Range, integer: boolean, optional: boolean, label: string, apply: (n: number | null) => void)}
  {@const text = drafts[key] ?? (value == null ? '' : String(value))}
  {@const bad = drafts[key] !== undefined && !(optional && !text.trim()) && parseNumber(text, range[0], range[1], integer) === null}
  <span class="num" class:bad title={bad ? t('presetsPage.field.range', { min: range[0], max: range[1] }) : undefined}>
    <TextField value={text} {label} placeholder={optional ? '—' : undefined} clearable={false} disabled={readonly}
      oninput={(s) => onNum(key, s, range, integer, optional, apply)} />
  </span>
{/snippet}

<div class="editor">
  <Card>
    <ListRow title={t('presetsPage.field.name')}>
      {#snippet trailing()}
        <span class="wide">
          <TextField value={readonly ? presetLabel(preset, t) : (nameDraft ?? preset.name)} placeholder={describeOp(op, t)}
            label={t('presetsPage.field.name')} clearable={false} disabled={readonly} oninput={setName} />
        </span>
      {/snippet}
    </ListRow>
    <ListRow title={t('presetsPage.field.action')}>
      {#snippet trailing()}
        <Select value={op.type} options={kinds} disabled={readonly} label={t('presetsPage.field.action')} onchange={setKind} />
      {/snippet}
    </ListRow>
  </Card>

  {#if op.type === 'convert'}
    <Card>
      <ListRow title={t('presetsPage.field.target')}>
        {#snippet trailing()}
          <Select value={op.to} options={targets} disabled={readonly} label={t('presetsPage.field.target')}
            onchange={(to) => setOp({ type: 'convert', to })} />
        {/snippet}
      </ListRow>
    </Card>
  {:else if op.type === 'edit'}
    {@const edit = op}
    <Card padded>
      <div class="steps-head">
        <span>{t('presetsPage.field.steps')}</span>
        <span class="hint">{t('presetsPage.field.stepsHint')}</span>
      </div>
      <ol class="steps">
        {#each edit.ops as s, i (i)}
          <li>
            <span class="n">{i + 1}</span>
            <Select value={s.type} options={stepTypes} disabled={readonly} label={t('presetsPage.field.steps')}
              onchange={(v) => setStepType(i, v)} />
            {#if s.type === 'pixelate'}
              {@render num(`step-${i}`, s.block, RANGES.block, true, false, t('presetsPage.field.stepValue'), (n) => n !== null && setStep(i, { type: 'pixelate', block: n }))}
            {:else if s.type === 'blur'}
              {@render num(`step-${i}`, s.sigma, RANGES.sigma, false, false, t('presetsPage.field.stepValue'), (n) => n !== null && setStep(i, { type: 'blur', sigma: n }))}
            {:else if s.type === 'scale'}
              {@render num(`step-${i}`, s.percent, RANGES.percent, true, false, t('presetsPage.field.stepValue'), (n) => n !== null && setStep(i, { type: 'scale', percent: n }))}
            {/if}
            {#if !readonly}
              <IconButton label={t('presetsPage.field.removeStep')} size={24} onclick={() => removeStep(i)}><X size={13} /></IconButton>
            {/if}
          </li>
        {/each}
      </ol>
      {#if !readonly}
        <Button size="sm" variant="plain" onclick={() => setSteps([...edit.ops, defaultStep('grayscale')])}>
          {#snippet icon()}<Plus size={14} />{/snippet}
          {t('presetsPage.field.addStep')}
        </Button>
      {/if}
    </Card>
    <Card>
      <ListRow title={t('presetsPage.field.editTarget')}>
        {#snippet trailing()}
          <Select value={edit.to ?? ''} options={editTargets} disabled={readonly} label={t('presetsPage.field.editTarget')}
            onchange={(to) => setOp({ ...edit, to: to || null })} />
        {/snippet}
      </ListRow>
    </Card>
  {:else if op.type === 'cropAspect'}
    {@const crop = op}
    <Card>
      <ListRow title={t('presetsPage.field.aspect')}>
        {#snippet trailing()}
          <span class="pair">
            {@render num('aspect-w', crop.w, RANGES.aspect, true, false, t('presetsPage.field.aspectWidth'), (n) => n !== null && setOp({ ...crop, w: n }))}
            <span class="sep">:</span>
            {@render num('aspect-h', crop.h, RANGES.aspect, true, false, t('presetsPage.field.aspectHeight'), (n) => n !== null && setOp({ ...crop, h: n }))}
          </span>
        {/snippet}
      </ListRow>
    </Card>
  {:else if op.type === 'compress'}
    <Card>
      <ListRow title={t('presetsPage.field.level')}>
        {#snippet trailing()}
          <SegmentedControl options={levels} value={op.level} size="sm" disabled={readonly}
            onchange={(level: CompressLevel) => setOp({ type: 'compress', level })} />
        {/snippet}
      </ListRow>
    </Card>
  {:else if op.type === 'speed'}
    <Card>
      <ListRow title={t('presetsPage.field.speed')}>
        {#snippet trailing()}
          <Select value={String(op.factor)} options={speeds} disabled={readonly} label={t('presetsPage.field.speed')}
            onchange={(v) => setOp({ type: 'speed', factor: Number(v) })} />
        {/snippet}
      </ListRow>
    </Card>
  {:else if op.type === 'copyTo'}
    {@const copy = op}
    <Card>
      <ListRow title={t('presetsPage.field.folder')} subtitle={copy.dir ?? t('presetsPage.field.askEachTime')}>
        {#snippet trailing()}
          {#if !readonly}
            {#if copy.dir}<Button size="sm" variant="plain" onclick={() => setCopyDir(null)}>{t('presetsPage.field.askEachTime')}</Button>{/if}
            <Button size="sm" onclick={() => pickFolder(setCopyDir)}>{t('presetsPage.field.choose')}</Button>
          {/if}
        {/snippet}
      </ListRow>
      <ListRow title={t('presetsPage.field.move')} subtitle={t('presetsPage.field.moveHint')}>
        {#snippet trailing()}
          <Toggle checked={copy.moveFiles} disabled={readonly} label={t('presetsPage.field.move')}
            onchange={(moveFiles) => setOp({ ...copy, moveFiles })} />
        {/snippet}
      </ListRow>
    </Card>
  {/if}

  {#if usesImageOptions(op)}
    <Card>
      <ListRow title={t('presetsPage.field.quality')} subtitle={o.quality == null ? t('presetsPage.field.qualityHint') : undefined}>
        {#snippet trailing()}
          {#if o.quality != null}
            <span class="slider">
              <Slider value={o.quality} min={1} max={100} disabled={readonly} label={t('presetsPage.field.quality')}
                oninput={(quality) => setOptions({ quality })} />
              <output>{o.quality}</output>
            </span>
          {/if}
          <Toggle checked={o.quality != null} disabled={readonly} label={t('presetsPage.field.quality')}
            onchange={(on) => setOptions({ quality: on ? settings.value.imageQuality : null })} />
        {/snippet}
      </ListRow>
      <ListRow title={t('presetsPage.field.maxSize')} subtitle={t('presetsPage.field.maxSizeHint')}>
        {#snippet trailing()}
          <span class="pair">
            {@render num('max-w', o.maxWidth, RANGES.size, true, true, t('presetsPage.field.maxWidth'), (maxWidth) => setOptions({ maxWidth }))}
            <span class="sep">×</span>
            {@render num('max-h', o.maxHeight, RANGES.size, true, true, t('presetsPage.field.maxHeight'), (maxHeight) => setOptions({ maxHeight }))}
          </span>
        {/snippet}
      </ListRow>
    </Card>
  {/if}

  {#if op.type !== 'copyTo'}
    <Card>
      <ListRow title={t('presetsPage.field.outputDir')} subtitle={o.outputDir ?? t('presetsPage.field.nextToSource')}>
        {#snippet trailing()}
          {#if !readonly}
            {#if o.outputDir}<Button size="sm" variant="plain" onclick={() => setOptions({ outputDir: null })}>{t('presetsPage.field.reset')}</Button>{/if}
            <Button size="sm" onclick={() => pickFolder((outputDir) => setOptions({ outputDir }))}>{t('presetsPage.field.choose')}</Button>
          {/if}
        {/snippet}
      </ListRow>
    </Card>
  {/if}
</div>

<style>
  .editor { display: flex; flex-direction: column; gap: var(--space-3); }
  .wide { display: block; width: 220px; }
  .steps-head { display: flex; align-items: baseline; justify-content: space-between; gap: var(--space-2); margin-bottom: var(--space-2); font: var(--text-body); }
  .hint { font: var(--text-callout); color: var(--text-secondary); }
  .steps { display: flex; flex-direction: column; gap: 6px; margin: 0 0 var(--space-2); padding: 0; list-style: none; }
  .steps li { display: flex; align-items: center; gap: var(--space-2); }
  .n { width: 14px; font: var(--text-caption); color: var(--text-tertiary); text-align: right; font-variant-numeric: tabular-nums; }
  .num { display: block; width: 72px; }
  .num.bad :global(.field) { box-shadow: 0 0 0 2px color-mix(in srgb, var(--red) 55%, transparent); }
  .pair { display: inline-flex; align-items: center; gap: 6px; }
  .sep { color: var(--text-tertiary); }
  .slider { display: inline-flex; align-items: center; gap: var(--space-3); width: 180px; }
  .slider output { min-width: 24px; text-align: right; color: var(--text-secondary); font-variant-numeric: tabular-nums; }
</style>
