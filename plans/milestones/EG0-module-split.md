# EG0: the module split of `crates/svelters-egui` (with SG0)

Design: `plans/research/ARCH-EMBED.md` §9 (module split) and §10 (EG0 row), with `plans/research/ARCH-SCENE.md` §13
(the refined `scene/*` cut, which supersedes ARCH-EMBED's `scene/{mod,node,props}.rs`) and §14 (SG0 = that cut, done
inside EG0). This file records EG0 as built. EM1, RA1 and SG1 start from it.

## Scope

- Move today's code into the files of ARCH-EMBED §9 and ARCH-SCENE §13, with no behaviour change.
- `kinds::KindRegistry`: a thin wrapper over the existing kind table.
- Empty seams, each a doc comment naming its content and writer lane: `embed/*`, `events.rs`, `style.rs`, `provider/*`,
  `scene/{ext,journal,query,hit,virt,dump}.rs`.
- Every public path keeps working through re-exports.

Gate: every existing svelters-egui test and op-log replay passes unchanged, and `metadata/semantics/egui.json` is
byte-identical (regenerated with `SVELTERS_UPDATE=1`, then `git diff --exit-code`).

## File map (as built)

| File | Content | From | Writer lane after |
|---|---|---|---|
| `lib.rs` | modules; re-exports `EguiBackend`, `Stats`, `run_app`, `egui`; compat paths `render` and `harness` | `lib.rs` | any (small) |
| `backend.rs` | `EguiBackend`, `Inner` (all fields `pub(crate)`), `Shared`, `Stats`, `new`, `vtable`, `frame_counter`, `stats`, `quit_requested`, `host`, `ctx`, `bump` | `backend.rs` | EMBED |
| `frame.rs` | `ui` (the frame), `before_pass`, `after_pass`, `close_vetoed`, `native_close`, `checkpoint`, `touched`, `layout_now` | `backend.rs` | EMBED |
| `windows.rs` | `root_window`, `walk` (presented windows: root `CentralPanel`, others `show_viewport_immediate`, pass flag), `window_props`, `window_chrome`, `on_close_requested` | `backend.rs` | EMBED |
| `walk.rs` | `Out`, `paint`, `paint_visual` (TR1), `paint_node` (culling), `window_ui` (layout when stale + paint of one window), `assert_no_pass` | `render.rs`, `backend.rs` | EMBED, then EVENTS |
| `leaf.rs` | `leaf`: built-in leaf painting and its outbox entries | `render.rs` | WIDGETS |
| `ops.rs` | `impl Backend for EguiBackend`; `kind_of`; `visual_op` (TR1 style channels) | `backend.rs` | shared |
| `ids.rs` | `node_id(h)` = `Id::unique(("svl", h))` | `scene.rs` (`new_node`) | EMBED |
| `a11y.rs` | `name(n)`: `accessible-name`, else a text field's hint | `render.rs` (`leaf`) | EMBED |
| `layout.rs` | taffy adapter (unchanged); `invalidate` re-exported from `scene/dirty.rs` | `layout.rs` | SCENE, WIDGETS, STYLE |
| `scene/mod.rs` | `Arena` (+ `Slot`); re-exports `Handle`, `Node`, `W`, `Common`, `Visual`, `Dirty`, `set_prop`, `get_prop`, `default_value`, `tag_of`, `new_node` | `scene.rs` | SCENE |
| `scene/id.rs` | `Handle`, index/generation constants, `split`/`join` handle packing | `scene.rs` | SCENE |
| `scene/node.rs` | `W`, `Common`, `Node`, `Visual`, `Dirty` | `scene.rs` | SCENE |
| `scene/tree.rs` | `new_node`; `detach`, `insert`, `remove`, `destroy`, `free_subtree` (scene halves of the ops and of `native_close`) | `scene.rs`, `backend.rs` | SCENE |
| `scene/props.rs` | value conversions, `set_prop`, `get_prop`, `default_value`, `tag_of` | `scene.rs` | WIDGETS |
| `scene/dirty.rs` | `invalidate`; `DirtyFlags` (`remeasure`, `layout_dirty`, `layout_size`, formerly fields of `Inner`) | `layout.rs`, `backend.rs` | SCENE |
| `scene/access.rs` | `with_scene`, `prop`, `handles_of` | `backend.rs` | SCENE |
| `scene/{ext,journal,query,hit,virt,dump}.rs` | empty | | RUSTAPI / SCENE / SCENE / SCENE / LISTS / SCENE |
| `kinds/mod.rs` | table types (`Ty`, `Def`, `P`, `Native`, `PropDef`, `SignalDef`, `MethodDef`, `KindDef`), table helpers, `KindRegistry` with the id scheme, the old free functions (answering through the registry) | `kinds.rs` | WIDGETS, RUSTAPI (registry) |
| `kinds/builtin.rs` | `KINDS` (order = kind ids), enum value lists, `Widget` and leaf tables | `kinds.rs` | WIDGETS |
| `kinds/containers.rs` | `window` and `box` tables | `kinds.rs` | WIDGETS |
| `provider/{mod,state,behavior,pass}.rs` | empty (`pass.rs` from ARCH-SCENE §13) | | RUSTAPI, EVENTS (`behavior`), SCENE (`pass`) |
| `embed/{mod,roots,slots,current,settle}.rs` | empty | | EMBED |
| `events.rs`, `style.rs` | empty | | EVENTS, STYLE |
| `spec.rs` | unchanged apart from its doc comment | `spec.rs` | RUSTAPI |
| `runner/mod.rs` | doc, `run_app`, `run_eframe` re-export | `runner.rs` | EMBED |
| `runner/eframe.rs` | `root_props`, `EframeApp`, `run_eframe` (both feature arms) | `runner.rs` | EMBED |
| `runner/kittest.rs` | `TestApp` (unchanged; `crate::harness` is an alias of this module) | `harness.rs` | EMBED |

