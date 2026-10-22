import fs from 'node:fs';
import path from 'node:path';
import { defineConfig } from 'vite';
import { svelteConfig } from '@svelters/build';

// The egui gallery's pages as embed exports (WIDGETS lanes): `src/pages/<Name>.svelte` exported as
// `<Name>` from dist/embed.js, so a lane adds pages by adding files. Only pages whose file name is a
// JS identifier are exported (the numbered `20-inputs.svelte` style pages are app-only). The kittests
// that mount them are crates/svelters-egui/tests/gallery_*.rs. Runs after vite.config.js (the app
// build), so it keeps dist/app.js.
const pages = Object.fromEntries(
  fs
    .readdirSync(path.join(import.meta.dirname, 'src/pages'))
    .filter((f) => f.endsWith('.svelte'))
    .map((f) => f.slice(0, -'.svelte'.length))
    .filter((name) => /^[A-Za-z_$][\w$]*$/.test(name))
    .sort()
    .map((name) => [name, `src/pages/${name}.svelte`]),
);

const config = svelteConfig(import.meta.dirname, { embed: pages });
config.build.emptyOutDir = false;
export default defineConfig(config);
