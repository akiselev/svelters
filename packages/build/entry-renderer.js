import '@svelters/renderer/install';
import { mount } from 'svelte';
import renderer from '@svelters/renderer';
import App from '@app';

mount(App, { target: globalThis.__svelters_root, renderer });
