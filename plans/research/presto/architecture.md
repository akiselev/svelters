# presto architecture

Part of the GUI toolkit proposal (`README.md` in this folder). Date: 2026-10-25. Status: proposal
for the user; nothing is built. "presto" is a working name (README, open question Q1).

This file says **what the toolkit is**: threads, data structures, interfaces, invariants. The
companion files are:

| File | Covers |
|---|---|
| `compiler-and-js.md` | integration with the svelters compiler and the QuickJS engine |
| `platform.md` | per-OS platform layer: winit 0.30.13 plus shims, then own backends; present paths |
| `plan.md` | work packages, milestones and gates |

**Evidence** is in `notes/`: round 1 (eight notes) and `round2/`
(fourteen notes). Each claim there carries its source and a grade. This file cites a note by name
(`round2/scheduler-latency.md`) where the constants and the source quotes live, rather than repeating
them.

It supersedes the architecture sections of `../2026-10-25-own-gui-toolkit.md` (the round-1
proposal). Where the two disagree, this file is right. The corrections are listed at the end
(§18).

## 1. Goals and budgets

Unchanged from round 1 §2, now backed by the scheduler design.

| Goal | Target |
|---|---|
| Click to photon, 120 Hz | p50 12–18 ms, p99 ≤ 30 ms (60 Hz: p50 25–35 ms) |
| JS-driven change | at most +1 frame over a native one, p99 ≤ 33 ms at 120 Hz |
| One changed label | ≤ 0.5 ms UI-thread CPU, a few hundred µs compositor CPU |
| Idle | 0 frames, 0 timer wakeups (counted) |
| Under load | fling, scroll, transitions and caret keep display rate while JS or layout runs 100 ms |
| Scale | 10⁶ PCB primitives with pan and zoom as one uniform change; million-row virtual lists |
| Text | crisp at 12–13 px on 96-DPI Windows and HiDPI macOS; deterministic in tests |

**Correctness invariants**, checked by the deterministic simulator (§16):
- at most one frame in flight per window;
- every native-owned property has exactly one writer at a time;
- a JS commit never overwrites newer native-owned state;
- every input sequence id terminates exactly once (presented, merged, no visual change, or dropped
  with a reason);
- idle means no wakeups.

## 2. Overview

```text
                           ┌─────────────────────── UI thread (the platform main thread) ───────────────────────┐
 OS events ─▶ platform ─▶  │ 1 hit test (last commit) → native defaults (focus, press, caret, IME, scroll start) │
 (winit+shims, later own)  │ 2 JS dispatch (QuickJS) + microtask drain → ops → tree (transaction)               │
                           │ 3 at BeginUiFrame(T): input flush → animations (UI side) → rAF → style → layout    │
                           │   → observers → record display lists → hit lists → AccessKit → Commit              │
                           └───────────┬───────────────────────────────────────┬────────────────────────────────┘
                                       │ Commit (Arc swap)                     │ fast commits: scroll offset,
                                       ▼                                       ▼ camera, drag transform
                           ┌──────────────────────────── compositor thread ─────────────────────────────────────┐
                           │ FrameClock per window: wait(waitable | display link | frame callback | commit | timer)│
                           │ apply newest Commit → animations at T → scroll physics → damage → encode → submit   │
                           │ → present (with damage) → presentation feedback → estimator → BeginUiFrame(T')      │
                           └───────────┬────────────────────────────────────────────────────────────────────────┘
                                       ▼
                     one wgpu Device/Queue: window surfaces + viewport/canvas surfaces on OS layers
                     (DirectComposition visuals · CAMetalLayer sublayers · Wayland subsurfaces)

 Workers (existing pool): shaping of large text, glyph rasterisation, image/SVG decode, canvas chunk
 tessellation, kernel jobs. Results enter the tree only in the UI phases.
```

The decisions behind this shape are in round 1 §3–§4 (the cross-toolkit survey) and round 2
(`scheduler-latency.md`, `wgpu-integration.md`, `js-runtimes.md`):

- **Retained tree, re-recorded only where dirty.** Every mature toolkit does this.
- **Properties apart from content.** Transform, opacity, clip and scroll offset change without
  relayout or re-recording.
- **The compositor thread can draw without the UI thread.**
- **JS runs on the UI thread at event time.**
- **The toolkit performs direct manipulation natively**: hover, press, focus, caret, scrolling,
  drags, transitions.

## 3. Crates

| Crate | Contents | Depends on | Builds headless |
|---|---|---|---|
| `presto-core` | tree, kind registry, change classes, transactions and revisions, property trees, display-list types, hit lists, event model, focus model, a11y projection, `Commit` | taffy, accesskit (types), svelters-style (§5) | yes (wasm too) |
| `presto-anim` | curves (cubic bezier, steps, linear()), keyframe tracks, Web Animations timing, springs (closed form), scroll physics. Pure functions of time, shared by both threads | — | yes |
| `presto-frame` | the frame scheduler state machines (UI and compositor sides), the estimator, an injected clock, and the deterministic simulator | — | yes |
| `presto-text` | parley, parley_engine, fontique, harfrust, skrifa; caches; the glyph rasteriser; the Slug band builder for canvas text | — | yes |
| `presto-render` | wgpu pipelines, instance buffers, atlases, batching, damage scissoring, canvas chunk buffers | wgpu (our fork, §8.6) | needs a GPU or lavapipe |
| `presto-compositor` | the compositor thread: layer tree, applying commits, animation and scroll sampling, present loop, OS layer surfaces | presto-render, -anim, -frame | — |
| `presto-platform` | the platform trait; winit 0.30.13 plus side-channel shims first, then own backends per OS, Windows first (`platform.md`); IME client, clipboard, DnD, AccessKit adapters | winit 0.30.13, windows, objc2, smithay-client-toolkit, arboard, accesskit adapters | — |
| `presto-widgets` | built-in kinds: today's svelters-egui `widgets/`, `views/`, `interact/` and `paint/`, re-hosted | presto-core, -text, -anim | yes |
| `presto-egui-island` | the `EguiIsland` layer kind: an `egui::Context` rendered on presto's device, input translated, AccessKit grafted | egui 0.37, egui-wgpu | — |
| `presto-test` | headless `Harness` with kittest's verbs, event injection, AccessKit snapshot, CPU-side goldens | all | lavapipe |
| `svelters-presto` (svelters repo) | the backend ABI implemented on presto; the generated semantic spec `presto.json` | svelters-host, presto-* | — |
| `presto_berth`, `presto_quire` | dock and editor renderers beside `egui_berth` and `egui_quire` | berth-core, quire-core, quire-text | — |

