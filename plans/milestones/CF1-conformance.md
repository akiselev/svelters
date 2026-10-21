# CF1: Svelte conformance

Raise the share of Svelte's own runtime tests (`tests/runtime-runes/samples` of stock svelte@5.57.2,
`dom` variant, `SVELTE_NO_ASYNC=true`) that pass when compiled by `svelters-compile` and run on
`@svelters/native-runtime` over the jsdom test host (`experiments/native-compiler/conformance`).
The baseline ratchet (`baseline.json`) only lets the passing set grow.

Command: `UPSTREAM=<dir holding svelte/> TIMEOUT_MS=60000 node run.mjs --check` in
`experiments/native-compiler/conformance`, with `UPSTREAM/svelte` the extracted GitHub tarball of the
tag (`https://codeload.github.com/sveltejs/svelte/tar.gz/refs/tags/svelte@5.57.2`).

## 1. Buckets before (master @ 8e92af95)

Reproduced: **500 / 774** (rejected 168, runtime errors 27, mismatches 76, timeouts 0, skipped 3);
baseline 500: +0 -0.

Every non-passing sample in exactly one bucket. A rejected sample that needs an out-of-subset feature
goes to that feature's bucket even if it also hits a gap (the `R-*` rows are in priority order:
transitions before legacy before ... before `use:`). `F-*` rows were assigned by reading each sample
and our output.

