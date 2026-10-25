# How Apple and Android get low latency and smooth frames: research notes for cadatomic's retained GUI framework

Written 2026-10-25 by a research subagent. All sources were accessed 2026-10-25. Each claim has a
source URL and a grade:

- **[D]** documented: Apple or Google docs, a talk transcript, or AOSP/androidx source code.
- **[S]** secondary: a forum post, a third-party write-up or a press report.
- **[I]** inferred: my own reading or synthesis, not stated by the source.

Version notes: Apple docs were read as the current `.md` renderings of developer.apple.com, which
carry availability metadata. AOSP code was read at `refs/heads/main` (Choreographer,
InputEventReceiver) and `android16-release`/`android-16.0.0_r2` (DrawFrameTask, Resampler) unless
a section says otherwise.

---

## Part 1: Apple

### 1.1 Core Animation: layer tree, model, presentation and render trees

- **Layers are model objects that cache content in a bitmap.** Changing a property changes state,
  not pixels. [D] Core Animation Programming Guide, "Core Animation Basics" (archived guide):
  <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CoreAnimation_guide/CoreAnimationBasics/CoreAnimationBasics.html>
  - "a layer captures the content your app provides and caches it in a bitmap, which is sometimes
    referred to as the backing store. When you subsequently change a property of the layer, all you
    are doing is changing the state information associated with the layer object."
  - The guide contrasts this with view-based drawing, where "changes to the view itself often result
    in a call to the view's drawRect: method ... done using the CPU on the main thread. Core
    Animation avoids this expense by ... manipulating the cached bitmap in hardware."
- **There are three trees.** [D] Same page.
  - The *model layer tree* holds target values, and is the tree the app mutates.
  - The *presentation tree* holds "the in-flight values for any running animations". It is read-only
    and is used to read current values, for example to start a new animation from where an old one
    is now.
  - The *render tree* objects "perform the actual animations and are private to Core Animation."
- **Animations are declarative.** "All you have to do is specify the start and end points of the
  animation and let Core Animation do the rest." Frame-by-frame interpolation is not app code. [D]
  Same page.
- **The render server is a separate process.** WWDC14 session 419: "the application process is
  actually not doing the actual rendering work for Core Animation". The view hierarchy is committed
  to "this render server which is a separate process". [D] Transcript at
  <https://nonstrict.eu/wwdcindex/wwdc2014/419/> (Apple video). Apple's current responsiveness
  article says the same thing: "The render server is a separate process that handles all rendering
  of the various processes that need to draw to the screen. It composites the layer trees from the
  apps and the system processes into a final image." [D]
  <https://developer.apple.com/documentation/xcode/understanding-user-interface-responsiveness>
  - On iOS this process is commonly named as backboardd. That is [S], from third-party write-ups;
    Apple's docs say only "render server". On macOS, WindowServer composites windows [I, general
    knowledge].
- **Once an animation is committed, it runs in the render server** with no IPC per frame. [S]
  wwdcnotes summary of WWDC14 419 <https://www.wwdcnotes.com/notes/wwdc14/419>. This is consistent
  with the render-tree description above [D].
- **The cost to watch is offscreen passes.** Shadows without `shadowPath`, masks, rounded corners
  without enough information, and blur/vibrancy each force the GPU to "render a layer by first
  rendering it somewhere else and then copying it over". [D] Tech Talk 10857 (2020), "Demystify and
  eliminate hitches in the render phase": <https://developer.apple.com/videos/play/tech-talks/10857/>
  - In the demo, a cell went from 36 offscreen passes to 0 once explicit geometry was supplied
    (`shadowPath`, `cornerRadius`, `cornerCurve`).
  - The rasterization cache (`shouldRasterize`) is bounded by a screen-size multiple. The transcript
    is inconsistent on the multiple: "2.5x the screen" in one place, "25.5" in another. [D-ish]
    WWDC14 419 transcript.

### 1.2 CATransaction: the commit

- [D] <https://developer.apple.com/documentation/quartzcore/catransaction> (macOS 10.5+, iOS 2+):
  - "Every modification to a layer tree must be part of a transaction."
  - "Implicit transactions are created automatically when the layer tree is modified by a thread
    without an active transaction and are committed automatically when the thread's runloop next
    iterates."
  - Explicit `begin`/`commit` can nest. Only the outermost commit starts the animations. `flush()`
    flushes an implicit transaction early. `setDisableActions` turns off implicit animations.
    `lock()`/`unlock()` is a "recursive spin-lock" for property atomicity.
- **A commit has four sub-phases.** [D] Tech Talk 10856 (2020), "Find and fix hitches in the commit
  phase": <https://developer.apple.com/videos/play/tech-talks/10856/>
  1. **Layout**: `layoutSubviews` runs for every view marked by `setNeedsLayout`.
  2. **Display**: `drawRect` runs for every view marked by `setNeedsDisplay`. This is CPU drawing
     into the backing store.
  3. **Prepare**: image decoding and format conversion.
  4. **Commit**: "the view hierarchy is packaged recursively and sent to the render server ... deep
     view hierarchies will take longer to be packaged up."
  - Advice from the talk:
    - Avoid empty `drawRect`, because "the system must do extra work".
    - Prefer `hidden` to add/remove.
    - Avoid `layoutIfNeeded`.
    - "A view shall only invalidate itself or its children, but not its siblings or its parent."
- **Implication [I]:** Apple avoids re-recording unchanged content in two ways. Dirty flags
  (`setNeedsLayout` and `setNeedsDisplay`, per view) mean only invalidated views lay out or draw.
  And the commit ships a tree of property deltas plus already-rasterized backing stores, not draw
  commands.
- **iOS 9 "low-latency Core Animation".** Commits no longer wait for the next frame before CA work
  starts. Rendering begins as soon as the app commits. It is automatic, and "automatically disabled
  when you have animations active". [D] WWDC15 233 transcript
  <https://nonstrict.eu/wwdcindex/wwdc2015/233/>; notes <https://wwdcnotes.com/documentation/wwdc15-233-advanced-touch-input-on-ios/>

### 1.3 The render loop and hitches (Apple's vocabulary)

