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
      // EM2 (crates/svelters-egui/tests/viewports.rs)
      Pane: 'src/Pane.svelte',
      Editor: 'src/Editor.svelte',
      Animator: 'src/Animator.svelte',
      // WG1 (crates/svelters-egui/tests/widgets_panel.rs)
      TaskPanel: 'src/TaskPanel.svelte',
    },
  }),
);
