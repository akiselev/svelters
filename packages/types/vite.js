// Vite aliases for the generated non-UI type modules: `import { File } from 'svelters:gtk/gio'`.
// Both build paths (packages/build for the stock renderer, packages/native-runtime/vite.js for the Rust
// compiler) spread `typesAliases()` into their `resolve.alias`. Each module resolves to its `index.js`,
// which re-exports one file per class, so an unused class never reaches the bundle (`sideEffects: false`).
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));

/** Absolute directory of the generated GTK modules. */
export const gtkDir = path.join(here, 'gtk');

/** Absolute directory of the generated Qt modules (`svelters:qt/core`, `svelters:qt/gui`, `svelters:qt/widgets`). */
export const qtDir = path.join(here, 'qt');

/** `resolve.alias` entries (array form) for every `svelters:<toolkit>/<ns>` module. */
export function typesAliases() {
  return [
    { find: /^svelters:gtk\/([a-z]+)$/, replacement: path.join(gtkDir, '$1/index.js') },
    { find: /^svelters:qt\/([a-z]+)$/, replacement: path.join(qtDir, '$1/index.js') },
  ];
}