**Rules for the crates:**
- **presto does not depend on svelters' JS host.** A pure-Rust app can use it.
- **svelters-style is the exception, and a deliberate one.** It is the CSS engine, it links no
  toolkit, and presto uses it as its style system (§5).
- The `-core`, `-anim`, `-frame` and `-text` crates build headless and run in cloud CI. The compiler
  can therefore link `presto-core`'s kind tables to generate the spec (`compiler-and-js.md` §2).

## 4. The tree (`presto-core`)

### 4.1 Storage

svelters-egui's scene arena moves here; it does not become a mirror. The porting inventory measured
23,247 LOC that move nearly as they are (`round2/svelters-inventory.md`).

What it already provides:
- column storage (`core`, typed `payload` per kind, interned `Arc<Common>`, `ext` columns);
- `NodeId = generation << 24 | index`;
- dirty bits, the journal, queries, hit lists, virtualisation (`RowPool`) and compaction.

What changes on the move:
- egui's value types (`Id`, `Rect`, `Vec2`, `ViewportId`) are replaced by presto's own, in device-
  independent `f32` points, with an explicit scale per window.
- The flag collision is fixed: `SCROLL_CENTER` and `INK_OVERFLOW` are both `1 << 21` today
  (`svelters-egui/src/scene/node.rs:290,296`).
- **Kind and member ids become stable.** Today they are positional: `(kind index + 1) << 16 |
  (member index + 1)`, `registry.rs:41-47`, so adding a row renumbers everything. Each id gets a
  fingerprint in the generated spec, and the host checks it at load (`compiler-and-js.md` §2).

### 4.2 Kinds

The egui-shaped extension traits go: `Widget::ui(&mut egui::Ui) -> Response`, `Behavior` with its
`Painter`, and `ScenePass::overlay`. They are replaced by one retained `Kind` trait. Round 1's
sketch was missing hooks that the inventory found in use: layout style, prepare, hit test, virtual
children, key and text hooks. The full trait:

```rust
pub trait Kind: 'static {
    /// Static description: props (with change class), signals (passive/cancelable, sync),
    /// slots, a11y role, the text-content prop. Generated into the svelters spec (§4.3).
    fn spec() -> &'static KindSpec where Self: Sized;

    /// Layout participation: taffy style from the computed style, plus measure for leaves.
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style { cx.default_layout() }
    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> { taffy::Size::ZERO }

    /// After layout, before recording: allocate text layouts, ask for images/glyphs, place children it owns.
    fn prepare(&mut self, cx: &mut PrepareCx) {}

    /// Write this node's display list (node-local coordinates). Called only when content-dirty.
    fn record(&self, cx: &mut RecordCx);

    /// Precise hit test inside the node's bounds (default: the rounded rect of the box).
    fn hit_test(&self, cx: &HitCx, p: Point) -> HitResult { cx.box_hit(p) }

    /// Native defaults for input reaching this node (press, toggle, key handling, drag start).
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled { Handled::No }

    /// Text input client, for kinds that edit text (§11.3).
    fn text_input(&mut self) -> Option<&mut dyn TextInputClient> { None }

    /// Native-owned state the kind exposes (scroll offset, selection, drag value) — one owner (§4.5).
    fn native_state(&self) -> Option<&dyn NativeState> { None }

    /// Children supplied on demand (virtual list rows, tree rows).
    fn virtual_children(&mut self) -> Option<&mut dyn VirtualChildren> { None }

    /// AccessKit node from props and state.
    fn a11y(&self, cx: &mut A11yCx);
}
```

The interaction units in `interact/` (2,986 LOC: Pressable, Draggable, Focusable and the rest) are
already state machines over input. Their input moves from `egui::Response`/`InputState` to presto
`Event`s. They keep egui's click semantics on purpose: Pressable copies them, and about 1,000
kittest assertions depend on them.

### 4.3 Change classes

Every prop and every CSS property carries one of four classes. It is declared in `KindSpec` and in
svelters-style's property table, and emitted into the semantic spec so the compiler can tag writes
(`compiler-and-js.md` §3.4).

| Class | Examples | Work it causes |
|---|---|---|
| `Structure` | create, insert, remove, reorder, slot change | relayout to the boundary, record, a11y, hit list |
| `Layout` | text, font, padding, width, gap, visibility (display) | style, then layout only if the intrinsic size changed (the existing measure guard), then record |
| `Paint` | color, background, border color, icon, selected-state visuals | record this node only |
| `Property` | transform, opacity, clip, scroll offset, z-raise, canvas view, visibility (visibility: hidden) | compositor only: no UI phases |

A `set_prop` marks exactly the work of its class. The journal records (node, class), and damage
comes from the journal (§7.4), not from diffing render trees.

### 4.4 Transactions and revisions

- **A transaction** is one JS task plus its microtask drain, or one Rust command batch, or one
  native default (a press, a scroll step). It applies atomically. `revision` increments once per
  transaction.
- **Every event carries an `InputSeq`.** The transaction records the sequence numbers it consumed,
  and each Commit carries `(revision, last_input_seq)`.

### 4.5 One owner per property

Native-owned state lives in the tree as state of its kind:
- scroll offset, fling and smooth-scroll animations;
- drag position;
- pseudo-states (`:hover`, `:active`, `:focus-visible`);
- text selection and composition;
- the presentation value of a running animation;
- a splitter's position during a drag.

**Rules:**
- **JS reads native-owned state and is told about changes.** It reads through `bind:` (observe) and
  events.
- **JS may request a change,** such as `scrollTo` or `selection = …`. The request goes through the
  kind, which takes over or aborts the native animation.
- **A JS write is rejected when native code has changed that state since the event JS saw.** The
  write is ignored with a dev warning, so it cannot paint stale state over a live drag. This is the
  revision check; RN's revision lock and Chrome's main/impl scroll reconciliation do the same.
