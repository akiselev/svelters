// Element prop types for `.svelte` files. `gtk.d.ts` and `qt.d.ts` are generated
// (tools/metadata/gtk/gen_backend.py, and svelters-qt-build via examples/counter-qt/build.rs);
// this file only wires them into svelte-check / svelte2tsx.
//
// Use it from an app's tsconfig:  "types": ["@svelters/renderer/types"]
// or add `/// <reference types="@svelters/renderer/types" />` to an ambient .d.ts file.
import type { GtkElements } from './gtk';
import type { QtElementDescriptors } from './qt';
import type { BindableProps, ToplevelAttrs, ToplevelTag } from './bindings';

export type { GtkElements, QtElementDescriptors };
export type { BindableProps } from './bindings';
export type { NativeElement, StaleHandleError } from './native';
export type { ClassValue, StyleAttrs };

// Styling attributes every native element takes (docs/site: styling guide). `class` accepts
// Svelte's class values (strings, objects, arrays); `style` is a portable declaration string.
type ClassValue = string | null | undefined | false | ClassValue[] | { [name: string]: unknown };
type StyleAttrs = {
  class?: ClassValue;
  style?: string | null;
  id?: string | null;
  [directive: `class:${string}`]: unknown;
  [directive: `style:${string}`]: string | number | null | undefined;
};

// `bind:<name>` attributes per tag, from the binding metadata (bindings.d.ts).
type WithBindings<E> = {
  [K in keyof E]: StyleAttrs &
    (K extends keyof BindableProps
      ? E[K] & { [N in keyof BindableProps[K] as `bind:${N & string}`]?: BindableProps[K][N] }
      : E[K]) &
    (K extends ToplevelTag ? ToplevelAttrs : unknown);
};

declare global {
  namespace svelteHTML {
    // GTK's property tables are the superset; Qt apps check against `QtElementDescriptors`.
    interface IntrinsicElements extends WithBindings<GtkElements> {}
  }
}