Sources:
- Tech Talk 10855 (2020), "Explore UI animation hitches and the render loop":
  <https://developer.apple.com/videos/play/tech-talks/10855/>
- "Understanding hitches in your app" (current Xcode docs):
  <https://developer.apple.com/documentation/xcode/understanding-hitches-in-your-app>
- "Understanding user interface responsiveness":
  <https://developer.apple.com/documentation/xcode/understanding-user-interface-responsiveness>

- **Phases.** [D] Event (app) → Commit (app) → Render prepare (render server, CPU) → Render execute
  (render server, GPU) → Display. "Each phase of the Render Loop is performance-sensitive and has a
  deadline. The deadline is the next VSYNC." Stages pipeline: "our app can prepare a new frame
  while the system is rendering the previous one."
- **Deadlines are computed backwards from the presentation time.** [D] Hitches doc:
  - *Presentation time* is the vsync at which the frame should appear.
  - *Commit deadline* is normally one vsync before that. "To maximize the time it can spend on a
    frame, the render server starts rendering immediately after the commit deadline."
  - *Begin time* is normally one vsync before the commit deadline, and equals the previous frame's
    commit deadline.
- **Event delivery is not fixed to vsync.** [D] Hitches doc: "To reduce input delays, the system may
  process events as soon as they come in, depending on the rendering mode, the input device, and the
  platform. Alternatively, the system might hold events and only deliver them to the app's main
  thread at the begin time to maximize the time the app has".
