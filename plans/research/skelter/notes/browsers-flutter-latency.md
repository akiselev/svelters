# Browsers, Flutter and latency research: notes for the cadatomic retained GUI/renderer

Research date: 2026-10-25. Web research only, nothing built or measured.

Grades used below:
- **[doc]**: stated by the cited primary source (the vendor's docs, design doc, source code or the author's own post).
- **[src2]**: from a secondary or third-party source (mirror, press, research note). Treat it as a lead.
- **[inf]**: my inference from the cited material. It is not stated in any source.

Each source carries its date or version. Many Chrome docs are from 2021–2022, so details may have drifted.

---

## 1. Chromium

### 1.1 RenderingNG pipeline (stages and threads)

Source: "RenderingNG architecture", developer.chrome.com, last updated 2021-07-26.
https://developer.chrome.com/docs/chromium/renderingng-architecture

- [doc] Twelve stages: **animate** (mutates computed style and property trees over time), **style**, **layout** (immutable fragment tree), **pre-paint** (computes property trees and invalidates display lists and raster tiles), **scroll** (mutates the scroll offsets in property trees), **paint** (display list), **commit** (copies property trees and the display list to the compositor thread), **layerize** (splits the display list into a composited layer list), **raster / decode / paint worklets** (to GPU texture tiles), **activate** (builds a compositor frame), **aggregate** (Viz merges every visible compositor frame into one global frame), **draw** (the GPU executes it).
- [doc] Threads and processes. The render process main thread runs style, layout, pre-paint and paint, plus the main-thread parts of animate and scroll. The render process compositor thread ("impl") runs input, scrolling, animations, layerization and activation, and coordinates decode, worklets and raster. Viz runs raster and draw on its GPU main thread, and aggregation on its display compositor thread. Viz has **one display compositor thread, kept separate so it can respond even when the GPU main thread is slow.**
- [doc] **Compositor-only fast path:** "visual effects (scroll, transform, clip, filter, opacity, blend) can skip layout, pre-paint and paint, and run entirely on the compositor thread". A transform animation ticks by mutating property trees and re-running only the compositing stages. On scroll, the compositor applies the offset first and tells the main thread afterwards so that it can fire `scroll`.
- [doc] Stages are skipped when not needed, which is why "animation and scroll can be fast even while a main-thread update is in progress".

Source: "BlinkNG", developer.chrome.com, last updated 2022-04-19. https://developer.chrome.com/docs/chromium/blinkng
- [doc] Principles: a uniform entry point (always start at the top of the pipeline); deterministic stages with constant inputs and **immutable outputs**; self-consistent state at the end of each stage; each computation done once. `DocumentLifecycle` enforces the stage order with assertions. Pipeline entry is reduced to two cases: a full update, or a targeted query such as `offsetTop`.
- [doc] Composite-after-paint: layerization moved **after** paint, which removed a circular dependency between invalidation and layer decisions.
- [doc] Lesson: "an operation in the wrong pipeline phase eventually causes stuck dependencies". A clean pipeline is the **prerequisite** for threading work such as Non-Blocking Commit and off-main-thread compositing.

### 1.2 Data structures: property trees, display lists, paint chunks, compositor frames

Source: "Key data structures in RenderingNG", developer.chrome.com, last updated 2021-08-24.
https://developer.chrome.com/docs/chromium/renderingng-data-structures

- [doc] There are four **property trees**: transform (includes scrolling), clip, effect (opacity, filter, mask, blend, clip-path) and scroll (scroll chaining, needed for compositor-thread scrolling). Each element has a 4-tuple "property tree state".
- [doc] **Display items** are low-level draw commands in paint order. Unchanged items are copied from the previous list, and unchanged stacking contexts are skipped entirely.
- [doc] **Paint chunks** are runs of display items that share one property tree state. Layerize merges chunks by default but **keeps separate the chunks whose state will change on the compositor thread** (scroll, transform animation).
- [doc] A paint chunk's transform is separate from its content, so scrolling moves an existing composited layer **without re-raster**. Property trees are much cheaper to send across threads and processes than DOM state.
- [doc] A compositor frame is a list of render passes, and a render pass is a list of quads (tile, solid colour, video or canvas texture, surface reference). It carries no pixels. A **SurfaceDrawQuad** embeds another client's frame by id, and Viz substitutes it at aggregation.

### 1.3 cc (the compositor): commit, pending and active trees, scheduler, damage

Source: "How cc works", chromium.googlesource.com docs/how_cc_works.md (HEAD, read 2026-10-25).
https://chromium.googlesource.com/chromium/src/+/HEAD/docs/how_cc_works.md

- [doc] **Commit blocks the main thread**: "Rather than sending an ipc, commit is done by blocking the main thread and copying data over."
- [doc] **Pending tree / active tree.** The pending tree is "the staging area to wait until all of the asynchronous rasterization work is complete". The active tree is drawn and keeps animating and scrolling while the pending tree rasters. **Activation** pushes the pending tree into the active tree. The old pending tree becomes a recycle tree, which saves reallocation.
- [doc] TileManager raster priority: tiles needed to draw the active tree, then tiles needed to activate the pending tree, then tiles near the viewport, then offscreen image decodes. Tasks cannot be cancelled once they start; unstarted ones are dropped by submitting a new graph. Tiles are about 256×256 for software raster and about viewport width × ¼ viewport height for GPU raster.
- [doc] Scheduler: there are two begin-frame kinds. **BeginImplFrame** asks whether cc should produce a compositor frame; it comes from a `viz::BeginFrameSource` driven by the display compositor. **BeginMainFrame** asks whether Blink should run its update and commit. The full pipeline is "BeginImplFrame -> BeginMainFrame -> Commit -> ReadyToActivate -> Activate -> ReadyToDraw -> Draw".
- [doc] **Deadline**: "The cc::Scheduler maintains a deadline by which it expects its embedder to respond." If main misses it, cc draws without waiting for the commit, carrying only impl-side animation and scroll.
- [doc] **High latency mode**: entered when main is slow. It "trades off latency for throughput by increasing pipelining". The scheduler can skip a BeginMainFrame to catch up and return to low latency mode, using timing history and heuristics.
- [doc] Damage: DamageTracker forwards damage with the CompositorFrame. *Invalidation damage* comes from raster invalidation; *expose damage* comes from layers being added, removed or reordered. **Partial swap** lets the display compositor update only part of the screen, which saves power. With hardware overlays, damage shows that only the overlay changed.

