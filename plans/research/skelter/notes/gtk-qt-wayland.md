# GTK4, Qt Quick/Widgets, the Wayland/Linux present path, and Wt: mechanisms and numbers for a low-latency retained GUI

Research notes for cadatomic's own retained GUI framework and renderer, which will replace egui 0.37.
Gathered 2026-10-25 by a research subagent, from web and primary sources (no code was built).

**Grades.** **[doc]** means the source states it: API docs, a protocol XML, a blog post by the
developers. **[src]** means I read it in the source file named, on its main/dev/master branch as of
2026-10-25. **[inf]** means my inference from the cited material. **[mem]** means background knowledge
I could not re-fetch today (marked so it can be checked).

Source file URLs are the raw URLs I fetched on 2026-10-25:
- GTK: `https://gitlab.gnome.org/GNOME/gtk/-/raw/main/<path>`
- Qt: `https://raw.githubusercontent.com/qt/<module>/dev/<path>`
- Mutter: `https://gitlab.gnome.org/GNOME/mutter/-/raw/main/<path>`
- KWin: `https://invent.kde.org/plasma/kwin/-/raw/master/<path>`
- Mesa: `https://gitlab.freedesktop.org/mesa/mesa/-/raw/main/<path>`
- wayland-protocols: `https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/<path>`
- wgpu: `https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/<path>`
- winit: a shallow clone of `github.com/rust-windowing/winit` at `0c73728` (2026-10-24).

---

## 1. GTK 4

### 1.1 GdkFrameClock: phases and pacing

- **Phases [doc].** The frame clock has these signals, in order:
  - `flush-events`: motion events "being batched up and compressed together";
  - `before-paint`;
  - `update`: "the first step of toolkit and application processing", where animations advance;
  - `layout`;
  - `paint`;
  - `after-paint`: "ends processing of the frame";
  - `resume-events`: "Emitted after processing of the frame is finished."

  Source: https://docs.gtk.org/gdk4/class.FrameClock.html (GTK 4 docs, current).
- **Idle by default [doc].** "A frame clock is idle until someone requests a frame with
  gdk_frame_clock_request_phase()". From the drawing model docs: "If nothing requires the
  Update/Layout/Paint phases we will stay in the Events phase forever". GTK does no work when nothing
  changes, so idle CPU is near zero. Source: https://docs.gtk.org/gtk4/drawing-model.html
- **Animation time [doc].** Frame time is in microseconds and "does not advance during the time a frame is being
  painted". `get_frame_time()` "Gets the time that should currently be used for animations". Animations
  are computed for "the estimated time the next frame will be visible" (drawing-model page).
  `get_refresh_info()` "Predicts a presentation time, based on history". The clock keeps a ring of
  `GdkFrameTimings`.
- **Requesting work [doc].** These calls request phases:
  - `gtk_widget_add_tick_callback()`: per-frame work in Update;
  - `gtk_widget_queue_resize()`: Layout;
  - `gtk_widget_queue_draw()`: Paint;
  - CSS animations and transitions request them implicitly.
- **Events are paused during a frame [src].** `gdk/gdksurface.c`, the flush-events handler:
  ```c
  _gdk_event_queue_flush (surface->display);
  gdk_surface_ensure_motion (surface);
  _gdk_display_pause_events (surface->display);
  gdk_frame_clock_request_phase (clock, GDK_FRAME_CLOCK_PHASE_RESUME_EVENTS);
  surface->frame_clock_events_paused = TRUE;
  ```
  The resume-events handler unpauses. So input that arrives during update, layout or paint is not
  dispatched until after paint. The frame always works on a consistent snapshot of input.
  - The latency cost is at most one frame of processing time.
  - The win is no re-entrancy and exactly one input-state change per frame.
- **Motion compression keeps history [src].** `gdk/gdkevents.c`,
  `_gdk_event_queue_handle_motion_compression`:
  - It walks queued trailing `GDK_MOTION_NOTIFY` events from the same surface and device.
  - It folds them into the last one with `gdk_motion_event_push_history (last_motion, ...)`, then drops
    the rest.
  - `gdk_event_get_history()` then gives handlers every intermediate point, for example for pen strokes
    and lasso.
  - Scroll events have a matching `gdk_event_queue_handle_scroll_compression`.
- **Idle frame clock pacing [src].** `gdk/gdkframeclockidle.c`:
  - Default `FRAME_INTERVAL ((G_NSEC_PER_SEC + 30) / 60)`, which is 16.667 ms.
  - When the clock is not vsync-driven, `min_next_frame_time = smooth_cycle_start +
    smoothed_frame_time_period` avoids busy-looping. The comment: "We're not synced to vblank, so wait at
    least until this before next cycle".
  - The source priority is `GDK_PRIORITY_EVENTS + 1` when flush-events is requested, otherwise
    `GDK_PRIORITY_REDRAW`.
  - Frame-time smoothing rounds to whole refresh intervals and corrects toward real time by
    error²/interval, only on vsync-caused ("thaw") cycles. Reported time is monotonic.
  - The comment: "As long as we don't get a 'frame drawn' signal from the compositor, the clock cycles
    will occur every about frame_interval."
- **Wayland backend [src].** `gdk/wayland/gdksurface-wayland.c`:
  - For every frame, GDK requests a `wl_surface.frame` callback for the main surface and for each
    subsurface. It also requests `wp_presentation_feedback` when the global exists.
  - With presentation-time feedback, the `presented` timestamp feeds the frame timings, and
    `discarded` is handled.
  - Without it, the refresh comes from the output mode (mHz), and presentation time is estimated as "the
    frame time we received plus one refresh interval".
  - The callback calls `gdk_draw_context_frame_stop_throttling()`. The frame clock is throttled from
    commit until the compositor's frame callback.
  - Damage is posted with `wl_surface_damage_buffer` when `wl_surface` is version 4 or later, otherwise
    full-surface `wl_surface_damage`.

### 1.2 Render nodes (GSK)

- **Immutable [doc].** "All GskRenderNodes are immutable, you can only specify their properties during
  construction". They are refcounted and "meant to be transient". There are about 30 node types: container,
  transform, clip, rounded clip, color, linear/radial/conic gradient, border, shadow, text, texture,
  texture-scale, repeat, mask, fill/stroke (paths), blend, cross-fade, opacity, color-matrix, blur,
  debug, cairo, subsurface, and others.
  - Since 4.16, `get_opaque_rect()` returns a rectangle known to be fully opaque, for occlusion.
  - Since 4.22, `get_children()`.
  - Source: https://docs.gtk.org/gsk4/class.RenderNode.html
