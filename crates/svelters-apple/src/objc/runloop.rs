//! CFRunLoop integration (research note §1.5): everything is added to the main run loop in
//! `kCFRunLoopCommonModes`, so checkpoints keep running during menu tracking, live resize, slider
//! drags and modal panels.
//!
//! - `wake`: a version-0 `CFRunLoopSource`; any thread signals it and wakes the main loop
//!   (`CFRunLoopSourceSignal` + `CFRunLoopWakeUp` are documented thread-safe). Wakes coalesce.
//! - `schedule_wake`: one repeating `CFRunLoopTimer` with a huge interval whose next fire date is
//!   re-armed (a non-repeating CF timer is invalidated after it fires). Cancel = far future.
//!   Delays of 20 ms and more get a 10% tolerance (the `CoarseTimer` case, App Nap friendly).
//! - `frame_fallback`: a second timer of the same kind for `request_frame`: 16 ms out when no
//!   `CADisplayLink` is available, and as a watchdog behind one that may never tick.

use std::ffi::c_void;

type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopTimerRef = *mut c_void;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFIndex = isize;
type CFOptionFlags = usize;
type CFAbsoluteTime = f64;
type CFTimeInterval = f64;

#[repr(C)]
struct CFRunLoopSourceContext {
    version: CFIndex,
    info: *mut c_void,
    retain: Option<unsafe extern "C" fn(*const c_void) -> *const c_void>,
    release: Option<unsafe extern "C" fn(*const c_void)>,
    copy_description: Option<unsafe extern "C" fn(*const c_void) -> *const c_void>,
    equal: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> u8>,
    hash: Option<unsafe extern "C" fn(*const c_void) -> usize>,
    schedule: Option<unsafe extern "C" fn(*mut c_void, CFRunLoopRef, CFStringRef)>,
    cancel: Option<unsafe extern "C" fn(*mut c_void, CFRunLoopRef, CFStringRef)>,
    perform: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[repr(C)]
struct CFRunLoopTimerContext {
    version: CFIndex,
    info: *mut c_void,
    retain: Option<unsafe extern "C" fn(*const c_void) -> *const c_void>,
    release: Option<unsafe extern "C" fn(*const c_void)>,
    copy_description: Option<unsafe extern "C" fn(*const c_void) -> *const c_void>,
}

type TimerCallBack = unsafe extern "C" fn(CFRunLoopTimerRef, *mut c_void);

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopCommonModes: CFStringRef;
    fn CFRunLoopGetMain() -> CFRunLoopRef;
    fn CFRunLoopSourceCreate(alloc: CFAllocatorRef, order: CFIndex, ctx: *mut CFRunLoopSourceContext) -> CFRunLoopSourceRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, src: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopSourceSignal(src: CFRunLoopSourceRef);
    fn CFRunLoopWakeUp(rl: CFRunLoopRef);
    fn CFRunLoopTimerCreate(
        alloc: CFAllocatorRef,
        fire: CFAbsoluteTime,
        interval: CFTimeInterval,
        flags: CFOptionFlags,
        order: CFIndex,
        callout: TimerCallBack,
        ctx: *mut CFRunLoopTimerContext,
    ) -> CFRunLoopTimerRef;
    fn CFRunLoopAddTimer(rl: CFRunLoopRef, timer: CFRunLoopTimerRef, mode: CFStringRef);
    fn CFRunLoopTimerSetNextFireDate(timer: CFRunLoopTimerRef, date: CFAbsoluteTime);
    fn CFRunLoopTimerSetTolerance(timer: CFRunLoopTimerRef, tolerance: CFTimeInterval);
    fn CFAbsoluteTimeGetCurrent() -> CFAbsoluteTime;
}

/// What the run loop calls back into (on the main thread).
pub trait LoopEvents {
    /// The wake source fired: run a host checkpoint.
    fn wake(&self);
    /// The `schedule_wake` timer expired.
    fn timer(&self);
    /// The fallback frame timer expired.
    fn frame(&self);
}

struct Callbacks {
    events: std::cell::RefCell<Option<std::rc::Rc<dyn LoopEvents>>>,
}

impl Callbacks {
    fn with(&self, f: impl FnOnce(&dyn LoopEvents)) {
        let ev = self.events.borrow().clone();
        if let Some(ev) = ev {
            // Our own pool: a checkpoint runs JS, which creates autoreleased temporaries.
            objc2::rc::autoreleasepool(|_| f(&*ev));
        }
    }
}

unsafe extern "C" fn perform(info: *mut c_void) {
    let cb = unsafe { &*(info as *const Callbacks) };
    cb.with(|e| e.wake());
}

