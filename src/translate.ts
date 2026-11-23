// Screen translation entry: translated paragraphs drawn over the screen (its own small bundle;
// never imports src/app/).
import { mount } from 'svelte';
import './lib/ui/theme.css';
import Translate from './translate/Translate.svelte';

mount(Translate, { target: document.getElementById('translate')! });
