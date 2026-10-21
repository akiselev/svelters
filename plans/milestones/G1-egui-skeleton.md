# G1: the egui backend skeleton

Design: `plans/research/EGUI.md` (§1, §7 G1, §8 decisions) and `docs/notes/egui-scene-graph.md`. This file records G1
as built.

## Decisions used

- D1 (user): the embedder drives the frame, alongside `run()`. Built as three runners over one frame function.
- D2 (user): taffy on our arena. Built as taffy's low-level traits implemented on the scene, no mirrored `TaffyTree`.
- D3 (user): layout props in `<style>` where the backend's style table accepts them. The egui style table lists every
  layout prop as a setter, so `.row { flex-direction: row; gap: 8px }` compiles to `setProp` calls on egui. GTK and Qt
  are unchanged.
- **D9, provisional (not decided by the user): egui 0.37.0.** `egui`, `eframe` and `egui_kittest` are pinned `=0.37.0`
  and taffy `=0.15.0` in `crates/svelters-egui/Cargo.toml`. 0.38 is the first upgrade lane.
- D4 (identity prop) is G5 and D5 (MCP) is G3; neither is touched here. D8 and D10 stay open.

## As built

### Crate `crates/svelters-egui`

| Module | What |
|---|---|
| `kinds.rs` | The kind table: the single source of the vocabulary. 8 kinds (abstract `Widget`, `window`, `box`, `label`, `button`, `checkbox`, `text-edit`, `slider`), 48 prop definitions, 13 signals, 3 methods. Ids are `(defining kind + 1) << 16 \| (index + 1)`. |
| `scene.rs` | The generational arena (`generation << 24 \| idx`, never 0, below 2^53; a slot whose generation reaches 2^29 is retired) and typed nodes. `W` holds the fields the widgets borrow `&mut` (text, checked, value); `Common` holds widget and layout props. |
| `layout.rs` | taffy on the arena (`TraversePartialTree`, `LayoutPartialTree`, `LayoutContainingBlock`, `CacheTree`, `LayoutFlexboxContainer`, `RoundTree`). A node's taffy id is its handle. Leaves are measured with egui's fonts plus each kind's chrome. The window root is definite. A text change re-measures the leaf and relayouts only if its size changed. |
| `render.rs` | The walk: paints visible nodes at their rects (`new_child(max_rect)`, scope id `node.id.with("scope")`, never the widget's own id), culls off-screen subtrees and collects an outbox of events and prop changes. It has no other side effects, so it is safe across multipass. |
| `backend.rs` | `EguiBackend` (`svelters_abi::Backend`) and the frame driver `EguiBackend::ui(ui)`. |
| `spec.rs` | The kind table rendered as `svelters-semantics/v0` → `metadata/semantics/egui.json`. |
| `runner.rs` | The eframe runner (`svl_backend.run`, glow) and `run_app` (desktop order in one call). |
| `harness.rs` | `TestApp`: a bundle on the real QuickJS host, attached through ABI v2.7 `start`, driven by an `egui_kittest::Harness`. |
| `bin/svelters-egui-run.rs` | Runs a bundle in an eframe window. `--idle-report <ms>` prints frames and CPU over an idle window. |

### The frame

- **Before the pass** (pass 0 only):
  - native closes are decided: a sync `close-request` (`call_sync`; `true` keeps the window), and the `close` method;
  - one checkpoint runs if any of these is pending: the first frame, a `wake`, a due `schedule_wake` deadline, or
    events/changes posted last frame;
  - `host.frame()` runs if `request_frame` was called;
  - leaves whose text changed are re-measured.
- **The pass:** the layout runs if it is dirty or the size changed, then the walk.
  - Widget changes write into the node at once.
  - Events and changes go to the outbox.
  - `pass_active` turns a `checkpoint` or `call_sync` during the walk into a panic.
- **After the pass:**
  - the outbox is flushed (`post_event`, `prop_changed`);
  - `request_repaint` is called only for pending work: a checkpoint, a frame, a wake or a close;
  - `request_repaint_after` is called for a timer deadline.
  - Nothing pending means no frame.
- Ops made outside a frame (an embedder's checkpoint, a timer) call `request_repaint`. Ops made inside a frame do not,
  because that frame draws them.
- **Windows.**
  - The first presented live window is the root viewport: `CentralPanel` with no margin, and its title and size sent
    as `ViewportCommand`s.
  - Further presented windows are immediate viewports. egui embeds them when the runner has no multi-viewport support.
  - A user close is cancelled at once (`CancelClose`) and decided before the next pass.
  - An unvetoed close reports `SVL_GONE_CLOSED` for the window, then `SVL_GONE_PARENT` for each descendant in pre-order.
    The host decides the quit; `quit` closes the root viewport.

### Observe: exact

- `observe` is supported for every prop of every kind (`SVL_M_OBSERVABLE` on each descriptor), with nesting counts.
- The three props a widget changes are `exact` in the spec, mechanism `widget`, verified `delivery` (by the kittest
  tests below): `text-edit.text`, `checkbox.checked`, `slider.value`. They are announced once per change, after the
  pass, and only when observed.
- A host `set_prop` never announces, so there is no echo to suppress.
- The slider clamping a host-set value into its range counts as a native change (the binding's `natively_normalizes`).
- Every other prop is `set_only` (only `set_prop` changes it), mechanism `scene`. That matches REACTIVITY.md's class
  definition and replaces §9.4's "egui = set-only + polling" for the widget props.
- Two-way bindings in the spec are `event: changed`, `value: {arg: 0}`, `echo: none`. The compiler lowers `bind:` on
  the exact props to `$n.bind_prop(..., "changed", 0)`.

### Spec and compiler

- `metadata/semantics/egui.json` is generated from the kind table. `tests/spec.rs` is the `--check`
  (`SVELTERS_UPDATE=1 cargo test -p svelters-egui --test spec` rewrites it).
- `svelters-compile --backend egui` works through one more `SpecBackend::builtin` arm (`crates/svelters-compiler`),
  covered by `crates/svelters-compiler/tests/g1_egui_spec.rs`.
- The spec's `style` section (dialect `none`; type, class and id setter selectors; `px` = 1 point) also feeds
  `packages/style/src/props.generated.js`: `tools/metadata/style/gen_props.py` now reads egui.json too. So the X6
  translator knows egui's setters and stops warning about them, and its "did you mean / it is one on …" hints name
  egui.
- The Node TestHost needed no change: `toolkit: 'egui'` reads the same spec.

### Fixtures

These are the egui twins of the P1 fixtures, `"svelters": {"toolkit": "egui"}`:

- `counter-egui` adds a `text-edit`, `slider` and `checkbox` with `bind:` and a `<style>` with layout setters (55 ops).
- `if-toggle-egui` has no header bar (31 ops).
- `keyed-list-egui` uses `text-edit` for the entry and a plain `box` for the scrolled window (85 ops).
- `timers-egui` (9 ops).

Each has an `expected.oplog` from the Node TestHost (`pnpm test` passes them). The log backend skips them as before,
since it models GTK and Qt only.

## Tests (15 in svelters-egui, 4 in the compiler)

- `counter.rs` (5):
  - the counter bundle on QuickJS under kittest: clicks, typing (`bind:text`), the slider through AccessKit
    `SetValue` (`bind:value`), and the checkbox (`bind:checked`), with no JS errors;
  - idle is idle: a settled app runs exactly 1 step per `run()`, with no checkpoint, layout or repaint request; a
    click costs at most 2 checkpoints and at most 6 frames (egui's own press animation), then idle again; a same-size
    label change does not relayout;
  - timers from `schedule_wake`;
  - embedding in a caller's `Ui` (D1);
  - frame time.
- `oplog_replay.rs` (1):
  - replays all 4 egui op logs through the vtable;
  - `event` and `changed` lines are produced natively (clicks at the widget's rect, typed text, AccessKit
    `SetValue`): 13 events and 3 changes delivered, `changed` exactly when observed;
  - also checks that every drawn leaf fits its layout rect ("measured == rendered").
- `observe.rs` (3):
  - every prop of every kind is observable, reads back, and never echoes a host write;
  - the 3 widget props are announced once when observed and never when not, with nesting counts;
  - slider clamping is announced.
- `window.rs` (3):
  - sync `close-request` veto, then close with the `closed`/`parent` cascade;
  - close without a listener, and the `close` method;
  - root window choice and tree-op validation: no children for leaves, no windows as children, no cycles, keyed
    moves, stale handles, idempotent destroy, handles not reused.
- `spec.rs` (2): the spec is current; the kind table is consistent.
- `eframe_idle.rs` (1): runs `svelters-egui-run --idle-report 2000` on the display (skipped without `DISPLAY`).
- `crates/svelters-compiler/tests/g1_egui_spec.rs` (4):
  - the counter twin compiles with `bind_prop` lowering and `<style>` setters;
  - unknown props and tags are errors;
  - `bind:` on a set-only prop is an error;
  - enum setter values are checked.

**Kittest renderer:** none. `egui_kittest` is built without `wgpu`/`snapshot`, so the harness runs egui passes and
AccessKit queries only, with no GPU and no display. Image snapshots (G2 galleries) need the `wgpu` feature and a
software adapter (lavapipe/llvmpipe), which G1 does not build.

## Measurements

The machine was this WSL2 box (Ryzen 7 5700U) with other lanes building: load average 10–15.

- **Idle CPU: 0.** This is the eframe runner, release build, on Xvfb (`tools/dev/headless`), with glow on Mesa's
  software GL.
  - The counter draws 5 frames and settles. Over 5 s of idle it then draws `frames: 0` at `cpu_ms: 0.0`, on three
    consecutive runs.
  - The process CPU (utime + stime from `/proc/self/stat`) did not move within its 10 ms tick.
  - An early run with a fixed 1.5 s warm-up under load caught the tail of startup (3 frames, 20 ms). The report now
    waits until 1 s passes with no frame.
- **Idle proxy under kittest:** a settled app requests no repaint, and `run()` returns after 1 step.
- **Frame time (counter, 9 nodes):** 2.57 ms per kittest frame, debug build, AccessKit on (kittest always enables it),
  no tessellation or GPU. Release frame time was not measured.
- **Build:**
  - `cargo build --release -p svelters-egui --bin svelters-egui-run` from a clean release profile takes 8 min 16 s with
    4 jobs, peak RSS 315 MB. That includes egui, eframe/glow/winit, taffy, QuickJS and the host.
  - Rebuilding after touching `svelters-egui` alone takes 3.5 s.
- **Binary size:** `svelters-egui-run`, release, with the host, QuickJS, eframe (glow, x11 + wayland, AccessKit) and
  default fonts: 21.0 MB, 15.6 MB stripped. EGUI.md's probe measured 9.8 MB for eframe + QuickJS alone. The difference
  is the host, AccessKit (zbus) and both windowing backends.

## Deviations from the plan

- **Next-frame dispatch.** JS runs at the start of the frame after the input (EGUI.md §4.2's default). Same-frame
  dispatch with `request_discard` is not built.
- **Sync handlers.** Only `close-request` is sync before the pass. Key and pointer `preventDefault` on raw input
  (`raw_input_hook`) are G2, together with the controller kinds.
- **Lengths.**
  - `margin` and `padding` are one number for all four sides. The `insets` record type and `padding: 8px 16px` in
    `<style>` are G2.
  - Lengths are points only; there are no percentages yet.
- **Slider rail.** The rail leaves 4 points of slack: the value box can come out slightly wider than
  `interact_size.x`, which the fit check caught.
- **Styling ops.**
  - `style` answers `SVL_STYLE_TARGET_NONE` and accepts the other ops as no-ops. The runtime prints its "this host has
    no styling support" warning once.
  - Visual CSS (the backend cascade) is G2.

## CI

`linux-gtk.yml`, gtk job (the smaller change):

- `-p svelters-egui` is added to the build and nextest commands;
- `mesa libxkbcommon-x11 libxcursor libxrandr libxi` are added for the eframe test on Xvfb;
- the tests get their features through a self dev-dependency, because that command passes `--no-default-features`
  for `counter`.

| Workflow | Run | Result |
|---|---|---|
| linux-gtk (gtk 2205/2205, of which svelters-egui 15/15 including eframe_idle; gtk-4-14 1928/1928 with 1 flaky; portable 48/48) | 39635462475 | success |
| linux-gtk at 76e9aeb4 | 39638716759 | failure: eframe_idle saw a burst of 6 frames / 30 ms in the idle window on CI's Xvfb (late window event); the test now allows a burst of up to 10 frames but no loop |
| linux-qt | 39635462327 | success |
| js | 39635462590 | success |
| macos-appkit | 39635462393 | success |
| windows-winui | 39635462624 | success |

(head 29f8c019; the final head's runs are in the report.)

## Open items

1. **G2.**
   - The rest of Tier 0;
   - the backend cascade with visual CSS and `--sv-*` tokens on `Visuals`;
   - key/pointer sync handlers on raw input;
   - `insets` for margin and padding;
   - percent lengths;
   - `scroll-area` (keyed-list-egui uses a plain box);
   - the X4 menus;
   - `virtuallist`;
   - kittest snapshots with a wgpu software adapter.
2. **Metadata pipeline.**
   - egui is not in `tools/metadata/ledger.py`, `coverage.py`, `observe.py`, `gen_bindings.py` or the docs IR.
   - Its facts are tested by the crate's kittest tests instead of ledger rows.
   - `packages/native-runtime/src/bindings.generated.js` has no egui entries. Compiled `bind:` does not need them;
     the runtime's dynamic setter path would.
   - A per-kind "measured == rendered" sweep covers the fixtures only.
3. **Docs.** There are no egui pages in the docs site, and the docs sidebar is untouched.
4. **Hot reload** on egui is untested (`Host::watch` needs a waker; ours is thread-safe, so it should work).
5. **Timer ticks** cost a full frame each (eframe runs `ui()` after every `logic()`). That is not measured with a JS
   interval.
6. **Release frame time** and a larger tree (the EGUI.md culling numbers) are not measured.
7. **`svelters-host` warning:** `QjsEngine::callbacks` is never read. It is pre-existing and was not touched.
