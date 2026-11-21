// Line entry: the clothesline clip strip (its own small bundle; never imports src/app/).
import { mount } from 'svelte';
import './lib/ui/theme.css';
import Line from './line/Line.svelte';

mount(Line, { target: document.getElementById('line')! });
