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

## 2. Result

Full run on `ci/cf1` (`run.mjs --check --update-baseline`, `TIMEOUT_MS=60000`, on `06dd6360` plus
the harness changes of `73014b07`), master (E2, E3, FU2) merged in:

| | pass | expected rejection | failing | skipped |
| --- | ---: | ---: | ---: | ---: |
| before (`8e92af95`) | 500 | (not separated) | 271 | 3 |
| after | **565** | 161 | 45 | 3 |

So 565 / 774 (73.0%) pass. Of the 610 samples that are neither expected rejections nor skipped,
565 (92.6%) pass. `baseline.json` went from 500 to 565 (+65 −0), and no sample that passed before
fails now. By outcome: rejected 132 (all but 12 expected), runtime errors 24, mismatches 50,
timeouts 0.

## 3. Expected rejections (`expected.json`)

`expected.json` lists the samples that need a feature outside the native subset or design, by
category, each with the reason and the document that decides it. `run.mjs` scores them apart:

- **pass / expected rejection / failing / skipped** add up to the samples with a `dom` variant;
- an expected rejection never counts as a pass;
- a listed sample is "expected" only while it fails the way its `match` says (compile-time reasons,
  or the runtime message; pinned per sample where the assertion message is generic). One that
  passes or fails differently is listed as **stale** in REPORT.md and counts as failing, so a real
  bug cannot hide behind the list. The final run has no stale entry.

| category | samples | why | decided in |
| --- | ---: | --- | --- |
| spread | 38 | spread attributes on native elements | runtime-contract.md "Spread attributes"; language-subset.md |
| transition | 33 | `transition:` / `in:` / `out:` / `animate:` | svelte-feature-survey.md §8; custom-renderer-trial.md |
| legacy | 21 | Svelte 4 syntax and legacy mode (`export let`, `<slot>`, `let:`, `on:`, `<svelte:fragment>`, `runes={false}`, `$$props`/`$$restProps`/`$$slots`, class components, `createClassComponent`) | language-subset.md "Legacy mode" |
| dom-node | 20 | DOM APIs (`textContent`, `id`, `dataset`, `tagName`, `click()`, `focus()`, `addEventListener`, `style`) on a native handle | svelte-support.md (`bind:this`: the runtime node); svelte-feature-survey.md §4.2 |
| foreign-element | 17 | SVG, MathML, custom elements, `<script>`, unknown tags | language-subset.md (`$host()`); semantic-spec.md |
| svelte-element | 8 | `<svelte:element this>` (a dynamic kind, unchecked like a spread) | svelte-support.md (error); runtime-contract.md |
| dom-only-element | 7 | `<svelte:window/document/body/head>`, `<title>` | svelte-feature-survey.md §9 |
| html | 5 | `{@html}` | svelte-feature-survey.md §9 |
| raw-snippet | 5 | `createRawSnippet` (HTML strings) | custom-renderer-trial.md |
| dom-css | 4 | the scoping hash class, `<svelte-css-wrapper>` for `--css-props` | styling-syntax.md |
| dom-meta | 3 | dev `__svelte_meta` on DOM nodes | svelte-support.md |
| | **161** | | |

They were not listed: the test-spec gaps (`R-spec`, `R-style-dir`), the `$inspect` stack frames, the
template-effect order and the dom-host attribute semantics. Those are fixable, or need a decision,
so they stay failures.

## 4. Fixes

Each fix follows Svelte's own code (the visitor or runtime function named), and each sample listed
was seen passing in the full run.