Scheduler details from source, via a third-party research note (grida.co, "Chromium Compositor Scheduler", citing `cc/scheduler/*` line ranges): https://canary.grida.co/docs/wg/research/chromium/scheduler
- [src2] Deadline modes: `IMMEDIATE` (fires as soon as possible when the active tree is ready or no main work is pending), `REGULAR` (`args.deadline`, used when impl has animations but waits for a main commit), `LATE` (`frame_time + interval`, nothing on impl), `BLOCKED` (full-pipeline/headless), `WAIT_FOR_SCROLL` (`frame_time + 0.333·interval`, off by default).
- [src2] Adjusted deadline = `args.deadline − DrawDurationEstimate() − kDeadlineFudgeFactor (1 ms)`.
- [src2] `kMaxPendingSubmitFrames = 1`: at most one compositor frame is in flight to Viz. `main_frame_before_activation_enabled` defaults to false.
- [src2] `main_thread_missed_last_deadline_`: if main missed the deadline but its critical path (BeginMainFrame queue to activation) fits in interval minus draw time, the next BeginMainFrame is sent immediately; otherwise it is deferred "to avoid pipelining that increases latency".
- [src2] At most one BeginMainFrame per frame; on displays above 120 Hz main frames may be throttled to about 60 Hz unless a scroll is blocked on main-thread paint.
- [doc] That `main_thread_missed_last_deadline` is tracked explicitly at the end of the BeginImplFrame deadline is confirmed by the 2015 review https://codereview.chromium.org/1229573004 and the `scheduler_state_machine.h` header.

"Life of a frame" (chromium docs/life_of_a_frame.md, HEAD): https://chromium.googlesource.com/chromium/src/+/HEAD/docs/life_of_a_frame.md
- [doc] The stage list is: BeginFrame → compositor updates → BeginMainFrame → main-thread updates → commit → wait for raster → activation → **wait for deadline** → SubmitCompositorFrame → AggregateSurfaces → draw frame → RequestSwap → wait until ready to draw → queueing delay → GPU draw → swap → presentation. PipelineReporter trace events time each segment.
- [doc] Examples: when the main thread is late, the compositor "reached the deadline and submitted the compositor updates that were ready". Two overlapping reporters' updates can be "submitted to the GPU process and presented together".

### 1.4 Chrome's per-event latency breakdown (EventLatency)

Source: `base/tracing/stdlib/chrome/event_latency_description.sql` (Chromium HEAD, read 2026-10-25).
https://chromium.googlesource.com/chromium/src/+/HEAD/base/tracing/stdlib/chrome/event_latency_description.sql

[doc] Chrome measures **every input event from the OS hardware timestamp to presentation**, split into 38 named stages. The fast (compositor-only) path:
`GenerationToBrowserMain → BrowserMainToRendererCompositor → RendererCompositorQueueingDelay` (waiting in the compositor queue for the BeginImplFrame that will carry the event) `→ RendererCompositorProcessing → …ToSubmitCompositorFrame → SubmitToReceiveCompositorFrame → ReceiveCompositorFrameToStartDraw → StartDrawToSwapStart → Swap (SwapStartToBufferAvailable, BufferAvailableToBufferReady, BufferReadyToLatch, LatchToSwapEnd) → SwapEndToPresentationCompositorFrame`.
The slow path adds `RendererCompositorToMain` ("only on the slow path, usually blocking JS listeners or complex layout"), `SendBeginMainFrameToCommit`, `Commit` (main blocked) and `EndCommitToActivation`/`Activation`. The `…ToTermination` stages record events that were ignored or **merged into another frame** (coalesced).
- [inf] This is the model for our telemetry: give each input event a hardware timestamp and record stage boundaries until the frame that contains its effect is presented (presentation feedback). Report p50/p95/p99 per stage.

### 1.5 Input on the compositor thread

Source: Mariko Kosaka, "Inside look at modern web browser (part 4)", 2018-09-21. https://developer.chrome.com/blog/inside-browser-part4
- [doc] The compositor marks areas with handlers as the **non-fast scrollable region**. Input there goes to the main thread; elsewhere the compositor scrolls without asking main. One delegated handler on `document.body` makes the whole page non-fast scrollable.
- [doc] `{passive: true}` lets the compositor scroll without waiting for the listener. CSS `touch-action` (`pan-x`) declares the allowed gestures up front, with no handler.
- [doc] **Continuous events** (wheel, mousemove, pointermove, touchmove) are **coalesced and dispatched just before the next rAF**. **Discrete events** (keydown/up, mousedown/up, touchstart/end) are dispatched immediately, flushing pending continuous events first so that order is kept.

Source: "Aligning input events", developer.chrome.com, last updated 2017-06-22. https://developer.chrome.com/blog/aligning-input-events
- [doc] Frame-aligned input shipped in **Chrome 60**; `getCoalescedEvents()` in **Chrome 58**. The experiment measured **35% fewer hit tests**. Touch input runs at 60–120 Hz and mice at about 100 Hz (up to 2000 Hz), against 60 Hz displays. Coalesced events are not hit-tested, and preventDefault/stopPropagation on them has no effect.

Source: WICG EventListenerOptions explainer (passive listeners, 2016). https://github.com/WICG/EventListenerOptions/blob/gh-pages/explainer.md
- [doc] Chrome for Android: **80% of touch events that block scrolling never prevent it; 10% of those add more than 100 ms before scrolling starts; 1% of scrolls are delayed by at least 500 ms.** Passive listeners shipped in Chrome 51 and Firefox 49.

Source: "Scrolling intervention", 2017-01-10. https://developer.chrome.com/blog/scrolling-intervention
- [doc] Chrome 56 treats root-target touch listeners as passive by default. The slowest 1% of scroll starts went from just over 400 ms to just over 250 ms (−38%). About 80% of cancelable root touch listeners were conceptually passive.
- [inf] Lesson: **if a handler *may* block a gesture, the platform has to wait for it.** Make non-blocking the default, and require a declared, static opt-in for "this handler may cancel" (as touch-action does).

### 1.6 Prediction