Old public paths that still work: `svelters_egui::{backend, kinds, layout, scene, spec, runner, harness, render}::*`,
`layout::invalidate`, `scene::{new_node, set_prop, get_prop, default_value, tag_of, Dirty, Visual, …}`, every
`kinds::` function and table, `harness::TestApp`. No caller outside the crate needed a change (the only users are
the crate's own tests and `bin/svelters-egui-run.rs`; `svelters-compiler` only reads `egui.json`).

## Deviations from the two docs

- **`backend.rs` stays.** §9 has no file for the backend struct itself; until EM1's `Embed` replaces it, `EguiBackend`
  and its state live in `backend.rs`, and each concern adds an `impl EguiBackend` block in its own file. `Inner`'s
  fields became `pub(crate)` for that.
- **`style.rs` stays empty.** TR1 (merged after the docs were written) added `visual_op`, the paint-side half of the
  `style` op. It stays in `ops.rs` next to `fn style`, so STYLE can take both together.
- **`scene/tree.rs` also holds `free_subtree`**, the scene half of `native_close` (`frame.rs`), so the tree edits are
  in one file.
- **`ids.rs` and `a11y.rs` hold one function each** (the current id derivation and accessible-name rule), pulled out
  of `new_node` and `leaf` so EM1 edits those files instead of `scene/tree.rs` and `leaf.rs`.
- **`provider/pass.rs`** is created now (ARCH-SCENE §13 says `provider/` gains it) to save a later cross-lane edit.
- **No `lib.rs` prelude.** §9 mentions one; nothing exists to put in it yet.
- **Spec provenance.** `egui.json` names `crates/svelters-egui/src/kinds.rs` (`provenance.kind`, `source.kinds`). The
  strings were kept so the file stays byte-identical. RA1, which owns `spec.rs`, can update them together with its
  fragment work.

## Notes for SG1, EM1 and RA1

- **`KindRegistry` holds no slice.** It reads `KINDS` directly. A copy of the slice made in a static initializer
  pointed at different `PropDef` addresses than `KINDS` itself, and `tests/spec.rs` compares definitions by address
  (`std::ptr::eq`). RA1, building the registry from `inventory` at startup, keeps that property: hand out references
  into one table. Internal callers still use the free functions (`kinds::find_prop`, …) and index `KINDS[k]`
  directly (`Node::tag`, `new_node`, `ops.rs` checks, `spec.rs`). RA1 moves them to the registry.
- **The flags in `DirtyFlags` are still global** (one `layout_dirty` for every window). SG1 replaces them with
  per-root `RootDirty`; their readers are `frame.rs` (`before_pass`, `layout_now`, `native_close`), `walk.rs`
  (`window_ui`) and `ops.rs`.
- **`insert`/`remove`/`destroy` are split**. Kind checks (slots, `toplevel`, self-insert), `layout_dirty`, `touched()`,
  egui memory cleanup and the window list are in `ops.rs`. Cycle check, `before` validation, detach, orphaning and
  free are in `scene/tree.rs`. `tree::insert` returns `Ok(false)` for the "already in place" no-op, which sets no
  flag and requests no repaint (as before). SG1's `SceneMut` path should absorb the `ops.rs` half's flag-setting.
- **The walk spans two files.** `windows.rs::walk` iterates the windows and sets `pass_active`, and
  `walk.rs::window_ui` lays out and paints one. EM1's `show_slot` replaces the first and reuses the second.
- **`Node.id` is assigned in `new_node`** through `ids::node_id`. EM1 moves the call to SG1's attach hook.

## Verification (this lane, local)

- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets`, plus `--no-default-features --lib`: clean.
- `tools/dev/cargo-slot cargo check --workspace --all-targets`: clean (pre-existing warnings only, none in svelters-egui).
- `tools/dev/cargo-slot cargo nextest run -p svelters-egui`: 17/17 (counter, timers and motion bundles built locally
  with `pnpm --filter` after `cargo build -p svelters-compiler --bin svelters-compile`).
- `SVELTERS_UPDATE=1 … --test spec`, then `git diff --exit-code metadata/semantics/egui.json`: no diff.
- Not run locally: workspace tests, conformance, the other backends' suites. CI runs them.
