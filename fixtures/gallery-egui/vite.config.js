import fs from 'node:fs';
import path from 'node:path';
import { defineConfig } from 'vite';
import { svelteConfig } from '@svelters/build';

// The egui gallery (WIDGETS lanes): one embed entry per page, `src/pages/<Name>.svelte` exported as
// `<Name>` from dist/embed.js, so a lane adds pages by adding files. The kittests that mount them
// are crates/svelters-egui/tests/gallery_*.rs.
const pages = Object.fromEntries(
  fs
    .readdirSync(path.join(import.meta.dirname, 'src/pages'))
    .filter((f) => f.endsWith('.svelte'))
    .sort()
    .map((f) => [f.slice(0, -'.svelte'.length), `src/pages/${f}`]),
);

export default defineConfig(svelteConfig(import.meta.dirname, { embed: pages }));
