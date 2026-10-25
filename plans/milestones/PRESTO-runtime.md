# PRESTO-runtime: native runtime and host (presto batch 1, lane RUNTIME)

Branch `ci/presto-runtime` (from `ci/presto-base`, which is merged in). Brief:
`plans/cloud/presto-b1/LANE-runtime.md`. Design: `plans/research/presto/compiler-and-js.md` §3.2,
§3.3, §3.10, §5.4–§5.6.

## Landed

### 1. Lazy `Native` objects (W-C3)
- `packages/native-runtime/src/index.js`: the `Native` constructor sets every field a node can
  carry (`requested`, `snippets`, `inertFlag`, `anchorScope`, `explicit`, `ruled`, `inlineProps`,
  `src`, `vis`, `styleDecl`, …), so no read falls through to the provider-method `Proxy` on the
  prototype. `last`, `captured`, `tokens`, `handlers`, `listening`, `cattrs`, `passive`, `slots`
  are `null` until first written; `attrs` is a frozen shared empty object until the first `attr`.
  Every reader and writer is updated (`bind-observe.js` too).
- `set()`: scalars skip `plainData` and the content compare; objects keep RT1's copy.
- New export `setp(node, prop, value)`: the typed scalar write the compiler emits behind its own
  inline compare (`v !== p.x && $n.setp(n, P, p.x = v)`), for props no `track`/`capture`/`token`
  uses and never `anchor` or a snippet. A numeric prop goes to `host.setp` (the raw fast op); on a
  host with `host.dedupes` nothing is kept in JS, elsewhere `last` dedupes as `set` does.
