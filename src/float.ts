// Float entry: an image pinned on screen (its own small bundle; never imports src/app/).
import { mount } from 'svelte';
import './lib/ui/theme.css';
import Float from './float/Float.svelte';

mount(Float, { target: document.getElementById('float')! });
