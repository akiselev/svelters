//! Frame sources (`platform.md` §4, macOS): per-window vsync ticks for the compositor thread.
//!
//! - [`FrameSourceKind::MetalDisplayLink`]: `CAMetalDisplayLink` on the window's `CAMetalLayer` (macOS
//!   14+), `preferredFrameLatency` 1, ticks at the frame's `targetPresentationTimestamp`. It also
//!   acquires a drawable per tick; [`FrameSourceConfig::drawable`] hands it to the compositor (wgpu
//!   patch P3 presents it). Without that sink the drawable goes back to the layer unused, which costs
//!   one of the layer's three drawables while a frame is in flight.
//! - [`FrameSourceKind::ViewDisplayLink`]: `NSView.displayLink` (macOS 14+), timing only.
//! - [`FrameSourceKind::CoreVideo`]: `CVDisplayLink` for macOS 13 and earlier (deprecated in 15).
//!
//! [`MacFrameSource`] implements `presto_platform::FrameSource` (the compositor's vsync thread
//! blocks in `wait`): ticks wake waiters through a condvar; a paused link resumes on the next `wait`,
//! and a Core Animation link pauses itself when nobody has waited for [`IDLE_PAUSE`]. Window occlusion
//! (`windowDidChangeOcclusionState:`) reaches the window's sources as `FrameWait::Occluded`.
//!
//! The Core Animation links run on a thread of their own with its own run loop: ticks never wait for
//! the main thread, and pausing is a block performed on that run loop. GPUI's `display_link.rs`
//! documents two crash classes from stopping a `CVDisplayLink` while its callback runs; here the
//! callback's context is leaked (one small allocation per source) and checked for liveness, so a late
//! callback after drop is harmless.

use crate::time;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, Message, define_class, msg_send, sel};
use objc2_app_kit::NSView;
use objc2_core_foundation::{CFRetained, CFRunLoop, kCFRunLoopDefaultMode};
use objc2_core_video::{CVDisplayLink, CVOptionFlags, CVReturn, CVTimeStamp};
use objc2_foundation::{
    NSDefaultRunLoopMode, NSObject, NSObjectProtocol, NSOperatingSystemVersion, NSPort,
    NSProcessInfo, NSRunLoop,
};
use objc2_quartz_core::{
    CADisplayLink, CAMetalDisplayLink, CAMetalDisplayLinkDelegate, CAMetalDisplayLinkUpdate,
    CAMetalDrawable, CAMetalLayer,
};
use presto_base::time::{Span, Time};
use presto_platform::PlatformError;
use presto_platform::frame::{FrameSource, FrameWait};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak, mpsc};
use std::thread::JoinHandle;

/// One display refresh, as a frame source reports it to [`FrameSourceConfig::on_tick`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VsyncTick {
    /// When the source fired.
    pub now: Time,
    /// When the frame being prepared will reach the display.
    pub target: Time,
    pub interval: Span,
}

/// A Core Animation link with no `wait` for this long pauses itself.
pub const IDLE_PAUSE: Span = Span::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameSourceKind {
    MetalDisplayLink,
    ViewDisplayLink,
    CoreVideo,
}

impl FrameSourceKind {
    /// The best timing-only source this OS has: `NSView.displayLink` on macOS 14+, else Core Video.
    /// (`MetalDisplayLink` is chosen explicitly, once the compositor presents its drawables.)
    pub fn best_timing() -> Self {
        if macos_at_least(14) {
            FrameSourceKind::ViewDisplayLink
        } else {
            FrameSourceKind::CoreVideo
        }
    }
}

pub(crate) fn macos_at_least(major: isize) -> bool {
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
        majorVersion: major,
        minorVersion: 0,
        patchVersion: 0,
    })
}

/// A drawable `CAMetalDisplayLink` acquired for one frame. Sending it to the compositor thread is
/// what Core Animation expects (the link's update is consumed off the main thread).
pub struct MetalDrawable(pub Retained<ProtocolObject<dyn CAMetalDrawable>>);

// SAFETY: CAMetalDrawable is used from render threads by design; the wrapper only moves ownership.
unsafe impl Send for MetalDrawable {}