| bucket | root cause | samples | in subset? |
| --- | --- | ---: | --- |
| R-spread | spread attributes on native elements | 34 | no: decision (runtime-contract.md "Spread attributes") |
| R-transition | `transition:` / `in:` / `out:` / `animate:` | 33 | no: forbidden (svelte-feature-survey.md §8) |
| R-ns | svg / mathml / custom elements / `<script>`: no such kinds in the dom test spec | 19 | no (DOM namespaces, custom elements) |
| R-use | `use:` actions | 16 | **gap** |
| R-destructure | destructuring / defaults in snippet params, `{#each}`, `{@const}`, `{:then}` | 15 | **gap** (deviations.md) |
| R-legacy | `<slot>`, `let:`, `<svelte:fragment>`, `export let` | 11 | no: runes only |
| R-spec | DOM props/events the dom test spec lacks (`bind:group`, `<select defaultValue>`, media) | 8 | test-spec gap |
| R-domonly | `<svelte:window/document/head/body>`, `<title>` | 8 | no: DOM only |
| R-html | `{@html}` | 7 | no: forbidden (survey §9) |
| R-svelte-el | `<svelte:element>` and other `svelte:` elements | 6 | no: dynamic tag, unchecked like a spread |
| R-decl-tag | declaration tags `{let x = $state()}` / `{const ...}` | 4 | **gap** |
| R-style-dir | `style:` directives on the dom test backend (no `style` prop) | 4 | test-spec gap |
| R-attr-comp | `let:` on components | 2 | no: legacy |
| R-debug | `{@debug}` | 1 | **gap** |
| F-dev-warn | dev-mode checks and warnings not emitted (ownership, `binding_property_non_reactive`, `event_handler_invalid`, `each_key_volatile`, `snippet_without_render_tag`, ...) | 17 | deviations.md "Rune transform" 4 |
| F-misc | single-sample compiler/runtime bugs, triaged one by one | 15 | **gap** |
| F-inspect-stack | `$inspect` stack frame: the native dispatch path is on the stack | 12 | dev, environment-specific |
| F-dom-node | the test drives a DOM node that the component only sees as a native handle (attach, action, `bind:this`, event target) | 10 | no: native handles are not DOM nodes |
| F-trace | `$inspect.trace()` logs nothing beyond the label | 7 | gap (dev) |
| F-await | `{#await}` pending timing (deviations.md), `getAbortSignal` | 6 | gap |
| F-legacy | legacy (non-runes) mode semantics; runes mode is forced | 6 | no |
| F-mount | programmatic `mount()`/`unmount()` from `svelte` with a DOM target | 5 | DOM target |
| F-raw-snippet | `createRawSnippet` (HTML strings) | 5 | no: DOM |
| F-harness | harness: `vitest` import, HMR output path, `.svelte` module resolution | 5 | harness |
| F-css | DOM CSS output: scoping hash class, `--css-props` wrapper | 4 | no: DOM CSS |
| F-order | template effect order (`DEFER_TEMPLATE_EFFECTS` off) | 4 | deviation (op-log owners' decision) |
| S-skip | `skip: true` upstream | 3 | n/a |
| F-meta | dev `__svelte_meta` on DOM nodes | 3 | no: DOM |
| F-memo | template call expressions not memoized (re-evaluated on unrelated changes) | 2 | **gap** |
| F-name | component function named after the file uncapitalized (`await.svelte` -> `function await`) | 1 | **gap** |
| F-async-destructure | destructuring assignment with `await` in a default: the IIFE is not `async` | 1 | **gap** |
| | **not passing** | **274** | |

Samples per bucket:

- R-spread (34): attachment-component, attachment-component-falsy, attachment-component-spread, attachment-spread, attachment-spread-stable, attribute-spread-call-expression, attribute-spread-reactivitiy, bind-and-spread, bind-and-spread-precedence, bind-state-property, bind-value-input-type-dynamic, binding-interop-derived, deferred-events-consistency-3, dynamic-spread-and-attribute-directive, element-is-attribute, event-attribute-after-binding, event-attribute-delegation-5, event-attribute-spread, event-attribute-spread-collision, event-attribute-spread-update, event-spread-rerun, form-default-value-select, form-default-value-spread, select-default-value-preserve-selection, select-default-value-single-observer, select-falsy-value, select-spread-option-selected, snippet-select, spread-element-input-select, spread-props, spread-props-2, state-snapshot-uncloneable-ignored, svelte-element-draggable, value-attribute-isolated-update-spread
- R-transition (33): animate-no-transition-events, bind-this-order, component-transition-hmr, deferred-outro-restored, directives-with-member-access, dynamic-component-transition, dynamic-if-component-transition, dynamic-transition, each-updates-12, each-updates-13, effect-tracking-transition, if-nested-template, if-transition-inert, if-transition-undefined, mount-intro-transition, transition-component, transition-delayed, transition-delayed-events-css, transition-delayed-events-js, transition-derived-uninitialized, transition-each, transition-each-2, transition-each-3, transition-each-4, transition-evaluation, transition-global-nested-resume, transition-global-nested-resume-each, transition-global-nested-resume-flipback, transition-if, transition-if-nested-access-property, transition-if-nested-static, transition-snippet, transition-static-subtree
- R-ns (19): attribute-spread-casing, clsx, custom-element-attributes, custom-element-injected-styles, custom-element-slot-in-snippet, custom-element-svelte-class, dynamic-element-dynamic-namespace, effect-tracking-unowned, nested-script-tag, snippet-namespace-1, snippet-namespace-2, style-update, svelte-element-custom-element-css-hash, svg-attribute-case, svg-element-attribute-serialize, svg-namespace-if-block, svg-namespace-if-block-2, svg-namespace-if-block-3, svg-namespace-infer-a-tag
- R-use (16): action-context, action-sequence, action-state-arg, action-state-arg-deep, attachment-from-action, await-render-error-restore-reaction, deferred-events-consistency-4, effect-untrack-teardown, error-boundary-reset-destroyed, event-attribute-delegation, event-attribute-delegation-3, event-spread-timing, lifecycle-render-order-for-children-6, muted-without-bind-works, nullish-actions, store-directive
- R-destructure (15): destructure-derived-event, each-block-default-arg, each-keyed-computed-destructuring-key, event-attribute-delegation-6, inspect-exception, reactive-map, snippet-argument-destructured, snippet-argument-destructured-multiple, snippet-complicated-defaults, snippet-default-arg, snippet-destructure-array, snippet-optional-arguments-defaults, snippet-reactive-args, snippet-typescript, validate-undefined-snippet-default-arg
- R-legacy (11): before-update-in-legacy-child, component-let-directive, dynamic-event-handler-3, event-listener-spread, event-on-2, fine-grained-prop-reactive-statement, legacy-runes-ambiguous-export-let-2, slot, slot-static, snippet-spread, snippets-as-slots
- R-spec (8): bind-current-time-remove-listener, bindings-form-reset, contenteditable-state, event-attribute-after-spread-2, event-media-element-cleanup, proxy-coercive-assignment-warning, state-bind-group, video-src-object
- R-domonly (8): event-attribute-delegation-4, event-attribute-window, event-cleanup-before-attachment, event-global-mount-error-cleanup, if-dependency-order-2, multiple-head, svelte-head-anchor-cleanup, title-nullish
- R-html (7): each-dynamic-html, html-tag-contenteditable, html-tag-empty, html-tag-escaping, hydration-html-changed-ignored, mathml-namespace-infer, svg-namespace-infer
- R-svelte-el (6): action-void-element, attachment-svelte-element, dynamic-element-transition, slot-svelte-fragment-render-tag, svelte-element-css-hash, svelte-meta-dynamic
- R-decl-tag (4): declaration-tag-multiple-declarators, declaration-tags, declaration-tags-each, declaration-tags-no-script
- R-style-dir (4): directives, dynamic-attribute-and-attribute-directive, dynamic-attribute-and-attribute-directive-2, style-directive-memoize
- R-attr-comp (2): snippet-slot-let-error, snippet-slot-let-renamed-children-error
- R-debug (1): debug-tag-object
- F-dev-warn (17): binding-property-static, each-key-volatile, event-handler-component-invalid-warning, event-handler-invalid-values, event-handler-invalid-warning, legacy-recursive-reactive-block, non-local-mutation-discouraged, non-local-mutation-inherited-owner-3, non-local-mutation-with-binding-2, non-local-mutation-with-binding-3, select-multiple-invalid-value, side-effect-template, snippet-block-without-render-tag-dev, snippet-children-without-render-tag-dev, snippet-invalid-call, snippet-undefined, state-proxy-equality-mismatch
- F-misc (15): attribute-if-string, bind-getter-setter-2, bind-getter-setter-loop, binding-interop, bound-store-sub, class-state-constructor-predeclared-field, derived-read-outside-reaction, each-updates-4, each-updates-5, form-default-value, input-type-before-value-spread, inspect-console-trace, props-default-value-lazy, props-default-value-lazy-accessors, snippet-whitespace
- F-inspect-stack (12): inspect, inspect-deep, inspect-deep-array, inspect-derived-2, inspect-derived-3, inspect-derived-4, inspect-map-set, inspect-multiple, inspect-nested-effect, inspect-nested-state, inspect-new-property, inspect-recursive
- F-dom-node (10): attachment-basic, attachment-reactive, bind-this-no-state, each-bind-this-member, effect-tracking-binding-set, event-context, event-listener-moved-outside-container, event-on, legacy-class-transformation, state-each-bind-this
- F-trace (7): inspect-trace, inspect-trace-circular-reference, inspect-trace-class, inspect-trace-each, inspect-trace-nested, inspect-trace-reassignment, inspect-trace-store
- F-await (6): abort-signal-derived-rerun-on-reconnect, await-hydrate-maybe-promise, await-pending-destroy, await-resolve, await-resolve-2, get-abort-signal
- F-legacy (6): legacy-runes-ambiguous-explicit-false, legacy-runes-ambiguous-export-$$props, legacy-runes-ambiguous-export-$$restProps, legacy-runes-ambiguous-export-labeled, legacy-runes-ambiguous-export-let, props-and-slots
- F-mount (5): create-context-programmatic, flush-sync-inside-attachment, manual-mount, mount-props-updates, mount-unmount-anchor
- F-raw-snippet (5): create-raw-snippet-invalid-render, snippet-raw, snippet-raw-component, snippet-raw-component-ssr-dev, snippet-raw-teardown
- F-harness (5): hmr-dynamic-component, inspect-trace-null, media-query, non-local-mutation-inherited-owner-2, typescript-as-expression
- F-css (4): class-directive, component-css-props-falsy, dynamic-component-css-props, svelte-meta-css-wrapper
- F-order (4): lifecycle-render-order-for-children, lifecycle-render-order-for-children-2, lifecycle-render-order-for-children-3, lifecycle-render-order-for-children-4
- S-skip (3): deferred-events-consistency-2, error-boundary-18, tick-timing
- F-meta (3): svelte-meta, svelte-meta-if-else, svelte-meta-parent
- F-memo (2): snippet-argument-call-expression, text-multiple-call-expression
- F-name (1): await-pending-wait
- F-async-destructure (1): destructure-async-assignments

`{await p}` (FU3's diagnostic) is not a bucket here: with `SVELTE_NO_ASYNC=true` the async samples
are excluded, and no counted sample fails on it.

Plan, largest in-subset buckets first: R-use (16), R-destructure (15), F-misc (15), R-decl-tag (4),
F-await (6), F-memo/F-name/F-async-destructure (4), F-harness (5), F-trace (7), R-debug (1).
