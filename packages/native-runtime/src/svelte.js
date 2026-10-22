// `svelte` for code bundled with the native runtime (the `svelte` alias in ../vite.js and the
// conformance hooks): Svelte's own client API (`flushSync`, `tick`, `onMount`, `getContext`, ...,
// the same module instance the core adapter uses), except that `mount` / `unmount` / `hydrate`, which
// Svelte implements over the DOM, mount a compiled native component with the native runtime.
// Svelte's names and call shapes: `mount(Component, { target, props, events, context })` returns the
// component's exports, `unmount(exports)` destroys it.
import { DEV } from 'esm-env';
import * as $ from './core/index.js';
import { set_context } from './core/internal.js';
import { mount as nativeMount } from './index.js';

export * from 'svelte/index-client';

/** exports object -> its unmount (Svelte's `mounted_components`). */
const mounted = new WeakMap();

/**
 * Svelte's `mount`. `target`: a native node (`bind:this`, an attachment's node) or a host handle,
 * mounted into its default slot; none mounts at the root (toplevels are presented). `anchor`
 * (a DOM node to insert before) has no native meaning; `intro` (default true) plays the intro
 * transitions of the first render, as in Svelte.
 */
export function mount(component, options = {}) {
  const { target, anchor, props = {}, events, context, transformError, intro = true, onerror } = options;
  if (anchor != null) throw new Error('mount: `anchor` is not supported by the native runtime (mount into `target`)');
  // as Svelte's _mount: on the props object itself, so a state proxy stays one
  if (events) props.$$events = events;
  const Component = context
    ? ($$anchor, p) => {
        // Svelte's _mount pushes a context whose parent map is `context`
        $.push({}, true);
        for (const [k, v] of context) set_context(k, v);
        const exports = component($$anchor, p);
        $.pop();
        return exports;
      }
    : component;
  const unmountFn = nativeMount(Component, { props, target, transformError, intro, onerror });
  const exports = unmountFn.exports;
  mounted.set(exports, unmountFn);
  return exports;
}

/** Svelte's `unmount`: destroys a component `mount` returned; resolves at once (no outros). */
export function unmount(component, _options) {
  const fn = mounted.get(component);
  if (fn) {
    mounted.delete(component);
    fn();
  } else if (DEV) {
    console.warn('%c[svelte] lifecycle_double_unmount\n%cTried to unmount a component that was not mounted\nhttps://svelte.dev/e/lifecycle_double_unmount', 'font-weight: bold', 'font-weight: normal');
  }
  return Promise.resolve();
}

/** Hydration needs server-rendered DOM; native components have none. */
export function hydrate() {
  throw new Error('hydrate: native components cannot be hydrated (use mount)');
}
