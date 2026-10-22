# HMR: hot module replacement with state preservation

Status: research proposal, 2026-10-21. It answers "how advanced is our hot reload, what is the state of
the art in Svelte HMR, and what would full support for state preservation need in our platform and
compiler". Nothing here is implemented; one runtime probe was run (§3.1).

- **Facts vs proposals.** §1 and §2 are facts, each with its source: a repo file, the svelte@5.57.2
  source, a GitHub PR or issue, or official docs, as read on 2026-10-21. §3 mixes facts (marked with
  their source) and analysis. §4–§6 are proposals. Anything inferred rather than verified says so.
- **Builds on:** `plans/milestones/M2-abi-v2-devloop.md` (M2b dev loop), `docs/notes/m2b-devloop.md`,
  `plans/research/QUICKJS-RUNTIME.md` §3.4 (HMR "later", phase Q5), `plans/milestones/E3-exports-reactive.md`
  ("Persist and hot reload"), `docs/notes/styling-gtk-css-qss.md` §3.6 (CSS reload cost),
  `docs/notes/egui-scene-graph.md` §1.4 and `plans/research/EGUI.md` D4 (stable element identity),
  `docs/notes/custom-renderer-trial.md` §5 (stock `$.hmr` ran under QuickJS in the 2026 trial).
- **Probe:** `hmr-rt/test/hmr-probe.test.mjs` in this lane's scratchpad (not committed; it is reproduced
  in §3.1 and Appendix A is enough to redo it).

## Summary

- **Where we stand.** Our hot reload is a **full in-process restart of the JS world**: a new bundle is loaded
  into a fresh QuickJS runtime, the new app builds and presents its windows, then every old widget is destroyed.
  It is robust (a broken edit keeps the old version running, errors are source-mapped) and Rust state survives it
  (E3 services, app-wide stores, `persist()`ed objects). But **all JS state, every native widget and all native UI
  state are lost on every edit**: window geometry, focus, scroll, selection, open dialogs. An edit takes 1.5–2 s,
  of which ~1.3 s is Vite rebuilding the whole bundle. Svelte's own HMR is a different design: **per component,
  in place, in one runtime**. We emit none of it: the compiler has an `Hmr` enum that nothing reads
  (`crates/svelters-compiler/src/target.rs:24`).
- **Upstream Svelte 5 (5.57.2) does not preserve component state either.** `hmr: true` wraps each component in
  `$.hmr(fn)`. An edit **re-mounts that component in place** inside its parent:
  - kept: parent state, props and bindings (they live in the parent), ancestor contexts, module-level stores of
    modules that were not re-executed, and the parent's `bind:this` reference;
  - lost: the component's own `$state`, its whole subtree (children re-mount), and module state of re-executed
    `.svelte.js` modules.

  State preservation is the most-requested missing feature (issue #14434, open since 2024-11). The one attempt,
  PR #17995 (open), was rejected in review by Rich Harris for three reasons: label collisions between instances,
  initializer edits not resetting, and untagged sources.
- **The state of the art elsewhere** is React Fast Refresh and Flutter's stateful hot reload: keep the instance,
  key state by declaration identity, compute a **signature** (React includes `useState`'s initial-value source),
  **reset the component when the signature changes**, and re-run effects. Vue keeps state for template-only
  edits (`rerender`) and re-creates instances for script edits (`reload`). solid-refresh skips the re-mount when a
  component's code hash is unchanged and patches context identity across versions.
- **Our platform is well placed for more than upstream does.** A runtime probe (§3.1) shows that the native
  runtime's existing region swap (`$n.component`) already gives an upstream-style in-place swap: the parent's
  window, box and label get **zero ops**, only the edited component's widgets are replaced at the right position.
  A per-instance registry kept in the HMR wrapper's closure preserves `$state` per instance (two instances keep
  1 and 2: no collision). A changed initializer resets the state through its signature. These are exactly Rich
  Harris's first two objections, answered by construction. Our swap also builds the new instance *before* tearing
  down the old one, so a version that throws during init can be **rolled back** to the old instance. Upstream
  destroys first and cannot.
- **Recommendation:** five phases, H1–H5, plus a small H0 (§4).
  - **H1:** per-component in-place swap with Svelte 5 semantics, in one long-lived runtime fed per-module
    updates. The full reload stays as the fallback.
  - **H2:** CSS-only updates with no remount, setter rules included.
  - **H3:** `$state` preservation, on by default in dev, with signature reset, per-instance identity and a
    `// @hmr:reset` escape. Module-level `.svelte.js` state and `createContext` keys are kept too.
  - **H4:** native UI state (window geometry, scroll, focus, selection, pages) restored through a
    compiler-emitted stable element identity. This is the same identity EGUI D4 already approved.
  - **H5:** Rust-side interplay. Hot restart keeps plain-data state, and the full-reload fallback stays
    consistent with E3.

  None of H1–H3 needs an ABI change. H4 can be done with props alone, plus optional nullable members.
- **Main decision for the user (§6 D1):** how modules reach QuickJS in dev. Either run **Vite's dev server with
  Vite's own `ModuleRunner` inside QuickJS** (a self-contained 55 KB module with no imports that evaluates with
  `new AsyncFunction`, verified in vite@8.3.4), or wait for the **Rust module loader** of QUICKJS-RUNTIME Q1/Q5.
  Recommendation: probe the ModuleRunner for one day; if it runs, ship H1 on it, and keep the `import.meta.hot`
  contract identical so Q5 can replace it later.

## 1. What we have today (verified)

### 1.1 The reload (M2b)

`crates/svelters-host/src/reload.rs` (doc comment lines 1–17, `reload` at line 147):

1. A `notify` watcher thread on the bundle's directory debounces 120 ms (`DEBOUNCE`, line 36), sets a flag and
   calls the backend's thread-safe `wake`. The reload runs at the next checkpoint on the toolkit thread.
2. Old toplevels and timers are set aside and queued events dropped. E1 offloaded calls are generation-tagged
   (`ops_begin_generation`), E3 app objects are generation-tagged and the reactive stash is taken
   (`app_begin_generation`, `reactive_stash`).
3. A **fresh QuickJS runtime** is created and the bundle loaded (`core::load_bundle`, `core.rs:1177`). The new app
   creates and presents its windows while the old ones are still up, so the toolkit loop never sees zero windows.
4. On success: the old runtime's calls are cancelled, stale app handles released (E3 `app_release_stale`: persisted
   objects, app-wide stores and services keep theirs), the old runtime is dropped and the old toplevels destroyed,
   descendants first. A JS `Buffer<T>` still held by Rust pins the old runtime: a warning, then a 1 s wait and a leak (E2/E3).
5. On failure: whatever the failed load created is destroyed, the previous runtime, timers, generation and reactive
   stash are put back, and the source-mapped error is reported. The old version keeps running.

Styling: the first style op of a new bundle sends `SHEET` with `NULL`, which clears the previous bundle's sheets
(`packages/native-runtime/src/style.js:91`, `plans/architecture/backend-abi.md` line 483).

Measured in M2b on GTK/Qt (M2 completion record): edit to new UI 1.5–2 s, of which Vite's rebuild is ~1.3 s; host
debounce 0.12 s and load plus teardown 0.2–0.3 s (debug build). Dev loop: `pnpm watch <app>` (Vite `build --watch`,
`packages/build/bin/svelters-build.mjs`) plus the runner's `--watch`.

### 1.2 What survives a reload today

