// Shelf entry: the clipboard quick-paste panel (its own small bundle; never imports src/app/).
// Theme, language and data are handled inside Shelf.svelte.
import { mount } from 'svelte';
import './lib/ui/theme.css';
import Shelf from './shelf/Shelf.svelte';

mount(Shelf, { target: document.getElementById('shelf')! });