- **There are no side-channel fast paths.** Reanimated's direct prop writes broke hit testing and
  measurement (round 1, `js-runtimes.md`). presto's fast commits (§7.3) are writes of
  native-owned properties by their owner, recorded in the tree.

## 5. Style and layout

**Style is svelters-style, re-hosted.** It is the Rust CSS engine: cascade, selector matching with
change bitmasks and sibling sharing, transitions trigger, and utilities. It already "links no
toolkit".

What changes:
- **The `Element` view implements on presto's tree.** CSS.md CS5e: "the scene implements `Element`
  directly".
- **Computed values carry the change class (§4.3),** so a restyle that changes only paint
  properties records without relayout.
- **CSS transitions become engine animations (§12).** svelters-style keeps the trigger role: the
  before/after diff that starts one.

That last change also fixes the sampler's gaps (`round2/animation-scroll.md` §2.2):
- transforms do not transition;
- a reversed transition restarts at full duration (no shortening factor);
- no `transition*` events;
- no `linear()` easing.

**Layout is taffy 0.15 on the tree,** through its low-level traits, with no `TaffyTree` mirror. It
is done that way today (`layout.rs`); Bevy deleted its own mirror in 2026 for the same reason.
- **Text measure** goes through a `TextMeasure` trait implemented by `presto-text`. Today it calls
  egui's `ctx.fonts_mut`. parley gives min/max-content (`calculate_content_widths`) and re-breaking
  at a new width without reshaping (`break_all_lines`), which are exactly taffy's queries
  (`round2/text-stack.md` §1).
- **The existing guards stay:** re-measure on a text change and compare intrinsic size; layout
  boundaries; definite widths for roots and scroll content.
- **taffy is built at opt-level 3 in dev,** as Zed does (ZED-LESSONS §1.5).

## 6. Recording, property trees and layers

### 6.1 Display lists

A content-dirty node re-records one `DisplayList`:

```rust
pub struct DisplayList {
    pub bounds: Rect,                 // node-local ink bounds
    pub items: SmallVec<[Prim; 8]>,   // Quad, Shadow, Line, Text(run id), Icon, Image, Path, Surface, CanvasRef
}
```

- **Text items reference shaped runs by id.** They never hold atlas coordinates; those are
  resolved per frame. That is quire's lesson from egui atlas rebuilds (`round2/text-stack.md`).
- **Lists are `Arc`.** A Commit carries only the lists that changed.

### 6.2 Property trees

Four trees, as in Chromium:

| Tree | Holds |
|---|---|
| transform | 2D affine per node that has one, plus scroll translation |
| clip | rect or rounded rect, in the transform space of its node |
| effect | opacity, filter, blend mode |
| scroll | scroll container, offset, content size, physics state |

Each tree node refers to its parent. A tree node is created only for nodes that need one, so most
scene nodes share their parent's.

### 6.3 Layers

A **layer** is a subtree the compositor treats as one unit, with its own damage and, for some
kinds, its own GPU target. These become layers:
- each window root;
- scroll containers, with an optional tile cache (§7.5);
- subtrees with a running compositor animation;
- floating panes, popovers, menus, tooltips and the drag image;
- canvases (§9.5);
- 3D viewports, on their own OS layer when available;
- egui islands.

There are tens of layers, not thousands. Qt advises fewer than 10 batches, and Zed draws a window
in tens of draw calls.

### 6.4 The Commit

```rust
pub struct Commit {
    pub window: WindowId,
    pub revision: u64,
    pub input_seq: InputSeq,                       // last input this commit has seen
    pub frame: FrameId,                            // the BeginUiFrame(T) it answers, or Urgent
    pub layers: Vec<LayerDelta>,                   // added / removed / reparented / content-changed (with dirty rects)
    pub props: PropertyTreeDelta,                  // transform / clip / effect / scroll node changes
    pub lists: Vec<(NodeId, Arc<DisplayList>)>,    // re-recorded lists only
    pub text: Vec<(RunId, Arc<ShapedRun>)>,        // newly shaped runs
    pub resources: ResourceDelta,                  // images, icons, glyph raster requests
    pub animations: Vec<AnimationCmd>,             // start / cancel / retarget, with timing
    pub a11y: Option<accesskit::TreeUpdate>,       // incremental
    pub hit_list: Option<Arc<HitList>>,            // for the UI thread's next hit tests
}
```

**Building it costs microseconds,** because only changed parts are cloned.

**The compositor folds commits.** If two arrive before a draw, the second is applied over the first.
It never blocks the UI thread, and it applies the newest at its deadline.

## 7. The compositor (`presto-compositor`)

### 7.1 Ownership

One compositor thread per process owns:
- the wgpu `Instance`, `Device` and `Queue`;
- every surface;
- each window's committed layer tree, property trees and display lists;
- the animation and scroll engines;
- present pacing.

**It never calls winit or the OS window API** (§8.1). On macOS winit dispatches off-main calls
synchronously to the main thread, which would deadlock the resize handshake
(`round2/wgpu-integration.md` §1).

### 7.2 Its loop

The state machine is in `round2/scheduler-latency.md` §2. In outline, per window:
1. Block in one multiplexed wait: the DXGI waitable / display link / frame callback, the commit
   channel, the fast-commit mailbox, and the next animation deadline.
2. On the clock tick for target present time `T`:
   - post `BeginUiFrame(T)` to the UI thread at its computed start time `S_ui`;
   - set this window's compositor deadline `D_comp`.
3. At the deadline, or as soon as a commit for `T` arrives:
   1. apply the newest commit and the fast commits;
   2. sample animations and scroll physics at `T`;
   3. compute damage;
   4. acquire (late);
   5. encode, submit, present.
4. Presentation feedback releases the frame in flight and updates the estimator.

**Two-deadline rule.** If `D_comp` arrives without a commit and the compositor has its own damage
(animation, scroll, caret), it draws anyway. If it has none, it waits.

**Back-pressure.** The UI thread never starts a frame while its previous commit is unapplied.

### 7.3 Fast commits