- [doc] `PointerEvent.getPredictedEvents()` (MDN, baseline in all current browsers since about December 2024): https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent/getPredictedEvents
- [doc] Chromium's `input_event_prediction.{h,cc}` (2018 snapshot) chooses a Kalman, least-squares or empty predictor by field trial and generates **3 predicted events at an 8 ms interval**, attached to the coalesced event. It also handles resampling. https://chromium.googlesource.com/chromium/src.git/+/20e8909e694cab857da21d8fe9d5f1da170eaae2/content/renderer/input/input_event_prediction.cc
- [doc] Apple: `UIEvent.predictedTouches(for:)`, documented as the "system's best guess" and to be discarded at the next real event. https://developer.apple.com/documentation/uikit/minimizing-latency-with-predicted-touches
- [src2] WWDC 2019 (iPadOS 13): Pencil latency went from about 20 ms to **9 ms** through input/graphics stack tuning and better prediction. A French report says third-party apps get about 13 ms because roughly 4 ms comes from a private mid-frame event technique. https://www.macstories.net/stories/ios-and-ipados-13-the-macstories-review/23 ; https://iphonesoft.fr/2019/06/25/latence-reduite-apple-pencil-sera-13ms-devs-et-non-9ms
- [inf] Limits of prediction: it only works for smooth continuous motion (drag, ink, pan). Its errors are overshoot at direction changes and stops, and it never applies to discrete events (key, click). Predict at most about one frame ahead (Chrome uses 8 ms × 3), and draw predicted state as provisional, replaced by the next real sample.

### 1.7 Delegated ink and desynchronized canvas (bypassing the pipeline)

- [doc] W3C TAG review #473 "Delegated Ink Trail" (Microsoft, 2020): the app draws the stroke up to the last trusted point and hands that point plus style to the browser. The browser **forwards new points straight to the display compositor / OS**, which extends the trail just before presentation, optionally with prediction. https://tag-github-bot.w3.org/review/473 ; MDN Ink API: https://developer.mozilla.org/en-US/docs/Web/API/Ink_API
- [src2] Microsoft Edge blog (via press): "up to 240%" improvement in latency in early tests on Windows 11 Insider builds. Points go to the Windows compositor "outside of Microsoft Edge's application loop". The work was upstreamed into Chromium. https://windowscentral.com/microsoft-improved-inking-way-240 (Edge blog post https://blogs.windows.com/msedgedev/?p=24844 returned 404 on 2026-10-25)
- [doc] `getContext(..., {desynchronized: true})` (Chrome blog, 2019-05-02) "bypasses the usual DOM update mechanism … skip as much compositing as it is able … in some cases the buffer is sent directly to the screen's display controller", which removes the renderer compositor queue delay. It can tear on front-buffer devices. The post states that more than 50 ms hurts hand-eye coordination. https://developer.chrome.com/blog/desynchronized
- [doc] Android `androidx.graphics.lowlatency` (`CanvasFrontBufferedRenderer`, `GLFrontBufferedRenderer`, API 29+) uses the same split: **active content goes to a front-buffered layer that is presented while it is drawn; the rest goes to a normal multi-buffered layer.** https://developer.android.com/reference/androidx/graphics/lowlatency/package-summary

### 1.8 Viz, GPU raster, OOP-R

Source: "RenderingNG", developer.chrome.com, last updated 2021-06-22. https://developer.chrome.com/docs/chromium/renderingng
- [doc] Milestones: GPU raster everywhere (Mac 2016; Windows, ChromeOS and Android Go 2018); threaded GPU raster 2019; **OOP-R** (raster in the Viz/GPU process) shipped on Android, Mac and Windows in 2018 and everywhere except canvas in 2019; **OOP-D** (out-of-process display compositor) 2019; SkiaRenderer 2019–2021. Threaded scroll and animations date from 2011, image animations moved to the compositor in 2018, and percentage-transform and SVG animations were threaded in 2021. Scrolling "doesn't depend on JavaScript or layout".
- [src2] OOP-R serialises paint ops into the GPU process, which replays them with Skia into tile textures, so raster output never has to be uploaded (Intel, "Software vs GPU rasterization in Chromium"; grida tiling note). https://www.intel.com/content/www/us/en/developer/articles/technical/software-vs-gpu-rasterization-in-chromium.html
- [inf] Sandboxing is the main motive for Chromium's process split, and we do not have that motive. The parts worth keeping are the *thread* split (main versus compositor), the immutable hand-off (commit), and a frame-aggregation point that owns presentation.

### 1.9 INP (the responsiveness metric)

Source: web.dev "Interaction to Next Paint", published 2022-05-06, updated 2025-09-02. https://web.dev/articles/inp
- [doc] INP splits an interaction into **input delay** (until the first handler runs), **processing duration** (all handlers) and **presentation delay** (handlers done until the frame is presented). Thresholds at p75: good ≤ 200 ms, poor > 500 ms. It counts clicks, taps and key presses, not hover or scroll.
- [inf] 200 ms is a *web* floor, much too lax for an IDE. Our target is one to two frames (below).

---

## 2. Firefox WebRender

Source: Lin Clark, "The whole web at maximum FPS: How WebRender gets rid of jank", Mozilla Hacks, 2017-10-10.
https://hacks.mozilla.org/2017/10/the-whole-web-at-maximum-fps-how-webrender-gets-rid-of-jank/
- [doc] The original thesis: **repaint the whole screen every frame like a game engine**, merging paint and composite, and dropping invalidation rectangles and retained layers. Gecko layout sends an API-neutral **display list**.
- [doc] The render backend turns it into batched GPU draws: early culling; a **render task tree** that orders intermediate targets so the fewest offscreen targets are needed; **batching** (aim for at most about 100 draw calls per frame on desktop); **opaque pass front-to-back with the depth buffer (z-culling)**, then the translucent pass back-to-front.
- [doc] Glyphs are rasterised on the CPU (work-stealing, in parallel) into a **GPU glyph cache that persists across frames**. The goal is a steady 60+ fps whatever the display size or the amount of change.

Threads (from Servo/WebRender API docs, 2024–2026 builds): https://doc.servo.org/webrender/scene_builder_thread/struct.SceneBuilderThread.html , https://doc.servo.org/webrender/render_api/struct.Transaction.html
- [doc] There are three threads: the **scene builder** (heavy display-list → scene work, off the critical path), the **render backend** (holds the latest built scene and a retained frame builder, and builds frames), and the **renderer** (GL/GPU submission). A **Transaction** is applied atomically. `scene_ops` go through scene building; `frame_ops` (scroll offsets, async property updates for APZ, async images) **skip the scene builder** (`use_scene_builder_thread = false`). `AsyncPropertySampler` samples animated properties on the backend thread for each frame-generating transaction.
- [inf] This matches Chrome's main/impl split: "scene" changes are slow and asynchronous, while "frame" changes (scroll, transform, opacity) are fast and bypass rebuilding.

