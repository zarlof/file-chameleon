// Lucide icon for each wheel slot icon key (types.ts `SlotIcon`, which includes the file kinds).
// Per-icon imports keep the overlay bundle small.
import type { Component } from 'svelte';
import Archive from '@lucide/svelte/icons/archive';
import BellRing from '@lucide/svelte/icons/bell-ring';
import Captions from '@lucide/svelte/icons/captions';
import Contrast from '@lucide/svelte/icons/contrast';
import ClipboardCopy from '@lucide/svelte/icons/clipboard-copy';
import Copy from '@lucide/svelte/icons/copy';
import ScanText from '@lucide/svelte/icons/scan-text';
import Crop from '@lucide/svelte/icons/crop';
import Droplet from '@lucide/svelte/icons/droplet';
import FastForward from '@lucide/svelte/icons/fast-forward';
import File from '@lucide/svelte/icons/file';
import FileText from '@lucide/svelte/icons/file-text';
import Film from '@lucide/svelte/icons/film';
import FlipHorizontal from '@lucide/svelte/icons/flip-horizontal';
import FlipVertical from '@lucide/svelte/icons/flip-vertical';
import FolderInput from '@lucide/svelte/icons/folder-input';
import Grid3x3 from '@lucide/svelte/icons/grid-3x3';
import ImageIcon from '@lucide/svelte/icons/image';
import Layers from '@lucide/svelte/icons/layers';
import Merge from '@lucide/svelte/icons/merge';
import Music from '@lucide/svelte/icons/music';
import Package from '@lucide/svelte/icons/package';
import PackageOpen from '@lucide/svelte/icons/package-open';
import PenLine from '@lucide/svelte/icons/pen-line';
import PenTool from '@lucide/svelte/icons/pen-tool';
import Pin from '@lucide/svelte/icons/pin';
import RefreshCcw from '@lucide/svelte/icons/refresh-ccw';
import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
import RotateCw from '@lucide/svelte/icons/rotate-cw';
import Scaling from '@lucide/svelte/icons/scaling';
import Shrink from '@lucide/svelte/icons/shrink';
import Split from '@lucide/svelte/icons/split';
import SquareDashed from '@lucide/svelte/icons/square-dashed';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';
import Table from '@lucide/svelte/icons/table';
import VolumeX from '@lucide/svelte/icons/volume-x';
import type { SlotIcon } from '../types';

export type IconComponent = Component<{ size?: number; strokeWidth?: number; class?: string }>;

const icons: Record<SlotIcon, IconComponent> = {
  image: ImageIcon,
  vector: PenTool,
  video: Film,
  audio: Music,
  pdf: FileText,
  data: Table,
  subtitle: Captions,
  archive: Archive,
  grayscale: Contrast,
  rotateLeft: RotateCcw,
  rotateRight: RotateCw,
  rotate180: RefreshCcw,
  flipH: FlipHorizontal,
  flipV: FlipVertical,
  trim: SquareDashed,
  pixelate: Grid3x3,
  blur: Droplet,
  resize: Scaling,
  annotate: PenLine,
  compress: Shrink,
  mute: VolumeX,
  speed: FastForward,
  crop: Crop,
  merge: Merge,
  split: Split,
  zip: Package,
  extract: PackageOpen,
  copy: Copy,
  move: FolderInput,
  ringtone: BellRing,
  group: Layers,
  copyPath: ClipboardCopy,
  openTerminal: SquareTerminal,
  float: Pin,
  copyText: ScanText,
};

/** Icon for a slot (or a file kind); a plain file icon for null or an unknown key. */
export function slotIcon(key: SlotIcon | null): IconComponent {
  return (key !== null && Object.hasOwn(icons, key) ? icons[key] : undefined) ?? File;
}