pub struct FrameSourceConfig {
    pub kind: FrameSourceKind,
    /// Called on the link's thread for every tick, besides waking `wait`.
    pub on_tick: Option<Box<dyn Fn(VsyncTick) + Send + Sync>>,
    /// `MetalDisplayLink` only: receives each tick's drawable, after `on_tick`.
    pub drawable: Option<Box<dyn Fn(MetalDrawable) + Send + Sync>>,
    /// Tick from the start (otherwise from the first `wait`).
    pub active: bool,
}

impl FrameSourceConfig {
    pub fn new(kind: FrameSourceKind) -> Self {
        FrameSourceConfig {
            kind,
            on_tick: None,
            drawable: None,
            active: false,
        }
    }
}

#[derive(Default)]
struct WaitState {
    /// The newest tick and its number; `taken` is the last number `wait` returned.
    tick: Option<VsyncTick>,
    seq: u64,
    taken: u64,
    /// An occlusion change `wait` has not reported yet.
    occluded: Option<bool>,
}

struct Shared {
    alive: AtomicBool,
    active: AtomicBool,
    /// The last tick's target, for the interval when the source doesn't report one.
    last_target: AtomicI64,
    /// The last interval (ns), for `FrameSource::interval`.
    interval: AtomicI64,
    /// When `wait` was last called (presto ns).
    last_wait: AtomicI64,
    wait: Mutex<WaitState>,
    cond: Condvar,
    on_tick: Option<Box<dyn Fn(VsyncTick) + Send + Sync>>,
    drawable: Option<Box<dyn Fn(MetalDrawable) + Send + Sync>>,
}

impl Shared {
    /// Whether nobody waits any more (a Core Animation link then pauses itself).
    fn idle(&self, now: Time) -> bool {
        now.0 as i64 - self.last_wait.load(Ordering::Acquire) > IDLE_PAUSE.0
    }

    fn occluded(&self, occluded: bool) {
        if let Ok(mut w) = self.wait.lock() {
            w.occluded = Some(occluded);
        }
        self.cond.notify_all();
    }

    fn tick(&self, now: Time, target: Time, interval: Option<Span>) {
        if !self.alive.load(Ordering::Acquire) || !self.active.load(Ordering::Acquire) {
            return;
        }
        let last = self.last_target.swap(target.0 as i64, Ordering::AcqRel);
        let measured = target.0 as i64 - last;
        let interval = interval
            .filter(|s| s.0 > 0)
            .or_else(|| {
                (last > 0 && measured > 0 && measured < 100_000_000).then_some(Span(measured))
            })
            .unwrap_or(Span::from_micros(16_667));
        self.interval.store(interval.0, Ordering::Release);
        let tick = VsyncTick {
            now,
            target,
            interval,
        };
        if let Some(f) = &self.on_tick {
            f(tick);
        }
        if let Ok(mut w) = self.wait.lock() {
            w.tick = Some(tick);
            w.seq += 1;
        }
        self.cond.notify_all();
    }
}

/// Reports a window's occlusion to one of its frame sources (held weakly by the window).
#[derive(Clone)]
pub(crate) struct OcclusionHandle(Weak<Shared>);

impl OcclusionHandle {
    /// `false` once the source is gone.
    pub(crate) fn notify(&self, occluded: bool) -> bool {
        match self.0.upgrade() {
            Some(s) => {
                s.occluded(occluded);
                true
            }
            None => false,
        }
    }
}

struct SendRunLoop(CFRetained<CFRunLoop>);
// SAFETY: CFRunLoop's perform-block, wake-up and stop functions are thread-safe.
unsafe impl Send for SendRunLoop {}
// SAFETY: as above.
unsafe impl Sync for SendRunLoop {}

struct SendPtr<T>(Retained<T>);
// SAFETY: used to move a layer or link to the link thread once; the link thread is its only user
// afterwards (Core Animation objects are safe to use off the main thread inside transactions).
unsafe impl<T> Send for SendPtr<T> {}

/// A window's frame source; see the module docs.
pub struct MacFrameSource {
    kind: FrameSourceKind,
    shared: Arc<Shared>,
    run_loop: Option<Arc<SendRunLoop>>,
    thread: Option<JoinHandle<()>>,
    core_video: Option<SendCvLink>,
}