| fix | where | samples |
| --- | --- | ---: |
| `{#await}` on Svelte's microtask queue; branches built under the block via `capture()` (with the component-name fix for `await-pending-wait`) | runtime `await_block`, hidden modules async/batch/flags | 7 |
| destructuring template bindings (snippet params and defaults, `{#each}`, `{@const}`, `{:then}`/`{:catch}`) via `extract_paths` | compiler | 15 |
| `use:` actions (`$.action` with the runtime node, ordered with bindings) | compiler, core contract | 7 |
| `$inspect.trace` dependencies (tracing flag import, each items traced to `collection[index]`) | compiler, core | 7 |
| declaration tags `{let x = $state()}`, `{@debug}` | compiler | 4 |
| template call memoization (Svelte's `Memoizer`), component names (`get_component_name`), async destructuring IIFE | compiler | 3 |
| single-sample bugs: predeclared ctor fields, mutable each items over stores, `mark_store_binding`, hoisted nodes in whitespace cleanup | compiler | 6 |
| vitest shim, `matchMedia` mock, `./x.svelte` → `x.svelte.js` resolution | harness | 2 |
| dev snippets: `$.wrap_snippet` + `$n.validate_snippet_args`, wrapped `children`, `invalid_snippet` in DEV | compiler, runtime | 4 |
| `svelte` facade: `mount`/`unmount` from `svelte` mount the native component (`src/svelte.js`, aliased by `vite.js` and the hooks) | runtime, vite, hooks | 4 |
| event handlers as `build_event_handler`: memoized when they call; `$n.apply` (`event_handler_invalid`) in dev | compiler, runtime | 3 |
| `each_key_volatile` in DEV | runtime | 1 |
| DEV init on mount: Svelte's `Array.prototype` patches (`state_proxy_equality_mismatch`) | core (hidden module `dev/equality`) | 1 |
| `side-effect-template` (passes with the template memoization) | | 1 |
| | | **65** |

Other changes: the blocks-diagnostics fixture lost its three destructuring diagnostics (they compile
now). `esm-env` joins the runtime's dependencies (already in the lockfile through svelte), and
`wrap_snippet` joins the core contract. `deviations.md` and `runtime-contract.md` were updated.

## 5. Failing (45), with effort

| bucket | samples | cause | effort |
| --- | ---: | --- | --- |
| F-inspect-stack | 13 | `$inspect`'s stack line shows our `dispatch` frame, not `HTMLButtonElement.click` / `$effect` (`get_stack` filtering) | medium; environment-specific, low value |
| F-dev-warn | 8 | ownership (`non-local-mutation-*`, 5), `binding_property_non_reactive`, `select_multiple_invalid_value`, `legacy-recursive-reactive-block` | medium: needs Svelte's dev component identity (`Component[$.FILENAME]`, `$.push(.., Component)`, `$$ownership_validator.mutation/binding`) in the script transform, then `$.validate_binding` |
| R-spec | 8 | the dom test spec lacks the prop/event (`bind:group`, media elements, `contenteditable`, form reset) | small each, in `metadata/semantics/dom` |
| F-order | 7 | one `template_effect` per fragment (section 6) | decision |
| R-style-dir | 4 | `style:` directives: no `style` prop on the dom test backend | small (test spec) |
| F-dom-host | 3 | `dom-host.mjs` attribute semantics (`translate`, form default value, input type before value) | small, low value |
| F-misc | 2 | `error-boundary-reset-destroyed` (async `transformError`; our boundary is synchronous), `hmr-dynamic-component` (an HMR output path; `invalid_snippet` now) | medium / skip |
| | **45** | | |

## 6. Decision needed: one `template_effect` per fragment (F-order)

Svelte puts every dynamic update of a fragment into one `$.template_effect`, created after the
fragment's children (components included) are initialised. We emit one effect per dynamic op, in
element order, interleaved with child component init. The switch exists already:
`DEFER_TEMPLATE_EFFECTS` in `crates/svelters-compiler/src/codegen/component.rs`, off.

Measured by turning it on locally (then reverted):

- conformance: `lifecycle-render-order-for-children`, `-2`, `-3`, `-4` pass (+4).
  `lifecycle-render-order-for-children-6`, `props-default-value-lazy` and
  `props-default-value-lazy-accessors` still fail; they also need Svelte's grouping into one effect
  with memoized deps, not only the deferral;
- `experiments/native-compiler/compiled`: 32 op-log comparisons fail (49 of 81 pass). The four
  hand-written references are state-equal but in a different order. Every `expected.{gtk,qt}.oplog`
  moves its initial prop `set`s after the insertion of the element's children. The GTK/Qt TestHost
  op-log tests in the Rust crates were not run with it on.

Options:

- (a) defer and group as Svelte does, and regenerate the op logs and references (a few days with
  the grouping; +7);
- (b) defer only with `--dev` or in conformance (two orders to maintain);
- (c) keep per-op effects and record the 7 as a deviation.

Recommendation: (a). It matches Svelte, and the op-log churn is a one-off. The decision belongs to
the op-log owners.

## 7. CI

All five workflows (`js`, `linux-gtk`, `linux-qt`, `macos-appkit`, `windows-winui`) are green on
`ci/cf1`:

- `15a94918` (the master merge plus the fixture fix);
- `0cf42afd` (dev snippets);
- `06dd6360` (the last code change);
- `73014b07` (the final run's results and baseline).

On earlier heads, red came from the hidden-module aliases (fixed in `a47ee1b9`) and from the
blocks-diagnostics fixture after the merge (`15a94918`). Intermediate heads were cancelled by newer
pushes.

## 8. Conflict risks

- `crates/svelters-compiler/src/codegen/component.rs` and `transform/mod.rs` (snippets, event
  handlers, `Rewriter::dev`/`dev_location`): FU3 works in `lower/native_reads.rs` and `compile.rs`;
- `packages/native-runtime/{vite.js, package.json, src/index.js, src/core/*, src/svelte.js}`: the
  `svelte` alias and the new hidden modules (`svelte/index-client`, `dev/equality`);
- `tools/check-svelte-internals.mjs`, `docs/notes/native-compiler/{deviations,runtime-contract}.md`,
  `pnpm-lock.yaml` (esm-env).