- ABI v2.9 `SVL_CAP_DEDUPE` (lib.rs + header): the backend drops unchanged scalar writes. The host
  exposes it as `__svelters_host.dedupes`. egui advertises it and implements a borrowed
  `set_prop_ref`: an unchanged write to an explicit (JS-written) Bool/I64/F64/Str prop costs a
  compare against the scene's value and no owned copy (`Scene::js_write_unchanged`); other writes
  copy once as before. `$n.set` keeps its JS `last` map: it carries tracking (`capture`, `assume`,
  `track`, `presentTop`'s modal check), not just dedupe.

### 2. Event-time dispatch (W-H1)
- ABI v2.9 `svl_host.dispatch_now` (optional member), `HostCallbacks::dispatch_now`,
  `HostRef::dispatch_now`/`has_dispatch_now`, `Host::dispatch_now`, `core::dispatch_now`: delivers a
  signal at event time and drains the jobs its handlers queued (Svelte's flush), earlier queued
  changes and events first; `Busy` (queued) while JS runs; `Failed` with no listener. The
  checkpoint's change/event delivery is now one function (`deliver_queued`) both use.
- `svelters-backend-log`: a `{"now": [h, signal, args]}` script step (logs `now …` / `result #h
  ok|busy|error`).
- egui: `EmbedConfig::same_frame_reactions` (default on). A pass whose walk sent events or prop
  changes to JS is discarded (`Context::request_discard`) and the rerun pass runs `logic` first
  (legacy `ui()` and `Embed::logic`), so `onclicked`/`bind:` reactions are drawn in the frame of the
  input. JS still never runs during a walk. The R17 relaxation is recorded in
  `plans/research/ARCH-SCENE.md` as "Decided (user, 2026-10-25: follow the recommendations)".
  The host entry is not used by egui (widget signals are only known mid-walk, where JS cannot
  run); it is the entry presto-core's UI-thread dispatch calls.

### 3. Fast ops (W-J1)
- `crates/svelters-host/src/quickjs/fast.rs`: `createId`, `insertId`, `setp`, `listenId` as raw
  `rquickjs-sys` C functions (`JS_CFUNC_generic_magic`, magic = op), and `setter(propId)` returning a
  C function whose magic is the prop id. Numbers and booleans are read from the value tag, strings
  borrowed with `JS_ToCStringLen2`, string prop names looked up as atoms (cached per kind, freed at
  teardown). A scalar for a scalar slot goes to `Core::op_set_scalar` (no arena, no conversion
  tree, same backend call and ABI v2.6 read-back); anything else falls back to `op_set_prop` with the
  converter, so errors are word for word the general path's (tested).
- Node wrappers: `wrap(h)` → one `SveltersNode` object per live handle (class registered per
  runtime, `JS_SetOpaque` holds handle + generation), `wrapHandler(w, i, fn)`, `handleOf(w)`. The
  host holds each wrapper strongly while its node lives (`QjsCallbacks::wrappers`), lets go on
  destroy or native death (`CallbackEngine::handle_gone`, freed at `process_releases`), and the
  class's `gc_mark` reports the handlers, so a handler capturing its own node is collected.
- `Core::forget_handle` (the bookkeeping half of `op_destroy`) is shared by destroy, templates and
  `destroyRange`.

### 4. `host.instantiate` / `destroy_range` (W-C2, host and ABI side)
- ABI v2.9: `svl_template` (`svl_tpl_node`/`svl_tpl_prop`/`svl_tpl_listen`), optional backend
  members `instantiate` (contiguous handles, node k = base + k) and `destroy_range`;
  `Backend::supports_templates/instantiate/destroy_range`, `BackendRef::has_templates/instantiate/
  destroy_range`, `TemplateRef` (with `is_well_formed`, `expansion()`), `OwnedTemplate`.
- **The expansion order (the contract with lane COMPILER):** for each node in pre-order, `create`
  and its static props in table order; when a node's subtree is complete, its `insert` into its
  template parent (roots are not inserted), then its listeners. This is the order today's op logs
  show (`fixtures/forward-events/expected.oplog`).
- Host JS ops: `template({hash, nodes:[kind, parent+1, slot]*, props:[node, prop, value]*,
  listens:[node, signal, flags]*})` → id (same hash → same id, refcounted; names resolved and values
  converted once), `instantiate(id)` → base handle when the backend builds templates
  (`host.templates === true`), else the array of handles of the host's expansion (one crossing, the
  same ops and op-log lines as op by op), `destroyRange(base, n)`, `templateRelease(id)`.

### 5. Microbench (W-J0 code)
- `crates/svelters-host/benches/boundary.rs` (`harness = false`): rquickjs closure vs raw C function
  vs exotic class (`set_property` hook), numbers and a string, and the JS `Map` dedupe vs an inline
  compare. **Not run** (needs a quiet machine).

## Tests (written)
- `crates/svelters-host/tests/dispatch_now.rs` (+ `tests/js/dispatch_now.js`): run locally once,
  pass.
- `crates/svelters-host/tests/fast_ops.rs` (+ `fast_ops.js`, `slow_ops.js`, `wrappers.js`,
  `templates.js`, `templates_ops.js`): fast ops log and fail exactly as the general ops; a wrapper
  whose handler captures it is collected after `destroy` (GC count 2 → 1); a template expands to the
  op-by-op log. Run locally once (one failure fixed: an error message test compared a by-id write
  with a by-name one); the last fix is **not rerun**.
- `crates/svelters-egui/tests/input_latency.rs` (from svelters `ci/input-latency`, which has not
  merged) with its fixture moved to `fixtures/motion-egui/latency/` (a non-workspace app, like
  `motion-egui/cx3`): asserts 1 frame for `onclicked`, `bind:checked → {#if}` and DOM `onclick`;
  prints the rest. **Not run** (no node_modules in the container).
- `packages/native-runtime/test/lazy-nodes.test.mjs`: own fields, lazy collections, `setp` with
  and without a deduping host. **Not run.**
- Not run at all: conformance, op-log goldens, the runtime's other unit tests, GTK/Qt/AppKit/WinUI.
  `cargo check` ran for svelters-abi, svelters-host (all targets), svelters-egui (tests),
  svelters-backend-log; clippy on host/abi (no new warnings).

## Stubbed / left out
- The runtime does not call `createId`/`insertId`/`listenId` yet: `element(kind)` keys the style
  tables and TOPLEVELS by the kind *name*, so ids need the spec's id → name table the compiler lane
  emits. Once `$n.element(K_LABEL)` exists, `create` should call `host.createId` and keep the name.
- No backend implements `instantiate` natively yet (egui's handles are generation-tagged, not
  contiguous); every backend gets the host's expansion.
- `wrap` is not yet used by the runtime: presto's event-time dispatch is its user (Rust calls a
  node's handlers by slot). `convert.rs` does not yet accept a `SveltersNode` where a handle goes
  (it reads `svlHandle`).
- egui's dedupe compares strings by cloning the stored value (`props::get_prop`); a typed borrowed
  read would remove that clone.

## Cross-lane edits
- `crates/svelters-abi/**` (additive: v2.9 members and types), and the hand-written `svl_host` /
  `svl_backend` literals that must name the new optional members: `crates/svelters-gtk/src/backend.rs`,
  `crates/svelters-gtk/src/testing.rs`, `crates/svelters-gtk/tests/wake_priority.rs`,
  `examples/counter-qt/tests/*.rs` (9 files), `examples/counter/src/driver.rs` (the new `Step::Now`).
- `crates/svelters-backend-log/src/lib.rs` (the `now` step).
- `crates/svelters-egui/src/{backend,frame,ops}.rs`, `embed/mod.rs`, `scene/access.rs` (dispatch path
  and the dedupe check), `plans/research/ARCH-SCENE.md` (R17 relaxation).
- `fixtures/motion-egui/latency/` (new, not a workspace package).
- `crates/presto-base` untouched.

## For lane COMPILER
- Emit `$n.setp(node, P_ID, v)` (the export exists here) only for typed scalar, untracked props.
- `template.js`: use `host.template`/`host.instantiate` when the host has them (it always does on
  svelters-host); the result is a number (base, `+ k` addressing) or an array of handles (index
  k). Expand in JS only for hosts without `template` (the Node TestHost). Keep the expansion order
  above so op logs stay byte-identical.

## CI
- Final sha `0c981c33` (with `ci/presto-base` 7a947d45 merged): js 40223167311, linux-gtk 40223167387,
  linux-qt 40223167341, macos-appkit 40223167334, windows-winui 40223167390 all **green**.
- `presto` 40223167403 red, not from this lane: ubuntu fails building `yeslogic-fontconfig-sys`
  (no `fontconfig` on the runner, a presto crate's dependency); windows/macos fail in `cargo test`
  of the presto crates. The workflow needs `libfontconfig1-dev` on Linux.

## Questions for the coordinator
1. **egui's same-frame rerun costs one more pass on interaction frames.** Recommendation: keep it on
   by default (as the brief says) and let cadatomic turn it off per embed if pass cost bites
   (`EmbedConfig { same_frame_reactions: false, .. }`).
2. **`host.instantiate` returns a number or an array.** Recommendation: keep both shapes (the native
   path must stay allocation-free); `template.js` branches on `typeof`.
3. **The `presto` workflow on Linux needs `libfontconfig1-dev`** (yeslogic-fontconfig-sys).
   Recommendation: install it in `.github/workflows/presto.yml` on `ci/presto-base`.
