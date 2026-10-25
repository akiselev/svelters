# PRESTO-svelters-presto: the svelters backend on presto (lane record)

Lane SVELTERS-PRESTO of batch presto-b2 (`plans/cloud/presto-b2/LANE-svelters-presto.md`), branch
`ci/presto-svelters-presto`, session `session_01497YqbhMLBiY7YWGtiLKAX`, 2026-10-25. Handed off at the
context limit (see "Hand-off" below); the branch builds (`cargo check -p svelters-presto --all-targets`)
after merging `ci/presto-base` at 7a947d45 (CORE, TEXT, RENDER, COMPOSITOR, FRAME-ANIM skeletons).

## What landed (`crates/svelters-presto/**`)

| Module | What |
|---|---|
| `backend.rs` | `PrestoBackend`: `impl svelters_abi::Backend` (resolve, create, insert with slots, remove, destroy, set/get typed by id, call `present`/`close`, listen and `listen_ex` with capture and `LISTEN_PASSIVE` = 4, handle_info with borrowed slots, run through a `Driver`, waker, quit, schedule_wake, request_frame, style, observe with nesting counts, start, describe). Handles are the backend's own, counted up and never reused, so a template's nodes are `base + k`. The `Driver` trait is the seam to whoever runs the loop (the presto app, a harness). |
| `deliver.rs` | Event-time delivery of a `Dispatched`: observed native changes first (`prop_changed`), events in order, cancelable/sync ones through `call_sync` (its `true` cancels the default), one closing checkpoint. `pump()` delivers native output produced outside input. |
| `template.rs` | `Template`/`TemplateNode` (decoded table), `instantiate` (contiguous handles, CF2 order, roots left to the caller, rollback on failure), `destroy_range`, `Templates` (content-hash registry with refcounts). |
| `style.rs` | `style` op: `TARGET` → 4 (engine), sheets/classes/id/inline/scheme into the tree, TR1 visual channels (opacity, transform, clip, inert); `ANIMATE` answers 0 so the runtime plays animations itself until presto-motion is wired; `DECLS` refused. |
| `ids.rs`, `ids.txt`, `wire.rs` | Stable wire ids: an append-only table (one row per kind/prop/signal/method/slot, keyed by the declaring kind, so inherited props share their base's id), FNV-1a fingerprints per member, a spec hash. See Q1: presto-core chose hash ids instead. |
| `spec.rs`, `css.rs` | The `presto.json` generator (svelters-semantics/v0 like egui.json, plus `id`/`fingerprint` per kind and member, `ids.spec_hash`, `change` per prop and per engine CSS property, `passive`/`cancelable` per signal, `a11y` with `name_required`, `native_state`, `slot_ids`, `capabilities: ids templates native_transitions native_motion css_precompiled`). `css.rs` is presto's engine property table with change classes (egui's list plus `transform`, `transform-origin`, `visibility` as property-class). |
| `embed/` | The embed model minus egui: `Embed` (config, budgets, fault policy, load/load_from/load_app, slots as borrowed `box` nodes, mount/props/unmount/move/close/restart queued and applied in `logic()`, root info sync, faults per root, `settled()`), `roots.rs`, `slots.rs` (window/size/scale/focus reported by the app: `set_slot_layout`, `set_slot_focused`), `settle.rs` (egui's `Busy` plus `COMMIT_IN_FLIGHT`, set with `set_commit_in_flight`). Panes (CV1) not ported. |
| `value.rs` | ABI `Value` ↔ prop values by type (numbers across int/float, enums by name, `#rrggbbaa` colours, lists, records). |
| `shim/` | Stand-ins (below). |

Generated: `metadata/semantics/presto.json` (from `builtin_registry()`), `crates/svelters-presto/ids.txt`.
Staleness: `tests/spec.rs` (`SVELTERS_UPDATE=1 cargo test -p svelters-presto --test spec`).

Tests (written, **not run**, except `tests/spec.rs` run once with `SVELTERS_UPDATE=1` to generate the
spec and the id table): `tests/ops.rs` (the ABI through the real vtable), `tests/oplog_replay.rs` (every
Tier 0 egui fixture's `expected.oplog` — counter, if-toggle, keyed-list, motion, timers — replayed on
presto: ops succeed and read back; `event`/`changed` lines produced natively through the tree's
`emit`/`native_set` and delivered once, `changed` exactly when observed), `tests/template.rs`,
`tests/deliver.rs`, unit tests in `value.rs` and `style.rs`.

## Cross-lane edits

| File | Why |
|---|---|
| `crates/svelters-compiler/src/semantics/backend.rs`, `kinds.rs` | `SpecBackend::builtin("presto")`; presto in `every_builtin_spec_builds` (lane brief). |
| `crates/svelters-modules/src/source/transform.rs` | `styles()` checks the spec's `style.engine == "backend"` instead of `backend == "egui"`; the warning text names both engines. |
| `crates/svelters-host/tests/modules.rs` | Follows that warning text (lane RUNTIME's crate; one assertion string). |
| `packages/native-runtime/vite.js` | `runsCssEngine(backend)` reads the spec's `style.engine` (two `'egui'` checks). |
| `packages/testhost/src/index.js` | Same check for the default `styleDialect`. |
| `tools/metadata/style/gen_props.py` + `packages/style/src/props.generated.js`, `crates/svelters-style/src/tables.generated.rs` | presto in `TOOLKITS` (regenerated: a `PRESTO` table in `tables::ALL`). Rerun whenever presto.json's style section changes. |
| `tools/metadata/style/gen_utilities.py` | `--report` lists the utility rows presto's style table cannot apply (none today); outputs unchanged. |

## Shim (replaced at consolidation)

All in `src/shim/`; each item names the API it stands in for.
- `shim::{KindId, PropId, SignalId, MethodId, SlotId, Value, ValueType, DefaultValue, KindSpec, PropSpec, SignalSpec, MethodSpec, SlotSpec, A11ySpec, KindRegistry, ListenFlags, StyleInput, TreeError, PropError, JsEvent, Dispatched, ColorScheme, Visual}`: presto-core's vocabulary as the b1 brief sketched it.
- `shim::Document`: the tree half of `presto_core::Document` (create/insert/remove/destroy/set_prop/get_prop/listen/set_style/transaction, sheets, colour scheme, visual channels) plus `emit`/`native_set`/`take_dispatched` for native output. No style, layout, recording, hit testing, dispatch or frame.
- `shim::tier0`: the Tier 0 kinds' specs (Widget base, Window, Box, Frame, Label with runs, Button, ToggleButton, Checkbox, RadioButton, Switch, Slider, ProgressBar, Spinner, Separator, Icon, Image, Hyperlink, Stack, Sides, Grid, TextEdit, Run/Span/Bold/Italic/Code/Anchor), transcribed once from `metadata/semantics/egui.json` so the JS-facing surface is egui's; presto change classes, passive/cancelable and `name_required` added. Stands in for `presto_widgets::registry()`.

## Not done

- **Port to the real presto-core** (merged into `ci/presto-base` after this lane started). See Q1–Q4; the hand-off below.
- **`svelters-presto-run`** (runner binary through the `presto` facade): lane APP-TEST's `presto::App` was not merged when this lane ended. The `Driver` trait is the seam it implements.
- `bounds` (needs presto-core layout), `request-focus` and `scroll-into-view` (focus and scroll models): `call` returns "not implemented on presto yet"; `supports_bounds` is false.
- Native animations (`SVL_STYLE_ANIMATE` → presto-motion, lane MOTION-CSS): answers 0, the runtime plays them.
- `host.instantiate`/`destroy_range` ABI wiring and the table's wire form (lanes RUNTIME and COMPILER): the backend side is `PrestoBackend::instantiate(&Template)`; the template registry is not held by the backend yet.
- Event delivery uses `post_event` + `checkpoint`; lane RUNTIME's "dispatch and drain" entry replaces the closing checkpoint.
- `presto.d.ts` typings, spec fragments for provider kinds, DZ0 overrides, CV1 panes.

## Questions for the coordinator (each with a recommendation)

- **Q1 Wire ids.** presto-core made ids the FNV-1a-32 hash of (kind, member, type), with collisions rejected at registration; this lane made an append-only id table (`ids.txt`) per the compiler doc. *Recommendation:* presto uses presto-core's hash ids on the wire (presto.json publishes them as `id`, the hash doubles as the fingerprint), and `ids.txt`/`wire.rs` go at the port; a type change then changes the id, which the spec-hash check turns into a recompile. Egui keeps lane COMPILER's table.
- **Q2 JS-facing names.** presto-core's `COMMON` props use DOM names (`disabled`, `hidden`, `aria-label`, `title`); egui's (and cadatomic's `.svelte` sources and the fixtures') are `enabled`, `visible`, `accessible-name`, `tooltip`. *Recommendation:* presto.json publishes egui's names as `js` with core's as `native` (svelters-presto resolves both), so sources stay backend-neutral and the fixtures replay unchanged; or CORE renames to egui's names. Either way one decision for all widget lanes.
- **Q3 What presto.json needs that presto-core's `KindSpec` lacks:** docs, methods (`present`, `close`, `request-focus`, `scroll-into-view`), named signal arguments (bindings point at an argument by name; core has one `arg: Option<ValueType>`), prop `nullable` and `doc`, a11y name sources, a base-kind chain. *Recommendation:* CORE adds `doc`, `methods`, `SignalSpec.args: &[(&str, ValueType)]`, `PropSpec::{nullable, doc}`, `KindSpec.a11y_name_from`; until then svelters-presto keeps a side table keyed by kind name.
- **Q4 Windows.** presto-core creates a window root natively (`create_window(WindowId, kind, …)`); JS creates a `window` node, fills it, then calls `present`. *Recommendation:* core adds `Document::attach_window(window, node, size, scale)`; on `present` svelters-presto asks its `Driver` to open the OS window and attaches the JS-created node as its root.

## Hand-off (next session, in order)

1. Read Q1–Q4 answers. Port `backend.rs`, `deliver.rs`, `style.rs`, `template.rs`, `embed/` from `shim::Document` to `presto_core::Document` (`Document::new(registry, text)`, `listen`/`unlisten`, `add_sheet`/`remove_sheet` returning `SheetId`, `drain_changes` for native changes, `presto_core::Dispatched`); keep the `shim` only for what core still lacks (Q3 side table, Tier 0 kinds until presto-widgets registers them).
2. Regenerate presto.json from presto-core's and presto-widgets' registries (`spec.rs` reads `KindRegistry::iter()`), rerun `gen_props.py`, update `tests/spec.rs`.
3. Write `src/bin/svelters-presto-run.rs` on lane APP-TEST's `presto::App` (implement `Driver`), and switch `tests/oplog_replay.rs`'s native lines to presto-test's `Harness` input once it can click and type.
4. Wire `host.instantiate`/`destroy_range` when lane RUNTIME's ABI op lands; hold the `Templates` registry in the backend.

## Not run

Everything except `tests/spec.rs` (once, with `SVELTERS_UPDATE=1`). No CI result observed for the last push
at hand-off; the coordinator's CI check is the gate.