What they had to add later:
- [doc] **Picture caching** (Mozilla GFX blog, 2018-11-02): "re-render everything" struggled with very complex content (a pure-CSS oil painting was too heavy to redraw every scroll frame), and power mattered. A picture is cached only when a cost model says it is worth it, and the switch between cached and uncached is cheap because paint and composite are not separate stages. https://mozillagfx.wordpress.com/2018/11/02/webrender-picture-caching/
- [doc] `picture.rs` module docs (current WebRender): the scene is cut into **a few slices** (content, UI, background UI). Each slice is **fixed tiles of 2048×512 px (128×128 for UI)**. A tile is either cached raster content or a solid "clear tile". Each tile tracks its primitive dependencies in a **quadtree**; the union of invalidated leaves gives a per-tile **dirty rect**, used as a scissor when replaying and for **partial present**. Large or frequently changing primitives (video, canvas) can be promoted to **native compositor surfaces** so they update without invalidating tiles; tiles above them are drawn as alpha overlays. https://doc.servo.org/src/webrender/picture.rs.html
- [doc] Compositor configurations (`CompositorConfig`): **Draw** (WebRender composites the tiles itself, with optional partial present) or **Native** (the OS compositor composites the tiles, through a `Compositor` trait; "can save power"). https://doc.servo.org/webrender/enum.CompositorConfig.html
- [doc] macOS Core Animation integration (Mozilla GFX blog, 2019-10-22, Firefox 70). Before it, `NSOpenGLContext flushBuffer` could not say what changed, so Firefox **redrew the whole window on each change** and the window server copied the whole window as well. On integrated GPUs compositing is bandwidth bound: about **3 ms of full-frequency GPU time per screenful** (about 28 MB per screen at 1680×1050@2x across three passes). The fix: Firefox owns its swapchain, renders into IOSurface-backed **tiled CALayers**, separates opaque from transparent layers, uses CATransaction for atomic multi-layer updates, and redraws only invalid pixels. **Power fell 42–91%** (scrolling 16.4→9.4 W; Google Docs idle 7.4→1.6 W; loading animation 19.4→1.8 W; MacBook Pro 2013, HD 4000). https://mozillagfx.wordpress.com/2019/10/22/dramatically-reduced-power-usage-in-firefox-70-on-macos-with-core-animation/
- [inf] The lesson from WebRender: "redraw everything fast" is the right *fallback* and removes layerization cliffs, but you still need (a) retained tiles with dependency-tracked invalidation, (b) partial present, and (c) OS-compositor surfaces for video and 3D, both for power and for very complex content. Damage tracking is not optional for near-zero idle cost.

---

## 3. Flutter

### 3.1 Trees, layout and build

Source: "Inside Flutter", docs.flutter.dev, updated 2026-05-05 (Flutter 3.47). https://docs.flutter.dev/resources/inside-flutter
- [doc] Widgets are immutable configuration. **Elements** persist across frames and hold structure and state. **RenderObjects** do layout and paint. The build phase visits only dirty elements, each at most once per build, and an identical widget instance skips its subtree. Child lists are reconciled with an **O(N)** algorithm by runtime type and key, with no tree diffing.
- [doc] Layout: "constraints go down, sizes go up". "**One layout per frame, single pass**"; each render object is visited at most twice (down and up). Initial layout is linear and relayout is sublinear: an object whose layout is clean and whose constraints are unchanged returns immediately ("cutting off the walk").
- [doc] Relayout boundary (implementation of `RenderObject.layout`): `_isRelayoutBoundary = !parentUsesSize || sizedByParent || constraints.isTight || parent == null`. `markNeedsLayout` on a non-boundary propagates to the parent; a boundary registers itself with the PipelineOwner. https://api.flutter.dev/flutter/rendering/RenderObject/layout.html , https://api.flutter.dev/flutter/rendering/RenderObject/markNeedsLayout.html
- [doc] **RepaintBoundary** "creates a separate display list for its child". `markNeedsPaint` walks up to the nearest repaint boundary. The engine may rasterise and cache a static complex subtree. Debug aids are `debugRepaintRainbowEnabled` and `debugProfilePaintsEnabled`. https://api.flutter.dev/flutter/widgets/RepaintBoundary-class.html
- [doc] Architectural overview (updated 2026-10-24, Flutter 3.47): build → layout → paint → composite; `RenderView.compositeFrame()` builds a scene with `SceneBuilder` and `Window.render()` hands it to the engine. https://docs.flutter.dev/resources/architectural-overview

### 3.2 Threads, layer tree and pipelining

