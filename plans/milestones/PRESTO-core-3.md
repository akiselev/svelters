# PRESTO-core-3: presto-core, continued (presto batch 2, lane CORE-3)

Branch `ci/presto-core-3` from `ci/presto-base` (e3946c37). Session `session_012X4RdtAUrgfNuRfD1Vh4ge`,
2026-10-25. Owns `crates/presto-core/**`. Brief: the coordinator's message (CORE-2's queue, in order) on
top of `plans/cloud/presto-b2/BRIEF.md`, `plans/milestones/PRESTO-core.md` and `PRESTO-core-2.md`. All
additions; no presto-base change, no edit outside `crates/presto-core/**` and this record.

## 1. WIDGETS-BASIC (a)–(g)

| Item | API |
|---|---|
| (a) `::part(name)` styles | `KindSpec::parts: &[&str]`; the restyle computes each part with svelters-style's `compute_pseudo` (inheriting from the node; `:hover::part(x)` restyles with the host); a changed part repaints the node (and folding ancestors). Read with `NodeView::part(name) -> Option<&ComputedStyle>` in every context. |
| (b) JS listens | `NodeView::listened(signal)`; `Kind::listen_changed(&mut self, cx: &mut PropCx, signal, listened)` when a signal gains its first or loses its last listener. |
| (c) cancelable defaults | `Document::set_sync_delivery(Box<dyn FnMut(&JsEvent) -> Outcome>)` (svelters-presto's `call_sync`); `EventCx::emit_sync(signal, arg) -> Outcome { prevented, stopped }` delivers at once when JS listens (else queues as `emit` and reports nothing prevented). With a deliverer, `pointerdown`, `pointerup`, `click`, `dblclick`, `wheel`, `keydown`/`keyup` also reach JS **before** the native default (EV1): a prevented `pointerdown` keeps the press (no press state, focus, click or drag), `pointerup` the click, `click` the kind's `Click`, `wheel` the scroll, `keydown` the kind, Tab and scope activation. Events the deliverer took are not repeated in `Dispatched::events`. The deliverer runs while the document is borrowed: **svelters-presto queues the listeners' writes and applies them after the dispatch** (`js_transaction`). |
| (d) peer groups | `Kind::peer_group(&self, &NodeView) -> Option<PeerGroup { key: Option<(PropId, Value)>, stop: Option<PropId> }>` (members: same-kind siblings whose `key` prop matches); `EventCx::peer_request(PeerTarget::{Others, Rove(Rove::{By(n), First, Last})}, PeerMsg::{Roved, Notify(Option<Value>)})`, applied after the event in order through `Kind::peer(&mut self, cx, from, msg)` (a member's own requests queue behind). A group of two or more focusable members is one Tab stop (first member whose `stop` prop is true, else the first); roving skips members that cannot take focus (WIDGETS-BASIC Q2: yes), focuses the target (keyboard) and then sends it the message. |
| (e) container hooks | `Kind::chrome_padding(&self, &mut MeasureCx) -> Option<Insets>` (added to the length parts of the padding at restyle); `KindSpec::places_children` + `Kind::child_layout(&self, &ChildLayoutCx { parent, child, index, count }, &mut taffy::Style) -> bool` (a hidden child keeps its layout but is not painted, hit, focusable or in the a11y tree: `NodeFlags::CONCEALED`; `ChildLayoutCx::anchor_of(k)`); `Kind::children_inert(&self) -> bool` (asked after every prop write; descendants get `NodeFlags::INERT` and `:disabled`, lose focus; inserts and moves follow). |
| (f) prop-derived state | `KindSpec::prop_states: StateMask` + `Kind::prop_state(&self, &NodeView) -> StateMask`, applied after every prop write and at creation. |
| (g) scale | `NodeView::scale()` (the window's; 1 when detached), so `PrepareCx`, `RecordCx`, `MeasureCx` all have it. |

Also: a `Layout`-class write of a non-text prop now restyles the node, so a kind's `layout_style` (and
chrome) follows its props; before, only the cached taffy style was relaid out (a box's `orientation`, and
the common `hidden`, did not apply until the next restyle).

## 2. (i) scope activation, (j) label `for`

- Enter (Escape) without Ctrl/Alt/Meta/Shift that the focused kind did not handle (and JS did not prevent)
  sends an AccessKit `Click` to the default (cancel) button of the focus's scope (nearest `focus-scope`,
  else the window; nested scopes keep theirs). A button is a node whose kind has a bool `default`
  (`cancel`) prop, enabled and shown. `:default`: the button whose `default` is set, or, with
  `Document::set_auto_default(true)` (the bridge sets it from `Policy::auto_default`), the focused push
  button of a `dialog` scope; it follows focus.
- `EventCx::focus_anchor(&str) -> bool`; `Document::find_anchor(window, anchor)`. A node whose kind has a
  `for` prop labels the node with that anchor: AccessKit `labelled_by` (and the anchor is the author id).

## 3. Common (`"*"`) members

Props `anchor`, `anchor-scope`, `invalid` (`:invalid`, AccessKit `invalid`), `dialog-role` (enum, values
`common::DIALOG_ROLES`), `status-tip` (`Document::status_tip(window)`: innermost hovered, else focused
ancestry), `status-section` (enum `common::STATUS_SECTIONS`), `drag-data`, `drag-type`, `drop-types`;
signals `drag-begin` (void), `drag-end` (record `dropped`, `action`), `drag-enter` (string `type`),
`drag-leave` (void), `drop` (record `type`, `data`, `x`, `y`, `action`). Enum attribute selectors now
match by name (`[dialog-role=reject]`), for every enum prop.

In-app drag and drop (`input/dnd.rs`, svelters WG4b semantics): a primary press on a source that moves past
6 points (egui's `max_click_dist`) with the button held starts a drag (the press is cancelled with
`PointerCancel`, `:dragging`, `drag-begin`); the deepest accepting node under the pointer is the target
(`:drop-target`, `drag-enter`/`drag-leave`); the release drops (`drag-leave`, `drop`, then the source's
`drag-end`), Escape or a pointer cancel ends it undropped. Actions from the modifiers as Qt (copy: Ctrl,
Option on macOS; link: Ctrl+Shift, Cmd+Option). Cursor `grabbing`/`no-drop` during the drag.

Not done here: OS file drops onto targets (`text/uri-list`; `InputEvent::FileHovered/FileDropped` still
dispatch nothing), auto-scroll near a scroll container's edge during a drag, the drag ghost
(`::part(ghost)`: a widget's), the `grab` cursor over an idle source.

## Tests

Written, not run (the brief's rule): `src/tests/hooks.rs` (parts, listened, prop state, scale, chrome,
child layout, inert, peers and Tab stop and roving, Enter/Escape and auto-default, label for and
`labelled_by`, `invalid`, enum attributes, status tip, `emit_sync` and a prevented `pointerdown` through
the sync deliverer, drag and drop, type matching); `common::tests` covers the new ids.

## Built / not run

`cargo check -p presto-core -p presto-widgets --all-targets` after each part. No test run, no clippy, no
other OS. CI on the pushed sha is in the coordinator message.

## For WIDGETS-BASIC (host.rs bridge)

Map the shim to the new hooks: `Styles::parts` from `NodeView::part` over `KindSpec::parts` (declare the
widget's part names); `Kind::listen` from `listen_changed`; the `prevented` path through `emit_sync`
(the hyperlink's `open-url` only when `clicked` was not prevented); `PeerGroup`/`PeerRequest` →
core's (the shim's `PeerMsg::Checked(args)` → `PeerMsg::Notify(Some(Value::List(args)))`);
`chrome_padding`, `child_layout` (+ `places_children: true` in the spec), `children_inert`;
`prop_state` with the widget's bit mask in `prop_states`; `focus_anchor`; `scale` into the shim contexts;
`Document::set_auto_default(policy.auto_default)` where the policy is installed.

## Questions for the coordinator

1. **Sync delivery and re-entrancy.** The deliverer cannot touch the document (it is borrowed during
   dispatch). Recommendation: svelters-presto queues JS writes made inside `call_sync` and applies them
   right after `dispatch` returns, in `js_transaction(revision)`; the prevented bit is all core needs
   synchronously.
2. **`Layout` writes restyle the node.** This costs a cascade match per layout-prop write (text props are
   exempt: they only remeasure). Recommendation: keep it; a per-kind `KindSpec` flag to skip it is easy if
   profiling asks.
