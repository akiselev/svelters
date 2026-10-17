// Bench adapter over Svelte's own signals (stock 5.57.2, production build), bundled by build-bench.mjs.
import * as $ from 'svelte/internal/client';
import { flushSync } from 'svelte';
export const name = 'svelte-5.57.2';
export const signal = (v) => $.state(v);
export const read = $.get;
export const write = (s, v) => $.set(s, v);
export const computed = (fn) => $.derived(fn);
// an effect in a root, like a template effect in a component
export const root = (fn) => $.effect_root(fn);
export const effect = (fn) => $.template_effect(fn);
export const flush = () => flushSync();