Source: "The Engine architecture", flutter/flutter docs/about (master, read 2026-10-25). https://github.com/flutter/flutter/blob/master/docs/about/The-Engine-architecture.md
- [doc] "The Flutter engine does not create or manage its own threads"; the embedder supplies four task runners: **platform** (the OS main thread), **UI** (Dart: animate, build, layout, paint into a **layer tree**, semantics), **raster** (takes the layer tree, issues GPU commands, owns surfaces) and **IO** (image decode and upload, so raster doesn't do it). The layer tree is "client-rendering-API agnostic".
- [doc] Vsync: the root isolate requests a frame, the engine waits for the next vsync, and **pipelining lets the raster thread work on frame N while the UI thread prepares N+1**. The pipeline stops UI from queueing too much work.
- [doc] Pipeline depth, from current `shell/common/animator.cc` (master): `FramePipeline(2)` on Metal; otherwise **1 if the platform and raster runners are the same thread, else 2**; `pending_frame_semaphore_(1)` allows one outstanding vsync request. When the pipeline is full, BeginFrame requests another vsync and returns ("consumer is being too slow"). https://github.com/flutter/flutter/blob/master/engine/src/flutter/shell/common/animator.cc
- [doc] History: engine commit 2cdbc7f (2020-05-14) replaced the depth-2 pipeline with a single-slot "layer tree holder" (newest by target time wins, stale trees dropped). Its removed docs say a depth change from 1 to 2 made "a frame generated for one interval land two intervals later". Current master has the depth-2 `FramePipeline` again, so the holder was later reverted or replaced. https://dart.googlesource.com/external/github.com/flutter/engine/+/2cdbc7f92793125d9312138877c05af6ad708683%5E%21/
- [inf] **The latency cost of Flutter's pipelining:** with depth 2 and separate UI and raster threads, an input that lands just after vsync waits up to one frame for the next BeginFrame, then one frame of UI work, then one frame of raster, then the OS compositor and scanout. That is roughly 3–4 frames worst case, the same shape as Levien's "physics / record / GPU each a frame". Flutter trades latency for throughput and has no compositor-only input path: scrolls and animations run in Dart on the UI thread.
- [doc] **Thread merge:** Flutter 3.29 merged the UI and platform threads on iOS and Android (Dart runs on the platform thread). 3.33/3.35 did the same on macOS and Windows (breaking-change page updated 2026-05-05), and Linux followed. The rationale: the split thread prevented synchronous FFI calls into platform-thread-only native APIs. A forum summary of issue #150525 says the UI/platform state replication was "eventually consistent" and hard to get right. https://docs.flutter.dev/release/breaking-changes/macos-windows-merged-threads , https://forum.itsallwidgets.com/t/how-does-flutter-s-ui-platform-thread-merge-impact-platform-api-calls/2653 [src2 for the forum]
- [inf] Lesson: **putting the framework/scripting thread on a different thread from the OS event thread costs both latency and correctness** (state replication, resize sync). Flutter went back to one thread for the OS and app logic, while raster stays separate.

### 3.3 Impeller

Sources: https://docs.flutter.dev/perf/impeller (updated 2026-08-21, Flutter 3.47); Impeller README https://github.com/flutter/flutter/blob/master/engine/src/flutter/impeller/README.md
- [doc] Impeller "precompiles a smaller, simpler set of shaders at engine-build time so they don't compile at runtime", "builds all pipeline state objects upfront", and "the engine controls caching and caches explicitly". It can spread a single frame's work across threads and labels resources for capture.
- [doc] It is the default on iOS (the only engine there), on Android API 29+ (Vulkan, since 3.27), and on **macOS, Linux and Windows as of 3.47**. The web still uses Skia.
- [doc] docs.flutter.dev/perf/shader: first-run animation jank on mobile is avoided by using Impeller. The old Skia path compiled shaders (SkSL) at first use, which caused first-run jank.
- [inf] Lesson for wgpu: **create every render pipeline at startup or load time** (a small fixed set of uber-shaders for UI primitives), never lazily on the first draw of a new primitive or blend mode. Use a pipeline cache where the backend supports it.

### 3.4 Pointer resampling

Sources: https://api.flutter.dev/flutter/gestures/GestureBinding/resamplingEnabled.html ; PR #60558 (merged 2020-08-24) https://github.com/flutter/flutter/pull/60558
- [doc] `resamplingEnabled` (off by default) resamples touch positions at `frameTime + samplingOffset` (the offset should be negative) for "smoother event processing at the cost of some added latency". It helps when input and display rates don't divide evenly (e.g. 120 Hz input, 90 Hz display). The PR suggests an offset of about **−22 ms** for about 65 Hz touch input.
- [inf] Resampling buys smoothness by adding latency on purpose (sampling in the past to interpolate). For a mouse at 500–8000 Hz on a desktop IDE it is not needed. Coalesce to the newest sample and keep the history for ink or drag tools.

### 3.5 Desktop embedder notes

- [doc] Windows embedder: renders through ANGLE (GL→D3D11). Resize blocks the platform thread **up to 100 ms** waiting for a frame at the new size. Vsync goes through `eglSwapInterval`. https://api.flutter.dev/windows-embedder/flutter__windows__view_8cc_source.html
- [doc] Performance overlay: at 60 fps there is about 16 ms per frame, and both threads must fit. https://docs.flutter.dev/perf/ui-performance
- [inf] Resize synchronisation (the OS waiting for a correctly sized frame) is a classic desktop embedder problem. Plan for it: a synchronous "render at new size now" path, bounded by a timeout.

---

## 4. Latency research, swapchains and frame pacing

### 4.1 Perception thresholds

- [src2] Ng, Lepinski, Wigdor, Sanders, Dietz, "Designing for low-latency direct-touch input", UIST 2012 (DOI 10.1145/2380116.2380174), as cited by later papers and patents: commercial touchscreens had **50–200 ms** latency; with a 1 ms prototype, users could perceive **below 10 ms** for dragging; a reported floor is **about 6 ms (range 2–11) for dragging** and about 24 ms for tapping (Jota et al. CHI 2013 line of work). Sources: TriboTouch paper https://karan-ahuja.com/assets/docs/paper/tribotouch.pdf ; Jota et al., "How fast is fast enough?", CHI 2013, DOI 10.1145/2470654.2481317.
- [src2] Microsoft Applied Sciences demo (March 2012, Paul Dietz): typical touch latency was about 100 ms; at 10 ms lag is still visible; at 1 ms the object moves with the finger. At 1 m/s, 100 ms of lag means a 10 cm gap. https://onmsft.com/?p=17654 (the Microsoft blog URL now returns 410)
- [src2] Deber, Jota, Forlines, Wigdor, "How much faster is fast enough?", CHI 2015 (DOI 10.1145/2702123.2702300), as cited: JND for **direct dragging is about 11 ms**, against **55 ms indirect dragging (touchpad/mouse-like)**, 69 ms direct tapping and 96 ms indirect tapping. Users also notice *improvements* of a few tens of ms even above the JND.
- [src2] Practical targets cited in the TriboTouch paper: performance gains flatten beyond about **25 ms for dragging and about 50 ms for tapping**, and **20 ms** is recommended as a good trade-off.
- [inf] For a desktop IDE with a mouse (indirect), the hard floor is about 50 ms for noticing. But typing and dragging in an editor or canvas is compared directly against fast native apps and terminals (5–30 ms, below), so the target should be **under 20 ms p50 and under 33 ms p99 at 120 Hz**.

### 4.2 Measured end-to-end latency

Source: Dan Luu, "Computer latency: 1977–2017" (measurements 2017). https://danluu.com/input-lag/
- [doc] Keypress-to-screen with a 240/1000 fps camera: **Apple IIe 30 ms** (the fastest); a custom Haswell-E rig **50 ms at 165 Hz, 60 ms at 120 Hz, 80 ms at 60 Hz, 140 ms at 24 Hz**; typical modern machines 70–200 ms. iPad Pro with Pencil: 30 ms scroll.
- [doc] Fitting the refresh-rate series implies about **2.5 frames of buffering regardless of refresh rate**, plus about 36 ms of fixed cost at an infinite refresh rate. Cross-process handoffs (kernel → input daemon → window server → app → render server) dominate, not CPU work. Monitors advertising 1 ms can take about 10 ms for a full colour change.

Source: Dan Luu, "Keyboard latency" (2017, with a 2022 update). https://danluu.com/keyboard-latency/
- [doc] Keyboard-to-USB latency is **15 ms (Apple Magic, USB) to 60 ms (Logitech K360)**. USB polling at 8 ms adds about 4 ms on average, at 1 ms about 0.5 ms. Matrix scan at 100–200 Hz, debounce (often 5 ms, avoidable) and key travel (≥10 ms) dominate. Gaming keyboards were not faster.

Source: Dan Luu, "Terminal latency" (about 2017, macOS 10.12.5). https://danluu.com/term-latency/
- [doc] Median idle latencies: terminal.app 6 ms, emacs-eshell 5 ms, emacs-term 13, st 25, alacritty 31, hyper 32, iterm2 44 ms; p99.9 under load up to 111 ms (st). This is a software-side measurement.
- [inf] **Fast native text widgets reach about 5–15 ms above the OS floor.** GPU-heavy terminals (alacritty) were *slower* than the OS text path, which shows that GPU rendering alone does not buy latency; the scheduling and buffering do.

Source: Apple Developer Forums thread 841189 (August 2026). https://developer.apple.com/forums/thread/841189
- [src2] External keystroke-to-pixel measurement on macOS 15/26. Launching the same app via LaunchServices (`open`) gives about **one extra refresh** of latency compared with exec'ing the Mach-O directly (PyQt6 at 60 Hz: 29.7 vs 16.4 ms average; Xcode on M4 macOS 26: 35.8 vs 23.6 ms). Zed, Sublime and BBEdit show the same direction. There is no Apple reply.
- [inf] OS-level presentation scheduling can add a whole frame that the app cannot see from inside. **Measure externally** (a photodiode or high-speed camera) and keep a latency test harness for each OS.

### 4.3 Compositors, swapchains and pacing (Raph Levien)

Source: "The compositor is evil", 2020-09-13. https://raphlinus.github.io/ui/graphics/2020/09/13/compositor-is-evil.html
- [doc] The compositor adds **about 1.5 frames**. In a standard pipeline at 60 Hz a keypress takes about 33 ms best case, 66 ms worst case and **about 50 ms mean**. Render, composite and scanout each get a vsync-aligned slot, and input is snapshotted at render start. The bottom of the screen is later than the top (scanout order).
- [doc] Hardware overlays bypass composition. Windows DirectFlip promotes one window by heuristic, and promotion/demotion (a context menu opening) can jank. Smooth resize is hard (DX12 flip-model artifacts). The proposed future is beam-racing compositors and per-tile hardware composition.

Source: "Swapchains and frame pacing", 2021-10-22. https://raphlinus.github.io/ui/graphics/gpu/2021/10/22/swapchain-frame-pacing.html
- [doc] On hardware twice as fast, latency got **8 ms worse**, because a blocking clear and a finished frame waiting in the back buffer each add about half a frame of slack.
- [doc] Triple buffering lowers latency in the limit but wastes GPU power, causes thermal throttling and adds jitter, so it is not recommended. Pipelining physics, record and GPU across threads costs **about 50 ms more than an N64 on hardware 100× faster**.
- [doc] Android trace example: `acquireNextImage` blocked 7 ms, waiting for the GPU to start took 4 ms, GPU work took about 5 ms, and there were 9 ms of slack before the compositor latch, for about **37 ms input-to-scanout**. Starting 17 ms later would have hit the same deadline at about **17 ms**.
- [doc] **The ideal frame start is the deadline minus a high-percentile estimate of render time** (the post uses the 99.99th percentile, about one jank per minute). There are two tactics: a fixed-latency waitable swapchain (DXGI `SetMaximumFrameLatency`: 1 if simulation and render fit in one frame, 2 for most games, 3 trades latency for fewer drops), or an **adaptive control loop driven by presentation feedback** (`VK_GOOGLE_display_timing`, now `VK_EXT_present_timing`; DXGI `GetFrameStatistics`; Windows 11 composition swapchain).
- [doc] Animation timebase: a steady 30 Hz looks smoother than a 16.6/33.3 ms mix, so animate against the *predicted present time*, not wall time at frame start. Watch the feedback hazard of GPU clock scaling (a controller that locks in low clocks).
- [doc] Tools: PresentMon/ETW (Windows), Perfetto/systrace + AGI (Android), Instruments (macOS), keyboard-to-photon testers.

Source: "Towards a unified theory of reactive UI", 2019-11-22. https://raphlinus.github.io/ui/druid/2019/11/22/reactive-ui.html
- [doc] UI is a **pipeline of tree transformations** (app data → widget tree → render objects → layout → draw tree, then optionally layers). Each stage is either a stored tree or a trace. Incrementality means keeping deltas small down the pipeline; the hard part is dependency tracking. Identity comes from object reference, key path or unique id. The post is sceptical of relying on OS compositor layers long term.

Source: "xi-editor retrospective", 2020-06-27. https://raphlinus.github.io/xi/2020/06/27/xi-retrospective.html
- [doc] "Adding async made everything more complicated." Live resize with word wrap gave races and tearing between edits and rewraps. Scrolling took months because the front end had to query the core for out-of-cache text. The front-end/core process split was a mistake. Plug-ins in other processes (syntax highlighting, LSP-like) were fine because they were asynchronous, never on the typing path, and reconciled with OT-style revision tagging. JSON across the boundary was slow (Swift) and Serde bloated the code.
- [inf] Lesson for svelters/QuickJS: **the data the renderer needs to draw the next frame must be in-process and synchronous for the renderer.** JS may run asynchronously and late, but its outputs must be *revisioned* so that a stale JS result can never paint over newer direct-manipulation state.

### 4.4 Zed / GPUI

Source: Antonio Scandurra, "Leveraging Rust and the GPU to render user interfaces at 120 FPS", zed.dev, 2023-03-07. https://zed.dev/blog/videogame
- [doc] There are **8.33 ms per frame at 120 Hz**. Atom/Electron missed frames on GC and DOM relayout. GPUI uses one shader per primitive (rects with an SDF for rounded corners, shadows by the Evan Wallace closed form, glyphs from an alpha atlas with up to 16 subpixel variants packed with etagere, icons, images). The CPU only describes primitives; layout follows Flutter (constraints down, sizes up); paint fills a platform-neutral `Scene` of layers; text is shaped by CoreText and **cached per (text, font)**, so only changed text is reshaped.
- [doc] Zed commit #7583 replaced `CADisplayLink` with `CVDisplayLink` (fixing 60 fps on ProMotion and main-thread saturation). Zed's Windows backend later went back to `DwmFlush` for vsync after trying the DirectComposition compositor clock ("DwmFlush does various things that aren't just waiting for VSync, so it's not ideal"). https://git.secluded.site/zed/commit/67b96b2b4019e7a445518ac7308db6d27c777dc5 , https://git.secluded.site/zed/commit/2d34a4aec90823017566d74c6557409580ce6688 [src2 mirror]
- [doc] CVDisplayLink (thume.ca, 2017-12-10, with correction): on a single display it reads real vblank times, and the callback fired about 7 ms before vsync. On some multi-display setups it falls back to a free-running timer whose phase depends on start time and **can add nearly a frame of latency**; bind it to the window's current display. https://thume.ca/2017/12/09/cvdisplaylink-doesnt-link-to-your-display/

### 4.5 Just-in-time frame start (deadline scheduling)

- [doc] Chrome: BeginImplFrame waits until a deadline (`deadline − draw estimate − 1 ms`) for main, then draws impl-only updates (§1.3).
- [doc] DXGI waitable swapchain (Microsoft Learn, ms.date 2017-02-08, updated 2025-06-10): with plain flip-model Present, "the game is always waiting almost a full extra frame between the time it renders and the time it presents". The fix is to create the swapchain with `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT`, **wait on the object before processing input and rendering**, and keep the default per-swapchain latency **1** ("least possible latency … only render after each VSync, minimizing power"), or use 2 if CPU+GPU > 1 frame but each < 1 frame. https://learn.microsoft.com/en-us/windows/uwp/gaming/reduce-latency-with-dxgi-1-3-swap-chains
- [doc] "For best performance, use DXGI flip model" (updated 2026-04-15): DirectFlip, DirectFlip with panel fitters, and **DirectFlip with multi-plane overlays**; "Independent Flip" lets DWM sleep and gets windowed apps **down to 1 frame of latency**. `ALLOW_TEARING` goes lower still. "The first time you ask the GPU to write to the swapchain back buffer is the time that the GPU will stall … delay this point as far into the frame as possible." PresentMon shows which mode is in use. https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model
- [doc] Android `Choreographer.FrameTimeline` (API 33): `getDeadlineNanos()` (when the frame must be ready) and `getExpectedPresentationTimeNanos()`, with several candidate timelines per vsync callback. https://developer.android.com/reference/android/view/Choreographer.FrameTimeline
- [doc] Weston `repaint-window` defaults to **7 ms**: the compositor composites about 7 ms before vblank, which bounds when a client commit is still picked up. https://man.archlinux.org/man/weston.ini.5
- [doc] Wayland Protocols 1.38 added **fifo-v1** and **commit-timing-v1** ("present as closely as possible to, but not before, a specified time"); Mutter merged them in January 2025 (GNOME 48). https://www.phoronix.com/news/Wayland-Protocols-1.38 , https://gitlab.gnome.org/GNOME/mutter/-/merge_requests/3355
- [doc] NVIDIA Reflex (2020-09-01): "eliminating the GPU render queue and reducing CPU back pressure", so the CPU samples input and simulates **just in time** before the GPU needs the frame. Up to 33% lower PC latency on a GTX 1660 Super. A 12 ms vs 20 ms system latency gave a 182 ms faster aiming-task completion. Most gamers then ran at 50–100 ms system latency. https://www.nvidia.com/en-us/geforce/news/reflex-low-latency-platform/
- [src2] Croteam, Alen Ladavac, "The Elusive Frame Timing", GDC 2018: micro-stutter at a "60 fps" average comes from simulating with frame *start* times instead of actual *present* times; the fix uses present timing (`VK_GOOGLE_display_timing`). https://www.phoronix.com/news/Frame-Timing-Game-Stuttering

### 4.6 wgpu and egui defaults (relevant to today's sluggishness)

- [doc] wgpu-types 30.0.1 `SurfaceConfiguration::desired_maximum_frame_latency`: the default is **2** (via `get_default_config`). The docs say "1 minimises latency … suits small, low-latency workloads such as GUIs". Vulkan uses `latency+1` images; DX12 calls `SetMaximumFrameLatency(latency)`; Metal sets `maximumDrawableCount = latency+1`; **GL ignores it**. Mailbox with latency 2 caps at 2× refresh on Vulkan/Metal. https://docs.rs/wgpu-types/latest/wgpu_types/struct.SurfaceConfiguration.html
- [doc] `PresentMode::Fifo`: "frames are kept in a FIFO queue approximately 3 frames long" and `get_current_texture()` blocks for a slot. Mailbox is a single-frame replace queue (DX12 on Win10, NVIDIA on Vulkan, Wayland on Vulkan). Immediate tears. https://docs.rs/wgpu-types/latest/wgpu_types/enum.PresentMode.html
- [doc] egui-wgpu 0.37.0 `SurfaceConfig { present_mode, desired_maximum_frame_latency: Option<u32> }`, where `None` means wgpu's default (2). There are presets `LOW_LATENCY` and `HIGH_THROUGHPUT`. https://docs.rs/egui-wgpu/latest/egui_wgpu/struct.SurfaceConfig.html
- [inf] Our measured "2–3 frames of present latency" fits wgpu Fifo + latency 2, plus egui handling events on frame N and running JS handlers on N+1. The quick wins, independent of the new framework: latency 1, a waitable/acquire-late loop, and same-frame JS handling for discrete input.

---

## 5. Typical stage costs (budget inputs)

| Stage | Typical cost | Grade / source |
|---|---|---|
| Keyboard (scan + debounce + travel) | 15–60 ms (keypress → USB) | [doc] Luu keyboard-latency |
| USB polling | 0.5 ms (1 kHz) – 4 ms (125 Hz) average | [doc] Luu keyboard-latency |
| Mouse | about 100 Hz typical, up to 2000 Hz (8 kHz gaming now) | [doc] Chrome aligning-input (2017) |
| OS input stack → app thread | sub-ms CPU, but cross-process handoffs and scheduling dominate | [doc] Luu input-lag (qualitative) |
| Waiting for the next frame start (input arrives at a random phase) | 0–1 frame, mean 0.5 (4.2 ms at 120 Hz) | [inf] |
| App work | budget about 8.33 ms at 120 Hz in total | [doc] Zed |
| GPU render | app-specific, 1–5 ms typical for UI | [inf] |
| OS compositor (DWM/WindowServer/Mutter) | +1 frame composed; about 0 with independent flip / overlay | [doc] MS flip model; [doc] Levien ("1.5 frames" incl. scheduling) |
| Weston-style repaint window | composites about 7 ms before vblank | [doc] weston.ini |
| Scanout | 0–1 frame, top to bottom (mean about 0.5 frame at mid-screen) | [doc] Levien compositor-is-evil |
| Panel response | about 1–10 ms (a "1 ms" panel took about 10 ms for a full change) | [doc] Luu input-lag |
| Typical end-to-end, 2017 desktops | 70–200 ms keypress-to-photon | [doc] Luu |
| Good native text widget (software-side) | 5–15 ms | [doc] Luu term-latency |

---

## 6. Lessons and recommendations for the cadatomic framework

(All [inf], derived from the material above.)

### 6.1 Threads

1. **Main/UI thread = OS event thread + scene owner + layout + paint-record** (Flutter's 2025 merge; xi's lesson). Do not put the Rust framework on a different thread from winit/OS events.
2. **Render/compositor thread** (Chrome impl / WebRender render backend + renderer): it owns the GPU queue, swapchain, retained layer/tile cache and property trees. It can apply **scroll offsets, transforms, opacity, viewport camera matrices, caret blink, hover highlights and running animations** without the main thread (Chrome compositor-only and WebRender `frame_ops`).
3. **JS (QuickJS) thread**, off the critical path, like WebRender's scene builder. Handlers produce *scene transactions* tagged with the input sequence number and scene revision they saw. Transactions are applied atomically on main, and are discarded or rebased if superseded (xi's revisioning lesson).
4. **Worker pool**: text shaping, glyph raster, image decode, tessellation, kernel jobs (Flutter IO runner; Chrome raster workers).

