<!--
  Icon — renders a Lucide icon by name for the few dynamic cases (e.g. file kinds, toast kinds).
  Prefer importing the Lucide component directly when the icon is static.
  Props: name (string, see `iconNames`), size = 16, strokeWidth = 1.75, class.
-->
<script lang="ts">
  import {
    Braces, Check, ChevronDown, CircleAlert, CircleCheck, CircleX, Clipboard, Clock, Copy, Download, File,
    FileText, Film, FolderOpen, Image as ImageIcon, Inbox, Info, Link, Music, PenTool, Plus, RefreshCw,
    Search, Settings, Trash2, TriangleAlert, X,
  } from '@lucide/svelte';
  import type { Component } from 'svelte';

  const icons: Record<string, Component<any>> = {
    braces: Braces, check: Check, 'chevron-down': ChevronDown, 'circle-alert': CircleAlert,
    'circle-check': CircleCheck, 'circle-x': CircleX, clipboard: Clipboard, clock: Clock, copy: Copy,
    download: Download, file: File, 'file-text': FileText, film: Film, 'folder-open': FolderOpen,
    image: ImageIcon, inbox: Inbox, info: Info, link: Link, music: Music, 'pen-tool': PenTool, plus: Plus,
    'refresh-cw': RefreshCw, search: Search, settings: Settings, trash: Trash2, 'triangle-alert': TriangleAlert, x: X,
    // File kinds (types.ts `Kind`)
    vector: PenTool, video: Film, audio: Music, pdf: FileText, data: Braces,
  };

  interface Props {
    name: string;
    size?: number;
    strokeWidth?: number;
    class?: string;
  }
  let { name, size = 16, strokeWidth = 1.75, class: cls = '' }: Props = $props();
  const Cmp = $derived(icons[name] ?? File);
</script>

<Cmp {size} {strokeWidth} class={cls} aria-hidden="true" />
