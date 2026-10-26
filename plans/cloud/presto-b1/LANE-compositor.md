# Lane COMPOSITOR: presto-compositor

Branch `ci/presto-compositor`. Owns `crates/presto-compositor/**`. Read: architecture §7, §10, §12; notes
`round2/wgpu-integration.md` (threading, surfaces, the resize handshake per OS, latency knobs, wgpu facts),
`round2/scheduler-latency.md` §2.12 (the compositor loop pseudocode).

Depends on presto-render, presto-frame, presto-anim APIs (the brief) which other lanes are writing in
parallel: code against the brief's signatures; until those crates land, keep a thin internal trait at each
seam so your crate compiles, and switch to the real crates when the coordinator merges their skeletons into
`ci/presto-base` (merge `ci/presto-base` into your branch then).

Deliver (the brief's `Compositor`/`CompositorHandle` API first, within 45 min):
1. The thread: one per process; owns `wgpu::Instance` (exposed for main-thread surface creation), adapter
   selection (DX12 forced on Windows; Metal; Vulkan, then GL fallback; lavapipe/WARP allowed), `Device`,
   `Queue`; one multiplexed wait loop (channel + deadline timer; per-OS clock sources plug in through a
   `FrameSource` trait the platform lane implements later — start with a timer-based vsync estimate plus
   presentation timing from `queue.on_submitted_work_done`).
2. Per window: the committed layer tree, property trees, display lists, runs, resources; applying and folding
   commits; fast commits (scroll offset, transform, opacity, canvas view, caret); revision checks.
3. Animations and scroll physics sampled at the frame's target time (presto-anim); `ToUi::Animation` events;
   compositor-owned flings report `ToUi::ScrollMoved`.
4. Damage (architecture §7.4) per surface image (buffer-age correct), flattening to `FrameScene` items with
   resolved transforms/clips/opacity, `Renderer::render`, present; skip present on empty damage.
5. Frame pacing through `presto_frame::FrameClock` (`ToUi::BeginFrame` at `start_at`), frame latency 1,
   `desired_maximum_frame_latency = 1`, acquire late.
6. The resize handshake (`resize` → reconfigure → render → present → `ToUi::ResizeDone`), OS-layer surfaces
   (`add_os_layer`), registered textures (egui islands).
7. Device loss: rebuild from CPU-side state, `ToUi::DeviceReset`.

Tests (write, don't run): commit folding; fast-commit revision rejection; damage computation; a headless
frame (GPU-gated) of a small layer tree; the loop with a fake clock: a fling keeps producing frames with no
commits.