- **Snapshot and per-widget caching [doc].** During paint, each widget's `snapshot()` builds nodes
  through `GtkSnapshot`. Per the drawing model docs: "Each widget keeps a reference to its render node...
  Invalidating a widget (by calling gtk_widget_queue_draw()) discards the cached render node". An
  unchanged widget re-emits the same node pointer, so subtrees are shared between frames.
- **Damage by tree diff [src].** In `gsk/gskrenderer.c`, `gsk_renderer_render()`:
  - It does a full redraw when there is no `prev_node`, the region is NULL, or `GSK_DEBUG=full-redraw` is
    set.
  - Otherwise it calls `gsk_render_node_diff(prev_node, root, &(GskDiffData){clip,...})`, then keeps
    `prev_node = ref(root)`.
  - In `gsk/gskdiff.c`, container children are diffed with Myers' O(ND) algorithm, using pointer
    equality as the fast path and a per-type compare as the slow path. `MAXCOST` is 20. Past that, the
    diff aborts or takes a suboptimal split ("Enough is enough. We spent too much time here").
  - **[inf]** Because unchanged widgets give back identical node pointers, the diff is cheap in practice
    and damage falls out without explicit dirty-rect bookkeeping.
- **Buffer age [src].**
  - GL (`gdk/gdkglcontext.c`): it queries `EGL_BUFFER_AGE_EXT`, keeps the last
    `GDK_GL_MAX_TRACKED_BUFFERS` damage regions, and repaints the union of the last `age-1` regions plus
    the new damage. It presents with `eglSwapBuffersWithDamageKHR/EXT` when the painted region is not
    the whole buffer.
  - Vulkan (`gdk/gdkvulkancontext.c`): one accumulated `cairo_region` per swapchain image. New damage is
    unioned into every image's region, the acquired image's region becomes the frame's damage, and it is
    reset after present. Damage rectangles go to `VkPresentRegionsKHR` when `VK_KHR_incremental_present`
    is available.
- **Swapchain policy [src].** In `gdk/gdkvulkancontext.c`, the preferred present modes are
  `MAILBOX`, then `IMMEDIATE`, with `FIFO` as the fallback. `minImageCount = CLAMP(4, caps.min, caps.max)`.
  - **[inf]** GTK does not rely on FIFO blocking for pacing. It paces itself with Wayland frame
    callbacks (§1.1), so the driver never blocks in present.
- **Frames in flight [src].** In `gsk/gpu/gskgpurenderer.c`, `#define GSK_GPU_MAX_FRAMES 4`. When all
  four are busy, it waits on the oldest. Empty damage calls `gdk_draw_context_empty_frame()` and skips
  drawing.

### 1.3 Renderers

- **The new renderers [doc].** "New renderers for GTK", 2024-01-28,
  https://blog.gtk.org/2024/01/28/new-renderers-for-gtk/
  - NGL (GL 3.3+/GLES 3.0+) and Vulkan are "unified" renderers from one source tree (`gsk/gpu`). The
    shared code follows a Vulkan-like model and shares node traversal, transform state, and texture and
    glyph caches.
  - They use per-node shaders plus an "ubershader" that interprets data from a buffer, instead of the
    old renderer's frequent offscreens.
  - They antialias all primitives, have unlimited gradient stops, and handle fractional scale properly.
    Example: a 1200×800 window at 125% renders a 1500×1000 framebuffer, instead of letting the
    compositor downscale a 2400×1600 image.
  - Stated performance: "not faster yet"; "fast enough for 60 or 144 fps"; "the Vulkan renderer comes
    close to matching or surpassing the old GL renderer". NGL became the default in 4.13.6/4.14.
- **Vulkan default on Wayland since GTK 4.16 (2024-09) [doc].** NGL remains the default elsewhere.
  GTK recommends Mesa ≥ 24.2.
  - https://download.gnome.org/sources/gtk/4.16/gtk-4.16.0.news
  - https://www.phoronix.com/news/GTK-4.16-Released
- **Old GL renderer removed in 4.18 [doc].** The same cycle reworked measure/allocate "to reduce
  reliance on binary search". X11 and Broadway are deprecated, to be removed in GTK 5.
  https://blogs.gnome.org/gtk/2025/02/01/whats-new-in-gtk-winter-2025-edition/
- **Later work [doc].**
  - The future list in the 2024 post included "Off-the-main-thread rendering". I found no source that it
    has shipped as of 4.22.
  - 4.20 added path intersection APIs and a fill-mask cache: https://habr.com/ru/articles/942372
    (secondary source).
  - Pixel snapping (`gtk_snapshot_set_snap()`, `GSK_RECT_SNAP_ROUND`) shipped in 4.23.1. It is opt-in
    because automatic snapping makes slow animations and smooth scrolling jitter.
    https://blogs.gnome.org/gtk/2026/05/28/snapping/ (2026-05-28)

### 1.4 Graphics offload (subsurfaces and dmabuf)

- **GtkGraphicsOffload, GTK 4.14 [doc].** "Introducing graphics offload", 2023-11-15,
  https://blog.gtk.org/2023/11/15/introducing-graphics-offload/
  - `GtkGraphicsOffload` is a hint widget. A child whose content is a dmabuf `GdkTexture` (built with
    `GdkDmabufTextureBuilder`) is attached to a Wayland subsurface instead of being composited by GSK.
  - **Above mode:** used when nothing overlaps. It allows compositor direct scanout and a zero-copy
    decoder-to-display path.
  - **Below mode:** used when controls overlap. The subsurface goes under the main surface, and a
    translucent hole is cut in the main surface. This still offloads but loses direct scanout.
  - Clipping (for example rounded corners) prevents offload. It is Wayland only. The inspector shows
    offload state: magenta for below, gold for above.
  - The node type is `GskSubsurfaceNode` (listed in the GskRenderNode docs).
- **GTK 4.16 additions [doc].** "Graphics offload continued", 2024-07-12,
  https://blogs.gnome.org/gtk/2024/07/12/graphics-offload-continued/
  - Flips and rotations are passed to the compositor (buffer transforms), so transformed textures can
    offload. Category transforms (`gsk_transform_rotate`) are required; `gsk_transform_matrix` hides the
    category.
  - GL textures are exported as dmabufs through Mesa extensions, so GL content can offload too.
