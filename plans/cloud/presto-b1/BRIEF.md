# presto batch 1 (wave 1: foundations): common brief

Date: 2026-10-25. Coordinator: the session that wrote this brief (cadatomic/svelters batch coordinator).
The user asked for the whole presto toolkit to be built now ("implement the full framework … minimize
building and testing till consolidation … have it ready to go tonight"). This batch builds the
foundations; waves 2 and 3 follow as soon as the APIs below exist.

## Where you work

- Repository: svelters. Base: branch `ci/presto-base` (the shared crate `presto-base`, the empty presto
  crates, the CI workflow `.github/workflows/presto.yml`, this brief).
- Your branch: `ci/presto-<lane>` from `ci/presto-base`. Push only to it, early and often (at least every
  hour; first push within 45 minutes). No PRs. Never push `master` or another lane's branch.
- Your lane file: `plans/cloud/presto-b1/LANE-<lane>.md`. Your record: `plans/milestones/PRESTO-<lane>.md`
  (what landed, what is stubbed, what you did not run, questions for the coordinator).

## Read first

1. `CLAUDE.md` (svelters) — but this batch overrides its testing rule as below, by the user's instruction.
2. `plans/research/presto/README.md` (decisions), then `architecture.md`; the sections your lane file names.
3. `crates/presto-base` — the shared vocabulary. It is the contract between lanes. **Don't change it on
   your branch** except additively (new items), and list every addition in your record; anything else
   goes to the coordinator first.
4. The research notes your lane file names (`plans/research/presto/notes/`, `notes/round2/`).

## Building and testing in this batch (user, 2026-10-25)

- **Minimise building.** `cargo check -p <your crates>` only, when you need the compiler's answer. Set
  `CARGO_PROFILE_DEV_DEBUG=0`. No `cargo build` of other crates, no full-workspace checks.
- **Write tests, don't run them** (beyond a single filtered run of a test you are debugging, if you must).
  GitHub CI runs everything on every push to `ci/**`: the `presto` workflow (Linux, Windows, macOS) and
  svelters' five workflows. Read CI's results after a push, fix in one batch, push again.
- The consolidation runs the tests and fixes build breaks across lanes.

## Rules

- Stay in your files (your lane file lists them). An edit to another lane's file goes under "cross-lane
  edits" in your record, with the reason.
- **No placeholders that pretend to work.** An API that isn't implemented yet is `todo!()`-free: either
  implement it or leave it out and list it in your record. A stub that returns a fake success is a bug.
- Code reads like the surrounding svelters code: module docs that say what the module computes, no
  history in comments, names that say what they compute. Comment density like svelters'.
- Port code with attribution: GPUI files (Apache-2.0, `crates/gpui*` of zed-industries/zed only — never
  Zed's GPL crates) keep a header naming the source file and commit and "Modifications by the svelters
  authors"; the Windows Terminal gamma functions keep their MIT notice. Add each source to
  `THIRD_PARTY_NOTICES.md`.
- Never guess a missing fact (an OS API detail, a constant): look it up (the notes cite sources), or
  record it as a blocker.
- No credentials in committed files. Commit messages end with your session's attribution trailers.
- Questions: write them with a recommendation in your record and keep going; the coordinator answers.

## The cross-lane APIs (keep these signatures; extend freely)

Each lane owns one of these and pushes it **compiling, early** (stubs that are honestly unimplemented are
fine at first: keep the signature, return an `Err`/`None` that says "not implemented" — never a fake
success). The coordinator merges early skeletons into `ci/presto-base` so wave-2 lanes can start.

### presto-text (lane TEXT)
```rust
pub struct FontStore;                       // Send + Sync, shared as Arc
impl FontStore { pub fn new(cfg: FontConfig) -> Arc<Self>; }
pub struct FontConfig { pub system_fonts: bool, pub bundled: Vec<Vec<u8>>, pub default_family: String }
pub struct FontRasterizer;                  // impl presto_base::text::GlyphRasterizer
impl FontRasterizer { pub fn new(fonts: Arc<FontStore>) -> Self; }
pub struct TextSystem;                      // UI thread; impl presto_base::text::TextMeasure
impl TextSystem {
    pub fn new(fonts: Arc<FontStore>) -> Self;
    pub fn layout(&mut self, text: &RichText, max_width: Option<f32>) -> Arc<TextLayout>;
}
pub struct RichText { pub text: String, pub spans: Vec<Span> /* byte range + TextStyle */, pub base: TextStyle, pub align: Align, pub ellipsis: bool, pub max_lines: Option<u32> }
pub struct TextLayout { /* lines, size, runs: Vec<PlacedRun { run: Arc<GlyphRun>, origin: Point, style: u32 }> */ }
impl TextLayout {
    pub fn size(&self) -> Size;
    pub fn runs(&self) -> &[PlacedRun];
    pub fn hit_test(&self, p: Point) -> Cursor;                 // byte offset + affinity
    pub fn caret_rect(&self, byte: usize) -> Rect;
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect>;
}
```

### presto-render (lane RENDER)
```rust
pub struct Renderer;
impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat,
               glyphs: Arc<dyn presto_base::text::GlyphRasterizer>) -> Self;
    pub fn register_texture(&mut self, id: TextureId, view: wgpu::TextureView, size: DeviceSize);
    pub fn unregister_texture(&mut self, id: TextureId);
    /// Encode one frame into `target` (only `scene.damage`, scissored; full when empty).
    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder,
                  target: &wgpu::TextureView, scene: &presto_base::scene::FrameScene<'_>) -> RenderStats;
    pub fn end_frame(&mut self);            // atlas LRU generation, instance-ring advance
}
pub fn render_to_image(/* headless: device, queue, scene */) -> image bytes   // for tests
```

### presto-anim and presto-frame (lane FRAME-ANIM)
```rust
// presto-anim
pub fn sample(gen: &Generator, start: Time, now: Time) -> Sample;   // Sample { value: AnimValue, phase: Phase, done: bool, velocity: f32 }
pub fn placement(p: AnimProperty) -> Placement;
pub mod easing { pub fn eval(e: &Easing, t: f32) -> f32; }
pub mod interp { pub fn lerp(a: &AnimValue, b: &AnimValue, t: f32) -> AnimValue; pub fn compose(fns: &[TransformFn]) -> Affine; }
pub mod spring;  // closed form, Svelte mapping
pub mod scroll;  // per-OS wheel curves, flings, rubber band; ScrollAnimation { fn offset_at(t) -> Vec2; fn done_at(t) -> bool; fn retarget(..) }
pub mod css;     // parse CSS values Svelte passes in keyframes ("translate(10px, 0) scale(.9)", "0.5", "rgb(…)", "12px")

// presto-frame
pub struct FrameClock;   // compositor side, one per window, pure state machine
impl FrameClock {
    pub fn new(cfg: ClockConfig) -> Self;
    pub fn on_signal(&mut self, now: Time, sig: Signal) -> smallvec::SmallVec<[Action; 4]>;
    pub fn next_wakeup(&self) -> Option<Time>;
}
pub enum Signal { Vsync { target: Time, interval: Span }, CommitArrived { revision: Revision, frame: Option<FrameId> },
                  CompositorDamage, AnimationsActive(bool), Presented { frame: FrameId, at: Time, estimated: bool },
                  Visible(bool), HighRateInput, Timer, GpuDone { frame: FrameId, at: Time }, UiCost { frame: FrameId, cost: Span } }
pub enum Action { SubscribeVsync(bool), BeginUiFrame { frame: FrameId, target: Time, start_at: Time, deadline: Time },
                  Draw { frame: FrameId, target: Time }, SetTimer(Time) }
pub struct UiFrameState; // UI side: on_begin_frame / on_urgent_input(now) -> RunNow|Wait / commit_sent / commit_applied
pub mod sim;             // the deterministic simulator
```

### presto-compositor (lane COMPOSITOR)
```rust
pub struct Compositor;      // owns the thread
impl Compositor {
    pub fn spawn(cfg: CompositorConfig, glyphs: Arc<dyn GlyphRasterizer>, to_ui: Box<dyn Fn(ToUi) + Send>,
                 wake_ui: Box<dyn Fn() + Send>) -> CompositorHandle;
}
pub struct CompositorHandle;   // Send + Clone
impl CompositorHandle {
    pub fn instance(&self) -> &wgpu::Instance;   // create surfaces on the main thread with it
    pub fn add_window(&self, window: WindowId, surface: wgpu::Surface<'static>, size: DeviceSize, scale: Scale);
    pub fn add_os_layer(&self, window: WindowId, surface_id: SurfaceId, surface: wgpu::Surface<'static>, size: DeviceSize);
    pub fn remove_window(&self, window: WindowId);
    pub fn commit(&self, c: presto_base::commit::Commit);
    pub fn fast(&self, f: presto_base::commit::FastCommit);
    pub fn resize(&self, window: WindowId, size: DeviceSize, scale: Scale, seq: u64);  // replies ToUi::ResizeDone
    pub fn set_visible(&self, window: WindowId, visible: bool);
    pub fn register_texture(&self, id: TextureId, texture: wgpu::Texture);   // egui islands, offscreen viewports
    pub fn shutdown(self);
}
```

### presto-platform (lane PLATFORM)
```rust
pub trait AppHandler: 'static {
    fn init(&mut self, cx: &mut dyn PlatformCx);
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope);
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent);
    fn wake(&mut self, cx: &mut dyn PlatformCx);              // cross-thread wake (compositor, jobs)
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time);  // the deadline set with cx.set_timer
}
pub trait PlatformCx {
    fn create_window(&mut self, desc: WindowDesc) -> WindowId;
    fn close_window(&mut self, id: WindowId);
    fn surface_target(&self, id: WindowId) -> SurfaceTarget;              // raw handles for wgpu, main thread
    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)>;
    fn set_cursor(&mut self, id: WindowId, c: CursorIcon);
    fn set_ime(&mut self, id: WindowId, state: Option<ImeState /* area, purpose */>);
    fn clipboard(&mut self) -> &mut dyn Clipboard;
    fn set_timer(&mut self, at: Option<Time>);
    fn waker(&self) -> Waker;                                             // Send + Clone; wake() from any thread
    fn clock(&self) -> Arc<dyn Clock>;
    fn monitors(&self) -> Vec<Monitor>;
    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]);
    fn exit(&mut self);
}
pub fn run(handler: Box<dyn AppHandler>) -> Result<(), PlatformError>;
pub struct HeadlessPlatform;   // for tests: scripted input, virtual clock, no windows
```

### presto-core (lane CORE)
```rust
pub struct Document;        // the retained tree of one app (all windows), UI thread
impl Document {
    pub fn new(registry: Arc<KindRegistry>, text: Box<dyn TextMeasure>) -> Self;
    pub fn create(&mut self, kind: KindId) -> NodeId;
    pub fn insert(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>, slot: Option<SlotId>) -> Result<(), TreeError>;
    pub fn remove(&mut self, parent: NodeId, child: NodeId) -> Result<(), TreeError>;
    pub fn destroy(&mut self, node: NodeId);
    pub fn set_prop(&mut self, node: NodeId, prop: PropId, v: Value) -> Result<ChangeClass, PropError>;
    pub fn get_prop(&self, node: NodeId, prop: PropId) -> Option<Value>;
    pub fn listen(&mut self, node: NodeId, signal: SignalId, flags: ListenFlags);
    pub fn set_style(&mut self, node: NodeId, style: StyleInput /* classes, id, inline */);
    pub fn transaction<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R;
    pub fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched;  // native defaults + the JS-facing events to deliver
    pub fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit>;  // style, layout, record, a11y, commit
}
pub trait Kind: 'static { /* architecture §4.2 */ }
pub struct KindRegistry; pub struct KindSpec; pub enum Value { /* Bool, I64, F64, Str, Enum, Color, List, Record */ }
```
