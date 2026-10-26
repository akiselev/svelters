# RMEGUI-S: egui-free skelter-test and svelters-automation, a designer scene API, window readback

Lane S of the rmegui round (cadatomic is removing egui; cadatomic `docs/notes/rmegui/PLAN.md`). Branch
`ci/rmegui-svelters` from master `a15903a9`. Brief: `plans/cloud/rmegui/LANE-S.md`. Coordinator: the cadatomic
coordinator (`session_01Kt5cvt9hDGVcXNCATLsKRK`). `origin/master` merged (`029a48d2`, `43caa4b9`, docs only).

## What landed

| Step | Commit | What |
|---|---|---|
| 1 | `49d43a4c` | skelter-test: a non-default `egui` feature has `EguiHarness`, `CoreIslands`, the egui vocabulary of the verbs (`egui::Event`, `Key`, `Pos2`, `Vec2`, `Modifiers`, `PointerButton`, `Node::rect`) and the `egui`, `egui-wgpu` and `skelter-egui-island` deps. `Harness`, `Consumer` (AT-SPI) and `A11yTree` build without it. `Harness::unnamed_widgets` uses a copy of egui 0.37's `is_input`. New `tools/dev/check-egui-free`, which skelter.yml runs. |
| 2 | `a9a00584` | svelters-automation: an egui-free core plus feature `egui` (`kittest` and `run` imply it). skelter-test: `automation` is egui-free; `SkelterDriver` needs the new `automation-egui`. |
| 3 | `7e246372` | `svelters_skelter::designer`: the designer scene API (below); skelter-core `Document::{matched_rules, force_state, forced_state, hit_stack, set_sheet_info}`; `SheetSource` moves to svelters-style. |
| 4 | `1a5f1cf4` | skelter-compositor: `CompositorHandle::read_window` / `CompositorApi::read_window` (SV58), with a lavapipe test. |
| fix | `6ef8ce70` | (coordinator, first priority) skelter-widgets: a hovered (or pressed) control claims only pointer events (`shim::Event::is_pointer`), so Tab after a click reaches focus traversal: button, checkbox, hyperlink, icon, image, label, radio, slider, switch, toggle-button, and frame/collapsing's `down_on`. Test: skelter-test `tests/hover_keys.rs` (fails without the fix). Self-contained: cherry-pickable. |
| preview | `2567201f` | (coordinator) `SkelterBackend::{set_override, clear_override, clear_overrides, overrides}`: the designer's live preview, as svelters-egui's (runtime writes held back, the last one restored; freed nodes drop theirs). Test: svelters-skelter `tests/designer.rs` `overrides_preview_a_prop_and_give_the_runtimes_value_back`. |

### Step 2 in detail (svelters-automation)

- **Core (no egui, no svelters-egui):** selectors, engine, `SceneSource`, `SourceDriver`, `Session` (acts, waits,
  journal, JSON methods), server and client, `VirtualClock`. It has its own egui-shaped types:
  - `geom`: `Pos2`, `Vec2`, `Rect`, `pos2`, `vec2`;
  - `input`: `Event` (`PointerMoved`, `PointerButton`, `MouseWheel`, `ModifiersChanged`, `Key`, `Text`, `AccessKit`;
    `#[non_exhaustive]`), `Modifiers` (egui's fields: `command` is the platform's shortcut key), `PointerButton`,
    `MouseWheelUnit`, and `Key` (egui 0.37's name table, aliases included; a test checks it against egui's);
  - `Image`: RGBA8, not premultiplied, with `region`.
  `A11yNode` uses `accesskit` directly (one copy in the lock: the fork).
- **Feature `egui`:** svelters-egui's `SceneRef` as a `SceneSource`, the `Driver` trait (unchanged: egui types; every
  `Driver` is a `SourceDriver`: the blanket impl converts `Event` → `egui::Event`, `ColorImage` → `Image`),
  `Clocked`/`RealTime`, `role`, `engine::resolve`, and `From` both ways between egui's and the core's types.
- `MatchedRuleInfo`, `DeclInfo` and `StyleSource` moved to svelters-style; svelters-egui re-exports them.
- The generic types (`Ref`, `NodeInfo`, `ActResult`, `WaitReport`, …) default to `DefaultNode`: svelters-egui's
  `NodeId` with `egui`, `u64` without it (name the node type, e.g. `Ref<NodeKey>`).

### Step 3 in detail (the designer API; cadatomic-ui `designer.rs`'s calls)

| svelters-egui (today) | svelters-skelter (`svelters_skelter::designer`) |
|---|---|
| `backend.scene().read(\|s\| …)` | `SkelterBackend::scene().read(\|s\| …)` (`SceneRef` is `Copy`) |
| `s.by_source(file, offset)` | same; nodes in handle (creation) order |
| `s.hit_stack(ViewportId, egui::Pos2) -> Vec<Target>`, `t.node()` | `s.hit_stack(WindowId, skelter Point) -> Vec<Target>`, `t.node()` is the JS handle |
| `s.get(id)`, `s.shown(viewport, id).rect`, `n.drawn()` | `s.get(handle)`, `s.shown(window, handle)` (`rect`, `visible`), `n.drawn()` (world rect) |
| `n.id()` (`u64::from` → handle), `n.tag()`, `n.anchor() -> Option<&str>` | `n.id()` is the handle (`svl_handle`), `n.tag()`, `n.anchor() -> Option<String>` |
| `n.source_loc() -> Option<SourceLoc<'_>>` | `n.source_loc() -> Option<SourceLocBuf>` (owned; `raw()`, `element()`, `sites()`) |
| `n.matched_rules()` | same (`svelters_style::MatchedRuleInfo`, sources in dev builds) |
| `svelters_egui::scene::SourceLoc::parse` | `svelters_skelter::designer::SourceLoc::parse` |
| `svelters_egui::style::pseudo_state(name)` | `svelters_skelter::style::pseudo_state(name)`; `backend.pseudo_state(name)` resolves `:state(ns.name)` too |
| `provider::StateBits`, `StateBits(a.0 \| b.0)`, `StateBits::NONE` | `designer::StateBits` (svelters-style's `StateMask`, re-exported under that name) |
| `backend.force_state(handle, bits)` | same (`forced_state(handle)` too) |

- Forced bits live in skelter-core's per-node style column. They are ORed into the cascade's element state only:
  `Document::state`, events and a11y never see them, and a freed node drops them.
- svelters-skelter now keeps each sheet's name, and in dev builds its `<style>` source (`id SEP css SEP file SEP map`,
  which it used to drop), so matched rules carry `StyleSource`s exactly as on egui.
