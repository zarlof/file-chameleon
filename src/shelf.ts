// Shelf entry: the clipboard quick-paste panel (its own small bundle; never imports src/app/).
// OWNER: Task 11 (this file is a placeholder from the contracts commit).
import { mount } from 'svelte';
import './lib/ui/theme.css';
import Shelf from './shelf/Shelf.svelte';

mount(Shelf, { target: document.getElementById('shelf')! });