A **fast commit** is an update of a native-owned property by its owner, posted straight to the
compositor's per-window mailbox without UI phases. Examples:
- a wheel step on a scroller whose listeners are all passive;
- a canvas pan or zoom;
- the viewport camera during orbit;
- a drag transform.

Each one is also written to the tree, so the next Commit agrees. Its revision is newer than any
in-flight JS commit, which is why the revision check (§4.5) protects it.

### 7.4 Damage

**Inputs:**
- the journal's dirty nodes (old ∪ new bounds);
- layers whose property nodes changed;
- running animations (the bounds of the animated node, transformed);
- the caret.

**Per surface image:** damage accumulates per swapchain image, which gives correct buffer age.

**Drawing:** the compositor redraws only the damage, scissored, into the persistent back buffer, then
presents with damage:
- DX12 `Present1` dirty rects;
- Vulkan `VK_KHR_incremental_present`;
- a full present on Metal.

**Empty damage means no present.**

**Debug check:** recompute the full scene, diff it against the incremental one, and fail on a
missed dirty mark. Zed's `diff_with` from PR #59441 does this (`round2/gpui-port.md` #16).

### 7.5 Scroll layers and tiles

**Without tiles.** A scroll layer's content is re-encoded from its display lists, offset by the
scroll translation; only the newly exposed strip is "damage" in the content sense. On a fast GPU
this beats tile management.

**With tiles.** On lavapipe and integrated GPUs, a layer with expensive content (the editor, large
tables) may cache 256–512 px tiles in device pixels and scroll by offset. Keys are exact
device-pixel offsets: WebRender's ¼-px rounding bug left stale tiles on screen.

**Tiles are added only where a measurement shows redraw is the cost** (plan M4).

### 7.6 Surfaces and OS layers

- **Window surfaces and viewport/canvas surfaces** are created on the UI thread (in
  `can_create_surfaces`, from raw handles) and moved to the compositor. `wgpu::Surface` is
  `Send + Sync`.
- **Viewports and canvases get their own OS layer and wgpu surface on the same Device:**
  - a DirectComposition child visual through `SurfaceTargetUnsafe::CompositionVisual`;
  - a CAMetalLayer sublayer;
  - a Wayland subsurface on our own event queue.
- **Resize is the only synchronous handshake.** The UI thread sends `Resize { size, scale, seq }`
  and waits up to about one refresh for `ResizeDone`. The per-OS sequences are in `platform.md` §4:
  - a DComp commit on Windows;
  - `presentsWithTransaction` with the present on the main thread during macOS live resize;
  - subsurface sync mode on Wayland.

## 8. The renderer (`presto-render`)

### 8.1 Primitives

A closed set of instanced pipelines, created at startup off the main thread. They derive from GPUI's
renderer (Apache-2.0; `round2/gpui-port.md` items 1–6), adapted.

