// Element prop types for `.svelte` files. `gtk.d.ts` and `qt.d.ts` are generated
// (tools/metadata/gtk/gen_backend.py, and svelters-qt-build via examples/counter-qt/build.rs);
// this file only wires them into svelte-check / svelte2tsx.
//
// Use it from an app's tsconfig:  "types": ["@svelters/native-runtime/types"]
// or add `/// <reference types="@svelters/native-runtime/types" />` to an ambient .d.ts file.
import type { GtkElements } from './gtk';
import type { QtElementDescriptors } from './qt';
import type { BindableProps, ToplevelAttrs, ToplevelTag } from './bindings';
import type { MenuElements } from './menus';
import type { GtkObservedBindings } from './observe';
import type { Component, ComponentProps, SvelteComponent } from 'svelte';

export type { GtkElements, QtElementDescriptors };
export type { GtkDangerousMethods } from './gtk';
export type { BindableProps } from './bindings';
export type { MenuElements } from './menus';
export type { AppkitObservedBindings, GtkObservedBindings, QtObservedBindings, WinuiObservedBindings } from './observe';
export type { NativeElement, StaleHandleError } from './native';
export type { ClassValue, StyleAttrs };

// FW1 (ARCH-EMBED §8.7): a bare `on:<event>` inside a component forwards the component's own
// `on<event>` callback prop. svelte-check does not see that prop by itself, so the component
// declares it in its `$props()` type with these helpers:
//   let { label }: { label: string } & Forward<'button', 'clicked'> = $props();
//   let { title }: { title: string } & ForwardFrom<typeof Child, 'accepted'> = $props();
type ElementAttrs<Tag extends string> = Tag extends keyof svelteHTML.IntrinsicElements ? svelteHTML.IntrinsicElements[Tag] : {};
/** The `on<Event>` callback prop a bare `on:<Event>` on `<Tag>` forwards, typed as the element's own handler. */
export type Forward<Tag extends string, Event extends string> = {
  [K in `on${Event}`]?: K extends keyof ElementAttrs<Tag> ? NonNullable<ElementAttrs<Tag>[K]> : (event: any) => void;
};
/** The `on<Event>` callback prop a bare `on:<Event>` on a child component forwards, typed as the child's prop. */
export type ForwardFrom<C extends Component<any, any> | SvelteComponent, Event extends string> = {
  [K in `on${Event}`]?: K extends keyof ComponentProps<C> ? NonNullable<ComponentProps<C>[K]> : (...args: any[]) => void;
};

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

// `on:<event>` per `on<event>` handler of a tag: a listener (`on:clicked={h}`), or, bare
// (`on:clicked`), FW1's forwarding of the component's own `onclicked` callback prop.
type OnDirectives<P> = { [K in keyof P as K extends `on${infer N}` ? `on:${N}` : never]?: P[K] };

// `bind:<name>` attributes per tag, from the binding metadata (bindings.d.ts), plus the ones the
// observe facts allow and `bind:<name>|nonreactive` (observe.d.ts, milestone R2).
type WithBindings<E> = {
  [K in keyof E]: StyleAttrs &
    OnDirectives<E[K]> &
    (K extends keyof BindableProps
      ? E[K] & { [N in keyof BindableProps[K] as `bind:${N & string}`]?: BindableProps[K][N] }
      : E[K]) &
    (K extends keyof GtkObservedBindings ? GtkObservedBindings[K] : unknown) &
    (K extends ToplevelTag ? ToplevelAttrs : unknown);
};

type NativeElements = WithBindings<GtkElements & MenuElements>;
type HtmlNamedProps = NativeElements['button'] &
  NativeElements['image'] &
  NativeElements['label'] &
  NativeElements['menu'] &
  NativeElements['picture'] &
  NativeElements['switch'] &
  NativeElements['text'] &
  NativeElements['video'];

declare global {
  namespace svelteHTML {
    // GTK's property tables are the superset; Qt apps check against `QtElementDescriptors`.
    interface IntrinsicElements extends WithBindings<GtkElements & MenuElements> {}
    // Tags that are also HTML elements keep svelte-check's own HTML entry, which `extends` above
    // cannot override; it merges this interface into each of them, so their native props go here.
    interface HTMLAttributes<T extends EventTarget = any> extends HtmlNamedProps {}
  }
}