unsafe extern "C" fn fire_timer(t: CFRunLoopTimerRef, info: *mut c_void) {
    unsafe { CFRunLoopTimerSetNextFireDate(t, far_future()) };
    let cb = unsafe { &*(info as *const Callbacks) };
    cb.with(|e| e.timer());
}

unsafe extern "C" fn fire_frame(t: CFRunLoopTimerRef, info: *mut c_void) {
    unsafe { CFRunLoopTimerSetNextFireDate(t, far_future()) };
    let cb = unsafe { &*(info as *const Callbacks) };
    cb.with(|e| e.frame());
}

fn far_future() -> CFAbsoluteTime {
    unsafe { CFAbsoluteTimeGetCurrent() + 1.0e10 }
}

/// The main-run-loop sources of one backend. Created on the main thread; lives for the process
/// (the CF objects are never released, like the backend itself).
pub struct RunLoop {
    main: CFRunLoopRef,
    source: CFRunLoopSourceRef,
    timer: CFRunLoopTimerRef,
    frame_timer: CFRunLoopTimerRef,
    callbacks: *const Callbacks,
}

/// The thread-safe half of [`RunLoop`]: signal the source and wake the loop.
#[derive(Clone, Copy)]
pub struct Waker {
    main: CFRunLoopRef,
    source: CFRunLoopSourceRef,
}

// SAFETY: CFRunLoopSourceSignal and CFRunLoopWakeUp are thread-safe, and both objects live for the
// rest of the process.
unsafe impl Send for Waker {}
unsafe impl Sync for Waker {}

impl Waker {
    pub fn wake(&self) {
        unsafe {
            CFRunLoopSourceSignal(self.source);
            CFRunLoopWakeUp(self.main);
        }
    }
}

impl RunLoop {
    /// Adds the wake source and both timers to the main run loop (common modes). Main thread only.
    pub fn new() -> RunLoop {
        let callbacks = Box::into_raw(Box::new(Callbacks { events: std::cell::RefCell::new(None) }));
        unsafe {
            let main = CFRunLoopGetMain();
            let mut sctx = CFRunLoopSourceContext {
                version: 0,
                info: callbacks as *mut c_void,
                retain: None,
                release: None,
                copy_description: None,
                equal: None,
                hash: None,
                schedule: None,
                cancel: None,
                perform: Some(perform),
            };
            let source = CFRunLoopSourceCreate(std::ptr::null(), 0, &mut sctx);
            CFRunLoopAddSource(main, source, kCFRunLoopCommonModes);
            let mk = |cb: TimerCallBack| {
                let mut tctx = CFRunLoopTimerContext {
                    version: 0,
                    info: callbacks as *mut c_void,
                    retain: None,
                    release: None,
                    copy_description: None,
                };
                let t = CFRunLoopTimerCreate(std::ptr::null(), far_future(), 1.0e9, 0, 0, cb, &mut tctx);
                CFRunLoopAddTimer(main, t, kCFRunLoopCommonModes);
                t
            };
            let timer = mk(fire_timer);
            let frame_timer = mk(fire_frame);
            RunLoop { main, source, timer, frame_timer, callbacks }
        }
    }

    /// Installs the receiver of the run-loop callbacks (replacing any previous one).
    pub fn set_events(&self, ev: Option<std::rc::Rc<dyn LoopEvents>>) {
        unsafe { *(*self.callbacks).events.borrow_mut() = ev };
    }

    pub fn waker(&self) -> Waker {
        Waker { main: self.main, source: self.source }
    }

    /// (Re)arms the one-shot wakeup `ms` from now; `ms < 0` cancels.
    pub fn schedule_wake(&self, ms: i64) {
        unsafe {
            if ms < 0 {
                CFRunLoopTimerSetNextFireDate(self.timer, far_future());
                return;
            }
            let secs = ms as f64 / 1000.0;
            CFRunLoopTimerSetTolerance(self.timer, if ms >= 20 { secs * 0.1 } else { 0.0 });
            CFRunLoopTimerSetNextFireDate(self.timer, CFAbsoluteTimeGetCurrent() + secs);
        }
    }

    /// Arms the fallback frame timer `ms` out (re-arming moves it): the frame source when there
    /// is no `CADisplayLink`, and the watchdog behind one.
    pub fn frame_fallback(&self, ms: u64) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.frame_timer, CFAbsoluteTimeGetCurrent() + ms as f64 / 1000.0) };
    }
}

impl Default for RunLoop {
    fn default() -> Self {
        RunLoop::new()
    }
}
