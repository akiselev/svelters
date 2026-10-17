import '@svelters/dom/install';
import { mount } from 'svelte';
import App from '@app';

mount(App, { target: globalThis.__svelters_root });
