# svelters-egui: the retained scene graph on top of egui

Status: research note for the M7 proposal (2026-10-21). Scope: the node tree svelters-egui must own, how it renders through egui each frame, layout, events, reactivity and frames, accessibility and testing, virtualization and windows. Toolkit versions: egui/eframe/egui_kittest **0.37.0** (released 2026-10-15), taffy **0.15.0**, accesskit 0.25. The probe is in `docs/notes/scripts/egui-scenegraph-probe/` (§10); every number below comes from it unless a link is given.

## Summary (opinionated)

1. **The backend owns a scene: a generational arena of typed nodes.** A handle is `gen << 24 | idx` (below 2^53, 0 = null). A slot whose generation hits 2^29 is retired, never recycled, so the ABI's "handles are never reused" rule holds. Stale handles fail with the usual "stale or foreign handle" error. Props are typed per kind (generated structs), not a dynamic map: egui widgets bind `&mut String`, `&mut bool` or `&mut f64` straight into node fields.
2. **Every frame walks the scene, but only its visible part.** egui has no retained widgets, so "skip unchanged subtrees" really means "skip invisible subtrees". The scene knows each node's rect from last frame or from its own layout, so whole off-screen subtrees cost an `allocate_space` or nothing. Measured headless: a full walk costs about 5 µs per node (1.1k nodes 4.1–4.3 ms, 11k 50–56 ms, 55k 287–289 ms per frame). The culled walk costs 1.8–3.5 ms plus about 1 ms of tessellation for one 1280×800 screen, whatever the total node count (§10). Don't cache shapes per subtree, except for the X8 canvas display list.
3. **Layout: taffy, run over the scene and measured with egui's fonts, recomputed only when dirty.** egui's own layouts become escape-hatch container kinds (`scroll-area`, `panel`, `window`, `collapsing`, plus a "native island" for custom egui code). Taffy's style names are CSS names, so egui's ST1 style table can be setter-only and still read like CSS. Measured: cold layout 25–30 ms for 1.1k nodes, about 160 ms for 11k and 0.8 s for 55k. One changed label costs 1.5–2.3 ms, 8–9 ms and 43–53 ms respectively (10 measure calls; the time goes into re-running the flex algorithm over the dirty row's 100/1000/5000 siblings). A clean check costs about 1 µs. Big lists belong in `virtuallist` anyway.
4. **Identity: an `egui::Id` per node, derived from the handle by default.** `Id::unique(("svl", handle))` survives keyed reorders and reparenting, measured (§10). Optionally it comes from a compiler-provided stable identity instead, which also survives hot reload. The position-based auto ids that egui hands out inside our scopes do not survive a reorder (measured), so every stateful widget gets an explicit id. A `Ui` scope must never share an id with the widget inside it: when they did, the probe's text edit stopped taking clicks.
5. **Widget state:** anything JS can read or write lives in the node, which is the source of truth: text, checked, value, open, selected, and the scroll offset when bound. Ephemeral interaction state stays in egui memory under the node's Id: cursor and selection, IME preedit, scroll velocity, animations. Clear that memory when the node is destroyed.
6. **Events are queued during the pass and JS runs after it.** `host.post_event` and `host.prop_changed` only enqueue, so they are safe to call mid-pass. `checkpoint` runs once the pass has finished: in eframe's `App::logic` at the start of the next frame, or optionally at the end of `ui()` followed by `request_discard` for same-frame results. Sync handlers (`call_sync`) run only at two safe points:
   - *before the pass* (eframe `raw_input_hook`): keys to the focused node, pointer presses to the node hit-tested from last frame's rects, window close. A prevented event is removed from `RawInput`.
   - *after the pass*: virtual-list `setup` and `bind`.

   A sync signal raised mid-pass is delivered async, which is the existing `SVL_SYNC_BUSY` semantics.
7. **Reactivity gets better than on the retained toolkits.** Every native change to a node passes through our own code, so every widget-mutable prop is an `exact` observation with no echo: `set_prop` never notifies. This corrects the assumption in REACTIVITY.md §9.4 ("egui: every prop `set_only`, bindings by polling the frame").
8. **AccessKit comes through egui, keyed by our Ids.** egui builds AccessKit nodes from widget info. We add role, name and description from node props with `Response::accessible_name`, `UiBuilder::accessibility_*` and `ctx.accesskit_node_builder`. AccessKit node ids *are* egui Ids, so stable node ids give agents stable a11y ids. egui_kittest and egui_mcp then work against a Svelte UI unchanged; the probe ran a Svelte-style counter end to end under `Harness`. egui 0.37's kittest rejects an input widget without an accessible name, so the backend must forward labels.
9. **Runner: eframe first, with the scene runner-agnostic.** `Scene::{before_pass, ui, after_pass}` serves three runners: eframe (`svl_backend.run`), kittest (tests), and "embed into a caller's `Ui`" (the old SVR14 embedding contract, and the egui-workbench adapter later). A custom winit runner is the fallback if timer ticks that change nothing turn out to cost too much: eframe always runs `ui()` after `logic()`.

## 1. Node model

### 1.1 What the ABI asks for, and how the other backends do it

- Handles are minted by the backend, are monotonic and never reused, stay below 2^53, and 0 is null. A stale handle is an error, never a crash. `destroy` is idempotent. `on_destroyed` fires at most once and cascades (backend-abi.md "ABI v2", "As implemented (M2a)").
- GTK keeps `HashMap<u64, Entry>` with a monotonic `next` counter. Each entry holds a strong ref, the lifetime class, signal handler ids, the `(parent, slot)` it is inserted under, and observers (`crates/svelters-gtk/src/backend.rs:41-70`). The toolkit owns the tree and GTK holds the children.
- In svelters-egui **the scene is the tree.** JS ops (`insert`, `set_prop`, ...) never touch egui at all, which is the main structural difference from every other backend. It means:
  - (a) host ops are cheap: a field write plus a dirty bit, and no toolkit call;
  - (b) the only borrow that matters is our own scene's `RefCell`, not egui's `Ui`;
  - (c) `get_prop` reads our fields, never the toolkit.

### 1.2 Data structures (sketch)

```rust
pub type Handle = u64;                       // gen << 24 | idx; 0 = null; < 2^53

pub struct Scene {
    nodes: Arena<Node>,                      // Vec<Slot{gen: u32, node: Option<Node>}> + free list
    roots: Vec<Handle>,                      // toplevels (windows/viewports), in z/creation order
    id_to_handle: HashMap<egui::Id, Handle>, // rebuilt per frame for focus/hover/a11y reverse lookup
    dirty: DirtySet,                         // layout roots to recompute, a11y/style invalidations
    out: Outbox,                             // events / prop_changed / deaths, drained after the pass
    style: StyleState,                       // X6 sheets/classes/tokens → resolved egui::Style per class set
    host: HostRef,                           // post_event / prop_changed / checkpoint / call_sync / frame
}

pub struct Node {
    kind: KindId,                            // index into the generated KINDS table
    life: u32,                               // SVL_LIFE_OWNED | TOPLEVEL | BORROWED
    parent: Option<Handle>,
    slot: SlotId,                            // which named child slot of the parent (0 = default)
    children: SmallVec<[Handle; 4]>,         // ordered; `before` inserts are O(n), fine for UI fan-out
    child_attrs: SmallVec<[(AttrId, Value); 2]>, // parent:<attr> (grid row/column, panel side, flex-grow…)
    id: egui::Id,                            // Id::unique(("svl", handle)) or from `identity` (§1.4)
    flags: NodeFlags,                        // VISIBLE, ENABLED, LAYOUT_DIRTY, SUBTREE_DIRTY, STASHED, …
    listen: u64,                             // bit per signal id of this kind (+ capture bits)
    observe: u64,                            // bit per observable prop of this kind (refcount elsewhere)
    common: Common,                          // tooltip, a11y name/description/role, classes, opacity, cursor
    layout: LayoutData,                      // taffy::Style + taffy::Cache + unrounded/final Layout
    rect: egui::Rect,                        // screen rect last frame (hit test, culling, events)
    w: Widget,                               // per-kind typed state, generated
}

pub enum Widget {                            // generated from metadata/egui (curated, not reflected)
    Box,                                     // taffy container (flex/grid/block)
    Label { text: String, wrap: WrapMode, galley: Option<Arc<Galley>> },
    Button { text: String, selected: bool, small: bool },
    Checkbox { text: String, checked: bool },
    TextEdit { text: String, multiline: bool, hint: String, password: bool },
    Slider { value: f64, min: f64, max: f64, step: f64 },
    ScrollArea { offset: Vec2, bound_offset: bool, direction: Vec2b },
    Collapsing { title: String, open: bool },
    Canvas { ops: Arc<[egui::Shape]> },      // X8 display list, converted once on set_prop
    VirtualList(VListState),                 // M5 protocol (§8)
    Window(WindowState), Panel(PanelState), Popup(PopupState), Native(NativeIsland), …
}
```

- **Arena or HashMap.** The probe's arena (`src/tree.rs`) gives O(1) lookup with no hashing, and it lets the handle bits double as taffy's `NodeId` (Bevy does the same with `Entity::to_bits()`, see §7). A `HashMap` with a monotonic counter, as in the GTK backend, is equally correct. The arena is preferred because the frame walk does one lookup per visited node per frame.
- **Typed props, not a dynamic map.** Prop ids are resolved per kind, and generated code maps `(kind, prop) → field`, the same pattern as the other generated tables. A `HashMap<PropId, Value>` would force a conversion every frame before egui could borrow the value. The escape hatch for props without a field is a small `extras: Vec<(PropId, Value)>`.
- **Lifetime classes.**
  - `TOPLEVEL` is a window or viewport node.
  - `BORROWED` handles are rare. egui has no native objects to borrow, but "the scroll-area's inner content node" or "the window's title-bar slot" could be exposed as borrowed sub-nodes later.
  - `SHARED`/`VALUE` covers things like a loaded `TextureHandle` (images), which the scene refcounts.
- **Destroy.** Detaching (`remove`) and freeing (`destroy`) are separate operations, as in Blitz. `destroy` detaches the node from its parent, frees its slot (bumping the generation) and clears its egui memory, e.g. `TextEditState` and `CollapsingState` under `node.id`. A toplevel closed natively reports `SVL_GONE_CLOSED`, then `SVL_GONE_PARENT` for every descendant, as GTK and Qt do.
- **Children and slots.** `insert(parent, child, before, slot)` works as on GTK: kinds with named slots (window `menubar`/`content`, panel sides, collapsing `header`) record the slot on the child. Child attributes (`parent:row`, `parent:flex-grow`, `parent:side`) live on the child and are read by the parent's layout.

### 1.3 Per-node widget state: node or egui memory?

| State | Owner | How it reaches egui | Change detection |
|---|---|---|---|
| text, checked, value, selected index | node | `&mut` field passed to the widget (`TextEdit::singleline(&mut text)`, `checkbox(&mut checked)`) | `Response::changed()` → `prop_changed` if observed |
| collapsing `open` | node | `CollapsingHeader::open(Some(open))` forced each frame; toggle read from header response | header clicked → flip + `prop_changed` |
| scroll offset (when JS binds/reads it) | node | `ScrollArea::scroll_offset(v)` only when JS set it; read back from `ScrollAreaOutput::state.offset` | offset differs from last frame → `prop_changed` |
| text cursor/selection, IME preedit | egui memory under `node.id` | egui's `TextEditState` (load/store by Id) | exposed only through explicit methods (`select(range)`), not as a reactive prop |
| scroll velocity, animations, drag-in-progress, hover fade | egui memory | automatic | never exposed |
| window position/size | egui memory (`Area` state) plus node for bound props | `Window::current_pos` / `default_size` | `Window` response rect changed → `prop_changed` |

Rule: JS-visible state lives in the node, ephemeral interaction state lives in egui, and a destroyed node removes its egui entries.

### 1.4 Stable `egui::Id`s

egui 0.37 deprecates `Id::new` in favour of `Id::unique(src)` (globally unique) or an `IdSalt` (local), and `ui.id()` in favour of `ui.scope_id()`. `UiBuilder::scope_id(id)` gives a child `Ui` a scope that is "independent of the parent `Ui`", so its widgets "can be moved in the ui tree without losing state", as long as the id is globally unique. AccessKit `NodeId` = `Id::value()`.

Three sources were measured in the probe (`checks`: 20 keyed text edits; click #7, type, reverse the keyed order with remove+insert as the LIS diff does, type again; then destroy every node and rebuild with fresh handles and the same identities, as the in-process hot reload does, and type again):

| Id source | keyed reorder keeps focus + typing target | hot reload (new handles) keeps it |
|---|---|---|
| `Id::unique(("svl", identity))` (compiler-provided stable identity) | yes | yes |
| `Id::unique(("svl", handle))` | yes | **no** (new handles, so new ids: focus and cursor are lost) |
| egui positional auto ids (no explicit ids) | **no**: typing lands in whichever field now sits at position 7 | no |

Recommendation:

- **v1: handle-derived ids for every node.**
  - Correct under reorders and reparenting, and unique by construction.
  - State is lost on hot reload, which is what GTK and Qt do today: M2b's reload recreates every native widget.
- **v2: an optional `identity` prop** (one reserved, universal prop id; no ABI change). The compiler would emit a stable value per element: a hash of the component file, template position, each-block key path and component instance path.
  - The backend uses it for the egui Id when present, which preserves focus, cursor, scroll and collapsed state across hot reload.
  - It also gives agents and a11y stable ids.
  - Uniqueness must be enforced: if two live nodes claim one identity, the later one falls back to the handle id and a dev warning is printed. Otherwise egui reports an "Id clash" and interaction breaks.
  - This is the `NodeIdentity` of `plans/archive/architecture/semantic-graph.md` and would benefit every backend's devtools.
- **Every node has an explicit id, and a container's `Ui` gets `scope_id(node.id)`.** Every stateful leaf gets an explicit widget id: `TextEdit::id`, `ScrollArea::id_salt` under the node's scope, `CollapsingHeader::id_salt`. Leaves like `Button` have no id setter. Their auto id comes from the enclosing scope plus a counter, so it changes when siblings reorder. That only matters mid-press and for a11y id stability. Wrap such leaves in a per-leaf scope (cost measured in §10) or accept it. **Never give the scope and the widget the same `Id`.** The probe originally used `scope_id(id)` for a leaf scope and `TextEdit::id(id)` inside it; the text edit then never took focus. Use `scope_id(id.with("scope"))`, or the reverse.

## 2. Rendering pass

### 2.1 Frame lifecycle

```text
winit event / ctx.request_repaint (wake, schedule_wake, request_frame, egui animation)
  │
  ▼  eframe calls App::raw_input_hook(ctx, &mut RawInput)          ← BEFORE the pass, no Ui borrowed
  │    scene.before_pass(raw):
  │      • wake flag / due timer?            → host.checkpoint()      (JS runs, mutates the scene)
  │      • frame requested?                  → host.frame()           (rAF callbacks)
  │      • sync input dispatch: for key/pointer-press/close events with sync listeners,
  │        target = focused node (egui focus Id → handle) or hit-test on last frame's rects;
  │        host.call_sync(...) → if prevented: remove the event from raw.events
  │      • virtual lists: predict next visible range from last scroll offset + wheel delta;
  │        call_sync setup/bind for rows that will appear (plus overscan)
  │      • layout: if dirty roots or window size changed → taffy over the scene (measure via fonts)
  ▼  eframe calls App::logic(ctx) (optional; same place for non-input work)
  ▼  eframe calls App::ui(ui)  ── egui pass 1 (maybe pass 2 if request_discard) ──────────────────
  │    scene.ui(ui): walk roots → for each visible node emit egui calls at its rect / in its layout
  │      responses → out.events (post_event is queue-only, may be called directly)
  │      &mut field changed → node updated in place → out.prop_changed (queue-only)
  │      record node.rect, id_to_handle; a11y annotations
  │    measured size ≠ layout size? mark dirty; request_discard (bounded)
  ▼  pass ends; eframe tessellates and paints
  │    scene.after_pass(): flush outbox (post_event/prop_changed if not sent directly);
  │      virtual-list rows that became visible but were not bound → call_sync bind now (safe);
  │      anything queued → host.checkpoint() at the start of the next frame (or right now, see 4.2)
  │      + ctx.request_repaint()
  ▼  idle: no repaint requested → winit ControlFlow::Wait → 0% CPU
```

The multipass rule (from egui's `run_dyn`): input events are *moved* into the first pass (`new_input.take()`), and a discarded pass's widget output is thrown away but its input effects are not. So the outbox must accumulate across all passes of a frame and flush once afterwards. Pass 2 sees no clicks, so nothing is duplicated. The emit walk must be idempotent: it must not double-count anything that pass 1 already counted.

### 2.2 Avoiding re-walking unchanged subtrees

egui keeps no shapes between frames. Every frame re-emits the whole visible UI and re-tessellates it, so "skip unchanged" is not something egui supports. The options:

| Option | Verdict |
|---|---|
| Always walk everything | Fine up to a few thousand nodes; linear in total nodes, off-screen ones included (measured below). |
| **Cull by retained rects** (walk only nodes whose last rect, or layout rect, intersects the clip rect; `allocate_space(last_size)` for skipped children in egui-layout containers) | **Do this.** Cost becomes proportional to what is on screen. Possible only because we are retained: immediate-mode code doesn't know an off-screen child's size without running it. |
| Cache `Shape`s per subtree and `painter.extend` them | Only for non-interactive content (the X8 canvas display list, static decorations). Interactive widgets must run each frame to get `Response`s. |
| Cache galleys per node (`Arc<Galley>` in the node, keyed on text + font + wrap width + font generation) | Small win over egui's own galley cache, which hashes the `LayoutJob` every frame. Worth it for long labels. Must be invalidated on font or `pixels_per_point` change. |
| egui `Area`s/layers per subtree | Not a caching mechanism; only for z-order (§8). |

First-frame jitter: egui's single-pass layouts (`Grid`, `horizontal_centered`, right-to-left) size from the previous frame and call `request_discard` on first show (`Options::max_passes` defaults to 2). With taffy, the scene knows sizes before the pass, so the first frame is correct without a discard. A discard happens only when a widget's real size differs from the measured one: the fallback, bounded by `max_passes`.

## 3. Layout

### 3.1 The choice

| | egui-native container kinds (`vertical`, `horizontal`, `Grid`, `ScrollArea`) | taffy over the scene |
|---|---|---|
| Semantics | single pass, items sized as emitted, no shrink, limited justify/align, "expand" through `ui.available_*` | full flexbox + grid + block: grow/shrink/basis, wrap, gap, align/justify, min/max, percentages |
| First frame | right except Grid/centered layouts (discard) | right in one pass when leaves measure correctly |
| Text wrap | egui wraps at available width | measure function with `available_space.width` gives height-for-width |
| Cost | none extra | layout only when dirty (measured below); cached per node by taffy |
| Mapping from Svelte style | ad hoc props | ST1 setter table with CSS names (`flex-direction`, `gap`, `padding`, `align-items`, `width`, `min-height`, …) |
| Look & feel | identical to hand-written egui | identical widgets, different placement rules |

**Recommend taffy as the layout of the generic container kind (`box`), with egui-native containers as separate kinds.** The X6/ST1 rules fit:

- ST1 decided that layout properties are setters with native names, and that setter-only toolkits (WinUI, UIKit) are expected.
- For egui the "native" layout API is taffy, whose names are CSS names. So `metadata/style/egui.yaml` would read `gap: { via: setter, prop: gap, type: length }`, `flex-direction: { via: setter, ... }` and so on, and `<style>` setter rules work with type, `.class` and `#id` selectors exactly as on WinUI.
- Visual CSS properties (`color`, `background-color`, `font-*`, `border-*`, `padding`, `opacity`) are setters too. They resolve into a per-node `egui::Style` override (`UiBuilder::style`) or into `Frame` fill and stroke, and `--sv-*` tokens map to `Visuals`.
- egui's `UiBuilder::with_class` (0.35) and the experimental `StyleProvider` are worth watching, but setter resolution in the scene is enough.

### 3.2 How the two passes work

1. **Before the pass**, if the scene has dirty layout roots or the viewport size changed, run taffy. Leaves are measured with egui's fonts: `ctx.fonts_mut(|f| f.layout(text, font, color, wrap_width).size())` plus widget chrome from `ctx.global_style().spacing` (`button_padding`, `interact_size`, `icon_width + icon_spacing`). The probe's measure function (`scene.rs::layout_taffy`) does exactly this.
2. **During the pass**, paint each visible node at its rect. A leaf goes through `ui.new_child(UiBuilder::new().max_rect(rect).scope_id(..).layout(..))`, and a container's background and border through `Frame`/`painter`. Scroll containers wrap their subtree in `ScrollArea::show_viewport` with `ui.set_min_size(content_size)`.
3. **Native islands.** A custom egui widget, an egui-native container, or anything we can't measure from the outside gets its rect from taffy. It runs egui layout inside, and its measure result is last frame's response size (egui_taffy's approach): an unknown island is first drawn with `UiBuilder::sizing_pass().invisible()` plus `request_discard`.

Implementation note: the probe mirrors the scene into a `TaffyTree` for brevity. The production design should implement taffy's low-level traits (`TraversePartialTree`, `LayoutPartialTree`, `CacheTree`, `LayoutFlexboxContainer`, `LayoutGridContainer`, `RoundTree`) directly on the arena, storing `taffy::Cache` and the layout in the node. That is what Blitz does over its DOM. Bevy recently deleted its mirrored `TaffyTree` in favour of the same adapter, calling the mirror sync "a long time source of bugs" (bevy PR #25653, 2026-10-14). Dirty marking: `set_prop` on a measured prop (text, font) or a style prop marks the node, and the dirty flag propagates up to the root. Torin's "dirty reason + common ancestor root" idea is a later optimization.

Two cheap guards follow from the probe (§10.2):

- **Only mark a leaf dirty when its intrinsic size actually changes.** Re-measure the no-wrap size on `set_prop(text)` and compare. A counter label going from "count: 1" to "count: 2" usually keeps its width, so no relayout is needed.
- **The window root and scroll contents need definite widths.** An `auto` root shrinks to fit, so one row growing re-lays out all its siblings at the new width (measured: 100k re-measures instead of 10).

Relayout cost is proportional to the siblings along the dirty path. A flat 5,000-child column costs about 40 ms per change, which is one more reason long lists go through `virtuallist`.

## 4. Events

### 4.1 Responses to `post_event`

- In the walk, each node with listeners checks its `Response`. Signal names follow N1 (native names, kebab-case, here from `egui::Response`): `clicked`, `double-clicked`, `secondary-clicked`, `changed`, `drag-started`, `dragged`, `drag-stopped`, `gained-focus`, `lost-focus`, and `pointer-enter`/`pointer-leave` from hover transitions (the node keeps last frame's `hovered` bit). A node with listeners raises its `Sense`: listening to `dragged` adds `Sense::drag()`, as Qt turns on mouse tracking.
- `host.post_event` only enqueues (`crates/svelters-host/src/core.rs::push_event`); it never runs JS, so calling it mid-pass is safe. The same holds for `prop_changed` (`try_borrow_mut`, mark dirty, no JS). The rule "never run JS while a `Ui` is borrowed" is therefore enforced by *where we call `checkpoint` and `call_sync`*, not by buffering.
- `post_event` does not wake the backend. After the pass, if anything was posted, the backend calls `ctx.request_repaint()` and runs `checkpoint` at the next safe point.

### 4.2 When JS runs

- **Default: next frame, before the pass** (`raw_input_hook` or `logic`).
  - The click happens in frame N (egui paints the press visuals). JS reacts at the start of frame N+1 and N+1 shows the result.
  - With `request_repaint` the frame comes right after N's present, so the cost is about one vsync more than GTK, where the handler runs before the next frame clock tick.
  - Measured: in the probe, the click event exists after the frame with the release; the dispatched label change shows in the next frame.
- **Option: same frame.** At the end of `ui()` our scene borrow has ended, while egui's pass is still open. Run `checkpoint` there, then `ctx.request_discard("svelters: JS changed the scene")`: pass 2 runs in the same frame with the new scene and no input.
  - Cost: two walks on interaction frames.
  - Risk: JS that runs long blocks the frame (it would block GTK's main loop too).
  - Keep it behind a flag, and measure perceived latency before choosing it.

### 4.3 Sync handlers (`call_sync`)

ABI v2.1 requires a decision before the toolkit acts (`preventDefault`, `close-request` veto, typed results). In egui the toolkit "acts" inside the pass, when the widget reads `ctx.input()`. So a sync decision must be made **before the pass**, on raw input:

| Sync event | How |
|---|---|
| key down/up with `e.preventDefault()` | `raw_input_hook`: target = node whose `id` equals `ctx.memory(|m| m.focused())` (via `id_to_handle`), capture/bubble along scene parents (listen_ex CAPTURE). Prevented → remove the `Event::Key`/`Event::Text` from `raw.events`. |
| pointer press / context-menu with `preventDefault` | hit test the press position on last frame's rects (topmost layer first, egui's `ctx.layer_id_at(pos)` + our rects). Prevented → remove the `PointerButton` event. |
| window `close-request` | `raw.viewports[id].events` has the close event; call_sync on the window node; veto → `ViewportCommand::CancelClose` during the pass. Or keep ABI v2's posted `onRequestClose` with CancelClose always and `Close` sent when JS destroys the window. |
| virtual-list `setup`/`bind` | after the pass for rows egui showed but we had not bound (placeholders for one frame), and before the pass for predicted rows (§8). |
| anything raised mid-pass | delivered async: the `SVL_SYNC_BUSY` path, the toolkit default applies. |

`call_sync` can run JS that issues ops synchronously (`set_prop`, `create`, `insert`). Those borrow the scene, so no scene borrow may be held across a `call_sync`. In the walk the scene is borrowed throughout, which is one more reason sync calls never happen mid-pass. Debug builds should assert this with a "pass active" flag that turns any `call_sync`/`checkpoint` attempt into a panic.

### 4.4 Focus, keyboard, IME, DnD, hover, capture

- **Focus.** egui owns focus (`Memory::focused`, `request_focus(id)`, `surrender_focus`). A `focus()` method on a node calls `ctx.memory_mut(|m| m.request_focus(node.id))` and takes effect in the next pass. `gained-focus`/`lost-focus` come from `Response`. Tab order is egui's emit order: in taffy mode that is our tree order. Explicit `tabindex` would need egui's focus API (`Memory::move_focus` is limited), so leave it open.
- **Keyboard.** Shortcuts on actions (X4 menus) are checked in the pass with `ctx.input_mut(|i| i.consume_shortcut(..))` and post the action's `activate`.
- **IME.** egui's `TextEdit` handles `Event::Ime` and sets `PlatformOutput::ime`. We expose only the resulting `text` prop and never intercept IME events (HyperChad's note agrees: "native egui widgets remain the sole text/IME editors").
- **Drag and drop.**
  - In-app DnD uses egui's payload API (`Response::dnd_set_drag_payload`, `dnd_hover_payload`, `dnd_release_payload`) with the payload being a JS-side id (an integer, like virtual-list ids).
  - File drops come from `RawInput::hovered_files`/`dropped_files`, posted to the window node, or to the node under the pointer when it listens.
- **Hover.** Transitions come from `Response::hovered()` per frame. Pointer position and modifiers travel as event args.
- **Pointer capture.** egui keeps the dragged widget for the whole drag (`Sense::drag`, `ctx.dragged_id()`), even outside its rect. That is capture without extra work.

## 5. Reactivity and frames

- **`observe(h, prop)`.** The scene supports it for every prop a widget can change: `text`, `checked`, `value`, `selected`, `open`, `scroll-offset`, `rect`/size for layout reads. The class is `exact`, and props that never change natively are `constant`.
  - Change detection is free: `Response::changed()`, or a cheap compare of a field before and after the widget call.
  - `host.prop_changed(h, prop)` is called during the pass, and the host reads `get_prop` at the next checkpoint.
  - The `__svelters_changed` batch comes before events, as REACTIVITY §5.3 requires.
  - **No echo.** `set_prop` from the host never calls `prop_changed`, so the read-back/compare in R1 just sees equal values.
  - `SVL_M_PROPORTIONAL` (whole-text props) can be left off: reading a `String` field is cheap.
  - In the probe, typing "hi" produced exactly one `text` change and the field held "hi". egui reports `changed()` once per frame, not once per character.
- **`request_frame`.**
  - It sets `frame_pending` and calls `ctx.request_repaint()`.
  - `host.frame()` is called in `before_pass` of the next frame.
  - A `requestAnimationFrame` loop therefore runs at display rate, using eframe's vsync as the clock.
  - When the window is hidden or minimized, egui doesn't run `ui()`, but eframe still calls `logic()` (0.37 docs). Deliver frames there at a low rate, or not at all, matching GTK's watchdog of one frame per 250 ms.
- **`wake`** (thread-safe). Set an `AtomicBool` and call `ctx.request_repaint()`. `egui::Context` is `Send + Sync`, and eframe turns the request into a winit `EventLoopProxy` user event. The next `before_pass` sees the flag and calls `checkpoint`.
- **`schedule_wake(ms)`.** Store the deadline in a `Cell<Option<Instant>>` and call `ctx.request_repaint_after(ms)`; `before_pass` fires the checkpoint once the deadline is due. A new call replaces the deadline, and a negative value cancels it. egui coalesces repaint requests to the earliest one, so a cancelled deadline may still cause one empty frame. That is harmless.
- **Idle CPU is zero** when nothing is pending.
  - With no repaint requests, eframe sleeps in `ControlFlow::Wait`.
  - Exceptions come from egui itself: a focused `TextEdit` blinks its cursor, which requests a repaint about every 0.5 s, and hover and fade animations run briefly.
  - Each JS timer tick costs one full egui frame (walk + tessellate + present), even if the tick changes nothing, because eframe always calls `ui()` after `logic()`.
  - A custom runner on egui-winit could run checkpoint-only ticks and repaint only when the scene went dirty. Measure first: a 1 Hz clock at a few ms per frame is negligible.

## 6. Accessibility and testing

- **What egui gives.**
  - egui 0.37 always builds AccessKit (no feature flag since 0.34). It is enabled per context (`ctx.enable_accesskit()`, done by egui-winit when a screen reader asks and by kittest always).
  - Builtin widgets supply role, label, value and toggled state via `WidgetInfo`.
  - `Response::accessible_name`/`labelled_by`, `Ui::label_inputs_by`, `UiBuilder::accessibility_parent/label/role` and `ctx.accesskit_node_builder(id, |node| ..)` let us add anything else.
- **Our tree as the semantic source of truth.** Node props carry semantics: `role`, `aria-label`-style name, description, `labelled-by` relation by handle. The walk projects them onto egui's AccessKit node for the same Id.
  - Container nodes get `UiBuilder::accessibility_role` (list, group, toolbar) and `accessibility_parent` follows our parent links. The AccessKit tree then mirrors the scene, not egui's incidental `Ui` nesting.
  - Because the scene is retained, a semantic snapshot can also be produced **without egui**, for the TestHost-parity tests and the svelters inspector (`docs/notes/inspector-demo.md`).
  - We should not build AccessKit `TreeUpdate`s ourselves: egui would still emit its own nodes for its widgets, and two producers would fight over ids.
- **Cost.** Measured in the bench as AccessKit on vs off (§10).
- **Testing.**
  - `egui_kittest::Harness::new_ui_state(|ui, scene| scene.ui(ui), scene)` drives the real scene headless. `get_by_label("Increment").click()`, `harness.run()`, dispatch, `harness.run()`, then `query_by_label("count: 1")` all worked in the probe.
  - A backend test helper should wrap this as `Harness<Scene>` plus a fake `HostRef` that runs the real QuickJS host, so `.svelte` fixtures can be tested on egui headless. No Xvfb is needed, unlike GTK and Qt.
  - Snapshot tests: kittest's `snapshot` feature with wgpu (software rendering on CI; check that lavapipe/llvmpipe is available).
  - **0.37 fails a kittest run if an input widget lacks an accessible name** (seen in the probe: `1 widget(s) lack accessible name … TextInput`). The backend must forward a label prop (or a `parent:label`/`labelled-by` relation) to `Response::accessible_name`. The compiler could warn about inputs without one, which helps a11y on every backend.
  - egui_mcp/egui_inspection (`InspectionPlugin`, AccessKit-based) work unchanged on a svelters-egui app because the a11y tree is egui's. Stable node ids (§1.4) give agents stable locators.

## 7. Prior art

| Project | Link | Version (Oct 2026) | What transfers |
|---|---|---|---|
| Xilem + Masonry | https://github.com/linebender/xilem | 0.4.0 crates, `main` active | Retained widget tree under a reactive layer. A fixed list of flag-driven passes (mutate → update tree → focus → layout → compose → paint → accessibility); render and a11y passes are pure and may run more than once; measure is cached by `(axis, len_req, cross)`; a "stashed" state for hidden subtrees; `TestHarness` drives the app through user actions. Ids are not generational. |
| Blitz / dioxus-native | https://github.com/DioxusLabs/blitz | blitz-dom 0.2.4 / 0.3.0-beta.4 | Its `DocumentMutator` (create/insert/remove/set_attribute/flush) is nearly our ABI. `NodeId` carries the SlotMap version. Taffy's low-level traits are implemented on its own DOM; damage bits drive invalidation; detach and free are separate operations. |
| Freya / Torin | https://github.com/marc2332/freya | 0.4.3 / 0.5.0-rc | Its own layout engine (Torin) over a `TreeAdapter` trait; a dirty set with reasons; relayout from the closest common ancestor; event processing in a separate crate (`ragnarok`). |
| Iced | https://github.com/iced-rs/iced | 0.14 | Reactive rendering (redraw on invalidation), headless and E2E tests. Tree rebuilt by `view()`, so less relevant. |
| Slint | https://github.com/slint-ui/slint | 1.18.1 | A retained item tree with lazy, dependency-tracked property bindings; tests query by accessible label (`ElementHandle::find_by_accessible_label`). |
| Vizia / morphorm | https://github.com/vizia/vizia | 0.4.0 / morphorm 0.9 | Generational `Entity` ids with per-property side tables (ECS style); a one-pass layout is often enough. |
| egui_taffy | https://github.com/PPakalns/egui_taffy | 0.14.0, **egui 0.36**, taffy 0.9.2 | Taffy inside immediate-mode egui. State lives in egui temp memory and nodes are matched by position each frame (the whole tail is rebuilt after a mismatch). Leaves are "measured" as last frame's min/max response size, so new nodes take a sizing pass plus `request_discard`, and height-for-width text is weak. A retained scene avoids all three. |
| egui_flex | https://github.com/lucasmerlin/hello_egui | 0.9.0, egui 0.37 | Flex in one pass from last frame's sizes; no flex-shrink by design. Shows where single-pass layout stops working. |
| HyperChad egui renderer | https://github.com/MoosicBox/MoosicBox/tree/master/packages/hyperchad/renderer/egui | 0.5.0 | The closest analogue: a retained document, its own layout as "layout authority", a bounded FIFO of commands, generations that reject stale ids, and egui widgets as the sole text/IME editors. |
| egui-reactor | https://github.com/fand/egui-reactor | 0.1.1 | `rsx!` and hooks over egui with no retained tree; state is keyed by position, so moving a component resets it. A counter-example. |
| bevy_ui | https://github.com/bevyengine/bevy | 0.20 + PR #25653 | One taffy node per retained node. The mirrored `TaffyTree` was removed in favour of a trait adapter over ECS data (`NodeId = Entity::to_bits()`), with self/subtree/layout dirty flags. |
| bevy_egui | https://github.com/vladbat00/bevy_egui | 0.43.1 (egui 0.36) | UI systems must be re-runnable because of multipass. |
| Rerun (re_ui, egui_table, egui_tiles, kittest) | https://github.com/rerun-io/rerun, https://github.com/rerun-io/egui_crates | re_ui 0.38 | Layout statistics from frame N are used in N+1 to work around single-pass layout. `egui_table` virtualises millions of rows via `show_viewport`, with prefetch and variable row heights. Rerun also builds egui's kittest and inspection tooling. |
| egui-workbench (sibling, `/home/dev/projects/egui-workbench`) | local | egui 0.35 | Render reads `&App` and pushes intents into `VecDeque`s; mutation happens in `App::logic` under per-tick budgets; ids come from domain string keys; background threads wake the UI with `ctx.request_repaint()`. Same shape as our outbox/checkpoint. |
| egui_mcp / egui_inspection (sibling) | local | egui 0.37 | Reads only the AccessKit tree; addresses widgets by AccessKit id (= egui Id) or by role and label; injects `egui::Event`s via a plugin `input_hook`. Works on svelters-egui unchanged if ids are stable. |
| egui_tiles (sibling) | local | 0.17/0.18 | A retained `HashMap<TileId, Tile>` (monotonic ids, no generations); each tile gets `scope_id().with(tile_id)` and an explicit `accessibility_parent`. Its pitfall: a TileId can change kind between frames when rebuilt from hashes. |

What transfers, condensed:

- Generational ids (Blitz, Vizia), and taffy implemented directly on the retained tree (Blitz, Bevy).
- Flag-driven passes, with paint and a11y kept pure and repeatable (Masonry, bevy_egui).
- Mutation from a queue, applied outside rendering (egui-workbench, HyperChad).
- Accessibility labels as the test query surface (Slint, kittest).
- Measure from real text layout instead of last frame's size (the main improvement over egui_taffy and egui_flex).

No "dioxus-egui", "leptos-egui" or "svelte-egui" renderer exists.

## 8. Virtualization, overlays, windows, z-order

- **`virtuallist`** (M5 protocol, unchanged).
  - The row-id vector lives in the node, `splice` edits it, and `count`/`scrollTo`/`rowAt` are trivial.
  - It renders with `ScrollArea::show_viewport`: a uniform `rowHeight` gives the visible range by arithmetic, and variable heights use a prefix-sum of measured row heights (egui_table's approach). Rows are row-slot subtrees (`setRow(row, widget)`) laid out like any other subtree.
  - Slot pool = visible + overscan, assigned `slot = pos mod N` like Qt's pool.
  - `setup`/`bind` are `call_sync` before the pass for the predicted range. The prediction uses last offset + this frame's scroll delta from `RawInput` (wheel events) + overscan. A row that still appears unbound is drawn as a placeholder of the row height, bound after the pass, and the frame is repainted. `unbind`/`teardown` go async as on GTK.
  - Note that `show_rows` hands out ranges *during* the pass, so it must not be the place that calls JS.
- **Long non-virtual lists** (a keyed `{#each}` of 10k rows) are handled by generic culling (§2.2): the cost is visible rows plus an `allocate_space` per culled child, or nothing in taffy mode (measured).
- **Overlays.**
  - Popups, menus, tooltips and combo dropdowns map to egui's `Popup`/`Area`/`Tooltip` with `Order::Foreground`/`Tooltip`.
  - A popup node is a child of its anchor node in the scene. It is emitted when open, positioned from the anchor's rect, and its `Area` id is the node id.
  - Context menus use `Response::context_menu`.
  - Modals use `egui::Modal`.
- **Windows.**
  - In-canvas `egui::Window` is one kind (`window` with `embedded`), positioned by egui memory.
  - Real OS windows are viewports.
    - `show_viewport_immediate` takes a non-`'static` closure, so the scene can be borrowed. Use it in v1 for dialogs and secondary windows.
    - `show_viewport_deferred` needs `Fn + Send + Sync + 'static`, and the scene is `!Send` (QuickJS thread). It would need a main-thread-asserting wrapper; on native, eframe calls deferred viewports on the main thread.
  - Each toplevel node is a viewport with `ViewportId::from_hash_of(node.id)`. `close-request` follows §4.3, and natively closed viewports report `SVL_GONE_CLOSED`.
  - X7's "dialogs declared anywhere" maps naturally, since a toplevel's scene parent is irrelevant to where it renders.
- **Z-order.** Within a window it is emit order: the later sibling paints on top. That is our children order, and in taffy mode absolutely positioned nodes come last. Across layers it is egui `Order` plus `ctx.move_to_top(layer_id)` for windows that get focus. A `z-index` child attribute can sort siblings before emission.

## 9. ABI mapping (`svl_backend` members)

| Member | svelters-egui |
|---|---|
| `resolve` | generated tables (kinds, props, signals, methods, slots, attrs) from `metadata/egui/*` (curated: egui has no reflection; source = hand-written facts + rustdoc JSON for docs) |
| `create` / `construct` | allocate node (typed `Widget` default), handle = arena key; toplevel kinds get `SVL_LIFE_TOPLEVEL` |
| `insert` / `remove` | arena child-list edits + slot/attr checks; mark layout dirty on parent; `request_repaint` |
| `destroy` | detach, free slot (generation bump), clear egui memory under `node.id`, drop observers/listeners; toplevel → close viewport |
| `set_prop` / `get_prop` | generated field accessors; set marks measure/paint/a11y dirty as declared per prop; no egui call |
| `call` | per-kind methods: `focus`, `scrollTo`, `select`, `splice`, `setRow`, …; effects that need egui (focus, scroll) are queued for the next pass |
| `listen` / `listen_ex` | set bit; adjust `Sense`; capture flag for pre-pass key/pointer routing; `after` refused |
| `observe` | set bit (refcount); `exact` for widget-mutable props; `SVL_OBSERVE_UNSUPPORTED` never needed for real props |
| `handle_info` | from node (kind, lifetime) |
| `set_child_attr` / `get_child_attr` | stored on child; read by the parent's layout (grid row/col, flex-grow, panel side, z-index) |
| `style` | `SVL_STYLE_TARGET` → `NONE` (setter-only dialect); CLASSES/NAME used for setter-rule matching on the JS side; INLINE ignored (setters), OPACITY → `Ui::set_opacity`; SCHEME → `ctx.set_theme`; tokens → `Visuals` |
| `call_static` | rarely; e.g. `fonts.add(...)`, `image.load(...)` |
| `run` | build eframe `NativeOptions`, `run_native` with an `App` whose `raw_input_hook`/`logic`/`ui` delegate to the scene; returns on quit |
| `wake` | AtomicBool + `ctx.request_repaint()` (thread-safe) |
| `schedule_wake` | deadline cell + `ctx.request_repaint_after(ms)` |
| `request_frame` | flag + `ctx.request_repaint()`; `host.frame()` in the next `before_pass` |
| `quit` | `ViewportCommand::Close` on the root viewport (host decides quit; never auto-quit on last window) |
| `describe` / `describe_type` | generated descriptors, as other backends |
| host `post_event` / `prop_changed` | called directly during the pass (queue-only on the host side) |
| host `checkpoint` / `frame` / `call_sync` | only from `before_pass`/`after_pass` (debug-asserted) |

## 10. Probe: `docs/notes/scripts/egui-scenegraph-probe/`

Standalone crate with an empty `[workspace]` table: egui 0.37.0 (`default_fonts`), taffy 0.15.0, egui_kittest 0.37.0. Headless: `Context::run_ui` + `Context::tessellate`, no GPU, so the numbers exclude GPU upload and present.

- `src/tree.rs`: the arena (`gen << 24 | idx`, retire at 2^29, stale detection, insert-before/remove/destroy) plus a unit test.
- `src/scene.rs`: the walk in two layout modes. `Native` uses egui `top_down`/`left_to_right` per container, optional culling from last frame's sizes, and optional per-leaf scopes. `Taffy` uses a mirrored `TaffyTree`, a measure function built on `ctx.fonts_mut(..layout..)`, layout only when dirty, and painting with `new_child(max_rect)` and culling. Events and prop changes are pushed to an outbox during the pass.
- `src/main.rs`: `bench`, `layout`, `checks`.

Reproduce:

```sh
cd docs/notes/scripts/egui-scenegraph-probe
CARGO_TARGET_DIR=/tmp/egui-target CARGO_BUILD_BUILD_DIR=/tmp/egui-target/build \
  cargo run --release -- checks   # or: layout | bench | (none = all)
```

Tree shape: one root column with R rows, each row holding 10 leaves (6 labels, 2 buttons, 1 checkbox, 1 single-line text edit), so 11·R + 1 nodes. Window 1280×800, scrolled to the top (about 30 rows visible). Machine: AMD Ryzen 7 5700U (8C/16T, WSL2), **shared with other agents' builds and test runs** (load average 10–35 during the runs), so the absolute numbers are noisy upper bounds. Ratios between configurations are more trustworthy.

### 10.1 Correctness checks (`checks`)

```text
- id Identity: focus+typing survives keyed reorder: true; survives hot reload (new handles): true
- id Handle: focus+typing survives keyed reorder: true; survives hot reload (new handles): false
- id Positional: focus+typing survives keyed reorder: false; survives hot reload (new handles): false
- Native: events after press=0 release=1 (label now "count: 1"); typing 'hi' -> prop changes=1 text="hi"
- Taffy: events after press=0 release=1 (label now "count: 1"); typing 'hi' -> prop changes=1 text="hi"
- kittest: clicked via AccessKit query, 1 event(s) dispatched after the frame, label 'count: 1' found=true;
  AccessKit node ids are egui Ids (text edit id 0x3129c1a112f62bd3 = accesskit #3542575479455624147; ...)
```

- Clicks are reported on the frame with the release. The "JS" dispatch runs after `run_ui` returns, and its effect, the label text, appears in the next frame.
- Typing two characters in one frame is one `text` change.
- Two bugs came up while building the probe, and both belong in the design:
  1. **Scope id = widget id breaks interaction.** The taffy-mode text edit never got focus until the leaf scope's id became `id.with("scope")`.
  2. **`Layout::left_to_right(Align::Center)` in a `scope_builder` takes the whole available height.** The first row of the list was 800 px tall, because egui centres in the cross axis of the space it is given; `ui.horizontal` avoids this by starting at `interact_size.y`. Native containers need the same care, which argues for taffy owning placement.
- 0.37 kittest rejected the scene until the text edit got `Response::accessible_name`.

### 10.2 Layout timings (`layout`, two consecutive runs; load average about 16)

The scene is a mirrored `TaffyTree` with a definite root width. Leaves are measured with `fonts_mut(layout_no_wrap)`, and the intrinsic size is cached in the node until its text changes. "Taffy part" excludes the walk that turns relative layouts into absolute rects.

| nodes | mirror build ms | cold layout ms (taffy part; measure calls) | clean check µs | one label changed ms (taffy part; calls) | width −200 px ms (taffy part; calls) |
|---:|---:|---:|---:|---:|---:|
| 1,101 | 3.2–3.5 | 24.5–30.0 (24.4–29.9; 10,000) | 0.8–0.9 | 1.5–2.3 (1.4–2.2; 10) | 5.0–5.7 (5.0–5.6; 4,000) |
| 11,001 | 28 | 163 (161; 100,000) | 0.9–1.3 | 8.3–9.2 (7.2–7.9; 10) | 64–74 (63–72; 40,000) |
| 55,001 | 153–164 | 806–859 (799–850; 500,000) | 1.2–1.3 | 43–53 (36–43; 10) | 365–418 (359–411; 200,000) |

- With `width: auto` on the root (an earlier run), one changed label re-measured *every* leaf: 100,008 calls and 218 ms at 55k nodes. The root shrinks to fit its widest row, so a row that grows changes every row's width. That is correct CSS, and it is why the window root must be definite.
- Cold layout makes 10 measure calls per leaf, as the flex algorithm probes min, max and definite sizes. The intrinsic cache turns repeats into a `Cell` read.
- These numbers are for a mirrored `TaffyTree`. Running the traits directly on the arena is unmeasured.

### 10.3 Frame cost (`bench`, two runs; load average 14–20)

The table gives `run_ui` (all passes) and `tessellate` as medians over 10–60 frames after 3 warm-up frames, as run 1 / run 2. At the top of the scroll area about 35 rows (about 380 nodes) are visible. Every frame needed exactly 1 pass, including the first.

| nodes | mode | AccessKit | run_ui ms | tessellate ms | visited nodes |
|---:|---|---|---:|---:|---:|
| 1,101 | native, full walk | off | 4.14 / 4.26 | 1.60 / 1.61 | 1,101 |
| 1,101 | native, full walk | on | 8.56 / 10.17 | 1.02 / 1.05 | 1,101 |
| 1,101 | native, culled | off | 2.06 / 1.78 | 1.08 / 0.97 | 386 |
| 1,101 | native, culled, scoped leaves | off | 2.93 / 2.60 | 1.09 / 0.99 | 386 |
| 1,101 | native, culled | on | 4.05 / 3.41 | 1.17 / 1.04 | 386 |
| 1,101 | taffy, culled | off | 2.56 / 2.29 | 1.03 / 0.94 | 375 |
| 1,101 | taffy, culled | on | 5.11 / 4.29 | 1.08 / 1.00 | 375 |
| 11,001 | native, full walk | off | 55.8 / 49.9 | 1.09 / 1.08 | 11,001 |
| 11,001 | native, full walk | on | 91.0 / 93.8 | 1.17 / 1.09 | 11,001 |
| 11,001 | native, culled | off | 3.02 / 2.15 | 1.20 / 1.07 | 386 |
| 11,001 | native, culled, scoped leaves | off | 4.01 / 3.12 | 1.24 / 1.01 | 386 |
| 11,001 | native, culled | on | 3.99 / 3.28 | 1.14 / 0.93 | 386 |
| 11,001 | taffy, culled | off | 2.91 / 2.48 | 1.03 / 1.03 | 375 |
| 11,001 | taffy, culled | on | 4.77 / 5.54 | 1.05 / 1.04 | 375 |
| 55,001 | native, full walk | off | 286.8 / 289.4 | 1.37 / 1.48 | 55,001 |
| 55,001 | native, full walk | on | 655.9 / 336.8 | 1.59 / 1.03 | 55,001 |
| 55,001 | native, culled | off | 3.49 / 2.63 | 1.19 / 0.80 | 386 |
| 55,001 | native, culled, scoped leaves | off | 4.71 / 3.41 | 1.14 / 0.75 | 386 |
| 55,001 | native, culled | on | 4.55 / 3.30 | 1.08 / 0.97 | 386 |
| 55,001 | taffy, culled | off | 3.00 / 1.93 | 0.94 / 0.64 | 375 |
| 55,001 | taffy, culled | on | 4.95 / 2.85 | 0.96 / 0.64 | 375 |

Reading:

- **A full walk is linear, at about 5 µs per node.** A 16 ms budget allows roughly 3k emitted nodes. Off-screen widgets cost as much as visible ones, except in tessellation, which egui clips.
- **Culling makes cost proportional to the screen.**
  - About 2–3.5 ms + 1 ms for one dense screen (70 buttons, 35 text edits, 35 checkboxes, about 210 labels), whether the tree holds 1.1k or 55k nodes.
  - Native culling grows slightly with total size: one `allocate_space` per culled child, so 5,000 at 55k nodes.
  - Taffy culling stays flat: invisible children are skipped outright after a rect test.
- **AccessKit costs about +1–2 ms per screen** (+50–90%) when enabled, which happens only with a screen reader or under kittest/egui_mcp. On a full walk it doubles the cost.
- **Per-leaf scopes** (stable ids for `Button` etc.) add about 0.8–1.1 ms per screen (+35–45%). Use them only where an id matters.
- Taffy painting via `new_child(max_rect)` per leaf costs about the same as egui's own layouts.
- None of this includes the GPU (upload + draw) or eframe/winit overhead.

## 11. Open questions

1. **Identity prop (§1.4).** Should the compiler emit a stable `identity` per element (useful to all backends' devtools and a11y ids) so that egui keeps widget state across hot reload? It costs one reserved prop and a hash per element at creation.
2. **Same-frame vs next-frame JS dispatch (§4.2).** Next frame is simpler. Same frame via `request_discard` removes one vsync of latency but costs a second walk on interaction frames. Decide after measuring in a windowed demo.
3. **eframe vs our own winit runner (§5).** eframe gives viewports, AccessKit wiring, persistence and the `raw_input_hook`/`logic`/`ui` split for free. Its cost is that every timer tick is a full frame. Revisit if idle-with-timers profiles show it.
4. **Embedding mode and `run`.** Mounting into a caller-owned `Ui` (egui-workbench, Rerun-style apps) means the caller owns the loop and `svl_backend.run` is never called. The host needs a non-blocking start, the same "host start-order change" the iOS/Android notes ask for.
5. **Which egui-native containers become kinds**, and how they nest inside taffy: `scroll-area` (taffy overflow vs egui `ScrollArea`; the probe uses egui's), `panel` (egui `Panel` top/left/… inside a viewport), `grid` (taffy grid or `egui::Grid`), `collapsing`, `tabs` (egui has none; build on taffy + selectable labels).
6. **Metadata source for egui.** There is no GIR or metaobject to ingest. Options: hand-written `metadata/egui/*.yaml` checked by generated tests, or rustdoc JSON of a curated widget crate for the docs IR. This is a separate research slice.
7. **Tab order and `tabindex`.** egui's focus traversal follows emit order and geometry. Is that enough for Svelte apps?
8. **Text measurement mismatches.** The taffy measure must reproduce egui's widget chrome (button padding, checkbox icon, frame margins, `TextEdit` desired width). A wrong guess costs a `request_discard`. Can the generator test this per kind ("measured == rendered" for every kind and theme)?
9. **egui version cadence.** egui ships a breaking release about every 2–3 months (0.37 deprecated `Id::new` and `Ui::id`, replaced `App::update`, and renamed atoms), and egui_taffy and bevy_egui lag a release. Pin one egui version per svelters release and keep egui types out of the generated tables' public surface.

## 12. Risks

- **Per-frame cost grows with what is visible, not with what changed.** A dense visible screen pays the full walk + tessellation every frame while anything animates (a spinner, the text cursor blink). Mitigation: culling, a galley cache per node, the canvas shape cache, and keeping animations on the egui side.
- **Id hygiene.** Duplicate ids (scope = widget id, identity collisions) break interaction silently in release builds. Mitigation: debug assertions and egui's id-clash warnings in dev, plus a test per kind.
- **Re-entrancy.** One sync call placed wrongly (mid-pass) could re-enter the scene while it is borrowed. Mitigation: the "pass active" assertion in debug builds, and the `SVL_SYNC_BUSY` fallback.
- **Layout fidelity.** Taffy placement plus egui widgets won't match hand-written egui pixel for pixel. Apps that want "egui-native look" use the native container kinds.
- **Multipass.** Any side effect in the walk beyond the outbox (counters, toggles) runs twice on discard frames. Mitigation: keep the walk pure apart from the outbox.
- **Upstream churn** (see open question 9), and kittest's accessible-name enforcement, which makes a11y labels mandatory for inputs in tests.