- **[inf] For cadatomic:** this is exactly the pattern for 3D viewports. Render each viewport into its
  own dmabuf-backed swapchain or surface on a subsurface, so the UI never re-composites the 3D pixels.

### 1.5 Text

- **[doc]** "On fractional scales, fonts and hinting", 2024-03-07,
  https://blog.gtk.org/2024/03/07/on-fractional-scales-fonts-and-hinting/
  - The new renderers apply glyph subpixel positioning in device pixels.
  - There are two modes: uniform spacing (unhinted, subpixel-positioned), or crisp (hinted). With hinting
    on, positions round to whole device pixels vertically only, because the autohinter hints only
    vertically.
  - There is no subpixel RGB antialiasing, because compositing lacks component alpha. Font
    antialiasing is grayscale.
- **[doc]** GTK 4.16 added the `gtk-font-rendering` setting. Automatic mode is the default, choosing
  from resolution and scale. Manual mode honours `gtk-hint-font-metrics` and the xft settings; metrics
  hinting turns off subpixel positioning.
  https://docs.gtk.org/gtk4/property.Settings.gtk-font-rendering.html

### 1.6 CSS nodes and style caching

- **[src]** `gtk/gtkcssnode.c`:
  - Each widget has a `GtkCssNode` that caches its computed style and accumulates `pending_changes`, a
    `GtkCssChange` bitmask (id, name, class, state, parent-*, source, timestamp, animations).
  - `GTK_CSS_RADICAL_CHANGE` (id, name or class changes on the node or its parent, source or parent
    style) forces a full restyle. Otherwise GTK checks whether the static style's own change flags
    intersect the change, and keeps the style if not.
  - Invalidation is lazy. It marks the subtree and propagates "invalid" to the root, which queues one
    validate pass. `gtk_css_node_validate()` skips valid and invisible subtrees.
  - **Style sharing:** `lookup_in_global_parent_cache()` caches computed static styles on the parent,
    keyed by declaration plus first/last-child status. Siblings with identical selectors share one style
    object. It is disabled when a node has its own style provider.
  - Animated styles advance on `GTK_CSS_CHANGE_TIMESTAMP` from the root's frame clock without
    recomputing the static style.
  - `style-changed` is emitted only on a visible change.

### 1.7 Lists

- **[doc]** https://docs.gtk.org/gtk4/section-list-widget.html
  - `GtkListView`/`GtkGridView` create list items only for visible rows and recycle them by rebinding
    them to new model items (`GtkListItemFactory`).
  - State must live in the model, not the widget.
  - "generally scale to millions of items"; `GListStore` "works reasonably well for dataset sizes in the
    1,000,000 range".
  - The view must sit directly in a `GtkScrolledWindow`.

---

## 2. Qt

### 2.1 The Qt Quick scene graph

- **[doc]** https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html
  - Nodes (`QSGNode`, `QSGGeometryNode`, `QSGTransformNode`, `QSGClipNode`, `QSGOpacityNode`) "do **not**
    contain any active drawing code nor virtual `paint()` function". They are pure data (geometry plus
    material), which the renderer batches.
  - `QQuickItem::updatePaintNode()` is the only bridge from items to nodes.
