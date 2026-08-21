# SVR04 - Svelte 5 native client-runtime overlay

References: `plans/architecture/runtime-stack.md`.

## Goal

Run stock Svelte 5 compiled client code against the Svelters logical graph with the smallest sustainable divergence from upstream `svelte/internal/client`.

## Strategy

Pin an exact upstream Svelte commit/package version. Reuse upstream reactivity, props, stores, context, effects, runtime scheduling, and HMR logic where possible. Replace/adapt DOM-oriented operations with logical native Node semantics.

Do not implement a general browser DOM. Implement only the Node/Element operations required by current Svelte block/render algorithms plus Svelters-native attributes/events/bindings.

Track every copied/adapted upstream file in a machine-readable overlay manifest containing upstream path, upstream hash, local path, adaptation reason, and compatibility test set.

## Compatibility matrix

Cover at minimum:

- `$state`, `$derived`, `$effect`, `$props`, `$bindable`;
- component composition/context/lifecycle;
- text and attributes;
- `if`, keyed/unkeyed `each`, `key`, `await`, snippets/render;
- events and modifiers that make native semantic sense;
- `bind:this` and portable property bindings;
- actions/attachments where host-neutral semantics can be defined;
- HMR wrapper compatibility;
- deliberate diagnostics for browser-only facilities.

Hydration/SSR is out of initial scope unless an upstream implementation path is needed internally; native applications start from an empty logical graph.

## Upstream fixtures

Import a curated subset of upstream Svelte runtime/compiler fixtures with source commit metadata. Add a compatibility dashboard of `PASS`, `ADAPTED`, `BROWSER_ONLY`, `NOT_YET`, `BUG`.

## Acceptance

The rune counter and structural fixture suite execute against TestHost using code produced by the stock Svelte compiler. No compiler patch is required for acceptance. Every runtime overlay file has upstream provenance and an update test.