impl MacFrameSource {
    /// Creates the source for a window's view and layer. Main thread.
    pub(crate) fn new(
        _mtm: MainThreadMarker,
        view: &NSView,
        layer: &CAMetalLayer,
        config: FrameSourceConfig,
    ) -> Result<Self, PlatformError> {
        let shared = Arc::new(Shared {
            alive: AtomicBool::new(true),
            active: AtomicBool::new(config.active),
            last_target: AtomicI64::new(0),
            interval: AtomicI64::new(0),
            last_wait: AtomicI64::new(time::now().0 as i64),
            wait: Mutex::new(WaitState::default()),
            cond: Condvar::new(),
            on_tick: config.on_tick,
            drawable: config.drawable,
        });
        let mut source = MacFrameSource {
            kind: config.kind,
            shared: shared.clone(),
            run_loop: None,
            thread: None,
            core_video: None,
        };
        match config.kind {
            FrameSourceKind::MetalDisplayLink | FrameSourceKind::ViewDisplayLink => {
                if !macos_at_least(14) {
                    return Err(PlatformError::Os(format!(
                        "{:?} needs macOS 14",
                        config.kind
                    )));
                }
                let start = match config.kind {
                    FrameSourceKind::MetalDisplayLink => LinkStart::Metal(SendPtr(layer.retain())),
                    _ => {
                        let target = LinkTarget::new(shared.clone());
                        // SAFETY: `tick:` is implemented by LinkTarget with the signature CADisplayLink calls.
                        let link =
                            unsafe { view.displayLinkWithTarget_selector(&target, sel!(tick:)) };
                        LinkStart::View(SendPtr(link), SendPtr(target))
                    }
                };
                let (tx, rx) = mpsc::channel();
                let thread = std::thread::Builder::new()
                    .name("presto-vsync".into())
                    .spawn(move || link_thread(start, shared, tx))
                    .map_err(|e| PlatformError::Os(e.to_string()))?;
                let run_loop = rx
                    .recv()
                    .map_err(|_| PlatformError::Os("display link thread failed".into()))?;
                source.run_loop = Some(Arc::new(run_loop));
                source.thread = Some(thread);
            }
            FrameSourceKind::CoreVideo => {
                source.core_video = Some(SendCvLink::new(shared)?);
            }
        }
        Ok(source)
    }

    pub fn kind(&self) -> FrameSourceKind {
        self.kind
    }

    pub(crate) fn occlusion_handle(&self) -> OcclusionHandle {
        OcclusionHandle(Arc::downgrade(&self.shared))
    }

    fn perform(&self, f: impl Fn() + 'static) {
        if let Some(rl) = &self.run_loop {
            perform(rl, f);
        }
    }
}

/// Runs `f` on the link thread's run loop.
fn perform(rl: &SendRunLoop, f: impl Fn() + 'static) {
    let block = RcBlock::new(f);
    // SAFETY: the default mode is a valid mode; the run loop retains the block.
    unsafe {
        rl.0.perform_block(kCFRunLoopDefaultMode.map(|m| &**m), Some(&block))
    };
    rl.0.wake_up();
}

impl MacFrameSource {
    /// Starts or stops ticks (`wait` starts them by itself).
    pub fn set_active(&self, active: bool) {
        if self.shared.active.swap(active, Ordering::AcqRel) == active {
            return;
        }
        self.shared.last_target.store(0, Ordering::Release);
        match &self.core_video {
            Some(cv) => cv.set_running(active),
            None => {
                let paused = !active;
                self.perform(move || {
                    LINK.with(|l| {
                        if let Some(l) = l.borrow().as_ref() {
                            l.set_paused(paused);
                        }
                    })
                });
            }
        }
    }
}

impl FrameSource for MacFrameSource {
    fn name(&self) -> &'static str {
        match self.kind {
            FrameSourceKind::MetalDisplayLink => "CAMetalDisplayLink",
            FrameSourceKind::ViewDisplayLink => "NSView.displayLink",
            FrameSourceKind::CoreVideo => "CVDisplayLink",
        }
    }

    fn wait(&self, timeout: Span) -> FrameWait {
        let start = time::now();
        self.shared
            .last_wait
            .store(start.0 as i64, Ordering::Release);
        if !self.shared.active.load(Ordering::Acquire) {
            self.set_active(true);
        }
        let deadline = start + timeout;
        let Ok(mut w) = self.shared.wait.lock() else {
            return FrameWait::TimedOut;
        };
        loop {
            if let Some(o) = w.occluded.take() {
                return FrameWait::Occluded(o);
            }
            if w.seq != w.taken
                && let Some(t) = w.tick
            {
                w.taken = w.seq;
                return FrameWait::Vsync {
                    target: t.target,
                    interval: t.interval,
                    estimated: false,
                };
            }
            let left = deadline - time::now();
            if left.0 <= 0 {
                return FrameWait::TimedOut;
            }
            w = match self
                .shared
                .cond
                .wait_timeout(w, std::time::Duration::from_nanos(left.0 as u64))
            {
                Ok((g, _)) => g,
                Err(_) => return FrameWait::TimedOut,
            };
        }
    }

    fn interval(&self) -> Option<Span> {
        let i = self.shared.interval.load(Ordering::Acquire);
        (i > 0).then_some(Span(i))
    }
}

