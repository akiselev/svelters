import { defineConfig } from 'vite';
import { svelteConfig } from '@svelters/build';

export default defineConfig(svelteConfig(import.meta.dirname));
