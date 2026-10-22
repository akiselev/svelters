import { defineConfig } from 'vite';
import { svelteConfig } from '@svelters/build';

// EM1: an embed bundle (dist/embed.js) for crates/svelters-egui/tests/embed.rs; no App, no op log.
export default defineConfig(
  svelteConfig(import.meta.dirname, {
    embed: {
      Panel: 'src/Panel.svelte',
      Broken: 'src/Broken.svelte',
      Fragile: 'src/Fragile.svelte',
      Spinner: 'src/Spinner.svelte',
      Ticker: 'src/Ticker.svelte',
    },
  }),
);