impl Drop for MacFrameSource {
    fn drop(&mut self) {
        self.shared.alive.store(false, Ordering::Release);
        if let Some(rl) = self.run_loop.take() {
            let rl2 = rl.clone();
            perform(&rl, move || {
                LINK.with(|l| {
                    if let Some(l) = l.borrow_mut().take() {
                        l.invalidate();
                    }
                });
                rl2.0.stop();
            });
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

enum LinkStart {
    Metal(SendPtr<CAMetalLayer>),
    View(SendPtr<CADisplayLink>, SendPtr<LinkTarget>),
}

/// The link running on this thread (link threads run one link each), with the object it calls, which
/// it does not retain.
enum Link {
    Metal(
        Retained<CAMetalDisplayLink>,
        #[allow(dead_code)] Retained<MetalLinkDelegate>,
    ),
    View(
        Retained<CADisplayLink>,
        #[allow(dead_code)] Retained<LinkTarget>,
    ),
}

impl Link {
    fn set_paused(&self, paused: bool) {
        match self {
            Link::Metal(l, _target) => l.setPaused(paused),
            Link::View(l, _target) => l.setPaused(paused),
        }
    }
    fn invalidate(&self) {
        match self {
            Link::Metal(l, _) => l.invalidate(),
            Link::View(l, _) => l.invalidate(),
        }
    }
}

thread_local! {
    static LINK: std::cell::RefCell<Option<Link>> = const { std::cell::RefCell::new(None) };
}

fn link_thread(start: LinkStart, shared: Arc<Shared>, tx: mpsc::Sender<SendRunLoop>) {
    objc2::rc::autoreleasepool(|_| {
        let run_loop = NSRunLoop::currentRunLoop();
        let paused = !shared.active.load(Ordering::Acquire);
        let link = match start {
            LinkStart::Metal(layer) => {
                let delegate = MetalLinkDelegate::new(shared.clone());
                let link =
                    CAMetalDisplayLink::initWithMetalLayer(CAMetalDisplayLink::alloc(), &layer.0);
                link.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
                link.setPreferredFrameLatency(1.0);
                link.setPaused(paused);
                // SAFETY: this thread's run loop, in its default mode.
                unsafe { link.addToRunLoop_forMode(&run_loop, NSDefaultRunLoopMode) };
                Link::Metal(link, delegate)
            }
            LinkStart::View(link, target) => {
                link.0.setPaused(paused);
                // SAFETY: as above.
                unsafe { link.0.addToRunLoop_forMode(&run_loop, NSDefaultRunLoopMode) };
                Link::View(link.0, target.0)
            }
        };
        LINK.with(|l| *l.borrow_mut() = Some(link));
        // A port keeps the run loop running (CFRunLoopRun returns at once without sources) whatever
        // the link does.
        // SAFETY: this thread's run loop, in its default mode.
        unsafe { run_loop.addPort_forMode(&NSPort::port(), NSDefaultRunLoopMode) };
    });
    let Some(rl) = CFRunLoop::current() else {
        return;
    };
    if tx.send(SendRunLoop(rl)).is_err() {
        return;
    }
    // Runs until the source's Drop stops it.
    while shared.alive.load(Ordering::Acquire) {
        objc2::rc::autoreleasepool(|_| CFRunLoop::run());
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the ivars are Send + Sync and dropped normally.
    #[unsafe(super(NSObject))]
    #[name = "PrestoMetalLinkDelegate"]
    #[ivars = Arc<Shared>]
    struct MetalLinkDelegate;

    unsafe impl NSObjectProtocol for MetalLinkDelegate {}

    unsafe impl CAMetalDisplayLinkDelegate for MetalLinkDelegate {
        #[unsafe(method(metalDisplayLink:needsUpdate:))]
        fn needs_update(&self, link: &CAMetalDisplayLink, update: &CAMetalDisplayLinkUpdate) {
            let shared = self.ivars();
            if shared.idle(time::now()) {
                shared.active.store(false, Ordering::Release);
                link.setPaused(true);
                return;
            }
            shared.tick(time::now(), time::from_secs(update.targetPresentationTimestamp()), None);
            if let Some(sink) = &shared.drawable
                && shared.alive.load(Ordering::Acquire)
                && shared.active.load(Ordering::Acquire)
            {
                sink(MetalDrawable(update.drawable()));
            }
        }
    }
);

impl MetalLinkDelegate {
    fn new(shared: Arc<Shared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(shared);
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the ivars are Send + Sync and dropped normally.
    #[unsafe(super(NSObject))]
    #[name = "PrestoDisplayLinkTarget"]
    #[ivars = Arc<Shared>]
    struct LinkTarget;

    impl LinkTarget {
        #[unsafe(method(tick:))]
        fn tick(&self, link: &CADisplayLink) {
            let shared = self.ivars();
            if shared.idle(time::now()) {
                shared.active.store(false, Ordering::Release);
                link.setPaused(true);
                return;
            }
            let interval = time::span_from_secs(link.targetTimestamp() - link.timestamp());
            self.ivars().tick(time::now(), time::from_secs(link.targetTimestamp()), Some(interval));
        }
    }
);

impl LinkTarget {
    fn new(shared: Arc<Shared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(shared);
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }
}

/// A `CVDisplayLink` on the active displays; its callback runs on Core Video's own thread.
struct SendCvLink {
    link: CFRetained<CVDisplayLink>,
}

// SAFETY: CVDisplayLink is a thread-safe CF object.
unsafe impl Send for SendCvLink {}
// SAFETY: as above.
unsafe impl Sync for SendCvLink {}

impl SendCvLink {
    fn new(shared: Arc<Shared>) -> Result<Self, PlatformError> {
        let mut link: *mut CVDisplayLink = std::ptr::null_mut();
        // SAFETY: `link` is a valid out-pointer.
        #[allow(deprecated)]
        let status = unsafe {
            objc2_core_video::CVDisplayLinkCreateWithActiveCGDisplays(NonNull::from(&mut link))
        };
        if status != 0 || link.is_null() {
            return Err(PlatformError::Os(format!(
                "CVDisplayLinkCreateWithActiveCGDisplays failed ({status})"
            )));
        }
        // SAFETY: created above with a +1 retain count.
        let link = unsafe { CFRetained::from_raw(NonNull::new_unchecked(link)) };
        // Leaked on purpose: a callback may still be running when the source drops.
        let context = Arc::into_raw(shared) as *mut c_void;
        // SAFETY: `cv_callback` matches the callback type; `context` outlives the link (never freed).
        #[allow(deprecated)]
        unsafe {
            link.set_output_callback(Some(cv_callback), context)
        };
        let me = SendCvLink { link };
        // SAFETY: context is an `Arc<Shared>` pointer (see above).
        if unsafe { &*(context as *const Shared) }
            .active
            .load(Ordering::Acquire)
        {
            me.set_running(true);
        }
        Ok(me)
    }

    fn set_running(&self, running: bool) {
        #[allow(deprecated)]
        let _ = if running {
            self.link.start()
        } else {
            self.link.stop()
        };
    }
}

impl Drop for SendCvLink {
    fn drop(&mut self) {
        self.set_running(false);
    }
}

unsafe extern "C-unwind" fn cv_callback(
    _link: NonNull<CVDisplayLink>,
    now: NonNull<CVTimeStamp>,
    output: NonNull<CVTimeStamp>,
    _flags_in: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    context: *mut c_void,
) -> CVReturn {
    // SAFETY: Core Video passes valid timestamps; `context` is the leaked `Arc<Shared>`.
    let (now, output, shared) =
        unsafe { (now.as_ref(), output.as_ref(), &*(context as *const Shared)) };
    let interval = (output.videoTimeScale > 0).then(|| {
        Span::from_secs_f64(output.videoRefreshPeriod as f64 / output.videoTimeScale as f64)
    });
    shared.tick(
        time::from_mach_ticks(now.hostTime),
        time::from_mach_ticks(output.hostTime),
        interval,
    );
    0
}
