import { defineConfig } from 'vite';
import { svelteConfig } from '@svelters/build';

// The egui gallery app (dist/app.js): src/App.svelte lists every page in src/pages/ in a sidebar.
// vite.embed.config.js builds the same pages as embed exports (dist/embed.js); `pnpm build` runs both.
export default defineConfig(svelteConfig(import.meta.dirname));
