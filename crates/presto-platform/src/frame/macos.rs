//! macOS frame source: a `CVDisplayLink` (CoreVideo's C API) whose output callback, on CoreVideo's
//! own thread, records the next frame's display time and wakes waiters. Targets are the callback's
//! `inOutputTime.hostTime` (mach absolute time) on presto's clock. `CAMetalDisplayLink` (macOS 14+)
//! replaces this once its drawable hand-off is wired (platform.md §4, wgpu patch P3).

use super::{FrameSource, FrameWait};
use crate::clock;
use presto_base::time::{Span, Time};
use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

type CVDisplayLinkRef = *mut c_void;
type CVReturn = i32;
type CVOptionFlags = u64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CVSMPTETime {
    subframes: i16,
    subframe_divisor: i16,
    counter: u32,
    kind: u32,
    flags: u32,
    hours: i16,
    minutes: i16,
    seconds: i16,
    frames: i16,
}

/// `CVTimeStamp` (CoreVideo `CVBase.h`).
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CVTimeStamp {
    version: u32,
    video_time_scale: i32,
    video_time: i64,
    host_time: u64,
    rate_scalar: f64,
    video_refresh_period: i64,
    smpte_time: CVSMPTETime,
    flags: u64,
    reserved: u64,
}

type OutputCallback = extern "C" fn(
    link: CVDisplayLinkRef,
    now: *const CVTimeStamp,
    output_time: *const CVTimeStamp,
    flags_in: CVOptionFlags,
    flags_out: *mut CVOptionFlags,
    context: *mut c_void,
) -> CVReturn;

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(link_out: *mut CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkSetOutputCallback(
        link: CVDisplayLinkRef,
        callback: OutputCallback,
        context: *mut c_void,
    ) -> CVReturn;
    fn CVDisplayLinkStart(link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkStop(link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkRelease(link: CVDisplayLinkRef);
}

#[repr(C)]
#[derive(Default)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
}

#[derive(Default)]
struct Tick {
    seq: u64,
    target_host: u64,
    period: Span,
}

struct Shared {
    tick: Mutex<Tick>,
    cv: Condvar,
}

pub struct DisplayLinkSource {
    link: CVDisplayLinkRef,
    shared: *const Shared,
    timebase: (u32, u32),
}

// SAFETY: CVDisplayLink is a thread-safe CoreFoundation object (its API may be called from any
// thread), and `shared` points to a `Shared` (Mutex + Condvar) that lives until `Drop`.
unsafe impl Send for DisplayLinkSource {}
// SAFETY: as above; `wait` only touches the mutex-guarded state.
unsafe impl Sync for DisplayLinkSource {}

extern "C" fn on_output(
    _link: CVDisplayLinkRef,
    _now: *const CVTimeStamp,
    output_time: *const CVTimeStamp,
    _flags_in: CVOptionFlags,
    _flags_out: *mut CVOptionFlags,
    context: *mut c_void,
) -> CVReturn {
    // SAFETY: `context` is the `Shared` installed in `new`, alive until the link is stopped and
    // released in `Drop`; `output_time` is valid for the callback's duration.
    let (shared, out) = unsafe { (&*(context as *const Shared), *output_time) };
    let mut t = shared.tick.lock().unwrap_or_else(|e| e.into_inner());
    t.seq += 1;
    t.target_host = out.host_time;
    if out.video_time_scale > 0 && out.video_refresh_period > 0 {
        t.period = Span(
            ((out.video_refresh_period as i128 * 1_000_000_000) / out.video_time_scale as i128)
                as i64,
        );
    }
    drop(t);
    shared.cv.notify_all();
    0
}

impl DisplayLinkSource {
    /// `None` when CoreVideo cannot make a display link (no display).
    pub fn new() -> Option<Self> {
        let mut tb = MachTimebaseInfo::default();
        // SAFETY: plain query into a local.
        unsafe { mach_timebase_info(&mut tb) };
        let shared = Arc::into_raw(Arc::new(Shared {
            tick: Mutex::new(Tick::default()),
            cv: Condvar::new(),
        }));
        let mut link: CVDisplayLinkRef = std::ptr::null_mut();
        // SAFETY: CoreVideo calls with an out-pointer to a local and a context that outlives the
        // link (released only in `Drop`, after the link stops).
        unsafe {
            if CVDisplayLinkCreateWithActiveCGDisplays(&mut link) != 0 || link.is_null() {
                drop(Arc::from_raw(shared));
                return None;
            }
            CVDisplayLinkSetOutputCallback(link, on_output, shared as *mut c_void);
            if CVDisplayLinkStart(link) != 0 {
                CVDisplayLinkRelease(link);
                drop(Arc::from_raw(shared));
                return None;
            }
        }
        Some(DisplayLinkSource {
            link,
            shared,
            timebase: (tb.numer.max(1), tb.denom.max(1)),
        })
    }

    fn host_ns(&self, host: u64) -> u64 {
        ((host as u128 * self.timebase.0 as u128) / self.timebase.1 as u128) as u64
    }

    fn shared(&self) -> &Shared {
        // SAFETY: valid until `Drop`.
        unsafe { &*self.shared }
    }
}

impl Drop for DisplayLinkSource {
    fn drop(&mut self) {
        // SAFETY: stopping waits for an in-flight callback; after release nothing references
        // `shared`, so its reference is dropped last.
        unsafe {
            CVDisplayLinkStop(self.link);
            CVDisplayLinkRelease(self.link);
            drop(Arc::from_raw(self.shared));
        }
    }
}

impl FrameSource for DisplayLinkSource {
    fn name(&self) -> &'static str {
        "cv-display-link"
    }

    fn wait(&self, timeout: Span) -> FrameWait {
        let s = self.shared();
        let guard = s.tick.lock().unwrap_or_else(|e| e.into_inner());
        let seq = guard.seq;
        let (guard, res) =
            s.cv.wait_timeout_while(guard, Duration::from_nanos(timeout.0.max(0) as u64), |t| {
                t.seq == seq
            })
            .unwrap_or_else(|e| e.into_inner());
        if res.timed_out() {
            return FrameWait::TimedOut;
        }
        // SAFETY: plain query.
        let now_host = unsafe { mach_absolute_time() };
        let target: Time =
            clock::time_from_os(self.host_ns(guard.target_host), self.host_ns(now_host));
        let interval = if guard.period.0 > 0 {
            guard.period
        } else {
            Span(16_666_667)
        };
        FrameWait::Vsync {
            target,
            interval,
            estimated: false,
        }
    }

    fn interval(&self) -> Option<Span> {
        let p = self
            .shared()
            .tick
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .period;
        (p.0 > 0).then_some(p)
    }
}
