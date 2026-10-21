# CF1 handoff (Svelte conformance)

Branch `ci/cf1`, head `a47ee1b9` plus this note. Lane record: `plans/milestones/CF1-conformance.md`.
It holds the "before" buckets only; the after-numbers, CI table and open items still need adding.
**origin/master has NOT been merged.** Since the base `8e92af95` it has gained E2, E3 and the FU2
STATUS commits. Merge it before the final gates.

## How to run conformance

The upstream samples come from the GitHub tarball of the tag, extracted into a dir of its own:

```
mkdir -p $UP && curl -sSL https://codeload.github.com/sveltejs/svelte/tar.gz/refs/tags/svelte@5.57.2 -o $UP/svelte.tgz
mkdir $UP/svelte && tar -xzf $UP/svelte.tgz -C $UP/svelte --strip-components=1
```

My copy is at `/tmp/claude-1000/-home-dev-projects-svelters/43ac01ab-2dd7-46f5-8f87-b6ead7af185e/scratchpad/up`.

1. Build the compiler first: `tools/dev/cargo-slot cargo build -p svelters-compiler`.
2. Then, in `experiments/native-compiler/conformance`:
   `UPSTREAM=$UP TIMEOUT_MS=60000 JOBS=8 node run.mjs --check`. This takes about 9.5 minutes on the
   VM. Don't run it alongside nextest.
3. Filtered runs (`node run.mjs name-substr...`) print per-sample results and do not touch
   `results.json`, `REPORT.md` or the baseline.
4. After the full run, use `--update-baseline` to grow `baseline.json`.

Flakes: `effect-loop-infinite` and `error-boundary-20` time out under load with the default 20 s,
so always use `TIMEOUT_MS=60000`.

## Score

| | pass |
| --- | --- |
| before (reproduced on `8e92af95`) | **500 / 774** (rejected 168, runtime errors 27, mismatches 76, skipped 3) |
| now | **not measured with a full run**: about 51 more samples pass in filtered runs (listed below), so about 551 is expected if nothing regressed |

`baseline.json` is **not updated** yet. The committed `results.json` and `REPORT.md` are still
master's. The working tree has the reproduced "before" run, uncommitted and stale; overwrite it with
a fresh full run.

## Fixes so far (commits on `ci/cf1`)

Every sample below was seen passing in a filtered run.

1. `{#await}` follows Svelte's `await.js`:
   - `pending` comes from `queue_micro_task`, so `flushSync` shows it at once;
   - callbacks build the branch under the block effect via `capture()`;
   - the adapter gained hidden modules `reactivity/async.js`, `reactivity/batch.js` and `flags/index.js`;
   - runtime unit tests were updated to Svelte's semantics; `deviations.md` and
     `runtime-contract.md` were updated.

   Now passing (7): await-hydrate-maybe-promise, await-pending-destroy, await-resolve,
   await-resolve-2, await-pending-wait, abort-signal-derived-rerun-on-reconnect, get-abort-signal.
