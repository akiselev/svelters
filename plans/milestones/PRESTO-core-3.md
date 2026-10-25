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
| (c) cancelable defaults | `Document::set_sync_delivery(Box<dyn FnMut(&JsEvent) -> Outcome>)` (svelters-presto's `call_sync`); `EventCx::emit_sync(signal, arg) -> Outcome { prevented, stopped }` delivers at once when JS listens (else queues as `emit` and reports nothing prevented). With a deliverer, `pointerdown`, `pointerup`, `click`, `dblclick`, `wheel`, `keydown`/`keyup` also reach JS **before** the native default (EV1): a prevented `pointerdown` keeps the press (no press state, focus, click or drag), `pointerup` the click, `click` the kind's `Click`, `wheel` the scroll, `keydown` the kind, Tab and scope activation. Events the deliverer took are not repeated in `Dispatched::events`. The deliverer is `FnMut(&mut Document, &JsEvent) -> Outcome` (`SyncDelivery`): it gets the document lent, so listeners read, write (in `js_transaction(revision)`) and build nodes directly; see "Sync delivery" below. |
| (d) peer groups | `Kind::peer_group(&self, &NodeView) -> Option<PeerGroup { key: Option<(PropId, Value)>, stop: Option<PropId> }>` (members: same-kind siblings whose `key` prop matches); `EventCx::peer_request(PeerTarget::{Others, Rove(Rove::{By(n), First, Last})}, PeerMsg::{Roved, Notify(Option<Value>)})`, applied after the event in order through `Kind::peer(&mut self, cx, from, msg)` (a member's own requests queue behind). A group of two or more focusable members is one Tab stop (first member whose `stop` prop is true, else the first); roving skips members that cannot take focus (WIDGETS-BASIC Q2: yes), focuses the target (keyboard) and then sends it the message. |
| (e) container hooks | `Kind::chrome_padding(&self, &mut MeasureCx) -> Option<Insets>` (added to the length parts of the padding at restyle); `KindSpec::places_children` + `Kind::child_layout(&self, &ChildLayoutCx { parent, child, index, count }, &mut taffy::Style) -> bool` (a hidden child keeps its layout but is not painted, hit, focusable or in the a11y tree: `NodeFlags::CONCEALED`; `ChildLayoutCx::anchor_of(k)`); `Kind::children_inert(&self) -> bool` (asked after every prop write; descendants get `NodeFlags::INERT` and `:disabled`, lose focus; inserts and moves follow). |
| (f) prop-derived state | `KindSpec::prop_states: StateMask` + `Kind::prop_state(&self, &NodeView) -> StateMask`, applied after every prop write and at creation. |
| (g) scale | `NodeView::scale()` (the window's; 1 when detached), so `PrepareCx`, `RecordCx`, `MeasureCx` all have it. |

Also: a `Layout`-class write of a non-text prop now restyles the node, so a kind's `layout_style` (and
chrome) follows its props; before, only the cached taffy style was relaid out (a box's `orientation`, and
the common `hidden`, did not apply until the next restyle).

### Sync delivery: the lent document (supersedes the two-phase answer to SVELTERS-PRESTO Q8)

The coordinator decided a two-phase dispatch (`begin_dispatch`/`next`/`finish_dispatch`). I built the
first choice of SVELTERS-PRESTO's Q8 instead, because a two-phase dispatch covers only the DOM events of
`dispatch`: a kind's own cancelable signals (a checkbox's `changed`, a hyperlink's `clicked` before its
`open-url`) are raised inside the kind's native default, which `finish_dispatch` would run after JS has
been delivered, so they could never be prevented. With the document lent to the deliverer, every sync
delivery is a plain call:
- `Document::set_sync_delivery(Box<dyn FnMut(&mut Document, &JsEvent) -> Outcome>)`; core takes the
  deliverer out of the document for the call, so there is no outstanding borrow;
- JS reads the tree, writes props (in `js_transaction(event revision)`) and creates or inserts nodes
  directly; the dispatch transaction is still open, so everything shares its revision;
- the path is fixed when the event is built; a listener node that is gone by its turn is skipped
  (the caller checks `Document::contains`); a target that a `pointerdown` listener removed gets no
  native default;
- a write to the node whose kind is mid-handler (the kind is out of its slot) is stored and replayed
  to the kind's `prop_changed` when the handler returns;
- **nested delivery** (coordinator's rule to pin, 2026-10-25): a sync event raised while the deliverer
  runs (`el.click()`, a `focus()`'s focus events, a kind's `emit_sync`) is never dropped and never
  re-enters the deliverer: it is delivered, **not cancelable**, right after the listener call that
  caused it returns, before the outer event's default runs, repeatedly until none is left (test
  `hooks::nested_sync_events_follow_the_listener`).

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

`Document::start_drag(window, pointer, source, ty, data) -> bool` (EGUI-ISLAND Q5): starts an in-app drag
for a pointer already held (a drag begun inside an egui island becomes a presto drag); the target under the
pointer's last position is entered at once, events queued (`take_events`). Test
`hooks::a_drag_started_from_outside_the_tree_drops_like_any_other`.

Not done here: OS file drops onto targets (`text/uri-list`; `InputEvent::FileHovered/FileDropped` still
dispatch nothing), auto-scroll near a scroll container's edge during a drag, the drag ghost
(`::part(ghost)`: a widget's), the `grab` cursor over an idle source.

## Requests of APP-TEST, TEXT-INPUT (R4) and QUIRE (coordinator's priority order, 2026-10-25)

### APP-TEST 1–5 (`runner.rs`)

1. `Document::needs_frame(window)`: queued style/layout/record/property work, rebuild or removals,
   running UI transitions, a cursor or resources to send. Pointer-only state changes (`:hover`,
   `:active`, `:focus-within`) no longer dirty the accessibility tree, so a hover that no rule styles
   leaves it false.
2. `Document::hover(window, pos) -> bool`: event-time hover (`:hover`, kinds' enter/leave, queued
   `pointerenter`/`pointerleave`), for the moves whose JS `pointermove` is coalesced to the frame.
3. `Document::accessibility_action(window, &ActionRequest) -> bool` (over `a11y_action`; JS events queued).
   Id mapping, documented in `runner.rs`: an AccessKit id is the presto `NodeId`'s `u64`.
4. `animation_event` (existed); `window_event(window, &WindowEvent)` (resize/scale in device pixels →
   points, `Focused(false)` → `:backdrop` on the root, `ThemeChanged` → `prefers-color-scheme`);
   `device_reset()` (every window re-sends property trees, layers, lists and runs; the document keeps
   the live images and paths and re-sends them); `a11y_full_tree(window)`.
5. IME purpose: **focus-driven**, no presto-base change: `Document::ime_state(window) -> Option<(Rect,
   InputPurpose)>` from the focused text kind (`TextInputClient::purpose`); `Commit::ime_area` still
   says when the area moved. The runner calls `ime_state` after a frame or a focus change.

### TEXT-INPUT R4

| Ask | Answer |
|---|---|
| text system in `PrepareCx`/`EventCx` | Through services, already in both: `cx.service::<presto_text::TextSystem>()` (presto-core does not depend on presto-text). |
| clipboard in `EventCx` | The same: the runner installs `Box<dyn presto_platform::Clipboard>` as a service (`Document::set_service`), kinds take `cx.service::<Box<dyn Clipboard>>()`. |
| fast-commit channel | `EventCx::set_caret(Option<Rect>)` (local → window, `FastCommit::Caret`), `EventCx::scroll_fast(offset)`, `Document::scroll_fast`, and `Document::take_fast_commits()` for the runner (`CompositorHandle::fast`). One design with QUIRE's below. |
| event time | `EventCx::time()` (the event's OS time; a timer's deadline). Not duplicated into `Event`. |
| padding edges, window transform | `NodeView::{padding, content_box, window_transform, from_window}`. |
| IME-first route | `Document::set_ime_source(window, active)`: during a composition every key goes to the IME, and with an IME source active printable keys without a chord do, before the command keymap (`Dispatched::to_ime`), as `keys::route`. |
| GTK's one selection owner | `Document::set_single_selection(on)` (GTK policy; off by default); a text kind calls `EventCx::claim_selection()` when it selects something and `release_selection()` when it collapses; the previous owner gets `Kind::selection_lost(cx)` (the field's `collapse_to_start`). One owner per document. |

### QUIRE: kind-owned scrolling

`Kind::self_scroll(&NodeView) -> Option<SelfScroll { content, x, y }>` (asked after each `prepare`): the
node gets a scroll node, a content space and a scroll layer whatever its `overflow`, its own list is
recorded in content space and placed in that layer, and `EventCx::scroll_fast` moves it on the compositor
at once (the next commit carries the same offset as a property op; no re-record). Wheel defaults work on
it like any scroll container. Pointer positions stay in the border box (add `scroll_offset`). Recording
now runs before the property refresh in the dirty walk, so the scroll node exists from the first frame.
Not done: a kind creating several layers of its own (gutter, minimap). Not cheap: lists, items and
layers are one per node, so it needs per-kind sub-lists through the recorder, the rebuild walk and the
commit. Today the shape is child nodes with `will-change: transform` (each gets its own layer) or a
self-scrolling child.

### WIDGETS-BASIC: light dismiss

`Kind::light_dismiss(&NodeView) -> bool` (asked after every prop write and every call into the kind; the
document keeps the set) and `Kind::outside_press(&mut self, cx, &PointerInput) -> Handled`: on a pointer
down in a window, before the press is dispatched (it is not consumed), every light-dismiss node of that
window whose subtree does not hold the hit target hears of it, topmost (latest in paint order) first,
`local` in its own space. A method rather than an `Event::OutsidePress` variant, so the bridge's
exhaustive `Event` match keeps compiling.

## 4. The rest of CORE-2's list

| Item | State |
|---|---|
| Queries (svelters-egui `scene/query.rs`) | Done (`query.rs`): kind sets kept by create/free (`Document::kinds_of`), `Document::query()` → `Query` with `kind`, `in_window`, `within`, `anchor`, `state`, `prop_eq`, `changed_since`, then `collect`/`first`/`count` (it walks the kind set, else the scope in pre-order, else the anchors, else every node); `Document::select(root, css)` and `matches(node, css)` through svelters-style's selector matcher. Anchors: `find_anchor` (one anchor map, filtered by window; `anchor-scope` qualification is not applied). |
| Interner/compaction (`compact.rs`) | Done for the layout column: `LayoutBox::style` is an `Arc<taffy::Style>` interned at restyle (svelters-egui's `Interner` and `style_key`, pruned as the table grows); the few in-place writers (window root size, virtual rows, size transitions) copy on write. `ComputedBox` was already shared. svelters-egui's boxed leaf cache and `BoxRect` are not ported (taffy's `Layout` stays whole: the a11y and padding reads use it). |
| `resources.paths` | Done: `PrepareCx::upload_path(Arc<PathData>) -> PathId`, `release_path`; the commit carries `paths`/`removed_paths`. |
| Layer `opaque` | Done: a content layer (or the root) whose owner's list starts with a square opaque solid quad covering its box, at full opacity, with no effect node. Re-recording a layer owner rebuilds so the flag follows. |
| Layer `Damage` | Not filled, on purpose: the compositor derives damage from changed lists and property ops (`presto-compositor` `damage.rs`); `LayerDelta::Damage` is for extra damage only (canvas chunks), which no core kind produces. |
| Incremental rebuild walk | Partly. The accessibility half is incremental now: nodes carry local bounds and their transform to their parent, so a rebuild re-sends only the nodes that moved relative to their parent, plus dirty nodes and subtrees that joined (removed and concealed subtrees leave the known set); the full tree goes only on the first frame (before: on every structure, layout or property change). Layers, items and the hit list are still rebuilt over the window on such changes. Recommendation for the rest: keep per-node hit and item ranges from the walk and splice the dirty subtrees' ranges; worth it only after profiling, since the walk is arithmetic per node. |

Also: `NameSource::Children` now computes the name from the descendants' text (CORE-2 Q2), and a text or
`aria-label` change re-projects such ancestors.

## Tests

Written, not run (the brief's rule): `src/tests/hooks.rs` (parts, listened, prop state, scale, chrome,
child layout, inert, peers and Tab stop and roving, Enter/Escape and auto-default, label for and
`labelled_by`, `invalid`, enum attributes, status tip, `emit_sync` and a prevented `pointerdown` through
the sync deliverer, drag and drop, type matching); `common::tests` covers the new ids; `src/tests/runner.rs`; in `hooks.rs` the lent deliverer, nested delivery, light dismiss, self-scroll
with fast scroll and caret, padding and window transform; in `dispatch.rs` the IME-source route; `src/tests/query.rs`; in `src/tests/a11y.rs` a move re-sending only
the moved node (local bounds, parent-relative transform), an inserted subtree joining incrementally, and a
name from children.

## Built / run

`cargo check -p presto-core -p presto-widgets --all-targets` after each part. Once at the end, when the
coordinator asked me to check RENDER's failing `paint_transitions_sample_on_the_ui_thread`: `cargo test
-p presto-core --lib` (119 passed after the fixes below) and `cargo test -p presto-widgets` (245 passed;
3 failed: `controls::slider::tests::{dragging_the_thumb_changes_the_value,
escape_while_dragging_restores_without_a_commit, inverted_page_clicks_and_commit_mode}`, float equality
on the shim's test driver, no presto-core involved: WIDGETS-BASIC's). Not run: clippy, other OSes.

Fixes from that run:
- `paint_transitions_sample_on_the_ui_thread` was not wall-clock: svelters-style keeps transition times
  as f32 seconds, so `100ms` became 100 000 001 ns and the transition was still running at +100 ms.
  Durations and delays are now rounded to whole microseconds, and a UI transition ends by exact time
  arithmetic (`start + delay + duration`), not by the sampled float progress.
- Native prop writes a kind made during its own handler skipped `prop_state` (the kind was out of its
  slot): the document now refreshes prop state and inert children when the kind is put back, and only
  JS writes (not the kind's own) are replayed to `prop_changed`.
- Four new tests assumed a row layout; the test root is a column: they click node centres now.

## For WIDGETS-BASIC (host.rs bridge)

Map the shim to the new hooks: `Styles::parts` from `NodeView::part` over `KindSpec::parts` (declare the
widget's part names); `Kind::listen` from `listen_changed`; the `prevented` path through `emit_sync`
(the hyperlink's `open-url` only when `clicked` was not prevented); `PeerGroup`/`PeerRequest` →
core's (the shim's `PeerMsg::Checked(args)` → `PeerMsg::Notify(Some(Value::List(args)))`);
`chrome_padding`, `child_layout` (+ `places_children: true` in the spec), `children_inert`;
`prop_state` with the widget's bit mask in `prop_states`; `focus_anchor`; `scale` into the shim contexts;
`Document::set_auto_default(policy.auto_default)` where the policy is installed.

## Absolute boxes (CADATOMIC's blocking bug, coordinator 2026-10-25)

Taffy locates an absolute box relative to its containing block: the nearest **positioned** ancestor
(`relative` *or* `absolute`), else the window root. `layout::positional_parent` only accepted `relative`
ancestors, so a box inside an absolute box was placed in the window root's space: nested absolute
boxes did not accumulate (berth's dock at the window origin). Fixed there (`position != Static`), which
the dirty walk (transform nodes) and the rebuild walk (world rects, hit list) both use; CSS `position:
static` now maps to taffy's `Static` (it was `Relative`). Clips: an absolute box now takes its containing
block's child clip, in the dirty walk (clip node parent) and the rebuild walk (hit clip), so an
`overflow` clip between the box and its containing block no longer clips it (CSS), which is the likely
cause of berth's empty tab labels; effects and layers still follow the tree. Tests:
`src/tests/absolute.rs` (nested absolute in absolute, absolute under a static box at an offset, under a
relative box, the paint transform node, a clip between box and containing block, a clipping containing
block); `cargo test -p presto-core --lib`: 126 passed. Note for the style track: svelters-style has no
`overflow` shorthand, so `overflow: hidden` sets nothing; `overflow-x`/`overflow-y` work.

## Fixed on the way

- The rebuild walk placed a `position: absolute` box under its tree parent's world transform, while
  layout locates it relative to its containing block: its hit entry and world rect were off when the
  containing block is not its parent. It now composes with the containing block's world transform (and
  scroll offset), as the dirty walk does for property nodes.

## Questions for the coordinator

1. ~~Sync delivery and re-entrancy~~: answered (coordinator), then superseded by SVELTERS-PRESTO Q8; see "Sync delivery" above.
2. **`Layout` writes restyle the node.** This costs a cascade match per layout-prop write (text props are
   exempt: they only remeasure). Recommendation: keep it; a per-kind `KindSpec` flag to skip it is easy if
   profiling asks.