- **Default batching renderer [doc].** https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph-renderer.html
  - Opaque primitives are drawn front to back with depth testing, so hidden fragments are skipped and need
    no blending. Alpha primitives are drawn back to front afterwards, with depth writes off.
  - Batches form per unique material state plus geometry type. Overlapping alpha items cannot batch, but
    non-overlapping ones can.
  - Different textures batch only through a shared atlas. `RequiresFullMatrix` prevents batching.
  - A transform node that changes and has a complex subtree becomes a **batch root**. Its geometry is
    pre-transformed on the CPU and retained in GPU buffers, so a transform change updates only one matrix.
  - Static text batched with a changing label is re-uploaded (the docs' example is 88 vertices).
    `mutabilityGroup: Item.DynamicMutabilityGroup` (16 groups) separates them.
  - Tips: fewer than 10 batches, at least 3-4 of them opaque. There is no CPU viewport culling or
    occlusion, so use `visible:false`. Clip uses scissor, or stencil for non-90° rotations, and breaks
    batching.
- **Constants [src].** `qtdeclarative/src/quick/scenegraph/coreapi/qsgbatchrenderer.cpp`:
  - batch-root thresholds: `QSG_RENDERER_BATCH_NODE_THRESHOLD` defaults to 64 renderables and
    `QSG_RENDERER_BATCH_VERTEX_THRESHOLD` to 1024 vertices (`nodeChanged()` with `DirtyMatrix` calls
    `turnNodeIntoBatchRoot`);
  - `OPAQUE_LIMIT = 0.999f`;
  - VBO and IBO pools are capped at 2 MB each;
  - a static buffer becomes `QRhiBuffer::Dynamic` after 4 non-dynamic changes;
  - merging requires triangles or strips, and a set holds at most 0xfffe vertices with 16-bit indices.
  - The renderer keeps a **shadow tree** mirroring QSGNodes, with per-node `dirtyState` bits.
- **Merged vs unmerged batches [doc, inf].** In a merged batch, all members' vertices are
  pre-transformed into one buffer and drawn with one call. In an unmerged batch, each node is drawn
  separately with its own matrix. `QSG_VISUALIZE=batches` shows merged batches solid and unmerged
  batches hatched.

### 2.2 Render loops and threading

- **Threaded loop [doc].** It is the default with Vulkan everywhere, D3D11, Metal, EGLFS, mobile, and
  Linux GL except llvmpipe. The basic, non-threaded loop is used for WebAssembly, some GL drivers, and
  macOS GL. `QSG_RENDER_LOOP=threaded|basic` forces one.
- **Frame sequence [doc].**
  1. `update()` posts an event to the render thread.
  2. The render thread wakes and **blocks the GUI thread**.
  3. The GUI thread runs `polish` (`updatePolish`) before the block.
  4. Sync: `beforeSynchronizing`, then `updatePaintNode()` on changed items. "the only time the QML
     items and the nodes in the scene graph interact". The GUI thread is released.
  5. The render thread renders (`beforeRendering`, preprocess, record, `afterRendering`), presents,
     and emits `frameSwapped`.
- **Design rationale [src].** `qtdeclarative/src/quick/scenegraph/qsgthreadedrenderloop.cpp` header comment:
  - "the render thread is never blocked and the GUI thread will initiate a polishAndSync which will block
    and wait for the render thread to pick it up". There is one real block point.
  - "the render thread can run its own animation system which stays alive even when the GUI thread is
    blocked doing i/o, object instantiation, QPainter-painting or any other non-trivial task."
  - There is one render thread and one QRhi per window.
  - Qt 6 change: syncAndRender "always completes and presents a frame". Qt 5 skipped frames when sync
    found nothing to do, and that "proved to be simply too confusing in practice".
- **Animations [doc, src].**
  - The threaded loop installs a vsync-driven `QAnimationDriver` on both the GUI and render threads.
    `QQuickAnimator` types (Opacity/Scale/Rotation/XAnimator) run on the render thread and survive a
    blocked GUI thread.
  - It falls back to a system timer when more than one window is exposed, any window is unthrottled, or
    vsync is "bad". Bad vsync is detected over `PS_TIME_SAMPLE_LENGTH = 20` frames: average elapsed time
    below `0.5 × vsync interval` sets `badVSync` permanently for the window.
  - Comment: "the default 5 ms timer of `QWindow::requestUpdate()`". On non-Wayland, requestUpdate is a
    5 ms timer.
  - Since 6.5, `QSG_USE_SIMPLE_ANIMATION_DRIVER` selects an elapsed-time driver.
  - In the basic loop, animations use about 16 ms timers, independent of the display.
- **Frame-synchronous input [src].** `qtdeclarative/src/quick/util/qquickdeliveryagent.cpp`:
  - `polishAndSync()` first calls `flushFrameSynchronousEvents()`.
  - Touch updates containing only Updated or Stationary points are coalesced into one `delayedTouch` per
    frame and delivered there. The comment: "It would be a waste of CPU time to deliver events and have
    items in the scene getting modified more often than once per frame". `QML_NO_TOUCH_COMPRESSION`
    disables it.
  - The same flush sends a **synthetic hover** once per frame when items are dirty, so hover follows
    moving items (`QT_QUICK_FRAME_SYNCHRONOUS_HOVER_INTERVAL`).
- **QtGui/xcb compression [doc, src].**
  - `Qt::AA_CompressHighFrequencyEvents` defaults to true on X11. It merges mouse-move, touch-update and
    window-geometry events "so they don't pile up when the app falls behind".
  - `AA_CompressTabletEvents` is off by default.
  - Implemented in `qtbase/src/plugins/platforms/xcb/qxcbconnection.cpp` `compressEvent()`, covering
    `XCB_MOTION_NOTIFY`, `XI_Motion` and `XI_TouchUpdate`.
  - https://doc.qt.io/qt-6/qt.html
- **Qt Wayland pacing [src].** `qtbase/src/plugins/platforms/wayland/qwaylandwindow.cpp`:
  - `requestUpdate()` returns early if a frame callback is pending and delivers on the callback.
  - `mFrameCallbackTimeout = 100` ms (`QT_WAYLAND_FRAME_CALLBACK_TIMEOUT`). After it expires, the
    window is treated as not exposed and rendering stops.
  - The EGL context (`.../wayland-egl/qwaylandglcontext.cpp`) sets `eglSwapInterval(0)` when non-blocking
    swap is supported and calls `window->waitForFrameSync(100)` itself. Qt does its own frame-callback
    throttling rather than letting Mesa block in swap.

### 2.3 QRhi

- **[doc]** https://doc.qt.io/qt-6/qrhi.html
  - Backends: GL 2.1/ES 2.0+, D3D11.2, D3D12, Metal 1.2, Vulkan 1.0, and a Null backend.
  - `FramesInFlight` is "typically 2". `beginFrame()` blocks when the CPU gets too far ahead.
  - Resource updates go in `QRhiResourceUpdateBatch` and are committed at `beginPass()`.
  - A QRhi and its resources are bound to one thread.
  - Pipeline cache save/load is available. Since 6.6 the API is "semi-public" with no compatibility
    guarantee.
  - **[inf]** This is the same layer wgpu provides for us.

### 2.4 Text and shapes

- **Text [doc].** https://doc.qt.io/qt-6/qml-qtquick-text.html
  - `Text.QtRendering`: "a scalable distance field for each glyph". It is the faster option but uses more
    memory, and artifacts appear at large sizes. `renderTypeQuality` takes 26, 52, 104 or 208.
  - `Text.NativeRendering`: platform rasteriser, which looks poor under transforms.
  - `Text.CurveRendering`: "a curve rasterizer running directly on the graphics hardware". It uses less
    memory. **[mem]** Added in Qt 6.7.
- **Shapes [doc].** "Smooth Vector Graphics in Qt Quick", Eirik Aavitsland, 2023-10-04,
  https://www.qt.io/blog/smooth-vector-graphics-in-qt-quick
  - `Shape.CurveRenderer` was a tech preview in Qt 6.6 (`preferredRendererType`,
    `QT_QUICKSHAPES_BACKEND=curve`). It "preserve[s] the curvature information", has built-in
    antialiasing without MSAA, and scales or zooms at no extra cost after a one-time preprocessing cost.
  - Self-intersecting paths were not supported at the time. Preprocessing runs on the GUI thread even with
    `async`.
  - The default Geometry renderer flattens curves, giving jagged edges and broken thin strokes.

### 2.5 QML/V4: JavaScript in the frame

- **[doc]** https://doc.qt.io/qt-6/qtquick-performance.html
  - About 16 ms per frame at 60 FPS: "never spend more than a couple of milliseconds per frame within
    blocking functions".
  - Bindings re-evaluate whenever any referenced property changes, so keep them simple and accumulate in
    temporaries.
  - Do heavy work in a low-priority worker thread. Use `asynchronous` Image and Loader, but not for the
    first screen.
  - "Never enable clipping on a delegate". "Clipping is a visual effect, NOT an optimization".
  - Manual `gc()` "will block the GUI thread for a substantial period of time". Since Qt 6.8 the
    collector is incremental: "shorter, but potentially more interruptions".
- **Compiled bindings [doc].**
  - "The Numbers", Ulf Hermann, 2022-01-18,
    https://www.qt.io/blog/the-numbers-performance-benefits-of-the-new-qt-quick-compiler: JIT <
    qmlcachegen (bindings to C++) < qmlsc. The slow parts are dependency capture for non-QProperty reads
    and runtime lookups.
  - "Compiling QML to C++: A 4x speedup": https://qt.io/blog/compiling-qml-to-c-a-4x-speedup.
  - Qt 6.6/6.7 benchmark (DeltaBlue):
    https://qt.io/blog/qt-6.6-and-6.7-make-qml-faster-than-ever-a-new-benchmark-and-analysis.
- **[inf] Lessons.** QML's binding engine runs on the GUI thread, in the same thread as layout
  (polish). JS cost lands directly in `polishAndSync`, before the sync block. Qt's mitigations are:
  1. render-thread animators that don't need the JS thread;
  2. AOT-compiling bindings so that most "JS" never runs in an interpreter;
  3. one sync point per frame;
  4. frame-coalesced input.

### 2.6 Qt Widgets backing store

- **[doc]** https://doc.qt.io/qt-6/qwidget.html
  - "QWidget automatically double-buffers its painting". `update()` schedules a paint event. "Qt merges
    these events into one event with a larger region". `repaint()` paints immediately and "does not
    permit this optimization".
  - `Qt::WA_OpaquePaintEvent` skips background erase.
  - **[mem]** `QWidgetRepaintManager` collects dirty regions per top-level and syncs them at
    `QEvent::UpdateRequest`, the window's `requestUpdate`, which on Wayland is driven by frame callbacks.
    `QBackingStore::flush(region)` posts only the region.
  - **[inf]** Widgets is a classic immediate-paint-into-retained-raster model. Its strength is region
    damage; its weakness is CPU raster.

---

## 3. The Wayland and Linux present path

### 3.1 Core: frame callbacks, damage, subsurfaces

- **`wl_surface.frame` [doc].** `wayland/protocol/wayland.xml`:
  - "Request a notification when it is a good time to start drawing a new frame... The server should
    give some time for the client to draw and commit after sending the frame callback events to let it
    hit the next output refresh."
  - "A server should avoid signaling the frame callbacks if the surface is not visible". Hidden windows
    get no callbacks, which is why a blocking FIFO driven by callbacks can hang (§3.3).
  - The callback data is a ms timestamp with an undefined base.
- **`wl_surface.damage_buffer` [mem, doc].** Since `wl_surface` v4, damage is in buffer coordinates, so
  it is immune to scale and transform errors. Both GTK and Mesa use it when available; Mesa posts
  `INT32_MAX` full damage otherwise.
- **`wl_subsurface` [mem, doc].**
  - In sync mode, the child's commit is cached until the parent commits, giving atomic updates of the
    whole tree.
  - In desync mode, the child's commit applies immediately, unless an ancestor is in sync mode.
  - **[inf]** A desync subsurface lets a 3D viewport present at its own rate without the UI surface
    re-committing. Sync mode is needed when the viewport resizes together with the UI (a splitter drag),
    so the two geometries change in the same frame.
  - wp_fifo's "wait_barrier" is ignored on sync subsurfaces (see below).
  - Combine with `wp_viewporter` (scale and crop) and `linux-dmabuf` buffers for zero copy. The
    compositor may put an opaque subsurface on a KMS overlay plane.

### 3.2 Presentation time (stable)

- **[doc]** `stable/presentation-time/presentation-time.xml`, wp_presentation v3:
  - `feedback(surface)` is per commit. The `presented` event carries the time "when the content update
    turned into light the first time", `refresh` (ns to the next refresh, 0 if unknown), the MSC
    (`seq_hi/lo`), and flags:
    - `vsync`;
    - `hw_clock`;
    - `hw_completion`;
    - `zero_copy`: direct scanout or an overlay plane ("Compositing with OpenGL counts as copying");
    - `fixed_rate` (v3);
    - `variable_rate` (v3): "no sooner than 'refresh' nanoseconds after the previous presentation".
  - `discarded` means the update was superseded and never shown.
  - The clock is given by `clock_id`, normally `CLOCK_MONOTONIC`, and "Clients must be able to query
    the current clock value directly".
  - wayland-protocols 1.38 (2024-10) added a minor version for VRR.
    https://www.phoronix.com/news/Wayland-Protocols-1.38
- **[inf]** This is the input to a real frame predictor. It gives the actual photon time of frame N and
  the refresh period, so the client can target frame N+k.

### 3.3 fifo-v1 and commit-timing-v1 (staging, wayland-protocols 1.38, Oct 2024)

- **fifo-v1 [doc].** `staging/fifo/fifo-v1.xml` (Valve, 2023):
  - `set_barrier` sets a "fifo_barrier" condition "cleared immediately after the following latching
    deadline for non-tearing presentation".
  - `wait_barrier`: "this content update is not ready while a fifo_barrier condition is present". It
    gives Vulkan-FIFO semantics: each update is shown for at least one refresh, and the client queues
    without blocking on frame callbacks.
  - It is ignored for sync subsurfaces, and may be ignored for occluded surfaces. "Clients must use an
    additional mechanism such as frame callbacks or timestamps to ensure throttling occurs under all
    conditions."
- **commit-timing-v1 [doc].** `staging/commit-timing/commit-timing-v1.xml`:
  - `wp_commit_timer_v1.set_timestamp(t)`: "presented as closely as possible to, but not before, the
    specified time", in the presentation clock domain.
  - Updates still apply in order.
  - It allows scheduled presentation, for example for animation frames pre-rendered for a known vblank.
- **Compositor support [doc].** Per https://wayland.app (accessed 2026-10-25), fifo-v1 is listed as
  supported ("1") by:
  - Mutter 51;
  - KWin 6.7;
  - Weston 15.0.1;
  - Hyprland 0.52.1;
  - Jay 1.14.0.

  It is listed as unsupported ("x") by Sway 1.11, niri 26.04, COSMIC beta.8 and others. commit-timing
  shows Mutter 51, Weston 15.0.1, Hyprland 0.52.1 and Jay 1.14.0 as supported, and KWin as not. The
  Mutter fifo/commit-timing work landed for GNOME 48 (secondary source, Phoronix:
  https://www.phoronix.com/news/GNOME-48-Triple-Buffering-Jan).
- **Mesa Vulkan WSI [src].** `src/vulkan/wsi/wsi_common_wayland.c`. The Vulkan FIFO support merged for
  Mesa 24.3 (secondary source: https://phoronix.com/news/Mesa-24.3-Vulkan-FIFO). Present modes:
  - MAILBOX and FIFO are always offered; IMMEDIATE is offered only with `tearing-control`.
  - In FIFO with `wp_fifo`, every present does `set_barrier` and `wait_barrier`, plus an extra
    timestamped empty commit when commit-timing exists, so occluded windows make progress.
  - Without `wp_fifo`, Mesa falls back to `wl_surface.frame`, and QueuePresent blocks on it (the legacy
    path).
  - Image counts: `WSI_WL_DEFAULT_NUM_IMAGES 3`; `WSI_WL_BUMPED_NUM_IMAGES 4` for MAILBOX ("one to scan
    out from, one queued, one held by the compositor, one to render to"), and 4 for everything when
    fifo is absent. The comment says returning 4 for a FIFO swapchain "is deeply problematic due to
    excessive latency".
  - Mesa implements `VK_KHR_present_wait2`, `VK_KHR_present_id2` and **`VK_EXT_present_timing`** on
    Wayland, mapped to presentation feedback and commit-timing.
  - Khronos on present timing: https://www.khronos.org/blog/vk-ext-present-timing-the-journey-to-state-of-the-art-frame-pacing-in-vulkan
    and https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_present_timing.html

### 3.4 tearing-control-v1 and linux-drm-syncobj-v1

- **tearing-control [doc].** `staging/tearing-control/tearing-control-v1.xml`:
  - `presentation_hint` is `vsync` (the default) or `async`: "presented with minimal latency and tearing
    is acceptable", using asynchronous page flips.
  - Per wayland.app it is supported by KWin 6.7, Sway, Hyprland, Weston, labwc, river and Louvre, but
    **not Mutter 51**.
  - **[inf]** It is only useful for fullscreen or game-like viewports, not a docked IDE.
- **linux-drm-syncobj-v1 (explicit sync) [doc].** `staging/linux-drm-syncobj/linux-drm-syncobj-v1.xml`:
  - Each commit carries an acquire timeline point, which the compositor waits on, and a release point,
    which the compositor signals. `wl_buffer.release` becomes undefined.
  - Supported by Mutter 51, KWin 6.7, Sway 1.11, Hyprland, COSMIC, gamescope, labwc and river.
  - **[inf]** Vulkan WSI uses it internally. It matters to us only if we hand-roll dmabuf subsurfaces for
    viewports.

### 3.5 Compositor scheduling: what the compositor adds

- **KWin [src].** `src/core/renderloop.cpp`, `renderjournal.cpp`, `backends/drm/drm_commit_thread.cpp`,
  `drm_output.cpp`. Compositing is scheduled late, close to vblank:
  - `expectedCompositingTime = min(renderJournal.result() + safetyMargin, 2 × vblankInterval)`.
  - **RenderJournal:** an exponential average of measured render time (wakeup to GPU end, CPU and GPU)
    with time constant 500 ms, plus `2 × variance` (variance time constant 6 s). Each sample is at least
    2 ms ("timings are pretty unpredictable in the sub-millisecond range").
  - **Safety margin:** the DRM commit thread starts with `vblankTime + 1000 µs`
    (`KWIN_DRM_OVERRIDE_SAFETY_MARGIN`). It adds margin immediately when a commit was late and removes
    it slowly (by delta/10) otherwise.
  - **Triple buffering:** `setMaxPendingFrameCount(2)`. It switches to two pageflips in advance
    immediately when needed and returns to double after 10 good frames, or stays triple if
    `expected ≥ 0.95 × vblank`.
  - If more than 100 vblanks have passed since the last frame, KWin assumes the GPU is in a low power
    state and starts compositing almost a whole interval early.
  - In VRR and tearing modes there is at most 1 pending frame. Under VRR, the active window drives
    refresh if its frame time is ≤ 1/30 s.
  - History: KWin 5.21 introduced late compositing (CPU time only); KWin 6.0 added GPU timing; KWin 6.1
    added triple buffering. The latency-policy settings were removed in favour of measurement (MR 4408).
    https://planet.kde.org/xavers-blog-2024-06-24-fixing-kwins-performance-on-old-hardware ,
    https://invent.kde.org/plasma/kwin/-/merge_requests/4833 ,
    https://invent.kde.org/plasma/kwin/-/merge_requests/4408
- **Mutter [src].** `clutter/clutter/clutter-frame-clock.c`, `clutter-main.c`, `src/wayland/meta-wayland.c`:
  - Dispatch is timed from `max_render_time`, the sum of:
    - max update duration (dispatch to swap, plus max(GPU finish, KMS submit));
    - `deadline_evasion`;
    - vblank duration;
    - margin: `clutter_max_render_time_constant_us = 1000` plus a dynamic margin clamped to
      `refresh/4`. The margin rises quickly and decays over about a minute.
  - The estimate is clamped to 2× refresh (3× when triple buffering is allowed). The VRR fallback is
    `0.875 × refresh` (`SYNC_DELAY_FALLBACK_FRACTION`), and `MINIMUM_REFRESH_RATE` is 30.
  - **Dynamic triple buffering:** used only when the estimate is ≥ 1 refresh and the previous frame was
    not zero-copy. It merged 2025-02 for GNOME 48 (Daniel van Vugt, Canonical, more than 4 years of
    work). It raised 4K overview animations on a basic Intel GPU from 30 to 60 FPS, with "no latency
    penalty when the system is able to maintain full frame rate".
    https://www.phoronix.com/news/GNOME-48-Triple-Buffering ,
    https://discourse.ubuntu.com/t/triple-buffering-a-debrief/56314
  - **Frame callbacks (native backend):**
    - After a stage update, if the frame is pending presentation, callbacks are emitted immediately.
    - Otherwise they are deferred with a timerfd to the frame's **deadline**.
    - fifo barriers are cleared in `on_after_update`. Commit-timing transactions are unblocked in
      `on_before_update` against the expected presentation time.
- **[inf] What the compositor adds for a windowed, composited client.** A client that commits just
  before the compositor's dispatch (vblank − max_render_time) appears at that vblank. One that commits
  just after waits about one more refresh. The compositor therefore adds between about the compositing
  time (render estimate plus margin, typically 2-4 ms) and 1 refresh plus that, plus scanout.
- **Measured end-to-end figures [doc].**
  - **Marco Nett, click2photon** (500 Hz OLED, RTX 4070S, CachyOS, a fullscreen Proton game, so likely
    direct scanout, 300 clicks per configuration): medians of 4.21–4.93 ms across X11/Wayland, VRR and
    the DXVK low-latency fork. Wayland trails X11 by 0.14–0.22 ms. **XWayland adds 3.13 ms.**
    https://itsfoss.com/news/x11-vs-wayland-input-latency/ (2026-07-16),
    https://marco-nett.de/blog/measuring-input-latency-on-linux-x11-vs-wayland-vrr-dxvk/
  - **mort.coffee, GNOME 47 on Fedora 41** (Jan 2025, phone camera at 90 fps): Wayland had about
    6.5 ms more cursor latency than X11. https://mort.coffee/home/wayland-input-latency/
  - NVIDIA (KWin 6.7.4, Mutter 49.7) found X.Org and Wayland "very close"
    (https://phoronix.com/news/NVIDIA-Wayland-Xorg-Latency; the page returned 403, so this is from the
    search summary).

### 3.6 X11 Present (brief)

- **[mem; spec fetch failed: freedesktop cgit returned 503, gitlab raw blocked by Anubis].**
  - `PresentPixmap` carries `target-msc`, `divisor`, `remainder`, valid and update regions,
    `wait-fence` and `idle-fence`, and the options `Async`, `Copy`, `UST`, `Suboptimal`.
  - `PresentCompleteNotify` returns UST and MSC and the mode (`Copy`, `Flip`, `Skip`, `SuboptimalCopy`).
    `PresentIdleNotify` says when a pixmap can be reused.
  - It gives MSC-targeted presentation and completion feedback, the X11 analogue of wp_presentation plus
    commit-timing.
  - Mesa's X11 WSI uses it (the Wayland WSI comments say the "X11 backend has very similar logic").
  - GTK 4.18 deprecated its X11 backend.

### 3.7 What wgpu and winit support (as of 2026-10)

- **wgpu 30.0.1 [doc].** https://docs.rs/wgpu/latest/wgpu/type.SurfaceConfiguration.html
  - `desired_maximum_frame_latency` defaults to 2. On Vulkan, the swapchain gets `latency + 1` images.
    On OpenGL it is ignored.
  - "Choose `1` to minimize latency above all else... For applications like GUIs doing a small amount
    of GPU work each frame that need low latency, this is a reasonable choice."
  - Present modes: Fifo (guaranteed), FifoRelaxed, Immediate, Mailbox, and AutoVsync/AutoNoVsync. With
    Vulkan Mailbox and latency 2, the frame rate is limited to 2× the monitor's.
- **wgpu-hal Vulkan [src].** `wgpu-hal/src/vulkan/{adapter.rs,swapchain/native.rs,mod.rs}` on trunk:
  - The only present-timing extension enabled is optional `VK_GOOGLE_display_timing`, exposed as a
    hal-level `set_next_present_time`.
  - There is **no** `VK_KHR_incremental_present`, so no present damage. The wgpu issues "Incremental
    presentation" (#1397, closed) and #682 cover this:
    https://github.com/gfx-rs/wgpu/issues/1397 , https://github.com/iced-rs/iced/issues/367
  - There is **no** `VK_KHR_present_wait/present_id`, **no** `VK_EXT_present_timing`, and no buffer age.
- **winit [src, doc].** Shallow clone at `0c73728`, 2026-10-24, crate `winit-wayland`; docs for 0.30.13.
  - On Wayland, `Window::pre_present_notify()` "Schedules a frame callback to throttle
    `WindowEvent::RedrawRequested`".
  - winit binds `wp_viewporter` and `wp_fractional_scale_v1`.
  - It has **no** `wp_presentation`, **no** fifo or commit-timing, **no** tearing-control, and no
    API for application subsurfaces. Subsurfaces appear only inside the client-side decorations frame.
- **[inf] So with stock eframe/wgpu/winit:**
  - Pacing comes from the FIFO swapchain, which on Mesa ≥ 24.3 with wp_fifo no longer blocks on frame
    callbacks, plus optional frame-callback throttling.
  - The app has no presentation timestamps to predict from.
  - Every frame is presented with full damage.
  - With the default latency 2 (3 images), 1–2 frames of queueing sit on top of the compositor's ≤ 1
    refresh. That fits the observed "2–3 frames of present latency".

---

## 4. Wt (brief): server/client split

- **[doc]** https://www.webtoolkit.eu/wt/doc/reference/html/overview.html
  - The UI is a server-side tree of `WWidget`s per session (`WApplication::root()`).
  - "By default, Wt performs all event processing server-side". "Every connected event signal will
    cause the web browser to communicate with the web server".
  - The transport is Ajax long-polling, or WebSockets.
  - Latency escapes:
    - **stateless slot learning**: the visual effect of a stateless slot is recorded once and replayed
      client-side as JS;
    - `JSlot`: pure client-side JS;
    - `JSignal`: JS to C++;
    - `doJavaScript()`.
  - Progressive bootstrap starts as plain HTML and upgrades to Ajax "without repainting".
- **[mem]** Changes to the server tree are serialised as incremental JS and DOM updates, not full
  re-renders.
- **[inf] Lesson.** Wt's split matches our JS-runtime-vs-Rust-scene split.
  - The authoritative tree lives on the "slow" side (the server, or our QuickJS).
  - Latency-critical visual feedback must be precomputed on the fast side ("learned" or declared
    client-side behaviour).
  - For svelters: hover, press, focus rings, scrolling, splitter drags and selection highlight should be
    declarative state on the Rust scene (CSS pseudo-classes, transitions), and never wait for a JS
    handler round trip.

---

## 5. Lessons and recommendations for cadatomic's framework

### 5.1 Frame clock

1. **Use an idle-until-requested clock with GTK's explicit phases:** `FlushInput → Update(animations,
   JS-commit apply) → Style → Layout → Paint(record nodes) → Submit → ResumeInput`.
   - Pause input dispatch during the phases. Compress motion and scroll, but keep the history.
   - Nothing requested means no wakeups at all: zero idle CPU.
2. **Pace from the compositor, not the swapchain.**
   - Drive frames from `wl_surface.frame` plus `wp_presentation` feedback.
   - Present without blocking: MAILBOX, or FIFO with wp_fifo. This is what GTK (it prefers MAILBOX and
     paces on frame callbacks) and Qt Wayland (`eglSwapInterval(0)` plus its own `waitForFrameSync(100)`)
     both do.
   - Keep Qt's 100 ms callback timeout to detect hidden windows and stop rendering.
3. **Start frames late (deadline scheduling).**
   - Predict the next presentation time from wp_presentation (`presented`, `refresh`, MSC).
   - Start the frame at `target_present − compositor_latch_margin − (EWMA(cpu+gpu) + k·σ)`, as KWin's
     RenderJournal (average + 2σ, at least 2 ms) and Mutter's max_render_time do.
   - **[inf]** For a 120 Hz UI frame taking 1.5 ms CPU and 1 ms GPU, this cuts about 5 ms of queueing
     against "render immediately on callback".
   - Sample input at frame start, not at callback time.
   - Use commit-timing (Mutter 51, Weston) where present, to target an exact vblank for animations.
4. **One frame in flight for the UI surface.** Use `desired_maximum_frame_latency = 1` and 2–3 images.
   KWin and Mutter already add triple buffering dynamically when they are slow, so the client must not add
   its own queue too.

### 5.2 Render nodes and damage

5. **Use immutable, `Arc`-shared render nodes with per-widget cached subtrees (the GTK model).**
   - The node tree is the hand-off between the UI thread and the render thread. Because it is immutable,
     no sync block is needed. This is better than Qt's blocking sync.
   - Damage comes from pointer-equality diffing (GTK's Myers diff with a cost cap) or, more cheaply,
     from dirty bits on the retained scene, which we own anyway. Add the union of the last
     `buffer_age − 1` damages, or a per-swapchain-image accumulated region like GTK's Vulkan path.
6. **Use Qt's batching rules in the recorder.**
   - Opaque first, front to back with depth or occlusion (GTK's `get_opaque_rect`), then alpha back to
     front.
   - Batch by pipeline plus atlas, and keep instance and vertex buffers retained across frames.
   - Turn subtrees whose transform changes into batch roots: a scrolling list or a canvas pan updates
     one matrix. Qt's thresholds of 64 nodes or 1024 vertices are reasonable starting points.
   - Separate frequently changing content (Qt's mutability groups).
   - A GPU ubershader with an instance buffer (GTK's design) suits UI primitives.
7. **Text.**
   - Glyph atlas with subpixel positioning in device pixels and vertical-only hinting (GTK 4.14+).
     Grayscale antialiasing is enough if we composite with alpha.
   - For zoomable schematic and PCB canvases and 3D labels, consider SDF or MSDF glyphs (Qt's
     QtRendering) or a GPU curve renderer (Qt CurveRendering and Shapes.CurveRenderer: zoom-invariant at
     one-time preprocessing cost).
   - Render at device pixels for fractional scales. Offer opt-in pixel snapping, not automatic snapping,
     which jitters smooth scrolls (GTK 4.23).
8. **Style.** Use GTK-style change bitmasks, lazy invalidation that propagates to the root, and a
   parent-held style-sharing cache keyed by declaration plus structural position. Advance CSS animations
   by timestamp without restyling.
9. **Virtualised lists.** Recycle row widgets and keep state in the model (GtkListView, QML delegates).
   A docking IDE needs this for outliners, logs and the command palette.

### 5.3 Threading and JS

10. **Three lanes.** **[inf]** This combines Qt's render thread with Wt's server/client lesson.
    - **JS thread** (QuickJS). It runs Svelte, mutates a *pending* scene diff, and posts it to the UI
      thread. It is never awaited by a frame.
    - **UI thread** (Rust). It owns the retained scene, input, hit testing, style, layout, declarative
      animations and transitions, and node recording. It applies JS diffs that arrived before the frame
      deadline; late diffs roll to the next frame.
    - **Render thread.** It owns wgpu, the swapchains, upload and submit, and presentation feedback. It
      runs render-thread animators (opacity, transform, scroll offset), which keep animating while the
      UI or JS thread is busy, as Qt's QQuickAnimator does.
11. **Input feedback must never wait for JS.**
    - Hover, active, focus and pressed pseudo-classes, scrolling, kinetic scroll, drags, text caret and
      selection are resolved on the UI thread in the same frame.
    - JS handlers receive the event asynchronously; their DOM changes land in frame N+1 at worst.
    - Use Qt's frame-synchronous re-hover after layout changes.
12. **Don't block on JS GC.** Run QuickJS GC between frames, or on its own thread when JS is idle (Qt's
    `gc()` warning).
    - Make most bindings non-JS: svelters' compiler can lower simple `$derived` and attribute bindings to
      Rust-side expressions, the analogue of qmlsc.

### 5.4 The Linux present path for cadatomic

13. **UI surface.**
    - wgpu Vulkan with Mailbox, or Fifo when Mailbox is missing.
    - Latency 1, with our own frame-callback and presentation pacing on the `wl_surface`. That requires
      reaching past winit to the raw `wl_surface`, for example through
      `smithay-client-toolkit`/`wayland-client` on the same `wl_display`.
    - We need `wp_presentation` ourselves, since winit lacks it.
    - Present damage needs `VK_KHR_incremental_present`, which wgpu lacks. Either carry a small
      wgpu-hal patch (a fork, per cadatomic's fork rules) or accept full-surface damage at first. On
      integrated GPUs with partial updates, damage mainly saves compositor and GPU bandwidth and power,
      not latency.
14. **3D viewports, and optionally the PCB and schematic canvases: one `wl_subsurface` each, in desync
    mode.**
    - Each has its own wgpu surface created from the subsurface's `wl_surface`, so it presents at its
      own cadence while the UI surface idles. This is GTK offload's "above" mode.
    - Overlays drawn by the UI on top need the "below" arrangement (punch-through) or separate overlay
      subsurfaces.
    - Switch to sync mode for frames that resize the viewport together with the UI, so docking and
      splitter drags stay atomic.
    - The compositor may scan out an opaque viewport on an overlay plane (`zero_copy` flag in
      presentation feedback).
15. **Explicit sync and tearing.** Vulkan WSI already handles linux-drm-syncobj. Offer
    tearing-control's `async` only for fullscreen viewports (it is unsupported on Mutter). Use
    commit-timing (Mutter, Weston) for animation pacing where it exists.
16. **Fallbacks.** On X11, Present (MSC-targeted) is handled through Vulkan WSI. Avoid XWayland in the
    hot path, since it measured +3.13 ms.