| Primitive | Shader | Change from GPUI |
|---|---|---|
| `Quad` | SDF rounded rect: per-corner radii, per-edge borders, dashed borders, solid/gradient fill | premultiplied linear-in-sRGB colours instead of HSLA; **rounded clip** added (GPUI has rect clips only) |
| `Shadow` | closed-form erf blur, drop and inset | — |
| `Line` | capsules, dashes, wavy underline | — |
| `Mask` | R8 coverage sprite, gamma + contrast (Windows Terminal formula, MIT notice kept) | atlas key adds AA mode and dilation level |
| `Lcd` (later) | dual-source subpixel sprite on opaque layers | §9.4 |
| `Color` | premultiplied RGBA sprite: emoji, images, raster icons | — |
| `Path` | lyon tessellation into a 4× MSAA intermediate | cached per (path, scale); intermediate sized to the batch bounds, not the window |
| `Surface` | texture from another pass (offscreen viewport, egui island, tile) | **implemented** (a no-op in GPUI's wgpu renderer) |
| canvas kinds | segment, arc, annulus, pad, polygon, canvas glyph (Slug) | ours (§9.5) |

### 8.2 Batching and buffers

**Draw order** follows GPUI's `BoundsTree`: each primitive's order is 1 + the maximum order of what
it overlaps. GPUI's R-tree is insert-only, so ours is rebuilt per re-recorded layer. The renderer
then sorts by (order, pipeline, atlas page), giving about 10–50 draws per frame.

**Buffers:**
- A **ring of instance buffers, one per frame in flight**, written through a `StagingBelt`. GPUI's
  wgpu renderer has no ring and creates a bind group per kind per frame. Its Metal
  `InstanceBufferPool` is the model to copy.
- **Retained layers and canvas chunks keep persistent buffers.** A clean layer re-encodes from its
  cached instance range.

**Depth pre-pass.** Opaque quads front to back with depth write, then everything back to front with
depth test. GPUI's PR #61461 was **closed unmerged**, and the 0.96 → 0.44 ms figure comes from it.
Benchmark it on integrated GPUs and lavapipe before turning it on.

### 8.3 Blending space

- Surfaces are non-sRGB (`Bgra8Unorm`), and all blending happens in sRGB-encoded space.
- That is how browsers composite, so svelters CSS looks as it does on the web, and it is what the
  text gamma/contrast constants assume. Windows Terminal and Zed both pick non-sRGB targets for this
  reason.
- With `*Srgb` surfaces, dark text thins and light text thickens.
- **The decision applies to the whole renderer,** not just text (`round2/text-stack.md` §3).
- Image and gradient colours stay in sRGB space; linear-light blending is opt-in per effect.

### 8.4 Atlases

- **Packing:** etagere bucketed atlas pages of 1024², grown up to 2048² when a tile needs it.
- **Formats:** R8 for coverage, RGBA8 premultiplied for colour.
- **Generations and eviction:** each texture has a generation, and pages are evicted by
  frame-generation LRU. GPUI never evicts glyphs; we must, because the CAD canvases zoom over 10⁴×.
- **Uploads** are deferred to the start of the compositor frame.

**Icons:** the icon kit's SVGs are rasterised by resvg per (icon, size, scale, colour role) into the
colour atlas, as monochrome masks when the icon is single-colour. Rasterisation happens on workers,
and frequently used sizes are kept warm.

### 8.5 Device loss

The renderer keeps every CPU-side source: display lists, shaped runs, glyph raster cache keys, and
canvas chunks as CPU arrays. On device loss it recreates the device and surfaces, re-uploads lazily,
and draws again. egui-wgpu does not recover from device loss (performance.md item 14).

### 8.6 The wgpu fork

The patch series P1–P4 on wgpu 30.0.x is registered in `forks.toml`, and each patch is proposed
upstream (`round2/wgpu-integration.md` §4):

| Patch | What | Size |
|---|---|---|
| P1 | `Queue::present_with_damage`: Vulkan `VkPresentRegionsKHR`, DX12 `Present1`, EGL swap-with-damage, Metal ignores it | 350–450 lines |
| P2 | caller-chosen acquire timeout; Vulkan `NOT_READY` reported as a timeout | 30–60 lines |
| P3 | Metal acquire from a caller-supplied `CAMetalDrawable` (CAMetalDisplayLink pacing) | 40–80 lines |
| P4 | no NSWindow/NSView access in Metal acquire off the main thread | 15–30 lines |

**Upstream.** PR #10152 (`Queue::present_with_damage`) is open, so P1 tracks it rather than
diverging.

**Needs no patch** (available through `as_hal`, 30.0.1):
- the DXGI waitable and frame statistics (`DontWait`, `waitable_handle()`);
- DComp visuals;
- on Vulkan, present timing, present-wait and incremental present through trunk's chain hooks.

Those hooks need a ~150-line backport, or wgpu v31.

## 9. Text (`presto-text`)

Full evidence and versions are in `round2/text-stack.md`.

### 9.1 Stack

- **UI text:** parley 0.12 with fontique for fallback and harfrust for shaping. Gaps we fill
  ourselves: ellipsis (#304) and tab stops (#302).
- **Editor lines:** `parley_engine` 0.12 per logical line, with quire's own wrapping and its ASCII
  monospace fast path (0.74 ms against 1.69 ms for a 40-line fling, measured in quire). Not
  `PlainEditor`, which re-lays out the whole buffer on each edit.
- **Outlines and hinting:** skrifa, hinted vertically only
  (`HintOptions::auto_fallback().with_preserve_linear_metrics(true)`); advances are never hinted.
  skrifa 0.49's `raster_policy()` applies DirectWrite's `gasp` rules: horizontal-only antialiasing
  for instructed fonts up to 20 ppem, which is the NATURAL mode that makes Segoe UI crisp at
  12–13 px.
  - **Version skew:** parley pins skrifa 0.44, so until parley moves we copy the ~30-line policy.
- **Rasteriser:** our own small area-coverage rasteriser fed by skrifa paths. It is deterministic
  and has the x-only antialiasing mode. Colour glyphs go through COLR via skrifa's paint API, and
  sbix/CBDT as decoded PNGs.

### 9.2 Gamma, contrast, platform look

- **The shader:** Windows Terminal's grayscale formula (gamma 1.8, enhanced contrast 1.0 by default).
  - On Windows, gamma and contrast are read from `IDWriteRenderingParams1`.
  - Black text stays near identity, and white text is boosted.
  - One shader serves light and dark themes without knowing the background.
- **macOS:** contrast 0 plus CoreGraphics-style colour-dependent dilation (5 luminance levels, as
  `gpui_apple` replicates). Without it text draws lighter than native.
- **Linux:** GTK 4's "automatic" recipe: grayscale, slight hinting, unrounded positions.

### 9.3 Caches

| Cache | Key | Eviction |
|---|---|---|
| font set | generation counter | on font-set change |
| fallback | (script, locale, family stack), plus per-codepoint memos for Common-script symbols (⌘ ✓ → box drawing) | generation |
| shaped paragraph | text hash, style-runs hash, font generation, scale, features → `parley::Layout` whose brush is a **style id** (theme and hover changes never reshape) | two generations, plus an LRU byte budget |
| line breaks | last break per layout, content widths, height-for-width for the last 2–3 widths | with the layout |
| glyph raster | font and index, variation hash, glyph id, ppem (26.6), x-bin 0–3, hint mode, AA mode, dilation | page LRU by frame generation |
| editor line | line hash, token-run hash, font generation, ppem, coordinates; wrapped rows keyed by wrap width | two generations |

**Positioning:** subpixel positions are 4×1. Baselines and text-layer scroll offsets are integral
device pixels; Zed hit y-subpixel blur (#37460) and has an open bug where odd window heights blur
all text (#65664).

### 9.4 LCD text (later, gated)

- **Default:** grayscale with gamma.
- **Why plan LCD anyway:** Zed shipped grayscale in Sep 2025, then made LCD subpixel text the
  default on Windows and Linux in Jan 2026 (PR #45423) "for legibility on standard DPI".
- **How:** presto plans a dual-source `Lcd` sprite for text in opaque layers, behind a setting. It
  ships only after a screenshot review on 96-DPI Windows (plan M5, README Q8).

### 9.5 Canvas text and big canvases

- **Canvas text is Slug, our own WGSL port.** The patent has been public domain since 2026-03-17,
  and the reference shaders are MIT/Apache with attribution. A CPU band builder is fed by skrifa
  outlines, with dynamic dilation.
  - Used for TrueType labels on the PCB/schematic canvases and in 3D.
  - Below about 6–8 px on screen, labels switch to atlas glyphs or a greeked bar.
  - Stroke fonts (KiCad's newstroke kind) are geometry drawn with `Line`.
  - No MSDF: it needs regenerating across the ~10⁴× zoom range.
- **Canvases** are `Canvas` layers holding typed instance buffers in chunks (segment, arc, annulus,
  pad, polygon, glyph):
  - analytic shaders, with pan and zoom as one view-uniform change;
  - per-chunk culling with level of detail;
  - camera-relative offsets for the integer-nanometre coordinates;
  - the static content cached to a texture between edits;
  - hover, selection and the router's rubber band in an overlay layer that redraws alone.
- **Migration:** cadatomic's canvases render into their own targets today (`canvas2d/mod.rs`, a
  texture plus a blit). They start as `Surface` layers and move to native canvas layers one at a
  time.

### 9.6 Determinism

**Test configuration:**
- bundled fonts with system fonts off;
- fixed gamma constants;
- the skrifa rasteriser;
- native-look switches off (DirectWrite parameters, macOS dilation, LCD).

**Goldens:**
- **Exact goldens are CPU-side:** glyph-run lists, atlas tile bytes, display lists. These are what
  agents diff.
- **GPU pixel snapshots use a tolerance only,** on the pinned lavapipe of the cloud image (the
  CLAUDE.md snapshot rule).

## 10. Frame scheduling (`presto-frame`)

This section summarises the design; the state machine, the estimator, the clock sources and the
pseudocode are in `round2/scheduler-latency.md` §2–§4.

### 10.1 Deadline modes

There is one `FrameClock` per window on the compositor thread, modelled on Chromium's
`SchedulerStateMachine`. It separates "what to do next" from updating state, for testability.

| Mode | When |
|---|---|
| Immediate | a commit is ready, the frame is urgent, or it is the first frame after idle |
| Regular | the compositor has its own damage, so it draws at `D_comp` without the UI's commit (two-deadline rule) |
| Late | only waiting for the UI: wait to the next deadline |
| Blocked | headless scenarios |

### 10.2 Timing

- **UI start time:** `S_ui = T − (latch + margin) − est_gpu − est_comp_cpu − 50 µs − est_ui`.
- **Estimates:** p90 of a 64-sample ring with a 0.25 ms floor. The latch margin follows KWin's
  fast-attack, slow-decay rule.
- **GPU cost:** scheduling uses the `on_submitted_work_done` time. Timestamp queries are for
  diagnosis only.
- **Cold start:** UI 2 ms, compositor CPU 1 ms, GPU 2 ms, margin 1 ms. Until the first feedback
  arrives, draw immediately.
- **Urgent frames:** discrete input after idle, or when `now + est_ui_input ≤ D_ui(T)`, runs the UI
  phases at once. This is Mutter's `should_update_now`, "lowest average latency for sporadic user
  input".
- **Slow UI:** pipelined mode, capped at 2× the interval as KWin and Mutter do. It leaves the mode
  after 10 frames that fit with 1.5× headroom (Swappy's rule).

### 10.3 Display behaviour

- **VRR and ProMotion:** keep presenting for 1 s after high-rate input. GPUI's `InputRateTracker`:
  100 ms window, 60 events/s. Boost the Windows compositor clock during gestures.
- **Hidden windows:** unsubscribe from the clock and keep the damage; one urgent frame when visible
  again.
- **Multi-window:** one compositor thread with one wait, a clock per window. Windows on different
  displays pace independently, and GPU work is ordered by earliest deadline.

## 11. Input and events

### 11.1 Event-time dispatch

A **discrete event** (key, button, wheel start, focus change, IME commit) is handled when it
arrives, in this order:
1. Native hit test on the last commit's hit list. GPUI walks hitboxes in reverse with `BlockMouse`
   and `BlockMouseExceptScroll`.
2. Native defaults.
3. JS listeners: capture, target, bubble (EV1).
4. The microtask drain.
5. The resulting transaction requests an urgent frame (§10).

A **continuous event** (pointer move, wheel delta, touchpad pan):
- updates hover at once;
- stores its samples in the event's history;
- is delivered to JS once per frame in phase 1 (input flush), with `getCoalescedEvents()`.

**This needs one host change:** a "dispatch and drain microtasks" entry. Today `call_sync` drains
nothing, and widget signals (`onclicked`) wait in the outbox for the next frame
(`round2/svelters-inventory.md` risk 9).

### 11.2 Keys: IME first

This corrects round 1 §7.1. While a composition is active, or an IME input source is active,
printable keys go to the IME before keybinding matching. Otherwise a chord like `jj` breaks
Japanese input; GPUI routes keys this way (`round2/ime-a11y-os.md` §1).

Outside composition, keys resolve in this order:
1. the command layer's keymap (cadatomic's one command path);
2. the focused kind;
3. JS listeners.

cadatomic's keymap stores egui `Key` name strings (`keymap.rs`, `keylayout.rs`), so presto's key
enum must round-trip those names.

### 11.3 Text input

`TextInputClient` follows GPUI's `InputHandler` and adds what TSF, macOS and Wayland need:

```rust
pub trait TextInputClient {
    fn text_for_range(&self, utf16: Range<usize>) -> Option<String>;
    fn selected_range(&self) -> Option<Utf16Range>;           // caret = empty range
    fn marked_range(&self) -> Option<Utf16Range>;             // composition
    fn replace_text_in_range(&mut self, range: Option<Utf16Range>, text: &str);
    fn replace_and_mark_text_in_range(&mut self, range: Option<Utf16Range>, text: &str, new_selected: Option<Utf16Range>);
    fn unmark_text(&mut self);
    fn bounds_for_range(&self, range: Utf16Range) -> Option<Rect>;  // per-character candidate placement
    fn character_index_for_point(&self, p: Point) -> Option<usize>;
    fn surrounding_text(&self, max_bytes: usize) -> Option<(String, Range<usize>)>;
    fn purpose(&self) -> InputPurpose;                       // text, number, password, url, code …
}
```

- quire-text implements it. It already has preedit state, multi-cursor and word rules. It lacks
  bidi motion and document-wide UTF-16 offsets, which ropey makes cheap to add.
- The platform layer speaks IMM32/TSF, NSTextInputClient or text-input-v3 to it (`platform.md`
  §3).
- winit cannot cancel a composition (egui toggles IME off and on). The Windows shim cancels through
  `ImmNotifyIME`; the own backends cancel natively (`platform.md` §6).

### 11.4 Pointer

- **History and prediction:** the full pointer history is available to canvases, wire drawing, the
  router and ink (`GetPointerInfoHistory`, coalesced NSEvents, GDK history). Prediction is opt-in,
  at most one frame ahead, capped at 8 ms.
- **Capture:** the pressed node keeps the pointer until release, across windows during a tab drag.
  `presto_berth` needs global capture.
- **Hover** uses only the newest sample.
- **Raw input** is off (`DeviceEvents::Never`) except while a 3D view captures the mouse
  (`platform.md` §3.2).

### 11.5 Passive listeners

- **Default:** listeners on scroll and drag paths are passive. The compiler infers non-passive
  listeners (`compiler-and-js.md` §3.5), and `|nonpassive` overrides.
- **Effect:** only regions with a non-passive listener make a scroll wait for JS (Chrome's
  non-fast-scrollable region), and the inspector shows them.
- **Evidence:** over 80% of touch listeners and 98% of wheel listeners never call `preventDefault`.

### 11.6 Focus

Focus becomes presto's own model; today it is egui `Memory` (109 hits in 36 files). It keeps:
- tree order for Tab;
- `contain` and `dialog` scopes (EV3);
- `:focus-visible` rules;
- focus within;
- programmatic `focus()` taking effect in the same transaction.

### 11.7 Clipboard and drag-and-drop

The clipboard and primary selection come from arboard and smithay-clipboard; winit 0.30 has none. File drops
are winit's; the drag source is per OS (an OLE source that keeps dispatch running is in W-P3). Inside the app,
DnD carries a JS-side id payload. Between egui islands and presto, an `egui::DragAndDrop` bridge
lets cadatomic's Rust panels interoperate during migration.

## 12. Animation and scroll (`presto-anim`)

Full design and constants: `round2/animation-scroll.md`.

### 12.1 Engine

**Timelines:** one document timeline per window. Its time `T_ui` is the predicted present time,
frozen for the whole UI frame.

**Model:** Web-Animations-shaped (`Animation`, `KeyframeEffect`, `EffectTiming`, per-property tracks).
CSS strings are parsed once, at `animate()`. Each track is classified and runs in one of three
places:

| Track class | Properties | Runs |
|---|---|---|
| compositor | opacity, transform/translate/rotate/scale, non-layout clip | sampled by the compositor at `T_c`; no UI work |
| paint | filter, colours, shadows, stroke dash | sampled on the UI thread, re-recorded without layout |
| layout | width, height, padding, margin, border widths | style + layout per frame (Svelte's `slide`) |

**Sync with JS.** The compositor picks the start time and reports `Started`/`Finished`/`Aborted`.
JS reads (`getComputedStyle`, `currentTime`, `getBoundingClientRect`) re-evaluate the same pure
sample function on the UI thread at `T_ui`, as Firefox OMTA does. There is no synchronous read-back
from the compositor. `onfinish` and `transitionend` dispatch in the first UI frame whose `T_ui`
passes the end.

**The Web Animations subset Svelte 5.57.2 needs:**
- `Element.animate(keyframes, { duration, fill: 'forwards' })`, including empty keyframes and a 0 ms
  duration;
- on `Animation`: `onfinish`, `cancel()`, `currentTime`, `playState`, and the `effect = null` setter;
- `getAnimations()`;
- an asynchronous finish.

On the element side it also uses `getComputedStyle`, `getBoundingClientRect`,
`clientWidth`/`clientHeight`, `inert` and `style.*`. Svelte bakes its easing into keyframes sampled
at 60 Hz. The compiler can do better for known transitions (`compiler-and-js.md` §3.6).

**Springs** use a closed form evaluated at the present time:
- **Svelte's parameters** are mapped through the 60 Hz recurrence roots to continuous poles. The
  default spring is ω₀ = 33.4 rad/s, ζ = 1.45. It matches Svelte exactly at 60 Hz and is
  frame-rate independent; Svelte's own integrator is not (23% overshoot at 60 Hz against 0.7% at
  240 Hz for `{0.1, 0.25}`).
- **Canonical parameters** are `{duration, bounce}`, Apple's.

### 12.2 Scroll physics

The scroll engine lives in the compositor and owns the scroll offset during an animation. Defaults
per OS (constants in the notes §4.6):

| | Windows | macOS | Linux |
|---|---|---|---|
| wheel | 100 px per detent, Chromium's curve (200 → 100 ms, velocity-preserving retarget) | ×40 px with OS acceleration, no animation | 120 px per detent, Chromium's curve |
| touchpad | DirectManipulation viewport (own backend) or OS momentum | 1:1, momentum from `momentumPhase` | 1:1, our fling on `axis_stop` (last 150 ms regression) |
| fling | DirectManipulation inertia | the OS's | Chromium's `PhysicsBasedFlingCurve` |
| overscroll | none | rubber band, critically damped return | none |

- **Latching is per gesture.** A momentum stream that reaches an edge does not scroll the outer
  panel; today svelters chains per event.
- **Reduced motion** makes the smooth curves instant.

## 13. Accessibility

- **One producer.** presto projects the AccessKit tree from the tree's props and kind state, sending
  updates for dirty nodes only, at commit time.
- **A validating builder** sits in front of the platform adapter. AccessKit panics on an invalid
  update (a focus id not in the tree, for example), as Zed hit repeatedly. Our builder:
  - repairs the update or drops it with a dev assertion;
  - never panics in release.
- **Adapters:** the platform adapters directly (`accesskit_windows`, `_macos`, `_unix` 0.25).
  `accesskit_winit` 0.34 is stuck on winit 0.30. On macOS we use our own NSView, avoiding the
  subclassing adapter's crash on window close.
- **Text editing** maps to the UIA text pattern, macOS text-range methods and AT-SPI Text and
  EditableText.
- **Virtual lists** expose their realised rows plus set size and position, since AccessKit has no
  virtualised-list pattern.
- **egui islands** use AccessKit nested trees (`TreeId`, 0.23+). egui 0.37 always emits the root
  tree id and resends its whole tree each pass, so the island rewrites egui's tree id and maps
  incoming actions back.
- **Author ids** (EM1) stay the stable locators for agents and tests.

## 14. Windows, layers above panes, tear-off

- **Windows** are ordinary presto windows with their own frame clock. The deferred-viewport
  workarounds go away: the `SlotPainter` thread-local, the one root frame per off-thread wake, and
  `logic` not running for deferred viewports.
- **Overlays above panes** are layers in a per-window overlay stack: the tour overlay, designer
  adorners, the palette modal, menus, tooltips and the drag image. They are click-through unless
  they take input.
- **Tear-off needs these from the platform layer:**
  - global pointer capture across windows;
  - window rects, monitor work areas and z-order;
  - creating a window under the pointer;
  - on Wayland, `xdg-toplevel-drag-v1` (KWin, Mutter and Jay only; winit has none).

## 15. The egui island

`EguiIsland` is a layer kind for migration.
- It owns an `egui::Context` and an egui-wgpu `Renderer` on presto's device.
- Each frame it calls `Renderer::new` → `update_buffers` → `render` into its own texture (`Rgba8Unorm`;
  `register_native_texture` needs that format), and composites the texture as a `Surface`.
- Input inside its rect is translated to egui `RawInput`. Its AccessKit tree is grafted (§13).
- It repaints only when egui asks (`has_requested_repaint`), so the idle barrier keeps working.

**What runs in an island.** Every dock pane body can, unchanged, because `PaneHost::ui(&PaneId,
&mut egui::Ui)` is the only interface. That covers:
- cadatomic's Rust panels;
- the ECAD panes;
- the 3D viewport and canvases, if their egui-wgpu `CallbackTrait`s run on the island's renderer,
  which runs on presto's device;
- svelters-egui panels, until `svelters-presto` lands.

**What cannot be islanded:** the shell chrome and the dock frame, which host the islands; keyboard
pre-dispatch; close interception; and context-wide overlays (`round2/cadatomic-inventory.md`).

**The migration ratchet is "islands remaining".** egui leaves `Cargo.lock` at 0.

## 16. Testing, automation and measurement

### 16.1 Harness

`presto-test::Harness` keeps kittest's verbs, because 59 svelters-egui and 45 cadatomic test files
use them. Usage counts (cadatomic): `state` ~650, `run` 323, `step` 126, `get_by_label` 96,
`.click()` 100, `event` 89, `key_press` 112, `get_by_role_and_label` 80.

| Verb | Meaning in presto |
|---|---|
| `run` | until settled |
| `step` | one frame on the simulated clock |
| `get_by_*` / `query_by_*` | over the AccessKit snapshot with author ids |
| `click`, `type_text`, `key_press` | injected at the event-time dispatch point, so tests take the production path |
| `render` | lavapipe |

It also offers an `egui::Event`-shaped injection adapter for the transition. The tour live driver
and the pixel tier (`egui_inspection`, the `egui_mcp` tree projection) re-target the same
injection and snapshot API.

### 16.2 Deterministic scheduler tests

`presto-frame`'s simulator is seeded, in the style of Zed's `TestScheduler`. It simulates:
- the display (refresh, phase, jitter, VRR);
- the present engine;
- GPU and UI costs;
- input streams, timer error and visibility.

**Fault cases:** slow JS, a GPU spike, dropped or reordered feedback, a 60→144 Hz switch mid-drag,
hide mid-commit, and two windows at different rates.

The invariants of §1 are checked after every event. Latency is asserted in frames, for example
p99 ≤ 1 frame for native key presses within budget.

### 16.3 Latency instrumentation, built in from the first commit

- **Stamping:** every input gets an `InputSeq` and its OS timestamp, carried as stages from `os` to
  `presented`, named in the style of Chrome's EventLatency. Every sequence terminates exactly once.
- **Histograms:** in-app, per segment and per input class.
- **Trace export:** Chrome JSON with flow arrows (Perfetto), then Tracy.
- **On Windows:** `Intel-PresentMon` ETW events, so PresentMon's click-to-photon and present-mode
  columns line up with our records.
- **External rig:** a Teensy with a light sensor (Open-Source-LDAT firmware), with randomised gaps
  and ≥ 300 samples per configuration. Real milliseconds gate releases; CI gates frames in virtual
  time (`round2/scheduler-latency.md` §5).

### 16.4 wait-idle

`settle` (ARCH-EMBED §5.4) carries over and gains `COMMIT_IN_FLIGHT`. Compositor-only cosmetic
animation (the caret blink) does not count as busy; a running Svelte transition does. The dock's
drag-in-flight becomes a barrier source; today only `tour_busy` reads it.

## 17. Failure handling

| Failure | Handling |
|---|---|
| JS exception in a handler | the existing fault and boundary model (`<svelte:boundary>`, slot fault UI) |
| JS long task | the interrupt watchdog aborts at the budget and reports; compositor animations continue |
| panic in a Rust kind | caught at the kind boundary in debug; the node shows a fault box; the window survives |
| invalid AccessKit update | repaired or dropped by the validating builder (§13) |
| GPU device loss | rebuild from CPU-side sources (§8.5) |
| no hardware GPU | lavapipe or WARP, no MSAA, damage-limited redraw |
| frame-callback starvation (hidden window) | 100 ms timeout marks the window hidden, so it stops rendering (Qt's rule) |

## 18. Corrections to the round-1 proposal

| Round 1 said | Correct reading | Source |
|---|---|---|
| A depth pre-pass cut GPU busy time 0.96 → 0.44 ms (Zed) | That was PR #61461, closed unmerged; measure before adopting | `round2/gpui-port.md` #17 |
| GPUI paces Windows with the compositor clock | GPUI uses `DwmFlush`, `Present(0,0)`, 3 buffers, no waitable; our plan goes beyond it | `round2/gpui-port.md` #7–8 |
| Keymaps resolve in Rust before JS sees a key | IME first while composing; then keymap, kind, JS | `round2/ime-a11y-os.md` §1 |
| svelters-egui has "76 kinds" | 57 tags, 60 kind definitions, 26 svelters-drawn widget structs; cadatomic adds 5 widgets and 4 behaviours | `round2/svelters-inventory.md` |
| No LCD text | Grayscale first; an LCD path planned, gated on screenshots | `round2/text-stack.md` risk 1 |
| The compiler, ABI, loader and svelters-style are untouched | The compiler and runtime change substantially, by design | `compiler-and-js.md` |
| `metadata/style/egui.yaml` | Does not exist; egui's style table is generated by `spec.rs` + `gen_props.py` | `round2/svelters-inventory.md` |
| A wgpu fork for damage only | A four-patch series P1–P4 (§8.6); several needs are met through `as_hal` without patches | `round2/wgpu-integration.md` |
| winit registers raw input always; presto needs a winit fork | Raw input defaults to `WhenFocused` and turns off with `DeviceEvents::Never`; presto starts on winit 0.30.13 plus shims, then own backends; at most a one-patch bridge fork for macOS text input | `round2/winit.md` §2 |
