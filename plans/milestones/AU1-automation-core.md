# AU1: automation protocol core, in process (`svelters-automation`)

Spec: `plans/research/E2E-INSPECTOR.md` §1, §5 (AU1 row), §6 (Decided). Replaces TK2 and absorbs SG7
(`scene.snapshot`). Lane branch `ci/au1`.

## What landed

A new crate, `crates/svelters-automation`, plus two small read accessors in svelters-egui.

| Area | API | Notes |
|---|---|---|
| Switch | `enable()`, `disable()`, `is_enabled()` | **Off by default** (§6 Q1). Until the app calls `enable()` (cadatomic: its settings page), `Session::new` and every call fail with `automation.disabled`. No cargo feature is needed to ship it; the `kittest` feature only adds the test drivers. |
| Session | `Session::new(driver, Grants)`, `Session::for_test(driver)` | `Grants { read, act, tooling }`; `READ`, `ACT`, `ALL`. A read-only session's acts fail `grant.denied`. |
| Host idle | `Session::set_idle(Fn() -> bool)` | The agreed seam: no `wait.*` (and no act reply) answers while the predicate is false. cadatomic's IdleBarrier plugs in here. |
| Tooling roots | `set_tooling_roots(Fn(&str) -> bool)`, `include_tooling(bool)` | By root key. Queries, snapshots and `app.roots` skip them unless included (needs the `tooling` grant). |
| Query | `query(selector, limit)`, `locate(selector)` (strict) | Engines: `role=` (`[name="…"]`, `[name*=…]`, state names), `label=`, `text=` (`"exact"` or bare substring that ignores case), `anchor=`, `root=`, `pos=x,y`, `nth=`, `css=` (the default; svelters-style's matcher on the scene, provider states as `:state()`). Chained with `>>`. |
| Read | `node(Ref)`, `roots()`, `snapshot(scope)`, `a11y_tree()` | `NodeInfo {ref, tag, role, name, text, value, anchor, root, rect, states}`. The snapshot is SG7's text form in the aria-snapshot shape (`- checkbox "Agree" [checked]`, `- textbox "Name": Ann`). |
| Refs | `Ref {id, epoch}`, `bump_epoch()` | A ref from an older epoch, or to a freed node, fails `node.stale` (AU3 bumps the epoch on each swap). |
| Act | `click`, `hover`, `focus`, `fill`, `set_value`, `press(chord)`, `type_text` with `ActOptions {tier, force, timeout_ms}` | `Tier::Semantic` sends AccessKit action requests (`Click`, `Focus`, `SetValue`) to the AccessKit node drawn for the scene node; with no AccessKit node it falls back to synthetic. `Tier::Synthetic` (the default) sends egui pointer and key events at the node's centre in one frame. Actionability before each act (§1.5): attached, visible, stable over two frames, enabled, and `hit_test` at the centre finds the node, a descendant or an ancestor. Each act replies after the settle with `ActResult {frame, epoch, tier, changed}`. |
| Wait | `wait_settled(ms)`, `wait_for(selector, WaitState, Option<Expect>, ms)` | Settled = `Embed::settled()` empty, no pending injected input, host predicate idle. Timeouts count **virtual** ms (frames × the driver's step), and a timeout names the chain of busy reasons. |
| Capture | `screenshot(selector?)` | Crops to the node. Drivers without a renderer (kittest without wgpu) answer `capability.absent`. |
| Journal | `journal_subscribe(ChangeFilter)`, `journal_take()` | SG2 observers, coalesced per node, with `overflowed`. |
| Driver | `trait Driver { scene, busy, step, send, frame_ms, a11y_nodes, screenshot }` | Implemented for `harness::TestApp` and `harness::EmbedHarness` (feature `kittest`). cadatomic's `KittestDrive` wraps one; its windowed app can implement it over its own frame loop. |
| JSON | `Session::call(method, params) -> Result<Value, Error>` | `session.hello`, `app.roots`, `scene.query`, `scene.locate`, `scene.node`, `scene.snapshot`, `a11y.tree`, `input.click/hover/focus/fill/set_value/press/type`, `wait.settled`, `wait.for`, `capture.screenshot`, `journal.subscribe/take`. Targets are `{"selector": "…"}` or `{"ref": {"id", "epoch"}}`; acts take `tier`, `force`, `timeout`. AU2's IPC transport carries this schema unchanged. |
| Errors | `Error {code, message}` | Codes: `selector.invalid`, `node.not_found`, `node.ambiguous`, `node.stale`, `node.not_actionable`, `timeout`, `capability.absent`, `grant.denied`, `automation.disabled`, `method.unknown`, `params.invalid`. |

svelters-egui additions (read-only):
- `svelters_egui::accessible_name(NodeRef)`: the name AccessKit gets (`a11y.rs`, refactored so the walk and
  automation share one function). This is why `role=…[name=…]` agrees with screen readers.
- `SceneRef::root_key(RootIx)`: the inverse of `root_by_key`.

The kind→role table is `svelters_automation::role::role_of` (ARIA names: `button`, `textbox`, `slider`,
`checkbox`, `label`, `tab`, `combobox`, …; provider kinds and plain containers are `generic`).

## Stable API for cadatomic

Treat these as stable: `enable`, `Session::{new, for_test, set_idle, set_tooling_roots, include_tooling, query,
locate, node, snapshot, click, fill, set_value, press, type_text, wait_settled, wait_for, call}`, `Driver`,
`Grants`, `Ref`, `NodeInfo`, `ActOptions`, `Tier`, `WaitState`, `Expect`, `Error` codes, and the selector
syntax above. Additions will be additive.

Minimal use (in process, kittest):

```rust
let mut s = svelters_automation::Session::for_test(TestApp::from_source(dir)?);
s.set_idle(move || barrier.is_idle());                 // cadatomic's IdleBarrier
s.click(r#"role=button[name="Increment"]"#, Default::default())?;
s.wait_for("role=label", WaitState::Visible, Some(&Expect::Text(TextMatch::Exact("Count: 1".into()))), 5_000.0)?;
assert!(s.snapshot(None)?.contains(r#"- label "Count: 1""#));
```

## Tests

`crates/svelters-automation` (added to linux-gtk's nextest `-p` list):
- `selector::tests::parses_chains_and_engines`: the grammar, `>>` inside quotes, errors.
- `counter::engines_resolve_the_same_node`: role, text, css, nth and label resolve to the same `NodeId`, through Rust and
  through `call("scene.query")`; strictness (`node.ambiguous`), `node.not_found`, `capability.absent`, `method.unknown`.
- `counter::acts_reach_js_and_reply_after_settle`: synthetic and semantic clicks, semantic and synthetic fill, a
  numeric `set_value`, a JSON semantic click on the checkbox. Each reply comes after the settle, and the final
  snapshot is checked line by line.
- `counter::waits_count_virtual_time_and_the_host_predicate`: a 30 s virtual wait times out without 30 s of wall time
  (the whole test, app load included, takes 1.3 s locally in a debug build); the host predicate holds `wait_settled` and is named in the timeout.
- `counter::refs_journal_and_tooling_roots`: the journal after an act, `node.stale` after `bump_epoch`, roots.
- `disabled::automation_is_off_until_enabled` (its own binary, since the switch is per process): off by default,
  `grant.denied` for a read-only session, `disable()` stops calls.

All six pass locally (`cargo nextest run -p svelters-automation`).

## Not in AU1 (deferred, with the lane that takes it)

These AU1-row items are not done. The API leaves room for each.
- **Virtual clock in embed mode and seeds** (`clock.*`, `--seed`): these touch the host clock in `core.rs`, which
  goes through the coordinator (HOST owner). Waits already count virtual time through the driver's step
  (kittest's `step_dt`). This is the next AU1 follow-up (AU1b).
- **`#[svelters::test]`** and rewriting the gallery and `embed-egui` kittests with locators: AU1b.
- **`events`/`log`/`perf` streams**: only `journal` exists. The others need host hooks (outbox, console, TR3
  profile). Planned for AU1b or AU2.
- **`Origin::Tool(session)`** on writes a session causes: the scene's `Origin` has no tool variant yet, and acts go
  through egui input, so their writes are journaled as `Widget`/`Js`. This needs an `Origin` variant in svelters-egui
  (SCENE owner). Planned for AU3, with the inspector filter.
- **Test-only CSS pseudo-classes** (`:role() :name() :text() :visible :component()`): they need parser hooks in
  svelters-style. The `role=`/`text=` engines cover them for now.
- **Regex text matching** (`name=/…/i`): needs the `regex` crate. Selectors with `/…/` fail `selector.invalid`.
- **`component=` / `source=`** (AU3, compiler identity) and **`a11y=`** (with AU2): they parse but answer
  `capability.absent`. `a11y.tree` already returns the last frame's AccessKit nodes.
- **`style.computed`, `scene.layout`, `svelte.*`**: `style.computed` needs the backend's `computed_style` handle on
  `Driver`; `svelte.*` is AU3.
- **Virtual rows** (`Target::Virtual`) in selectors: candidates are scene nodes only.
- **Tooling roots on a real tooling `Embed`**: tested only as far as the counter app allows (its window root has no
  key). AU3's protected tooling `Embed` gets the full test.
- **Screenshots under kittest** need kittest's `wgpu` feature (lavapipe in CI). The `Driver::screenshot` hook is in
  place.

## Handoff

- Lane branch: `ci/au1`. Files: `crates/svelters-automation/**`, `crates/svelters-egui/src/{a11y.rs,lib.rs,scene/query.rs}`
  (additive), `Cargo.toml` (member + workspace dep), `Cargo.lock`, `.github/workflows/linux-gtk.yml` (`-p` list).
- `STATUS.md` is left for the coordinator.