### 6.2 Pipeline stages (immutable outputs, BlinkNG-style)

input → (main) dispatch/hit-test → state/style → layout (single pass, constraints down / sizes up, relayout boundaries by Flutter's rule) → pre-paint (property trees: transform/clip/effect/scroll) → paint (display items in paint chunks keyed by property tree state; unchanged chunks reused) → **commit** (a short copy or Arc-swap of changed chunks + property trees to the render thread) → (render) layerize (keep chunks whose properties animate or scroll separate) → raster/tiles (retained, dependency-tracked dirty rects as in WebRender) → draw → present (partial present / damage).
A lifecycle state machine should assert the order, and there should be one entry point.

### 6.3 Frame scheduling (the key to latency)

1. **Demand-driven**: no frame unless something is dirty or animating, so idle costs about 0 CPU (Chrome's "BeginMainFrame not expected", Flutter's `RequestFrame`).
2. **Just-in-time start**: begin frame work at `deadline − p99(frame cost) − margin`, not at vsync. The deadline is the OS latch: DXGI waitable object, `CAMetalLayer`/`CADisplayLink` target timestamp, Wayland frame callbacks + presentation-time/commit-timing, Weston's 7 ms window. Learn the cost estimate online from presentation feedback (Levien tactic 2).
3. **Swapchain**: `desired_maximum_frame_latency = 1`; acquire the surface texture **as late as possible**; Fifo (or Mailbox where available and the GPU is idle-cheap). Never let frames queue.
4. **Two deadlines**, as in Chrome: the render thread's deadline is final. If main (or JS) has not committed by then, draw with impl-side updates only (scroll, camera, animations) and switch the main side into a "high latency" mode only after repeated misses.
5. **Sample input at the last moment**: just before draw, the render thread pulls the newest pointer position and scroll delta, *late latching* for scroll and viewport-camera. Main-thread input is processed at frame start, at the latest point before layout.
6. **Animate against predicted present time**, not frame start time (Croteam, Levien).
7. Build every wgpu pipeline at startup (Impeller).

### 6.4 Input

- **Discrete events** (key, button, wheel-notch start) dispatch immediately on main. If the effect can be shown in the current frame budget, do it now, and wake a frame immediately (render "after input" without waiting for the next tick).
- **Continuous events** (pointer move, wheel, touch) are coalesced to one per frame, delivered just before layout. Keep the full history for ink, drag and selection tools (`getCoalescedEvents`), and give a short prediction (≤ one frame) to tools that opt in.
- **Compositor-side gesture handling**: scrolling of scroll containers, canvas pan/zoom and viewport orbit run on the render thread by default. JS or Rust handlers that might *cancel* a gesture must be declared statically (touch-action / passive analogue), and only regions with such handlers fall back to the main thread ("non-fast scrollable region").
- **JS handlers are passive by default.** Visible feedback for direct manipulation (press state, drag ghost, caret, text insertion into the editor buffer) is produced in Rust in the same frame. JS's result lands later as a revisioned transaction.
- **Delegated-ink-style** shortcuts for the hottest loops: the code editor's insertion and caret path, and cursor-following overlays in PCB and schematic routing, which the render thread extends from the newest input sample.

### 6.5 Compositor-side fast paths to provide

Scroll offset; transform/opacity animations; 3D viewport camera (re-render the viewport only, other tiles reused); caret blink; hover/selection overlays as separate layers; video/3D viewports as **separate OS compositor surfaces where available** (WebRender native compositor; DirectComposition / CALayer / Wayland subsurfaces), so their updates don't invalidate UI tiles and can use overlay planes.

### 6.6 Latency budget (120 Hz, 8.33 ms frames), keypress/drag → photon

| Segment | Target |
|---|---|
| Device + OS → app (mouse 1 kHz; keyboard is mostly beyond our control) | 1–2 ms (+ keyboard hardware) |
| Wait for frame work to start (JIT start ⇒ input usually caught by the next frame) | 0–8.3 ms, mean ≤ 4 ms |
| Main: dispatch + layout + paint record (incremental) | ≤ 2 ms p99 |
| Commit to render thread | ≤ 0.2 ms |
| Render thread: raster dirty tiles + draw + submit | ≤ 3 ms p99 |
| GPU → latch slack (safety margin) | 1–2 ms |
| OS compositor | 0 (independent flip/overlay) to 1 frame (8.3 ms) |
| Scanout to mid-screen + panel | about 4 ms + 1–5 ms |
| **Total (software, excl. keyboard)** | **p50 about 12–18 ms; p99 ≤ 25–30 ms** |
| JS-dependent visual change | +1 frame allowed (≤ 33 ms p99); never blocks direct-manipulation feedback |

For comparison: today (egui + eframe Fifo latency 2 + JS on the next frame) is about 3–5 frames, or 25–40 ms at 120 Hz and 50–80 ms at 60 Hz, plus the OS compositor [inf].

### 6.7 Measurement (from Chrome EventLatency)

Timestamp every input at the OS event (hardware timestamp where available), give it a sequence number, propagate it through dispatch → transaction → commit → draw → present, and use **presentation feedback** to close the loop. Report per-stage p50/p95/p99 histograms in the app's diagnostics and in scenarios. Validate externally per OS (a photodiode or high-speed camera): the macOS LaunchServices frame (§4.2) shows that in-app timestamps can miss a whole frame.
