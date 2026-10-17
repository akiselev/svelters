// Element prop types for `.svelte` files. `gtk.d.ts` and `qt.d.ts` are generated
// (tools/metadata/gtk/gen_backend.py, and svelters-qt-build via examples/counter-qt/build.rs);
// this file only wires them into svelte-check / svelte2tsx.
//
// Use it from an app's tsconfig:  "types": ["@svelters/renderer/types"]
// or add `/// <reference types="@svelters/renderer/types" />` to an ambient .d.ts file.
import type { GtkElements } from './gtk';
import type { QtElementDescriptors } from './qt';

export type { GtkElements, QtElementDescriptors };

declare global {
  namespace svelteHTML {
    // GTK's property tables are the superset; Qt apps check against `QtElementDescriptors`.
    interface IntrinsicElements extends GtkElements {}
  }
}