- **Latency arithmetic, documented exactly.** [D] Hitches doc:
  - **Double-buffer mode** (most devices) takes three vsync intervals from the app starting work to
    the last pixel lit: one for the main thread, one for the render server, one for display scanout.
    That is "50 ms at 60 Hz, or 25 ms at 120 Hz". The expected frame lifetime, from begin time to
    presentation, is 2 intervals.
  - **Triple-buffer mode** gives the render server two intervals and lets it work on two frames in
    parallel. The expected frame lifetime is 3 intervals ("25 ms at 120 Hz, and 50 ms at 60 Hz"),
    and app-start to last pixel is 4 intervals ("33.3 ms at 120 Hz, and 66.7 ms at 60 Hz").
  - The system enters triple buffering to recover from a render hitch, or to save energy ("running
    the GPU in a low-power mode"). The render server's **CPU** prepare still has to fit in one
    interval, because it cannot run in parallel; only the GPU work overlaps.
  - "Devices can also switch to a low-latency mode, where the time the render server spends on a
    frame is less than one vsync interval ... such as in drawing apps."
- **Per-stage budget.** At 120 Hz "each stage then has up to 8.3 ms". The layout part of the commit
  has "only a couple of milliseconds". [D] 10855.
- **Hitch types.** [D] 10855 and the hitches doc:
  - A *commit hitch* means the app missed the commit deadline. The render server starts one vsync
    later.
  - A *render hitch* means the render server missed presentation.
  - *Hitch duration* = actual frame lifetime − expected frame lifetime.
- **Thresholds.** [D]
  - Tech talk 10855: <5 ms/s of hitch time "good"; 5–10 ms/s "warning"; >10 ms/s "critical".
  - The current Xcode Organizer metric uses looser bands: ≤10 ms/s good, ≤25 warning, ≤50 critical,
    >50 needs immediate attention (hitches doc).
  - Hangs are discrete-interaction delays, noticeable at "about 50 ms to 100 ms or longer". A hitch
    can be as small as one refresh interval, "between 8 ms and 16 ms" (responsiveness doc).
  - On drag: "a delay of 50 ms ... between the finger changing direction and the icon changing
    direction may be noticeable ... However, it isn't obvious when the delay stays constant ... Apple
    devices often optimize for smooth motion while accepting a slightly longer interaction latency,
    if necessary." Pencil is the exception, where "both an extremely low interaction latency and
    fluid motion" are needed. [D] Responsiveness doc.

### 1.4 Run loop, event dispatch, coalescing (AppKit and UIKit)

- **The main thread owns events and the view tree.** The OS queues events per app, and "it's the
  responsibility of the main thread of the app to pick up the events ... the only thread in your app
  that can modify the view hierarchy". [D] Responsiveness doc.
- **AppKit mouse coalescing.** `NSEvent.isMouseCoalescingEnabled` (macOS 10.5+): "Mouse movement
  events include mouse-moved, mouse-dragged, and tablet events ... The default value of this
  property is true." [D]
  <https://developer.apple.com/documentation/appkit/nsevent/ismousecoalescingenabled>
- **Tracking loops.** A view can run its own loop with
  `nextEventMatchingMask:untilDate:inMode:dequeue:` in `NSEventTrackingRunLoopMode`. "the
  application's main thread is unable to process any other requests during an event-tracking loop
  and timers might not fire as expected." Apple recommends the three-method approach
  (`mouseDown`/`mouseDragged`/`mouseUp`) for most designs. [D] Cocoa Event Handling Guide:
  <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/HandlingMouseEvents/HandlingMouseEvents.html>
  and `RunLoop.Mode.eventTracking` ("The mode set when tracking events modally, such as a
  mouse-dragging loop"): <https://developer.apple.com/documentation/foundation/runloop/mode/eventtracking>
  - [I] Run-loop modes matter for scheduling. A display link or timer added only in `.default` mode
    stops during live resize, menu tracking or scroll tracking. Frame callbacks should be added in
    common modes.
- **UIKit coalesced touches.** [D]
  <https://developer.apple.com/documentation/uikit/getting-high-fidelity-input-with-coalesced-touches>
  - "UIKit usually delivers touches to your app at around 60 Hz, but some devices are capable of
    recording touch information at up to 240 Hz." Only the last sample is delivered;
    `coalescedTouches(for:)` returns the rest. They must be read during handling.
  - The iPad Air 2 scans touch at 120 Hz while the display stays 60 Hz, so at most one touch event is
    delivered per display frame. [D] WWDC15 233.
- **UIKit predicted touches.** [D]
  <https://developer.apple.com/documentation/uikit/minimizing-latency-with-predicted-touches>
  - `predictedTouches(for:)` returns "touches predicted to occur after the last actual touch. Always
    treat predicted touches as temporary data ... discard them upon receipt of each new touch
    event."
  - WWDC15: prediction gives "approximately a frame into the future".
- **WWDC15 233 latency budget** (60 Hz iPad). [D] Transcript
  <https://nonstrict.eu/wwdcindex/wwdc2015/233/>:
  - iOS 8: 4 frames for an optimized GL/Metal app (≈67 ms), 5 frames for complicated views.
  - iOS 9 low-latency CA: −1 frame.
  - 120 Hz touch scan with coalescing: −0.5 frame.
  - Prediction: about −1 frame.
  - The total gets to "approximately one and a half frames of latency" (≈25 ms).
  - A forum user measured about 5 frames on an iPhone 6. [S] <https://developer.apple.com/forums/thread/71301>
- **Apple Pencil.** About 20 ms on the 2018 iPad Pro, cut to about 9 ms in iPadOS 13 (2019) on
  120 Hz ProMotion iPads "using even more advanced prediction algorithms and optimizations". [S]
  Press coverage, e.g. <https://appleinsider.com/articles/19/06/08/apple-pencil-gets-impressive-updates-with-ipados>.
  The figures are Apple's stated numbers as reported by the press; I did not find them on an Apple
  page.
- **UIUpdateLink** (iOS 18 / iPadOS 18 / visionOS 2; not on macOS). [D]
  <https://developer.apple.com/documentation/uikit/uiupdatelink>,
  <https://developer.apple.com/documentation/uikit/uiupdateactionphase>, WWDC24 10118
  <https://developer.apple.com/videos/play/wwdc2024/10118/>
  - **Phases of one UI update, in order:**
    - `afterUpdateScheduled`
    - `beforeEventDispatch` → `afterEventDispatch`
    - `beforeCADisplayLinkDispatch` → `afterCADisplayLinkDispatch`
    - `beforeCATransactionCommit` → `afterCATransactionCommit`
    - [optional low-latency group] `beforeLowLatencyEventDispatch` → `afterLowLatencyEventDispatch`
      → `beforeLowLatencyCATransactionCommit` → `afterLowLatencyCATransactionCommit`
    - `afterUpdateComplete`
    - "Phases run one after another in the specified order without exiting back into the run loop."
  - **`wantsLowLatencyEventDispatch`**: "Only pencil events are low-latency eligible ... Low-latency
    eligible events dispatch in the middle of a UI update. This timing gives an app half the amount
    of time to handle them as standard events." `UIUpdateInfo.completionDeadlineTime` gives the
    deadline.
  - **`wantsImmediatePresentation`**: "requests immediate rendering of the display frame after the
    last CATransaction commit for the current UI update ... the rendered display frame presents one
    frame duration sooner." The cost is that the render-server work must be minimal.
  - **`requiresContinuousUpdates`**: "When false, the update link fires only when something else
    drives updates, such as a gesture or layer changes." This is on-demand scheduling.
  - Use `updateInfo.modelTime` for animation time.
- This is the clearest public statement of Apple's frame phases: input → display-link callbacks
  (animation) → CA commit, then an optional *second* late input sample and commit inside the same
  frame for ink. [I]
- **iOS 18 automatic trait tracking.** UIKit "records which traits you access inside" `layoutSubviews`
  and `drawRect`, and invalidates them automatically when those traits change. This is
  read-tracking invalidation, the same idea as Compose and SwiftUI. [D] WWDC24 10118.

### 1.5 CADisplayLink, ProMotion, frame pacing

- **WWDC21 10147, "Optimize for variable refresh rate displays".** [D]
  <https://developer.apple.com/videos/play/wwdc2021/10147/>
  - "CADisplayLink wakes up at every vsync ... This provides the application the entire 8ms".
  - NSTimer is "very unlikely to be in perfect sync with the display".
  - Use `targetTimestamp`, not `timestamp`, to compute animation state. Otherwise rate changes
    hitch.
  - Track deltas between target timestamps so that skipped callbacks are accounted for.
  - On macOS Adaptive-Sync: "your frame is presented to the display immediately after it is done,
    so your app incurs only a 1ms penalty"; "present frames at the highest rate your app can do so
    evenly".
  - Pacing with `presentDrawable:afterMinimumDuration:` using an EMA of GPU time (alpha 0.25), shown
    in sample code.
- **ProMotion rates.** [D]
  <https://developer.apple.com/documentation/quartzcore/optimizing-iphone-and-ipad-apps-to-support-promotion-displays>
  - iPad Pro: 24–120 Hz. iPhone: 10–120 Hz.
  - Discrete steps: 120 (8 ms), 80 (12), 60 (16), 48 (20), 40 (25), 30 (33), 24 (41), 20, 16, 15,
    12 and 10 Hz (iPhone).
  - "Core Animation arbitrates the animations it presents onscreen and determines the refresh rate".
  - UIKit, SwiftUI, CAAnimation and SpriteKit pace automatically.
  - On iPhone, rates above 60 Hz need `CADisableMinimumFrameDurationOnPhone`.
  - "Always use `targetTimestamp` to drive any animation"; "`targetTimestamp` is your deadline to
    submit changes for the next frame".
  - Working time = `targetTimestamp − CACurrentMediaTime()`.
  - "Lengthy callback execution times can prevent refresh intervals from receiving CADisplayLink
    callbacks."
  - "Group animations with similar timings together in one CADisplayLink."
  - Suggested ranges: high-impact animations `CAFrameRateRange(80,120,120)`; alpha or colour changes
    use the default; small slow animations use 8–15, 15–24 or 30–48 Hz.
- **`CADisplayLink.preferredFrameRateRange`** (iOS 15, macOS 14). The system "typically provides a
  consistent frame rate by choosing one that's a factor of the display's maximum refresh rate".
  Low Power Mode, thermal state and accessibility can cap it. [D]
  <https://developer.apple.com/documentation/quartzcore/cadisplaylink/preferredframeraterange>
- **macOS 14 `NSView.displayLink(target:selector:)`** returns a CADisplayLink tied to the view's
  display. "If the view is hidden, or not on any display, the callback will not be invoked." [D]
  <https://developer.apple.com/documentation/appkit/nsview/displaylink(target:selector:)>

### 1.6 CAMetalLayer latency knobs and CAMetalDisplayLink

All [D] unless noted.

- **`maximumDrawableCount`**: only 2 or 3 is allowed, and "The default value is 3." (iOS 11.2,
  macOS 10.13.2.)
  <https://developer.apple.com/documentation/quartzcore/cametallayer/maximumdrawablecount>
- **`nextDrawable()`**: "Waits until a Metal drawable is available ... If all drawables are in use,
  the layer waits up to one second for one to become available, after which it returns nil." This
  is the compositor's back-pressure. `allowsNextDrawableTimeout` changes it.
  <https://developer.apple.com/documentation/quartzcore/cametallayer/nextdrawable()>
- **`displaySyncEnabled`** (macOS 10.13+): if false, "the layer presents new content more quickly,
  but possibly with brief visual artifacts (screen tearing)". The default is true.
  <https://developer.apple.com/documentation/quartzcore/cametallayer/displaysyncenabled>
- **`presentsWithTransaction`**: the default is false, which means present "as quickly as possible
  and asynchronously to any Core Animation transactions. Core Animation doesn't guarantee that the
  Metal content arrives in the same frame as other Core Animation content."
  - When true, the drawable shows in the current CATransaction.
  - The recipe is: commit the command buffer, `waitUntilScheduled()`, then `drawable.present()`.
    Don't use `commandBuffer.present(_:)`.
  - WWDC15 233 describes the default as getting "your GPU content to the display as quickly as
    possible".
  <https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction>
- **`MTLDrawable.present(afterMinimumDuration:)`** (iOS 10.3, macOS 10.15.4): presents once "a
  previous drawable has been visible onscreen for the specified duration. Use this method to
  schedule drawables at a regular interval."
  <https://developer.apple.com/documentation/metal/mtldrawable/present(afterminimumduration:)>
- **`CAMetalDisplayLink`** (iOS 17, macOS 14): "a specialized way to interact with variable-rate
  displays when you need more control over the timing window to render your app's frames."
  <https://developer.apple.com/documentation/quartzcore/cametaldisplaylink>
  - It is created from a `CAMetalLayer`. The delegate's `metalDisplayLink(_:needsUpdate:)` receives
    an `Update` carrying the drawable and target timestamps. The display link supplies the drawable,
    so there is no separate `nextDrawable` call [D for the Update class existing; detail of fields
    from general knowledge, I].
  - It is added to any run loop, so it can run on a dedicated render thread [I from "add(to:forMode:)"].
  - **`preferredFrameLatency`**: "The amount of time, in frames, your app requests to render a frame
    ... The final latency may be bigger if the system needs more time, such as for windowed modes on
    macOS. ... The only acceptable values are 1.0 and 2.0."
    <https://developer.apple.com/documentation/quartzcore/cametaldisplaylink/preferredframelatency>
  - Using present-at-time variants with it asserts. [D, via search snippet of the delegate doc]
- **Field reports.** [S]
  - Apple DTS recommends CAMetalDisplayLink over CADisplayLink for pacing. It says two drawables are
    discouraged on iOS but acceptable on macOS within budget.
  - A developer saw fullscreen 120 Hz average about 80 Hz with long `nextDrawable` waits even though
    CPU+GPU work took 2–3 ms. Three drawables added latency.
  - <https://developer.apple.com/forums/thread/763426>
  - An iOS developer saw presentation land two vsyncs after GPU completion with
    `maximumDrawableCount=2`: <https://www.developer.apple.com/forums/thread/780668>
  - Lesson [I]: on Apple, frame acquisition and present timing are throttled by the compositor, and
    windowed (composited) mode adds latency that only direct-to-display fullscreen avoids.

### 1.7 SwiftUI: dependency (attribute) graph and diffing

- **WWDC21 10022, "Demystify SwiftUI".** [D] <https://developer.apple.com/videos/play/wwdc2021/10022/>
  - Views are value types that are cheap to create. Identity is structural (type and position) or
    explicit (`id`). State lives with identity.
  - "this is actually a graph, not a tree ... the dependency graph". "if the dependency changes,
    only those views will be invalidated". "Because views are value types, SwiftUI can efficiently
    compare them to only update the right subset of views."
- **WWDC23 10160, "Demystify SwiftUI performance".** [D] <https://developer.apple.com/videos/play/wwdc2023/10160/>
  - An update has three steps: produce a new view value, update the dynamic properties from the
    graph, then run `body`.
  - Updating recurses "only updating those views that have new values or other changed
    dependencies".
  - `@Observable` limits dependencies "to only that which is read".
  - List and Table IDs are gathered eagerly. Keep the number of views per element constant.
- **WWDC25 306, "Optimize SwiftUI performance with Instruments".** It is explicit about the
  **attribute graph**. [D] <https://developer.apple.com/videos/play/wwdc2025/306/>
  - "View structs are recreated frequently, but attributes keep their identity and maintain state".
  - A state change "creates a new transaction ... a change ... that needs to be made before the next
    frame". The transaction "will mark the signal attribute ... as outdated", then "walks down the
    chain of attributes that depend on the now-outdated attribute, marking each one as outdated by
    setting a flag. Setting the flag happens really quickly, and no additional work happens just
    yet."
  - Values are recomputed only when the next frame needs them. This is a push-dirty, pull-compute
    model.
  - "All of this work has to complete before each frame deadline."
  - "view bodies run on the main thread". Many small updates can also add up to a missed deadline.
  - Environment reads cost a check on every environment change, so frequently changing values
    (geometry, timers) should be kept out of the environment.

### 1.8 TextKit 2: viewport-based layout

- **WWDC21 10061, "Meet TextKit 2".** [D] <https://developer.apple.com/videos/play/wwdc2021/10061/>
  - "TextKit 2 uses viewport-based layout and rendering".
  - Layout is always noncontiguous: "we can lay out a piece of the text anywhere within the document
    without laying out the pieces that come before it".
  - Work covers only the visible area plus an over-scroll region.
  - `NSTextViewportLayoutController` drives a will-layout, configure-per-fragment, did-layout cycle.
    In the sample, each `NSTextLayoutFragment` is mapped to its own CALayer inside one
    `CATransaction`.
  - Fragments are immutable values.
  - Layout outside the viewport is estimated "unless you explicitly ask to ensure layout ... This
    call can be expensive".
  - Apple claims documents of "hundreds of megabytes being scrolled through at interactive rates"
    (no benchmark given).

---

## Part 2: Android

### 2.1 Choreographer: the vsync-driven frame

- **Project Butter, Android 4.1 (2012).** [D] <https://developer.android.com/about/versions/jelly-bean>
  - "Everything runs in lockstep against a 16 millisecond vsync heartbeat — application rendering,
    touch events, screen composition, and display refresh".
  - It added triple buffering.
  - Touch was synchronized to vsync and "anticipating where your finger will be at the time of the
    screen refresh".
  - A "CPU input boost at the next touch event" after idle.
  - The Choreographer API was introduced.
- **`Choreographer.java` (AOSP main).** [D]
  <https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/java/android/view/Choreographer.java>
  - **Callback order inside `doFrame`:**
    1. `CALLBACK_INPUT` (0, "Runs first")
    2. `CALLBACK_ANIMATION` (1)
    3. `CALLBACK_INSETS_ANIMATION` (2; "Both input and animation may change insets, so we need to
       run this after these callbacks, but before traversals")
    4. `CALLBACK_TRAVERSAL` (3, "Handles layout and draw")
    5. `CALLBACK_COMMIT` (4, "post-draw operations")
  - **Vsync is requested only when a frame is scheduled.** `scheduleFrameLocked` calls
    `mDisplayEventReceiver.scheduleVsync()` once per pending frame. With nothing to do there is no
    vsync and no wakeup [D code; idle implication I].
  - **Jitter handling.** If the frame starts at least one interval late, the frame time is snapped
    back to the last vsync boundary. It logs "Skipped N frames! The application may be doing too much
    work on its main thread" once N reaches 30 (default `debug.choreographer.skipwarning`).
  - **Vsync messages are posted async at the vsync timestamp** "to prevent incoming vsync events from
    completely starving" other messages.
  - **Frame timelines (Android 13+ API).** `FrameData` carries several `FrameTimeline`s (vsync id,
    deadline, expected presentation time). A late frame picks the next timeline whose deadline has
    not passed. `getExpectedPresentationTimeNanos()`: "This time should be used to advance any
    animation clocks." This is the equivalent of Apple's `targetTimestamp`.
- **Vsync offsets (DispSync).** [D] <https://source.android.com/docs/core/graphics/implement-vsync>
  - "DispSync is a software phase-lock loop (PLL) that generates the VSYNC and SF_VSYNC signals used
    by Choreographer and SurfaceFlinger", fed back by HWC retire fences.
  - At each vsync, the display shows frame N while SF composites N+1 and the app handles input and
    renders N+2. "The latency is at least two frames."
  - App and SF phase offsets (`VSYNC_EVENT_PHASE_OFFSET_NS`, `SF_VSYNC_EVENT_PHASE_OFFSET_NS`)
    shorten this. They "reduce the time available for app and composition, providing a greater
    chance for error".
  - HWC vsync callback latency is "typically less than 0.5 ms".
  - "More than a millisecond or two of latency is noticeable" (the doc's phrasing).
- **SurfaceFlinger.** [D] <https://source.android.com/docs/core/graphics/arch-sf-hwc>
  - "When SurfaceFlinger receives the VSync signal, it walks through its list of layers looking for
    new buffers ... if not, it continues to use the previously acquired buffer."
  - HWC decides between overlay and client (GPU) composition.
  - BufferQueue passes handles and never copies. "SurfaceFlinger wakes only when there's work to be
    done ... disables VSync if nothing is updating the screen."
    <https://source.android.com/docs/core/graphics/arch-bq-gralloc>

### 2.2 Input: batching, historical samples, resampling, unbuffered dispatch

- **Batching into frames.** [D]
  - `InputEventReceiver.onBatchedInputEventPending`: "The batched input event will continue to
    accumulate additional movement samples until the recipient calls consumeBatchedInputEvents or an
    event is received that ends the batch and causes it to be consumed immediately (such as a pointer
    up event)."
  - `consumeBatchedInputEvents(frameTimeNanos)`: "Should be called just before animating or drawing a
    new frame in the UI."
  - <https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/java/android/view/InputEventReceiver.java>
  - ViewRootImpl posts the consumption as a `CALLBACK_INPUT` Choreographer callback, and skips that
    when unbuffered dispatch is on (`if (!mUnbufferedInputDispatch) scheduleConsumeBatchedInput()`).
    [S] Blog excerpts of ViewRootImpl. Callback ordering [D] in Choreographer.
  - Takeaway [I]: MOVE samples are batched per frame, while DOWN/UP and keys flush the batch
    immediately, so discrete events are not delayed to vsync.
- **Historical samples.** "For efficiency, motion events with ACTION_MOVE may batch together multiple
  movement samples within a single object." Read them with `getHistoricalX/Y` before `getX/Y`. [D]
  <https://developer.android.com/reference/android/view/MotionEvent> (section "Batching").
- **Resampling.** [D] `Resampler.cpp` (android-16.0.0_r2):
  <https://android.googlesource.com/platform/frameworks/native/+/android-16.0.0_r2/libs/input/Resampler.cpp>
  - `resampleTime = frameTime − RESAMPLE_LATENCY (5 ms)`.
  - It *interpolates* between the last past sample and a future sample when one is queued (gap of
    at least 2 ms).
  - Otherwise it *extrapolates* from two samples 2–20 ms apart, capped at
    `min(delta/2, RESAMPLE_MAX_PREDICTION = 8 ms)` past the newest sample.
  - It is enabled by `ro.input.resampling` (default on). There is an optional one-euro-style filter
    (`FilteredLegacyResampler`: `minCutoffFreq`, `beta`).
  - The older InputTransport.cpp comment: a few milliseconds of latency "costs little but reduces the
    impact of mispredicted touch positions".
  - Android 16 also has an `InputConsumerNoResampling` with a comment that "When there's double (or
    triple) buffering, you may want to not consume all events currently available". [D]
    <https://android.googlesource.com/platform/frameworks/native/+/570e2d6e29/include/input/InputConsumerNoResampling.h>
- **`View.requestUnbufferedDispatch(MotionEvent)`** asks that events not be batched to vsync. The
  stylus codelab calls it on ACTION_DOWN: "Ask that the input system not batch MotionEvent objects,
  but instead deliver them as soon as they're available." [D]
  <https://developer.android.com/codelabs/large-screens/advanced-stylus-support>
  - Chromium added a switch to call it, as "preparation for enabling compositor event queue". In
    other words, an engine with its own vsync-aligned input queue turns off the OS batching. [D]
    <https://codereview.chromium.org/2710683003>
- **Motion prediction.** `androidx.input:input-motionprediction` (`MotionEventPredictor.record()` and
  `predict()`). Predicted points are drawn to the front buffer only, and "Final rendering ... should
  ignore the prediction". [D] Codelab above.

### 2.3 RenderThread and HWUI display lists (RenderNode)

- **The display-list model.** [D] <https://developer.android.com/develop/ui/views/graphics/hardware-accel>
  - The stages are invalidate the hierarchy, record and update display lists, draw the display
    lists.
  - "the Android system only needs to record and update display lists for views marked dirty by an
    invalidate() call. Views that have not been invalidated can be redrawn simply by re-issuing the
    previously recorded display list."
  - Example: `setAlpha(0.5)` on a ListView re-records only the parent's tiny list
    (`SaveLayerAlpha`/`DrawDisplayList`/`Restore`). "The complex drawing code of ListView was not
    executed."
  - Properties that need no re-record: alpha, x/y/translation, scale, rotation and pivot. They give
    "optimal invalidation and no redrawing of the targeted view."
  - Pitfall: "Forgetting [invalidate()] causes a view to look the same even after it has been
    changed."
  - Hardware layers (`LAYER_TYPE_HARDWARE`) cache a subtree as a texture for alpha/transform
    animations. They cost VRAM, so enable them only for the animation.
- **RenderNode (API 29) exposes this publicly:** `beginRecording`/`endRecording` and property setters.
  [D for existence; details as described in the hardware-accel doc]
  <https://developer.android.com/reference/android/graphics/RenderNode>
- **RenderThread (Android 5.0, 2014).** "A new system-managed processing thread called RenderThread
  keeps animations smooth even when there are delays in the main UI thread." [D]
  <https://developer.android.com/about/versions/lollipop>
  - Ripples run as `RenderNodeAnimator`s on RenderThread. The AOSP commit "Make ripples silky smooth"
    removed the software fallback. [D] <https://android.googlesource.com/platform/frameworks/base/+/7e4af7a%5E%21/>
  - Display-list creation and changes still happen on the UI thread. [S]
- **The UI/RenderThread handoff (sync).** [D] `DrawFrameTask.cpp` (android16-release):
  <https://android.googlesource.com/platform/frameworks/base/+/android16-release/libs/hwui/renderthread/DrawFrameTask.cpp>
  1. The UI thread calls `drawFrame()`, which runs `postAndWait()` and blocks.
  2. On RenderThread, `syncFrameState()` passes vsync id, deadline and interval to `timeLord`, then
     `prepareTree()`. That pushes staged RenderNode properties and display lists, and applies
     deferred layer updates.
  3. If `canUnblockUiThread` (texture uploads fit in cache), the UI thread is released **before** the
     GPU draw. Otherwise it is released after the draw.
  4. RenderThread draws and swaps. A skipped frame flushes and waits on fences.
  - Result [I]: the UI thread is blocked only for the sync (a copy of staged deltas). It then records
    frame N+1 while RenderThread draws N, a two-stage pipeline inside the app.
- **Where time goes.** [D] "Profile GPU rendering" bars:
  <https://developer.android.com/topic/performance/rendering/inspect-gpu-rendering>
  - Input handling and animation, Measure/Layout, Draw ("time used to record drawing commands"),
    Sync & Upload (bitmaps to GPU), Command Issue, Swap Buffers (waiting on GPU), and Misc/VSync
    delay.
  - Slow-rendering vitals:
    - Frame budgets are 16 ms at 60, 11 at 90 and 8 at 120 Hz.
    - Overrunning by 1 ms means "Choreographer drops the frame entirely".
    - Frozen frames are >700 ms.
    - A first-use bitmap upload takes "several milliseconds".
    - Path edits defeat caching.
    - Prefer `setTranslationX`/`setAlpha` to layout changes.
    - Bind should take "much less than one millisecond".
    - <https://developer.android.com/topic/performance/vitals/render>

### 2.4 BLAST and SurfaceControl transactions

- [D, commit messages] BLAST (BLASTBufferQueue) moves buffer submission into `SurfaceControl`
  transactions, so a buffer and geometry changes (size, position, other surfaces) apply atomically
  for one frame.
  - Launcher3: "the SurfaceControl transaction was being applied asynchronously, which could lead to
    it being executed out of sync with launcher drawing. This became an issue at higher refresh
    rates ... we can use BLAST transactions, which are annotated with a frame number."
    <https://android.googlesource.com/platform/packages/apps/Launcher3/+/217b1bfaf6>
  - BBQ caps a "shadow queue" at one frame during sync so that `dequeueBuffer` cannot block the UI
    thread or RenderThread: <https://android.googlesource.com/platform/frameworks/native/+/e9323b30ab>
- Mapping [I]: this is Android's version of Apple's `presentsWithTransaction`. Normally a surface
  (SurfaceView or a 3D viewport) presents independently and as early as possible. During resize or
  transitions, its buffer is synced into the parent's transaction to avoid tearing and flicker.

### 2.5 FrameTimeline and jank vocabulary

- [D] <https://perfetto.dev/docs/data-sources/frametimeline> (Android 12+)
  - Jank means the actual present time differs from the predicted present time.
  - Expected and actual timelines are kept per frame, with the Choreographer token (vsync id) on the
    app's `doFrame` and RenderThread slices and on SurfaceFlinger's.
  - Jank types:
    - `AppDeadlineMissed`
    - `BufferStuffing`: "app queues frames faster than they are presented, adding latency. Frames
      stay smooth, but input latency increases"
    - `SurfaceFlingerCpuDeadlineMissed`, `SurfaceFlingerGpuDeadlineMissed`
    - `DisplayHAL`
    - `PredictionError`
    - `Unknown`
  - Present types are early, on time, late or dropped.
- **JankStats / FrameMetricsAggregator** collect per-frame data in the field. Trace markers cost
  about 10 µs each. [D] vitals/render page.

### 2.6 Jetpack Compose: three phases and phase skipping

- [D] <https://developer.android.com/develop/ui/compose/phases>
  - Composition ("what"), layout (measure and place; "each node is visited once"), drawing.
  - A state read is tracked by the phase it happens in:
    - A read in a composable recomposes that scope.
    - A read in a measure/layout lambda (`Modifier.offset { }`) re-runs only layout and draw.
    - A read in a draw lambda (`drawBehind`, `Canvas`) re-runs only draw.
  - "Avoid backwards writes" (layout → state → composition loops).
- [D] <https://developer.android.com/develop/ui/compose/performance/bestpractices>
  - "Deferring state reads will ensure that Compose re-runs the minimum possible code".
  - "When you are passing frequently changing State variables into modifiers, you should use the
    lambda versions of the modifiers whenever possible."
  - An animated colour read in `drawBehind` skips composition and layout entirely.
- [D] `graphicsLayer`: <https://developer.android.com/develop/ui/compose/graphics/draw/modifiers>
  - "Drawing instructions captured as part of a layer can be re-issued efficiently by the rendering
    pipeline without re-executing application code" (RenderNode-backed).
  - Transforms "can be animated or modified without needing to re-execute the drawing lambda".
  - The lambda form is for animations.
  - CompositingStrategy is Auto (offscreen only if alpha<1 or a RenderEffect is set), Offscreen, or
    ModulateAlpha.
- **Scheduling.** [D] androidx source.
  - `withFrameNanos` is implemented with `Choreographer.postFrameCallback`.
    <https://raw.githubusercontent.com/androidx/androidx/androidx-main/compose/ui/ui/src/androidMain/kotlin/androidx/compose/ui/platform/AndroidUiFrameClock.android.kt>
  - `AndroidUiDispatcher` dispatches coroutines "during a handler callback or choreographer's
    animation frame stage, whichever comes first". Queued UI work is therefore drained inside the
    frame, before traversal, rather than spilling into the next frame.
    <https://raw.githubusercontent.com/androidx/androidx/androidx-main/compose/ui/ui/src/androidMain/kotlin/androidx/compose/ui/platform/AndroidUiDispatcher.android.kt>

### 2.7 Front-buffer rendering for ink (androidx.graphics.lowlatency)

- [D] `GLFrontBufferedRenderer` (requires API 29). KDoc in source:
  <https://raw.githubusercontent.com/androidx/androidx/androidx-main/graphics/graphics-core/src/main/java/androidx/graphics/lowlatency/GLFrontBufferedRenderer.kt>
  - "Active content is rendered first into the front buffered layer which is simultaneously being
    presented to the display. Periodically content is rendered into the multi buffered layer which
    will have more traditional latency guarantees". The multi-buffered layer exists "to minimize the
    impact of visual artifacts due to graphical tearing."
  - `renderFrontBufferedLayer(param)` renders each new segment incrementally. `commit()` re-renders
    the whole scene into the multi-buffered layer and hides the front layer.
  - Two SurfaceControls are updated atomically through transactions.
  - Callbacks run on a dedicated GL thread ("it is important this Runnable does not block").
- [D] Stylus codelab: <https://developer.android.com/codelabs/large-screens/advanced-stylus-support>
  - DOWN/MOVE → `renderFrontBufferedLayer(segment)`.
  - UP → `commit()`, which replays all segments into the double-buffered layer.
  - Prediction goes to the front buffer only and is erased with scissor.
  - Call `requestUnbufferedDispatch` on DOWN.
- **Numbers.** Android's low-latency APIs are cited as reaching "as low as 4ms" end-to-end (Samsung
  Tab S8 demo). [D, marketing] <https://android-developers.googleblog.com/2024/10/introducing-ink-api-jetpack-library.html>

---

## Part 3: Side by side

| Concern | Apple | Android | Notes |
|---|---|---|---|
| Who owns the view tree | Main thread only | UI thread only | [D] both |
| What crosses to the renderer | Layer tree deltas plus backing-store bitmaps, sent by IPC to the render server | RenderNode display lists plus staged properties, synced to the in-process RenderThread | [D]/[I] |
| Re-record avoidance | `setNeedsDisplay` dirty flag; layer properties change without redraw | `invalidate()` dirty flag; RenderNode properties change without re-record; Compose read-phase tracking | [D] |
| Animation off the main thread | CAAnimation runs in the render server (render tree); model vs presentation values | RenderNodeAnimator on RenderThread (ripples, reveal); ordinary ValueAnimators tick on UI-thread `CALLBACK_ANIMATION` | [D]/[S] |
| Frame phases | event → display-link callbacks → CA commit (layout, display, prepare, commit) → render prepare → render execute → display | input → animation → insets → traversal (measure, layout, draw/record) → commit; sync → RenderThread draw → SF latch → HWC | [D] |
| Animation clock | `targetTimestamp` / `modelTime` | `getExpectedPresentationTimeNanos` / frame time | [D] |
| Base latency | 3 vsync to last pixel (25 ms @120 Hz) double-buffered; 4 when triple-buffered | at least 2 frames without offsets, plus scanout | [D] |
| Input coalescing | Per frame, with coalesced plus predicted touches; AppKit mouse coalescing on by default | MOVE batched per frame, historical samples, resampled at frame−5 ms, extrapolation capped at 8 ms | [D] |
| Low-latency escape hatch | `wantsLowLatencyEventDispatch` (mid-frame, half budget), `wantsImmediatePresentation` (−1 frame), `presentsWithTransaction=false`, `displaySyncEnabled=false` (macOS) | `requestUnbufferedDispatch`, front-buffer rendering (about 4 ms), vsync phase offsets | [D] |
| Idle | `requiresContinuousUpdates=false`; display link fires only when driven; ProMotion drops to 10/24 Hz | vsync requested only when a frame is scheduled; SF disables vsync when idle | [D]/[I] |
| Sync of independent surfaces | `presentsWithTransaction` | BLAST transactions with frame numbers | [D] |
| Metrics | hitch time ratio ms/s; commit vs render hitch | FrameTimeline jank types; slow (>budget) and frozen (>700 ms) frames | [D] |

---

## Part 4: Lessons for cadatomic's framework (all [I], derived from the above)

1. **Use a two-stage pipeline in our own process, the way HWUI does, rather than one immediate-mode
   loop.**
   - The UI thread owns the retained scene. It applies input, ticks UI-thread animations, runs
     layout, records display lists for dirty nodes only, then does a short **sync** that hands
     staged deltas to a render thread.
   - The render thread owns wgpu. It replays cached per-node display lists and GPU resources, runs
     compositor-side property animations, and acquires and presents the surface.
   - The UI thread is released right after the sync, before GPU submission, like `canUnblockUiThread`.
   - JS (QuickJS) runs on its own thread. It is never something the frame waits on.
2. **Keep per-node display lists and compositor-only properties.**
   - Changes to transform, opacity, clip, scroll offset and visibility update a property on the
     render-thread copy and re-record nothing.
   - Only content changes (text, path, colour inside a node) re-record that node.
   - Rasterized layer caches are opt-in for subtrees that animate as a whole, such as dock tab drags,
     and are released afterwards (Android's advice on VRAM).
   - Expensive effects (blur, masks, shadows) get explicit geometry so they need no offscreen pass.
3. **Track reads per phase, the way Compose and SwiftUI do.**
   - When a Svelte `$state` read happens inside a style or binding that maps to a paint-only property,
     invalidate only paint for that node. A layout property invalidates layout. Only structural
     changes reach the JS/component layer.
   - Invalidation is push-dirty, pull-compute: mark flags when a change happens and compute values
     at the frame, which is SwiftUI's transaction model.
   - Lists need stable explicit ids, as SwiftUI and Compose require.
4. **Keep the model value separate from the presentation value.**
   - JS or Rust sets the target value plus a transition spec (duration, curve, spring). The render
     thread interpolates against the presentation timestamp each vsync.
   - Hit-testing and "start from current" read the presentation value.
   - Hover, press, focus rings, scroll inertia, splitter and tab drags, and viewport orbit feedback
     should all be toolkit-native or compositor animations, not JS.
5. **Schedule frames on demand and pace them to vsync.**
   - With no dirty state and no running animation, request no vsync, so idle CPU is zero
     (Choreographer, UIUpdateLink `requiresContinuousUpdates=false`).
   - When work arrives, request one frame. Drive animations from the *target presentation time*, not
     the callback time, and account for skipped frames with deltas.
   - Phase order within a frame:
     1. Drain and coalesce input.
     2. Animation tick.
     3. Bounded JS microtask flush.
     4. Layout.
     5. Record.
     6. Sync.
   - Compute deadlines backwards from the presentation time: commit deadline = presentation − render
     budget.
   - Start the UI phase as late as is safe (a vsync phase offset, or `presentAfterMinimumDuration`
     using an EMA of render cost) so the input sample is fresh.
6. **Input path.**
   - Dispatch discrete events (key down, click, pointer down/up) immediately on arrival. Don't wait
     for vsync, because Android flushes the batch on DOWN/UP and Apple "may process events as soon as
     they come in".
   - Coalesce continuous motion per frame, but keep every sample (like coalesced touches or
     `getHistorical*`) for the schematic and PCB canvases, the router and ink.
   - Resample to frame time minus a small latency. Android uses 5 ms and caps extrapolation at
     8 ms or half the sample gap.
   - Predict about one frame ahead for drag feedback and replace the prediction with real samples.
   - We own the queue, so OS-level batching should be bypassed where possible, as Chromium does.
7. **Present path on wgpu.**
   - Aim for 1–2 frames in flight. Set `desired_maximum_frame_latency` to 1–2, which matches Apple's
     `preferredFrameLatency` values of 1.0 and 2.0.
   - Acquire the surface texture as late as possible, because the acquire blocks under compositor
     back-pressure, like `nextDrawable`'s one-second wait.
   - Never queue frames faster than they are presented; that is Android's "buffer stuffing" jank,
     which adds latency.
   - Give the 3D viewports their own surfaces where the platform allows (a CAMetalLayer sublayer or
     a subsurface) so they present independently. Sync them with the UI frame only during resize or
     transitions (`presentsWithTransaction`, BLAST).
   - Default to vsync-on with a mailbox-like "replace pending frame" mode when supported. Offer
     tearing or immediate present only for pen/ink and fullscreen viewport modes.
8. **Budget for latency honestly.**
   - On macOS and iOS the best composited path is about 3 vsync intervals (25 ms @120 Hz). Anything
     beyond that is ours.
   - Today's egui and eframe stack adds about 2–3 frames because JS runs a frame late and present is
     queued. The target is input to commit inside the same vsync, with no extra queued frames.
   - For ink or pen-like interactions (sketching, freehand selection), consider a front-buffer-style
     overlay layer that is drawn incrementally and committed into the main scene on release.
9. **Text.**
   - The code editor and large schematic or netlist views should use viewport-based, noncontiguous
     layout (TextKit 2): lay out only the visible lines plus overscroll, with estimates elsewhere.
   - Cache one immutable layout fragment and display list per paragraph or line so that scrolling is
     a transform change.
10. **Instrument from the start.**
    - Record per-frame expected and actual timelines with frame ids across the UI thread, the JS
      thread and the render thread (FrameTimeline).
    - Classify misses as commit hitch (UI/JS late), render hitch (GPU late), or stuffing.
    - Report a hitch time ratio. The target is <5 ms/s, Apple's "good" band in tech talk 10855.