2. Template call memoization (`has_call`, Svelte's `Memoizer`): text, attribute and child-attr
   values become `template_effect` deps, and render args are deriveds.

   Now passing (2): text-multiple-call-expression, snippet-argument-call-expression.
3. Component function name follows Svelte's `get_component_name`: capitalized, and `generate()`d
   for uniqueness. This changes the goldens (`input` → `Input`).
4. A destructuring-assignment IIFE is `async` when a default awaits.

   Now passing (1): destructure-async-assignments.
5. Destructuring template bindings, via a new `Rewriter::destructure` over `extract_paths`:
   - snippet params and defaults;
   - `{#each}` contexts (keyed: destructured inside the key function);
   - `{@const}`;
   - `{:then}` / `{:catch}`.

   `derived_safe_equal` joins the core contract. All 15 R-destructure samples pass.
6. `use:` actions compile to `$.action`, ordered with bindings; `action` joins the contract.

   Now passing (7): action-context, action-state-arg, action-state-arg-deep,
   effect-untrack-teardown, event-attribute-delegation, event-attribute-delegation-3, nullish-actions.
7. Single-sample fixes:
   - a predeclared public field before the constructor's `this.x = $state()` is dropped;
   - `{#each}` over a store gets mutable items (`$n.EACH_ITEM_MUTABLE`);
   - `$.mark_store_binding()` for `bind:x={$store}` on components;
   - `text_runs` hoists snippet, const, debug and declaration nodes before whitespace cleanup.

   Now passing (6): class-state-constructor-predeclared-field, derived-read-outside-reaction,
   each-updates-4, each-updates-5, bound-store-sub, snippet-whitespace.
8. Declaration tags `{let x = $state()}`, emitted from the declaring fragment scope.

   Now passing (3): declaration-tags, declaration-tag-multiple-declarators,
   declaration-tags-no-script. `declaration-tags-each` calls `ref.click()` on a native handle.
9. `{@debug}` now compiles. Now passing (1): debug-tag-object.
10. Harness: a `vitest` shim (`shims/vitest.mjs`), and `matchMedia` is a `vi.fn` mock, as in
    Svelte's `tests/helpers.js`. Now passing (2): media-query, inspect-trace-null.
11. `$inspect.trace`:
    - in dev, the component imports `@svelters/native-runtime/core/tracing` first (Svelte's tracing flag);
    - each item sources carry Svelte's DEV `trace` link.

    Now passing (7): inspect-trace, -circular-reference, -class, -each, -nested, -reassignment, -store.

## Gates and CI

**CI on `ci/cf1` is red.** Runs 39680218xxx on `0a9f5fbc` and 39679534xxx on `e9b7640e` failed in
all five workflows, all at `pnpm -r build` in `experiments/native-compiler/handwritten`, where
rolldown could not resolve the hidden modules.

The cause: `packages/native-runtime/vite.js` hard-coded the aliases, so the new async and batch
modules were missing. `a47ee1b9` derives the aliases from `HIDDEN_MODULES`; CI has not run on it yet.
Check `gh run list --branch ci/cf1`.

Local checks after the last edit:

- `cargo nextest run -p svelters-compiler`: 175 passed after the golden update (UPDATE=1; the
  diffs were reviewed and intended);
- `pnpm test` in `packages/native-runtime`: 89/89, plus check-svelte-internals and svelte-check.

Not yet run on the final tree:

- `cargo check --workspace --all-targets`;
- full-workspace nextest (op-log fixtures may move with memoization, the each flag or the await
  rewrite);
- `pnpm -r build`, `pnpm test` and `pnpm docs:test`;
- the full conformance run.

## Remaining buckets (failures seen so far), with root causes

Out of the subset (don't implement; mark as expected rejections):

| bucket | samples | cause |
| --- | --- | --- |
| R-spread | 34 | native spread is rejected by decision |
| R-transition | 33 | transitions are forbidden |
| R-ns | 19 | svg, mathml and custom elements |
| R-legacy | 11 | legacy syntax |
| R-domonly | 8 | DOM-only elements |
| R-html | 7 | `{@html}` |
| R-svelte-el | 6 | svelte: elements |
| F-legacy | 6 | legacy mode; binding-interop belongs here too |
| F-raw-snippet | 5 | createRawSnippet |
| F-css | 4 | DOM CSS |
| F-meta | 3 | dev `__svelte_meta` |

`run.mjs` has no "expected rejection" support yet. Adding a reason map to `baseline.json`, used only
for the report, is a small harness change.

`F-dom-node` has grown. These tests call DOM APIs on native handles, which is by design:

- `.id`, `.dataset`, `.textContent`, `.click()`, `.focus()`, `addEventListener`;
- action-sequence, store-directive, attachment-from-action, await-render-error-restore-reaction,
  deferred-events-consistency-4, event-spread-timing, muted-without-bind-works;
- bind-getter-setter-2, bind-getter-setter-loop, declaration-tags-each, typescript-as-expression.

The next ones to attack:

- **F-dev-warn** (17, plus non-local-mutation-inherited-owner-2): dev checks are not emitted
  (deviations.md "Rune transform" item 4). The work is compiler output: ownership,
  `binding_property_non_reactive`, `event_handler_invalid`, `snippet_without_render_tag`, ...
  Medium-to-large; take them one warning at a time.
- **F-inspect-stack** (12, plus inspect-console-trace): the `$inspect` stack frame shows our
  `dispatch`, not `HTMLButtonElement.click`. This is environment-specific, and only fixable by
  making the native dispatch frames invisible to Svelte's `get_stack`. Low value.
- **F-order** (4, plus lifecycle-render-order-for-children-6, props-default-value-lazy and -accessors):
  Svelte uses one `template_effect` per fragment; we use per-op effects (`DEFER_TEMPLATE_EFFECTS`,
  plus grouping). Large, and it changes the GTK/Qt op logs, so it needs the op-log owners' decision.
- **F-mount** (5): `mount` from `svelte` is called with a DOM target. That needs the compiler or
  vite to map `svelte`'s `mount`/`unmount` to `$n.mount`, and the host to adopt the target. Medium.
- **DOM test-host attribute semantics** (attribute-if-string `translate`, form-default-value,
  input-type-before-value-spread): `dom-host.mjs` issues; low value.
- **error-boundary-reset-destroyed**: an async `transformError` (a promise); our boundary is
  synchronous. Medium.
- **hmr-dynamic-component**: an HMR output path; skip.

`{await p}` (FU3's diagnostic) is not a bucket. With `SVELTE_NO_ASYNC=true` the async samples are
not counted.

## Conflict risks with other lanes

- `crates/svelters-compiler/src/{codegen/component.rs, lower/{mod,ir}.rs, transform/{mod,visit}.rs, compile.rs}`:
  FU3 (diagnostics) and E3 (`native_type`).
- `docs/notes/native-compiler/{deviations,runtime-contract}.md`.
- `packages/native-runtime/{vite.js, src/index.js, src/core/*}`, and `tools/check-svelte-internals.mjs`.
- I did not touch `language-subset.md` (FU3's).