- `hit_stack` comes from the last frame's hit list, so `pointer-events: none` nodes are left out (egui lists them) and
  so are nodes JS doesn't know.

### Step 4 in detail (window readback, SV58)

`CompositorHandle::read_window(window, wait) -> Option<WindowImage>` (`size` in device pixels, `scale`, `rgba`):
the compositor thread renders the window's main surface as its last frame drew it (applied commits, sampled
animations, caret) whole into an offscreen texture (`Backend::read_back`) and reads it back. It doesn't depend on the
swapchain allowing copies or reads, so it works on every platform and for occluded windows. OS layers (video,
promoted viewports) are not in it. skelter's `CompositorApi::read_window` forwards to it, with a 10 s wait;
skelter-test's `HeadlessCompositor` keeps the default `None` (tests use `Harness::render`).

## What cadatomic must change

1. **Features:** `skelter-test` with no features (add `automation` for `SkelterNativeDriver`/`SkelterScene`; `egui`
   and `automation-egui` only while islands remain). `svelters-automation` with no features for a skelter-only
   host.
2. **svelters-automation API** (`waits.rs`, `tour/runner.rs`, `skelter/probe.rs`):
   - `SourceDriver` impls add `type Filter` (`()` when ignored); `observe` takes it; `send` takes
     `svelters_automation::input::Event`; `screenshot` returns `svelters_automation::Image`.
   - `Session::journal_subscribe` / `bridge_scene_changes` take `D::Filter` (`()` for `SkelterNativeDriver`;
     `ChangeFilter` on egui).
   - `NodeInfo.rect`, `visible_rect`, `A11yNode.rect`, `SceneSource::rect/hit/drawn` use `svelters_automation::{Rect,
     Pos2}`; `Session::screenshot` returns `Image`. `Wheel::lines/points`, `Drag::to/button/modifiers` take
     `impl Into<…>`, so egui values still work under `egui`; `At::Local` wants a `svelters_automation::Pos2`.
   - Use `svelters_style::MatchedRuleInfo` (or `svelters_egui::style::…`, still re-exported).
3. **Designer** (`cadatomic-ui/src/designer.rs`): use the table above. The changes: `hit_stack` takes the
   `WindowId` and a skelter `Point`; `n.id()` is already the handle; `anchor()` and `source_loc()` return owned values;
   `shown` is `(window, handle)`.
4. **Preview:** `backend.set_override(h, prop, &v)` / `clear_override(h, prop)` as on svelters-egui (`h` is the
   handle `n.id()` gives).
5. **Screenshots:** `screenshot.window` and tour shots call `CompositorHandle::read_window` (or
   `app.compositor()?.read_window(w)` through `CompositorApi`). The image is RGBA in device pixels; divide by `scale`
   for points.

## Tests

Added: svelters-automation `input::tests` (key table, against egui's with `egui`), `image::tests::region_crops_in_pixels`;
svelters-skelter `tests/designer.rs` (5, overrides included: by_source and SourceLoc, matched rules with sources and winners, a forced
hover restyles and clears, hit_stack order); skelter-compositor `tests/headless.rs`
`a_drawn_window_reads_back_as_rgba`.

Run locally, filtered, all passed: svelters-automation `--lib`, `--test canvas --test counter --test list_a11y`;
skelter-test `--test native_session --test scene_source --test automation`; svelters-skelter `--test designer`;
svelters-egui `--test style_source`; skelter-test `--test hover_keys`; skelter-compositor `--test headless` on lavapipe (installed in the container
for this; `WGPU_BACKEND=vulkan`). `cargo check --all-targets` on skelter-test, svelters-skelter, svelters-automation,
svelters-egui, skelter-core, svelters-style, skelter, skelter-compositor.
**Not run locally:** the full workspace, any whole crate suite, clippy, conformance. CI is the gate.

## Open questions (with a recommendation each)

1. ~~The designer's preview~~: done on the coordinator's request (row "preview").
2. **`hit_stack` and `pointer-events: none`:** skelter's hit list leaves those nodes out, so a designer can't pick
   one by pointing. Recommendation: leave it until a designer needs it; the fix is to record those nodes in the
   frame's list with a "not for input" bit.
3. **`SourceLoc` is duplicated** in svelters-egui and svelters-skelter (about 50 lines). Recommendation: leave it
   until svelters-egui is retired.

Readback on real devices: only lavapipe was run (here and in CI). macOS Metal and Windows DX12/WARP use the same
offscreen path (BGRA or RGBA, both converted); the skelter workflow's macOS and Windows jobs run the test only where
an adapter exists, so a real-device check by cadatomic's viewport-tours lane is advised.

## CI

All six workflows green on `2567201f` (the last code commit): js 40350066938, linux-gtk 40350066641, linux-qt
40350066407, macos-appkit 40350066597, skelter 40350067419, windows-winui 40350066636. The commit adding these ids is
docs-only.
