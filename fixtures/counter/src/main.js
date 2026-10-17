import '@svelters/dom/install';
import { mount } from 'svelte';
import App from './App.svelte';

mount(App, { target: globalThis.__svelters_root });