| Survives | Lost |
| --- | --- |
| The process and the toolkit loop | Every JS value: component `$state`, module state, stores written in JS, `setTokens`/`setColorScheme` choices |
| E3 services (`#[svelters::service]`), `StoreStatic`, app-wide stores, `persist(key, make)` objects (`E3-exports-reactive.md` "Persist and hot reload") | JS-created Rust objects that were not `persist`ed (their handles are released, `Drop` runs) |
| Native custom widgets registered with the backend (`CUSTOM-WIDGETS.md`) | Every native widget, so window position/size, focus, scroll offsets, text cursor and selection, notebook pages, paned positions, open dialogs |
| | Timers (old runtime's timers dropped; new ones start from zero), in-flight E1 calls (results dropped by generation) |

### 1.3 Tests

- `crates/svelters-host/tests/reload.rs`:
  - `errors_map_to_svelte_sources`;
  - `load_error_maps_to_svelte_source`;
  - `watch_build_edit_reloads_in_same_process`;
  - `failed_reload_keeps_the_process_and_recovers`.
- `examples/counter/tests/watch.rs`: `watch_reload_gtk`, `watch_reload_qt` (edit, rebuild, reloaded widget tree in
  the same process, `native_toplevels == 1`).
- E-series:
  - `crates/svelters/tests/reactive.rs`: `persisted_objects_and_app_stores_survive_a_reload` and
    `a_persisted_object_holding_js_memory_is_diagnosed_at_reload`;
  - `services.rs`: `services_start_with_the_app_survive_reloads_and_stop_at_shutdown`;
  - `offload.rs`: `reload_drops_results_of_the_old_runtime`;
  - `buffers.rs`: `reload_with_a_transferred_buffer_in_flight`.
- `MATRIX.md`: "Hot reload ✅ GTK, ✅ Qt, 🟡 AppKit, 🟡 WinUI (not tested)", JS-engine row "✅ in-process (GTK, Qt)";
  the G1 record lists egui hot reload as untested.

### 1.4 Compiler and conformance

- `svelters-compile` has no `hmr` output. `target.rs` defines `Hmr { Vite, HostReload, None }` in
  `DevHooks`, and nothing reads it. The Vite plugin (`packages/native-runtime/vite.js`) appends the
  component's style registration (`$$registerStyles(...)`, line 208) to the component's own JS module.
  So a `<style>` edit is a JS change, and a JS change is a full reload.
- **Conformance** (`experiments/native-compiler/conformance`; CF1 on `origin/master` a7cd805a):
  - **`hmr-removal` and `hmr-each-keyed-unshift` pass, but trivially.** The harness passes only
    `compileOptions.dev` through (`run.mjs:305`), so these samples run without any HMR wrapper. They become
    real tests once the compiler honours `hmr`.
  - **`hmr-dynamic-component` fails** (CF1 bucket F-harness/F-misc: "an HMR output path"). It imports the
    compiled child and calls `mod.default[HMR].update(...)`, which needs upstream's `HMR` symbol API.
  - **`component-transition-hmr` is an expected rejection** (R-transition: no transitions on native).

### 1.5 Earlier findings in the repo

- `custom-renderer-trial.md` §5 and `svelte-feature-survey.md`: in the renderer-fork era, stock `compile({dev, hmr})`
  plus an `import.meta.hot` rewritten to a host registry swapped a subtree in QuickJS. State reset, as upstream.
- `QUICKJS-RUNTIME.md` §3.4 and Q5 (3–5 d) plan component HMR on the future Rust module loader:
  - versioned re-import (`"<path>?v=<n>"`);
  - `import.meta` reachable from Rust (`Module::meta`);
  - a full-reload fallback.

  It notes, unverified, that QuickJS-NG never frees replaced modules, so a full reload is needed every N swaps.
- `styling-gtk-css-qss.md` §3.6: a GTK provider reload costs ~3 ms parse plus one restyle, and a Qt app-sheet
  re-set ~0.6–1 s per 1000 live widgets. Both are fine for HMR and need no widget re-creation.
- `egui-scene-graph.md` §1.4 measured that a compiler-provided stable identity keeps egui focus and the typing
  target across a reload with new handles, and handle-derived ids do not. EGUI.md D4 approved the identity prop
  (planned for G5).

## 2. State of the art

### 2.1 Svelte 5's built-in HMR (svelte@5.57.2)

**Compile option.** `hmr?: boolean`, default `false`, "compiles components with hot reloading support"
(`src/compiler/types/index.d.ts:187–192`). Codegen (`src/compiler/phases/3-transform/client/transform-client.js:512–529`;
snapshot test `tests/snapshot/samples/hmr`):

```js
function Hmr($$anchor) { /* component */ }

if (import.meta.hot) {
	Hmr = $.hmr(Hmr);
	import.meta.hot.accept((module) => {
		$.cleanup_styles('svelte-xyz');          // only when the component has scoped CSS
		Hmr[$.HMR].update(module.default);
	});
}
export default Hmr;
```

With `hmr`, a component that starts a fragment is never treated as static, so it always gets an anchor
(`3-transform/utils.js:289`). Custom elements are guarded with `customElements.get(tag) == null` (line 657).

**Runtime** (`src/internal/client/dev/hmr.js`):

- `$.hmr(fn)` creates `current = source(fn)` and returns a `wrapper(anchor, props)`. Each wrapper call, that is,
  each instance, runs a `block` effect (`EFFECT_TRANSPARENT`) that reads `current`.
- When `current` changes, the block **first destroys the old branch** (`destroy_effect`) and clears the old
  `instance` object's keys. It then creates a new `branch` that calls the new function with **the same anchor
  and the same `props` object**.
- Intros are suppressed during the swap (`set_should_intro(false)`).
- The returned `instance` object is kept and refilled with the new exports' property descriptors, so a parent's
  `bind:this={child}` keeps pointing at a live object.
- `wrapper[HMR].update(incoming)` sets the *first* version's source to `incoming[HMR].fn`, and points the
  incoming wrapper at the original source. Every later import of the module then drives the original blocks
  ("we'll create an ever-growing stack of block effects" otherwise).
- `HMR_ANCHOR` (`dom/blocks/branches.js:99–101`) forwards an anchor that a branch manager replaced (PR #18079,
  dynamic components).

**What survives an update of component C:**

| Kept | Reset |
| --- | --- |
| Everything outside C: parent state, sibling instances, the whole tree above | C's own `$state`, `$derived`, effects (old effects torn down, new ones run) |
| C's props: the same `props` object with the parent's getters, so current values and `bind:` targets | Every child component of C (re-created inside the new branch) |
| Ancestor contexts (the block runs under the parent's component context) | `<script module>` state of C (the module re-evaluates) |
| Module stores/state of modules not re-executed | `.svelte.js` modules that were edited (they are not self-accepting: the update propagates to importers, which re-execute) |
| The parent's `bind:this` instance object | Native/DOM UI state inside C (focus, scroll, selection) |

This matches Rich Harris's description in PR #17995 ("things simply reset to a known good starting point"), and
issue #14434 with its maintainers' replies.

**Failure handling.** The old branch is destroyed before the new one is built, so a new version that throws
during init leaves nothing, and the error goes to the nearest boundary. Two related PRs:

- #17680 ("catch errors thrown during component mount to prevent source from losing tracking") was closed
  unmerged.
- #19002 (open, 2026-10-14) only stops the accept callback reading `module.default` when Vite passes `undefined`
  for a failed update.

**vite-plugin-svelte** (v7.3.1 read from the repo's pnpm store; same behaviour in current docs):

- `compilerOptions.hmr` is on in `serve` when `server.hmr !== false` and off in production
  (`src/utils/options.js:211–215, 282–286`).
- `experimental.hmrPartialAccept` is enabled, and `import.meta.hot.accept(` is rewritten to
  `acceptExports(["default"], ...)` (`src/utils/compile.js:100–110`). A component with extra
  `<script module>` exports then propagates to importers that use those exports.
- **CSS-only updates:**
  - with `emitCss`, `*{}` is injected into every `<style>` (`compile.js:63–69`), so the scope class is on every
    element and CSS edits do not change the JS;
  - the CSS goes out as a virtual `?svelte&type=style&lang.css` module that Vite hot-swaps as CSS;
  - `hotUpdate` (`src/plugins/hot-update.js`) re-transforms the component and drops modules whose JS output is
    unchanged after normalising `$.add_locations` line numbers and `?t=` timestamps.

  So a style-only edit updates only the CSS module, with no remount.
- `.svelte.js`/`.svelte.ts` modules are compiled with `compileModule` and no HMR code
  (`src/plugins/compile-module.js`). An edit there propagates to the importing components, which re-mount.
  Module state resets, as users noted in #14434.

**History** (sveltejs/svelte PRs):

- #10100 (rixo, the svelte-hmr author, closed): a POC on `{#key}`. "If we wanted component level state
  preservation… this would surely not hold true. But previous experience has shown that this feature often
  proved more confusing than useful."
- #11106 (trueadm, merged 2024-04-10): "hot module reloading support for Svelte 5".
- #11112 added the `hmr` flag, #11132 simplified it, #11453 switched to `acceptExports`, and #12391 simplified it
  again.
- 2025–2026 fixes: #17380 (`await` loop), #17932 (boundary), #17975 (hydration), #17985 (start/end nodes),
  #18079 (dynamic components), #18655 (`onoutroend` twice).

**Upstream tests** (`packages/svelte/tests`):

- `runtime-runes/samples/hmr-removal`, `hmr-each-keyed-unshift`, `hmr-dynamic-component` and
  `component-transition-hmr`;
- `hydration/samples/css-props-hmr`;
- `snapshot/samples/hmr`.

None of them tests state.

### 2.2 State preservation in Svelte: history and the open request

**svelte-hmr (Svelte 3/4, rixo; `sveltejs/svelte-hmr`, "HMR commons for Svelte 3").**

- **Mechanism.** It re-created the edited component with `$$inject` props built from `cmp.$capture_state()`,
  which Svelte 3/4 dev builds emitted, together with `$inject_state`
  (`packages/svelte-hmr/runtime/svelte-hooks.js:27, 156–168`).
- **Default.** `preserveLocalState` (default `false`) has been off since 0.12: "too confusing and hard to
  anticipate".
- **Per-file overrides.** `// @hmr:keep-all` keeps a component's state, `// @hmr:keep` one variable, and
  `// @hmr:reset` (legacy `@!hmr`) forces a reset and wins over everything.
- **Caveats** from the README:
  - state is restored *after* init, so changing `let x = 1` to `let x = 10` after one increment leaves `x` at 2;
  - child components are re-created and lose their state;
  - side effects without a dispose handler (an un-cleared `setInterval`) keep running;
  - on a fatal init error it reloads the page (`noReload`, `optimistic`).
- **CSS.** It injects CSS instead of replacing the component when only the CSS changed (with compatible bundler
  APIs).

**Svelte 5.**

- **The request.** `svelte-hmr` was dropped, and `@hmr:keep` stopped working. sveltejs/kit#12985 asked about it;
  the maintainers removed the FAQ entry and pointed to a feature request.
- **Issue #14434**, "Svelte 5: Preserve local state during HMR" (open, `feature request`, 2024-11-25), has many
  "+1" comments (React and Flutter comparisons). The workarounds are:
  - a hand-written `import.meta.hot.data` snippet per variable;
  - a `vite:beforeUpdate` listener for `.svelte.ts` module state;
  - preprocessors that append the snippet to every component. These sync state *across all instances* of a
    component, which a commenter flags as a bug. Fixing it relies on mount order.
- **PR #17995**, "feat: preserve $state across HMR component swaps" (alexkahndev, open since 2026-03-23, last
  updated 2026-07-24). Its approach:
  - before `destroy_effect`, walk the old effect tree for sources with a dev `label` (`$.tag`);
  - put their values in a global `__hmr_preserved_state__`;
  - have `tag()` restore them during the new init.

  Rich Harris (2026-03-31):
  1. **Collisions.** "If I make an innocuous change to the first component, it will try to reinitialize with a
     random value of `count` from one of its children."
  2. **Changed initializers.** After changing the declaration from `$state(0)` to `$state(10)`: "In this case I
     would expect the value to be `10`, but instead it will be whatever the previous value happened to be."
  3. **Untagged sources** leave "a mixture of restored and non-restored state".

  He concluded: "while I'd love for state restoration to exist, there's a reason it doesn't. I don't know if these
  issues are solvable or not." The author pushed fixes:
  - per-component scoping via HMR block boundaries and a per-wrapper FIFO registry;
  - an `initial` deep-compare.

  The PR is unmerged as of 2026-10-21.

### 2.3 Other frameworks

| | Unit swapped | State kept | Reset rule | Effects | Sources |
| --- | --- | --- | --- | --- | --- |
| **React Fast Refresh** | component function, in the kept fiber tree | `useState`/`useRef` per hook position, also in children (fibers persist) | Babel computes a per-component signature: `hookName{lhs(initArg)}` joined, `useState`'s first argument and `useReducer`'s initial state included, plus custom hooks' signatures (`ReactFreshBabelPlugin.js` lines 257–268, 427–436). The runtime remounts when the signature differs, on class components, kind changes (memo/forwardRef), `/* @refresh reset */` (`ReactFreshRuntime.js` `canPreserveStateBetween`) | `useEffect`/`useMemo`/`useCallback` **always re-run** on refresh, deps ignored | reactnative.dev/docs/fast-refresh; facebook/react `packages/react-refresh` |
| **Module rules (React)** | A module that exports only components accepts itself. One with other exports re-runs its importers. One imported outside the React tree does a full reload | | | | same |
| **Errors (React)** | Syntax error: fix and save, no reload. Runtime error in render: remount with the new code. Error boundaries "retry rendering on the next edit" | | | | same |
| **solid-refresh** | Component behind a signal proxy, per component registry (`$$registry`, `$$component`) | Partial: **unchanged components are not re-mounted**, because `signature` is a hash of the component's code and its foreign `dependencies` are compared (`src/runtime/index.ts` `patchComponent`) | Changed signature or deps: re-mount, so local signals reset. `@refresh skip` / `@refresh reload` | n/a | github.com/solidjs/solid-refresh |
| **solid contexts** | `$$context` patches the new context's `id`/`Provider` to the old ones, so consumers keep working across a provider-module edit (`patchContext`) | | | | same |
| **Vue (core `hmr.ts` + plugin-vue `handleHotUpdate.ts`)** | Template-only edit: `rerender(id, render)` swaps the render function on live instances | **All state kept** (instance untouched) | Script edit: `reload(id, comp)` marks the definition dirty and **forces the parent to re-render, so instances are re-created** (state lost). A root instance triggers `appContext.reload()`. A new or removed style block, or a scoped-status or `v-bind` CSS-var change, forces a reload | re-created with the instance | vuejs/core `packages/runtime-core/src/hmr.ts`; vitejs/vite-plugin-vue |
| **Flutter stateful hot reload** | Dart VM injects changed libraries, then the framework rebuilds the widget tree | **All `State` objects kept**. `main()` and `initState()` are not re-run. **Globals and statics are state**: their initializers are not re-evaluated (`const` is) | Hot restart (state lost) is needed for enum↔class and generic signature changes, and native code. "If code changes affect the state… the data… might not be fully consistent" | `build()` re-runs | docs.flutter.dev/tools/hot-reload |

### 2.4 Techniques worth taking

1. **Instance identity comes from the swap point, not a global label.** React keeps the fiber, Svelte the HMR
   wrapper's block (one per call site and instance). Storing captured state *in the wrapper's closure* makes
   collisions impossible. This is the fix for objection 1, and it is what the PR's second revision approximates
   with a FIFO.
2. **Key state by declaration identity, gate it with a signature.**
   - Key: the binding name of a top-level instance-script `$state` declaration. Those run exactly once per
     instance. `$state` in functions, loops, class fields and closures is never keyed.
   - Signature: the ordered list of keyed declarations with their kind (`state`, `raw_state`, `proxy`) and the
     **normalized source of their initializer**, as React does for `useState`. A changed initializer changes the
     signature, so the state resets (objection 2).
   - **All or nothing per component**: React resets the whole component when any hook signature differs, because
     partially restored state can be inconsistent (objection 3). Untagged sources cannot occur, because the
     compiler, not a dev label, decides what is keyed.
3. **Restore before first use, not after init.** svelte-hmr's "restored after init" made `let x = 1; x++` misbehave.
   The restore must replace the initializer, so the initializer and its side effects do not run when a value is
   restored.
4. **A template-only edit is always safe** (Vue `rerender`): the script and its declarations are byte-identical,
   so the signature is unchanged by construction.
5. **An unchanged component is not re-mounted.** solid's code hash and vite-plugin-svelte's normalized-output
   comparison do this. Svelte has one component per file, so the equivalent is "drop the update if the compiled JS
   is unchanged".
6. **Effects re-run, derived values recompute** (React): the new instance runs new code against old state.
7. **Globals are state** (Flutter): module-level `$state` and stores keep their values across a module re-run
   when their declaration signature matches. Context keys keep their identity (solid's `patchContext`).
8. **Escape hatches**: a per-file reset pragma (`@refresh reset`, `@hmr:reset`), and a global off switch.
9. **Errors never cost the session**: keep the previous version, retry on the next edit. Fall back to a full
   reload only when the module graph can't be updated (e.g. a module imported outside any accepting boundary).

### 2.5 Other advanced features (catalogue)

| Feature | Svelte 5 + Vite | Others | Us today | Plan |
| --- | --- | --- | --- | --- |
| CSS-only update, no remount | ✅ (`*{}` trick + CSS module) | Vue style modules; svelte-hmr inject | ❌ (style registration lives in the component JS) | H2 |
| Per-component in-place swap | ✅ | React, Vue, Solid | ❌ (whole app) | H1 |
| Local `$state` preserved | ❌ (PR #17995 open) | React ✅, Flutter ✅, Vue template-only ✅ | ❌ | H3 |
| Module-level state (`.svelte.js`, stores) preserved | ❌ (importers re-run, state resets) | Flutter ✅ (globals are state) | ❌ | H3 |
| Context identity across module edits | ❌ (`createContext` makes a new key `{}` per evaluation: `src/internal/shared/context.js:10`) | solid `patchContext` ✅ | ❌ | H3 |
| Partial module-graph update | ✅ (Vite propagation, `acceptExports`) | all | ❌ (bundle) | H1 |
| `import.meta.hot` `data`/`dispose`/`prune`/`invalidate`/events | ✅ (Vite API) | webpack equivalent | ❌ (no `import.meta.hot`) | H1 |
| Effects and timers across a swap | effects with cleanup ✅; un-cleared timers leak | React re-runs all effects | full reload drops all timers (safe but blunt) | H1 (dev timer attribution) |
| Error overlay / recovery | Vite overlay for transform errors; runtime errors go to the console; failed init leaves the slot empty | React redbox, retry on next edit | console + `ReloadReport` hook; old version keeps running | H1 (rollback per component), overlay decision D9 |
| Scroll position, focus, selection | ❌ inside the swapped component (DOM re-created); browser keeps page scroll | Flutter keeps it (render objects kept) | ❌ (all widgets re-created) | H4 |
| Window geometry | n/a (the page stays) | n/a | ❌ (windows re-created) | H0/H4 |
| Unchanged output skipped | ✅ (`hasCodeChanged`) | solid signature | ❌ (any rebuild reloads) | H1 |
| Rust/native code changes | n/a | Flutter: full restart for native code | process restart (Cargo); services etc. restart | H5 (hot restart keeps plain-data state) |

## 3. Our platform: what full support needs

### 3.1 Architecture: one runtime per edit, or one runtime for the session

Svelte's HMR needs **one long-lived runtime** that receives new module versions. We create a **fresh runtime per
edit**. There are two ways to keep native widgets across an edit.

| | A: per-module updates in ONE runtime | B: fresh runtime + state transfer |
| --- | --- | --- |
| Native widgets | Untouched outside the edited component, same handles | All re-created, unless the new runtime "adopts" existing handles, which amounts to a native hydration pass over the whole tree (§3.7) |
| JS state transfer | By reference: the same proxies, objects, closures and app-object wrappers | Only plain data (structured clone through Rust). Functions, class instances, native nodes and app-object wrappers are lost or re-minted |
| Granularity | Per component (H1), per declaration (H3) | Whole app; identity must be reconstructed from scratch |
| Fits Svelte's model | Yes: `$.hmr` semantics, upstream tests apply | No |
| Loader work | `import.meta.hot`, versioned module names, accept/propagation | None beyond today's |
| Risk | QuickJS module records accumulate (to measure); old closures linger until GC | Small; it is today's design |
| E3/E1/E2 | Runtime never changes, so no generation churn for component edits | Today's generation machinery |

**Recommendation: A for edits that can be accepted, and B (today's reload) as the fallback** for edits that cannot:
the runtime itself, the entry, a module imported outside any boundary, too many swaps, or an `invalidate()`
reaching the root. That is also Vite's own split (`hot update` vs `full-reload`).

**Probe (2026-10-21).** I copied this branch's `packages/native-runtime/src` into the scratchpad and ran it on the
Node TestHost with stock svelte 5.57.2. I wrote a prototype `hmr(fn)` on the existing `$n.component` region swap
(Appendix A), and a `keep(key, sig, init, …)` standing in for H3's codegen. A parent (window > box > label + two
`Child` instances) mounted, then `a` was incremented once and `b` twice. The op log:

```
swap v1 -> v2 (template edit, same state signature):
  create label #6 / set #6 label "v2 a: 1" / insert #2 #6 before=#5 / remove #2 #4 / destroy #4
  create label #7 / set #7 label "v2 b: 2" / insert #2 #7 before=end / remove #2 #5 / destroy #5
swap v2 -> v3 (initializer `$state(0)` -> `$state(10)`: signature changed):
  create label #8 / set #8 label "v3 a: 10" / ... / set #9 label "v3 b: 10" / ...
after: set #8 label "v3 a: 11"     (the new instance's handlers are live)
```

What the probe shows:

- The window (#1), box (#2) and parent label (#3) received no ops.
- The new widgets were inserted at the right position (`before=#5`), and each instance kept its own value. Rich
  Harris's collision case does not arise.
- An initializer edit reset the state.
- The new branch is built before the old one is removed, which is the native runtime's `swap` order
  (`packages/native-runtime/src/index.js:884–945`). That is what makes rollback on a throwing init possible.

The probe ran on Node, not QuickJS. QuickJS runs the same runtime code in every fixture today
(`oplog_replay`).

### 3.2 Delivery: how module updates reach QuickJS

Today: `svelters-build --watch` (Vite `build --watch`) writes one self-contained `dist/app.js`, and the host
watches that file. A bundle has no module identities left, so it can't drive per-module HMR. The options:

1. **Vite dev server + Vite's `ModuleRunner` inside QuickJS.** Verified in vite@8.3.4:
   - `dist/node/module-runner.js` is 55 KB, has **no imports**, and implements the HMR client: `HMRContext` with
     `accept`/`acceptExports`/`dispose`/`prune`/`data`/`invalidate`/`on`/`send`, plus full-reload handling.
   - It evaluates modules with `new AsyncFunction` (`ESModulesEvaluator`). Our context is `Context::full`
     (`quickjs.rs:178`), which includes eval.
   - The transport is pluggable (`ModuleRunnerTransport { connect, send, invoke, timeout }`); HMR needs `connect`.
     The host would bridge it, either to a child-process Vite over stdio JSON lines or to a Rust WebSocket client.
   - The Environment API is "release candidate" per the Vite docs, with some APIs still experimental.
   - Gains:
     - Vite's propagation, `acceptExports`, error events (`vite:error`) and per-module transforms (only the
       edited file is recompiled; the 1.3 s whole-bundle rebuild goes away);
     - the existing `svelteNative` plugin, aliases and `@svelters/style` unchanged.
   - Costs:
     - a Node process in the dev loop (already true: `pnpm watch`);
     - an IPC round trip per module at startup;
     - inline per-module source maps that must reach `sourcemap.rs`, or be handled by the runner's
       `prepareStackTrace` interceptor (`Error.prepareStackTrace` CallSites work on QuickJS-NG: `QUICKJS-RUNTIME.md`
       §3.2).

   Not verified: that the runner runs unmodified on QuickJS-NG, or how a custom environment's `consumer` setting
   interacts with our plugin (vite-plugin-svelte's HMR is client-consumer only; ours is our own plugin).
2. **Rust module loader** (QUICKJS-RUNTIME Q1 + Q5).
   - A `svelters-modules` crate holds the module graph, resolves imports with `oxc_resolver`, and compiles
     in-process with `svelters-compiler`. The loader defines `import.meta.hot` through `Module::meta` and
     dynamic-imports `"<path>?v=<n>"`.
   - Vite leaves dev only after Q3 (the CSS scoping and translator port), so a style-using app keeps Vite until
     then.
   - Size: Q1 3–4 d + Q5 3–5 d (+ Q3 4–6 d for style-using apps).
3. **Vite `build --watch` with per-module output** (`preserveModules`) and our own diff of the output directory.
   **Rejected.** We would re-implement Vite's HMR graph badly, and every rebuild still pays for a full build.

Both 1 and 2 need the same **JS-side contract**: `import.meta.hot` with Vite's API and a per-module registry, so
generated code is identical. Choose 1 for speed to value and 2 for the Node-free dev loop (§6 D1).

**QuickJS module records.**

- A module name is instantiated once per context. A new version needs a new name (`?v=n`, or the runner's own
  ids). The runner evaluates functions, not module records, which sidesteps the question.
- Replaced versions are believed never to be freed while the context lives (QUICKJS-RUNTIME §3.4, unverified).
- So: **measure in H1** (memory per swap for a 50-component app), and force a full reload (B) every N swaps or
  above a memory threshold (QUICKJS-RUNTIME open question 9).

### 3.3 Compiler (`svelters-compiler`)

What an `--hmr` (dev-only) compile must emit:

1. **The wrapper, upstream-shaped.**

   ```js
   if (import.meta.hot) { C = $n.hmr(C, "<id>"); import.meta.hot.acceptExports(["default"], (m) => m && C[$n.HMR].update(m.default)); }
   ```

   `$n.HMR` is the same symbol object as svelte's `HMR` (re-exported through the core adapter), so the upstream
   sample `hmr-dynamic-component`, which calls `mod.default[HMR].update(...)`, runs unchanged. The callback
   checks `m` because Vite passes `undefined` for a failed update (Vite HMR docs; upstream #19002).
2. **A stable component id**: the file path relative to the app root, as the CSS scope hash already uses
   (`vite.js` `nativeSheets`). It is used for the registry and diagnostics.
3. **A component state signature** (H3): a hash of the ordered keyed declarations `(name, kind, normalized
   initializer source)`, emitted as one constant per component. `normalized` drops whitespace and comments.
   - Changes inside an initializer's *callees* are not seen. React has the same limit, and the per-file reset
     pragma covers it.
   - `// @hmr:reset` (Svelte-community spelling; React uses `@refresh reset`) emits a signature that never
     matches.
4. **Keyed declarations** (H3), at the site where dev mode already wraps declarations in `$.tag`
   (`src/transform/visit.rs:261–320`).
   - Top-level instance-script `let x = $state(init)` becomes
     `let x = $.state($n.hmr_keep("x", () => init))`.
   - `$state.raw` gets the same treatment, and proxies keep the restored proxy object itself (`$.proxy(v)`
     returns an existing proxy unchanged).
   - Excluded: `bind:this` targets (they would restore a dead native node; the binding re-assigns the new node
     anyway), `$derived` (recomputed), `$props()` (owned by the parent), and `$bindable` fallbacks while the
     parent does not bind them (D5 below). Writable `$derived` overrides are excluded too.
5. **Module-level state** (H3): `.svelte.js` top-level `$state`, `writable(...)` stores and `createContext()`
   calls get the same `hmr_keep` treatment, keyed in `import.meta.hot.data` by declaration name + signature.
   The module stays non-self-accepting, so importers re-run as upstream does, but they find the old values,
   the old store objects and the old context keys.
6. **CSS out of the component module** (H2): the plugin emits `registerStyles` as a separate virtual module
   (`<file>?svelters-style`) that self-accepts. The component JS no longer changes when only CSS declarations
   change, because every native element already gets the scope class when `--css-scope` is set (checked with
   `svelters-compile --css-scope`: `label` and `entry` both get `svelte-abc` for a `label { }` rule).
7. **Setter declarations become data in HMR mode** (H2). Today `box { spacing: 12 }` compiles into
   `$n.set($$n1, "spacing", 12)` in the component JS (checked). With `--hmr` the compiler instead emits the
   setter rules as runtime rules (`registerStyles(id, { rules })`). The native runtime already matches those
   against tag, class and id, records the native value before the first override, and writes it back when no
   rule applies (`packages/native-runtime/src/style.js` header). Release builds keep compile-time lowering.
8. **Unchanged output → no update.** The plugin compares normalized output before and after (as
   vite-plugin-svelte's `hasCodeChanged`), so a save with no semantic change does nothing.
9. **An element identity** (H4): an optional reserved prop per native element, a hash of the component id and the
   static template position. At runtime the runtime combines it with the each-block key path and the component
   instance path. This is EGUI D4; HMR is its second user.

None of this changes the release output: everything sits behind `import.meta.hot`, or the `--hmr` flag that
only dev sets. That also keeps hot reload out of App Store builds (`apple-gui-backend.md` §2.2).

### 3.4 Native runtime (`@svelters/native-runtime`)

- **`hmr(fn, id)`.** The probe's wrapper, hardened:
  - a `$n.component`-style region swap;
  - upstream's "first version's source drives every update" rule (`wrapper[HMR].update`);
  - the `instance` object refilled for `bind:this`;
  - `FILENAME`.

  It needs no new Svelte internals: `$n.component` already uses `block` and `branch` through the core adapter, so
  `tools/check-svelte-internals.mjs` covers it.
- **Rollback.** If the new function throws during init, destroy the partial new branch and keep the old one. Then
  report the error through the host (mapped) and mark the module "failed" so the next edit retries. Upstream
  can't do this (it destroys first). React's "retry on next edit" is the model.
- **State registry (H3).**
  - The wrapper closure holds `prev: Map<key, {read}>` and the signature of the live instance.
  - `hmr_keep(key, init)` reads it during the new init. With a matching signature it returns the old value and
    skips `init`; otherwise it calls `init()`.
  - The registry for the next swap is built as declarations run.
  - Instances in each blocks are separate wrapper calls, so they are separate registries.
- **Children of an edited component** (H3c, optional). Upstream and svelte-hmr reset them, React keeps them.
  Keeping them needs a positional key for each child wrapper call: the parent's instance key, the static call-site
  id, the if-branch, the each key, and an index among calls at that site. A child's captured registry is then
  parked under that key while its parent re-renders, and claimed by the new child at the same position. Measure
  before committing to it (D6).
- **Timers (dev).** `setTimeout`/`setInterval`/`requestAnimationFrame` live in the host prelude
  (`crates/svelters-host/src/prelude.js:51–82`). In dev, the runtime can attribute each timer to the active
  ownership `Scope` (the runtime already tracks natives and handlers per scope) and clear the ones a destroyed
  instance left behind, with a one-time warning naming the component. Upstream and svelte-hmr leak them.
- **Effects.** They are re-created with the instance: old teardowns run, new effects run once (React
  semantics). Effects of components that were not swapped do not re-run.
- **Context.** Ancestor contexts are untouched, because the block runs under the parent's context. Contexts set by
  the swapped component are set again by the new init. `createContext` keys survive module re-runs through
  `hmr_keep` (§3.3 item 5).
- **Styles.** `registerStyles(id, …)` already replaces a sheet in place by id and re-sends it
  (`style.js:127–139`). Setter rules re-collect and re-apply to all elements (`collectRules`, `markStyleDirty`).

### 3.5 Host (`svelters-host`)

- **Dev module path.** Either the ModuleRunner bridge (transport ops and a startup entry that imports the runner,
  §3.2-1) or the Rust loader. Production keeps `load_bundle`.
- **`import.meta.hot` delivery.** One dev-only built-in (e.g. `svelters:hot`), or the runner's own context.
  Updates arrive as transport messages. The host wakes the toolkit thread with the same wake path the watcher uses
  today, and applies them at a checkpoint, never while JS runs (as `reload`).
- **Fallback.** A Vite `full-reload` message, an error in the accept path, or the swap budget calls
  `Reloader::reload`. It keeps every E-series guarantee unchanged.
- **Source maps.** Maps are kept per module id instead of one `<bundle>.map` (`sourcemap.rs`). This also fixes the
  M2b open item, where a failed reload replaced the running bundle's map.
- **`ReloadReport` for HMR**: which modules, accepted or fallback, rollback or not, elapsed. Tests and a dev
  overlay can use it.
- **E3 interplay** (H5). In single-runtime HMR the runtime is not replaced, so:
  - `svelters:app/*` modules are never re-evaluated (their names are not versioned) and Rust objects keep their
    handles. No generation churn.
  - A swapped component's JS-created app objects die when nothing references them. `hmr_keep` keeps them if they
    are in keyed `$state`, and `persist()` keeps working as before.
  - Store observations are refcounted per (handle, prop) (R1), so the old instance unsubscribes and the new one
    subscribes. Nothing is delivered twice.

  The full-reload fallback keeps today's generation-aware release and its `Buffer<T>` diagnostic.
- **Hot restart with state** (H5).
  - Before a full reload, or a process restart for a Rust change, the runtime serializes every live registry
    (component instance paths from H3c plus module data) as plain data (`$.snapshot`), and the host keeps it.
  - The new runtime's `hmr_keep` reads it by key and signature.
  - Values that do not round-trip (functions, class instances, native nodes, app objects) are dropped per key
    with a dev note.

  This is the "B" path, used only when A cannot apply.

### 3.6 Styles per backend (H2)

| Backend | CSS declarations | Setter declarations (style table) | Cost per update |
| --- | --- | --- | --- |
| GTK | `SHEET` replaces one `GtkCssProvider` per component id | runtime rules via `setProp` | ~3 ms parse + one display restyle (`styling-gtk-css-qss.md` §5.2) |
| Qt | `SHEET` re-renders the app stylesheet once per loop turn | runtime rules | ~0.6–1 s per 1000 live widgets (software rendering), batched per update |
| AppKit, WinUI | none (no stylesheet: MATRIX "style-table setters only") | runtime rules; revert writes back the recorded native value | `setProp` per affected element |
| egui | `SHEET`/`SCHEME` accepted as no-ops (`svelters-egui/src/backend.rs:672–676`); layout props in `<style>` are setters (G1 D3) | runtime rules | next frame |

Runtime setter rules are therefore the CSS-HMR path for AppKit, WinUI and egui. They also cover the GTK and Qt
layout props.

### 3.7 Native-only state (H4)

A component swap re-creates only that component's widgets, so H1 already keeps all native state outside it. Inside
it, and on every full reload, these are lost:

| State | GTK | Qt | AppKit | WinUI | egui |
| --- | --- | --- | --- | --- | --- |
| Window size / maximized / position | default size, `maximized`; no position on Wayland | `geometry`, `windowState` | `frame` | `AppWindow` size/position | eframe viewport |
| Scroll offset | `GtkAdjustment:value` of the scrolled window | `QAbstractScrollArea` scroll bar `value` | `NSClipView` bounds origin | `ScrollViewer.ChangeView` (method, not a prop) | `ScrollArea` state in `Memory` by `Id` |
| Focus | `grab_focus()` (method), `has-focus` read | `setFocus()`, `hasFocus` | `makeFirstResponder:` | `Focus(FocusState)` | `Memory` focus by `Id` |
| Text cursor / selection | `GtkEditable:cursor-position`, `selection-bound` | `cursorPosition`, `setSelection` | field editor `selectedRange` | `SelectionStart`/`SelectionLength` | `TextEdit` state by `Id` |
| Pages, panes, expanders | `GtkNotebook:page`, `GtkPaned:position`, `GtkExpander:expanded` | `currentIndex`, splitter `sizes` | tab view selection, split view positions | `SelectedIndex`, … | collapsing state by `Id` |

(The table names the usual mechanism per toolkit from the toolkits' APIs. Which props our metadata exposes and
observes is for the H4 ledger to verify.)

Two ways to keep it:

1. **Capture and restore by identity (recommended).**
   - Before a branch is closed in an HMR swap, or before a full reload, the runtime reads a per-kind list of
     **ephemeral UI state** for every element with an identity: props via `getProp`, methods via a
     metadata-declared getter.
   - After the new branch is built, it restores them on the element with the same identity and kind, *unless the
     new render set that prop explicitly*. Restores run in a fixed order: size, then content, then scroll, then
     selection, then focus last.
   - The per-kind list is a new fact class in the metadata ledger: `ephemeral: [prop|method...]`, inferred and
     then verified per toolkit like R0's `observe` facts.
   - egui needs only the identity itself: the backend uses it as the `egui::Id`. Measured in
     `egui-scene-graph.md` §1.4.
2. **Widget reuse ("native hydration").** The new render *adopts* the old widget with the same identity and kind
   instead of creating one. This keeps everything, including state no prop exposes, but any prop the old version
   set and the new one does not would stay behind unless reset to a default from the metadata. Defer; revisit if
   (1) misses too much.

**ABI.** (1) needs no ABI change: `getProp`, `setProp` and `call` exist, and the identity is a reserved prop as
EGUI D4 describes. Optional nullable members can come later for state no prop or method reaches:

- `snapshot_state(h) -> value`;
- `restore_state(h, value)`.

Window geometry is the most visible loss. App.svelte usually *is* the `<window>`, so every App edit re-creates
the window today and also under H1. It is worth doing first (H0) for toplevels only: identity = component id +
position among toplevels; restore size, maximized and, where the platform allows, position.

### 3.8 Backends and the ABI

- **H1–H3: none.** The swap uses `create`, `insert` with `before`, `remove` and `destroy` exactly as `{#if}`
  does. The probe's op log contains no op the backends lack.
- **H2: none.** `style` (ABI v2.3) `SHEET` already replaces by id.
- **H4:** a reserved identity prop (universal, no ABI change), plus the optional nullable members above.
- **Reparenting across a swap** (moving an old widget into the new tree) is *not* needed for any phase. It would
  only matter for widget reuse (H4 option 2).

## 4. Plan

Sizes are lane-days in the repo's style; each phase lists its tests. The phases depend on each other in order,
except that H2 can run in parallel with H1's second half, and H4 needs only H1.

### H0: full-reload hardening (2–3 d, no dependencies)

- Window geometry carry-over across today's reload: toplevels matched by component id + position among toplevels.
  Restores size, maximized and position where the platform allows. Use host-side reads before teardown.
- Reload tests on AppKit, WinUI (CI) and egui (kittest), to turn MATRIX 🟡/untested into ✅.
- Keep the old runtime's source maps after a failed reload (M2b open item).
- Tests:
  - `counter --watch` on each backend asserts the same window size after an edit;
  - an egui kittest reload keeps the window size;
  - a failed-then-good reload maps stack positions correctly.

### H1: per-component in-place swap, Svelte 5 semantics (6–8 d with delivery option 1; +Q1/Q5 if option 2)

- Delivery per D1: ModuleRunner bridge (probe day 1: the runner on QuickJS-NG through a stdio transport), or the
  Rust loader.
- Compiler `--hmr`:
  - the wrapper (§3.3 items 1–2);
  - unchanged-output suppression (item 8);
  - the `Hmr` dev hook in `target.rs` drives it.
- Runtime `hmr()` with rollback, `HMR`/`FILENAME` compatibility, and dev timer attribution.
- Host:
  - updates applied at a checkpoint, with the full-reload fallback (Vite `full-reload`, a failed accept chain,
    the swap budget);
  - per-module maps and the HMR `ReloadReport`.
- Measure: edit-to-UI time (target well under 0.5 s), and QuickJS memory per swap → the swap budget.
- Tests:
  - **Conformance:** pass `hmr: true` through (`run.mjs`). `hmr-removal`, `hmr-each-keyed-unshift` and
    `hmr-dynamic-component` must pass *with* the wrapper (the last moves from failing to passing).
    `component-transition-hmr` stays rejected.
  - Port upstream PR #18079's scenario: a dynamic component swapped while it is being replaced.
  - **Runtime unit tests** (`packages/native-runtime/test/hmr.test.mjs`, the probe's shape):
    - parent handles untouched;
    - insertion position at the start, middle and end of a slot;
    - each-block instances swap independently;
    - `bind:this` instance object refreshed;
    - a throwing new version rolls back and the next good version applies;
    - a timer without cleanup is cleared with a warning.
  - **Fixture** `fixtures/hmr` with a scripted swap on the log backend (op-log golden), replayed on GTK and Qt by
    `oplog_replay`.
  - **Integration**:
    - GTK/Qt `watch` tests edit a child component and assert the window handle and the parent's widget handles
      are unchanged, and the child's text changed;
    - an edit to a `.svelte.js` module re-runs its importers;
    - an edit to the entry falls back to a full reload.

### H2: CSS-only updates (2–3 d, after H1's compiler work)

- Styles in a separate self-accepting module, and setter declarations as runtime rules under `--hmr` (§3.3
  items 6–7, §3.6).
- Tests:
  - editing a colour emits only `style sheet` ops (no `create`/`destroy`) and the GTK computed colour / Qt
    `grab()` pixel changes (reuse X6's read-back helpers);
  - editing a setter (`spacing`) emits only `set` ops on matching elements;
  - removing it writes back the native value;
  - AppKit and WinUI setter-rule HMR on CI;
  - adding a selector that matches a new element still updates only styles (scope class on every element).

### H3: `$state` preservation (4–6 d; H3c +3–4 d, optional)

- **H3a, components:**
  - keyed declarations, the component signature, `hmr_keep`, per-instance registries;
  - `// @hmr:reset` per file and a global switch (the build option `hmr: { preserveState }`);
  - default per D2.
- **H3b, modules:** `.svelte.js` top-level `$state`, stores and `createContext` keys kept through
  `import.meta.hot.data`.
- **H3c, children of an edited parent:** positional keys (D6).
- Tests:
  - **Rich Harris's two cases verbatim, as conformance-style samples** (`hmr-preserve-state-*`):
    - (1) a parent with `{#each Array(count)}` of `<Counter/>`: editing the parent keeps `count`, and children
      never receive a parent's value;
    - (2) `$state(0)` → `$state(10)` resets to 10.
  - Two instances keep separate values (the probe).
  - Adding, removing or renaming a declaration resets the component.
  - A proxy value is the same object after the swap.
  - `$derived` recomputes with new code against kept state.
  - Effects re-run once.
  - A `bind:this` target is not restored.
  - `@hmr:reset` always resets.
  - A `.svelte.js` counter keeps its value while its importers re-run.
  - A `createContext` provider-module edit keeps consumers working (no `missing_context`).
  - Upstream PR #17995's `hmr-preserve-state` sample.

### H4: native UI state (4–6 d, after H1; shares the identity prop with EGUI G5)

- Compiler element identity (§3.3 item 9), and the runtime identity path (each keys, instance path).
- The `ephemeral` fact class in metadata (GTK/Qt probes, AppKit/WinUI inferred until CI-verified).
- Capture and restore around HMR swaps and full reloads.
- egui uses the identity as `egui::Id`.
- Tests per backend: scroll a list, focus an entry and select text, switch notebook page, resize the window; edit
  the component; assert the same values after the swap. The egui kittest repeats `egui-scene-graph.md`'s
  focus-and-typing check across an HMR swap.

### H5: Rust-side interplay and hot restart (2–3 d, after H3)

- Plain-data snapshot of all registries across the full-reload fallback and across a process restart (`cargo run`
  after a Rust edit), restored by key and signature (§3.5).
- Document the rules:
  - Rust objects survive HMR without `persist`;
  - `persist` is only needed for the full-reload path;
  - a Rust edit is a restart.
- Tests:
  - E3's `persisted_objects_and_app_stores_survive_a_reload`, extended with an HMR swap in between: no extra
    store delivery, no released handle;
  - a full-reload fallback restores a component's plain `$state` and drops a function-valued one with a note;
  - a restarted host restores state from the snapshot file.

### Gates

The repo's: `pnpm test`, `pnpm docs:test`, `cargo-slot xvfb-run -a cargo nextest run --workspace`, the conformance
check, and the CI workflows for AppKit/WinUI. Docs: the site's "Hot reload" rows
(`guides/svelte-support.md:112`, `guides/styling.md:275`), and MATRIX rows split into "full reload", "component
HMR", "state preserved", "native state preserved".

## 5. Risks

- **Coupling to Svelte internals.** `hmr()` uses `block`/`branch`/`destroy` through the core adapter, as `{#if}`
  does, so `check-svelte-internals.mjs` covers it, and RUNTIME.md phase 2 (own core) will carry it. Upstream's
  `hmr.js` is not used, so its fixes must be tracked by reading upstream changes (dynamic components, boundaries,
  `await`, offscreen anchors: §2.1 list).
- **Inconsistent kept state.** A logic edit that changes what a state value *means* without touching its
  declaration (Flutter's warning) keeps a now-wrong value. Mitigations: the per-file reset pragma, a global
  switch, and a dev "reset state" command (H1 overlay / CLI key).
- **Stale identities in kept values.** A kept object can be an instance of the *old* version of a class from a
  re-executed module, so `instanceof` the new class fails. A kept closure runs old code. React has the same issue.
  Proxies and plain data are fine. Document it, and reset when a kept value's prototype comes from a replaced
  module (detectable in dev: the module registry knows replaced classes).
- **QuickJS memory growth** per swap (unmeasured), handled by the swap budget.
- **Toplevels in swapped components** are re-created (new window, then the old one closed) until H4 restores
  geometry. Dialogs that were open inside a swapped component close and reopen if their condition still holds.
- **Two dev paths** (ModuleRunner now, Rust loader later). Contained by one `import.meta.hot` contract and one
  test suite run against both.
- **ModuleRunner on QuickJS** may need small shims (e.g. `AbortController`, `performance`, `URL`). The 1-day
  probe decides.
- **Upstream divergence.** If upstream lands state preservation with other semantics, we either follow (codegen
  hooks are ours) or keep ours. Since the wrapper is ours, the upstream API we depend on is only `HMR`/`FILENAME`.

## 6. Open decisions

Each is a question with options and a recommendation.

1. **D1: delivery of module updates to QuickJS.**
   - (a) Vite dev server + `ModuleRunner` in QuickJS over a host transport;
   - (b) the Rust module loader first (QUICKJS-RUNTIME Q1+Q5, Vite stays for CSS until Q3);
   - (c) both: (a) now, (b) replaces it.

   **Recommendation (c):** a 1-day probe of (a). If it runs, H1 ships on it, and Q5 later implements the same
   `import.meta.hot` contract.
2. **D2: is `$state` preservation on by default?**
   - (a) default on in dev (React, Flutter);
   - (b) opt-in per file or variable (svelte-hmr's `@hmr:keep`);
   - (c) off, as upstream.

   **Recommendation (a)**, with `// @hmr:reset` per file and a global switch. The signature and per-instance
   identity remove the reasons svelte-hmr turned it off. Desktop apps lose more on a reset (open windows,
   dialogs, deep navigation) than a web page that reloads anyway.
3. **D3: behaviour when the state shape changes.**
   - (a) reset the whole component (React);
   - (b) reset only changed or added declarations and keep the rest;
   - (c) keep values whose name matches, whatever their initializer (svelte-hmr).

   **Recommendation (a).** Partial restores are how inconsistent states arise (Rich Harris's third objection).
4. **D4: what the signature covers.**
   - (a) names + kinds + normalized initializer source (React's `useState` rule);
   - (b) names only.

   **Recommendation (a).** Changing an initializer is a request to see the new value.
5. **D5: `$bindable` fallbacks and other edge state.** Should an unbound `$bindable(x)` fallback be kept like
   `$state`?

   **Recommendation: yes**, keyed as `prop:<name>`, when the parent does not bind it. Writable `$derived`
   overrides are never kept.
6. **D6: children of an edited component.**
   - (a) re-created with fresh state (upstream, svelte-hmr);
   - (b) kept by positional key (React).

   **Recommendation: (a) in H3, then (b) as H3c** once (a) is in use and its cost measured. (b) matters most for
   App.svelte edits, where every child resets.
7. **D7: module-level state in `.svelte.js` and stores.**
   - (a) reset on edit (upstream);
   - (b) keep by declaration key + signature through `import.meta.hot.data` (Flutter's "globals are state").

   **Recommendation (b).**
8. **D8: native UI state.**
   - (a) capture/restore by identity through metadata-declared ephemeral props and methods;
   - (b) widget reuse (native hydration);
   - (c) both.

   **Recommendation (a)**, with the identity prop shared with EGUI G5. (b) only if (a)'s ledger shows big gaps.
9. **D9: error surface.** Native apps have no DOM overlay. Options:
   - (a) console + `ReloadReport` only, as today;
   - (b) a dev-only error window drawn by the runtime from a built-in component (message, mapped location,
     "reset state" and "full reload" buttons);
   - (c) both, (b) behind a flag.

   **Recommendation (c).** Rollback means the app keeps working underneath.
10. **D10: timers and other side effects without cleanup.** Should dev HMR clear timers a destroyed instance left
    running? Options: (a) yes, with a warning; (b) no (upstream leaks them).

    **Recommendation (a).**
11. **D11: upstream.** Should we offer the per-instance-registry + signature design in PR #17995 / issue #14434
    once H3 is proven? **Recommendation:** yes, as a comment with our test cases. Optional; it costs nothing to
    our plan.

## 7. Sources

All read on 2026-10-21.

- **Svelte 5.57.2 source** (extracted tarball, `packages/svelte`):
  - `src/internal/client/dev/hmr.js`;
  - `src/compiler/phases/3-transform/client/transform-client.js` lines 512–529 and 657;
  - `3-transform/utils.js:289`;
  - `src/compiler/types/index.d.ts:187–192`;
  - `src/compiler/validate-options.js:127`;
  - `src/internal/client/dev/css.js`;
  - `src/internal/client/constants.js:71–72`;
  - `dom/blocks/branches.js:99–101`;
  - `src/internal/shared/context.js:10`;
  - `src/internal/client/dev/tracing.js:139`;
  - tests `runtime-runes/samples/{hmr-removal,hmr-each-keyed-unshift,hmr-dynamic-component,component-transition-hmr}`,
    `hydration/samples/css-props-hmr`, `snapshot/samples/hmr`.
- **sveltejs/svelte PRs and issues:**
  - [#10100](https://github.com/sveltejs/svelte/pull/10100) (rixo POC);
  - [#11106](https://github.com/sveltejs/svelte/pull/11106) (HMR for Svelte 5);
  - [#11453](https://github.com/sveltejs/svelte/pull/11453) (`acceptExports`);
  - [#14434](https://github.com/sveltejs/svelte/issues/14434) (preserve local state, open);
  - [#17995](https://github.com/sveltejs/svelte/pull/17995) (state preservation PR and review, open);
  - [#17680](https://github.com/sveltejs/svelte/pull/17680) (closed);
  - [#18079](https://github.com/sveltejs/svelte/pull/18079);
  - [#19002](https://github.com/sveltejs/svelte/pull/19002) (open);
  - [sveltejs/kit#12985](https://github.com/sveltejs/kit/issues/12985).
- **@sveltejs/vite-plugin-svelte 7.3.1** (pnpm store in the main checkout): `src/plugins/hot-update.js`,
  `src/utils/compile.js`, `src/utils/options.js`, `src/plugins/compile-module.js`.
- **svelte-hmr:** [README](https://github.com/sveltejs/svelte-hmr/tree/master/packages/svelte-hmr) ("Preservation
  of local state", options) and `packages/svelte-hmr/runtime/svelte-hooks.js`.
- **Vite:**
  - [HMR API](https://vite.dev/guide/api-hmr);
  - [Environment API: runtimes / ModuleRunner](https://vite.dev/guide/api-environment-runtimes) (docs for
    v8.3.4);
  - `vite@8.3.4` `dist/node/module-runner.js` (55 KB, no imports, `HMRContext`, `new AsyncFunction`).
- **React:**
  - [Fast Refresh docs](https://reactnative.dev/docs/fast-refresh);
  - facebook/react `packages/react-refresh/src/ReactFreshBabelPlugin.js` (`getHookCallsSignature`, `useState`
    key rule) and `ReactFreshRuntime.js` (`canPreserveStateBetween`, `haveEqualSignatures`).
- **Solid:** [solid-refresh](https://github.com/solidjs/solid-refresh), `src/runtime/index.ts` (`$$component`,
  `patchComponent`, `patchContext`).
- **Vue:** vuejs/core `packages/runtime-core/src/hmr.ts` (`rerender`, `reload`); vitejs/vite-plugin-vue
  `packages/plugin-vue/src/handleHotUpdate.ts`.
- **Flutter:** [Hot reload](https://docs.flutter.dev/tools/hot-reload).
- **Repo:** files cited inline (paths and line numbers on this branch, master d110cb3d; CF1 facts from
  `origin/master` a7cd805a).

## Appendix A: the probe's prototype

The wrapper and `keep` as run, on this branch's `packages/native-runtime/src` and the Node TestHost, with stock
svelte 5.57.2. Run with `node --conditions=browser --import <hook for the three hidden Svelte modules> --test`.

```js
let init = null; // registries of the instance whose init is running

function hmr(fn) {
  const current = $.state(fn);
  function wrapper(anchor, props) {
    const instance = {};
    let prev = null; // key -> { sig, read } of the live instance
    $n.component(anchor, () => {
      const f = $.get(current);
      return (a, p) => {
        const next = new Map();
        const saved = init;
        init = { prev, next };
        try { return f(a, p); } finally { init = saved; prev = next; }
      };
    }, props, (exports) => { if (exports) Object.defineProperties(instance, Object.getOwnPropertyDescriptors(exports)); });
    return instance;
  }
  wrapper.update = (incoming) => $.set(current, incoming);
  return wrapper;
}

function keep(key, sig, initial, wrap, read) {
  const old = init?.prev?.get(key);
  const s = wrap(old && old.sig === sig ? old.read() : initial());
  init?.next.set(key, { sig, read: () => $.untrack(() => read(s)) });
  return s;
}
// in a component, as H3 codegen would emit:  let count = keep('count', 'sig:0', () => 0, (v) => $.state(v), $.get);
```

H1/H3 differ from the probe in three ways:

- the signature is per component, not per declaration (D3);
- `update` follows upstream's first-version rule;
- a throwing init rolls back.
