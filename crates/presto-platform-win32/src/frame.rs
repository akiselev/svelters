//! The frame source the compositor thread waits on (`platform.md` §4, Windows):
//!
//! 1. Windows 11's compositor clock (`DCompositionWaitForCompositorClock`, build 22000+): a
//!    display-independent heartbeat that also waits on the compositor's own handles, boosted
//!    (`DCompositionBoostCompositorClock`) during gestures. Both functions are looked up at run time,
//!    since importing them would stop the process from loading on Windows 10.
//! 2. The swap chain's DXGI frame-latency waitable, when the compositor hands it over.
//! 3. `DwmFlush`.
//!
//! The clock and `DwmFlush` return at once when the display is off or the desktop occluded; a wait
//! that returns in under 1 ms sleeps one refresh interval instead and reports an estimated frame
//! (GPUI's rule, `gpui_windows/src/vsync.rs`).

use crate::shim::{FrameSource, FrameWake};
use crate::util::{counts_to_nanos, qpc_frequency, qpc_now, qpc_to_time};
use presto_base::time::Span;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HANDLE, HWND, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmFlush, DwmGetCompositionTimingInfo};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};
use windows_core::{BOOL, HRESULT, s, w};

/// Waits shorter than this mean the OS did not actually wait for a frame.
pub const EARLY_RETURN: Duration = Duration::from_millis(1);
pub const DEFAULT_INTERVAL: Span = Span::from_micros(16_666);

type WaitForCompositorClock = unsafe extern "system" fn(count: u32, handles: *const HANDLE, timeout_ms: u32) -> u32;
type BoostCompositorClock = unsafe extern "system" fn(enable: BOOL) -> HRESULT;

#[derive(Clone, Copy)]
struct ClockFns {
    wait: WaitForCompositorClock,
    boost: Option<BoostCompositorClock>,
}

fn load_clock() -> Option<ClockFns> {
    let dcomp = unsafe { LoadLibraryW(w!("dcomp.dll")) }.ok()?;
    let wait = unsafe { GetProcAddress(dcomp, s!("DCompositionWaitForCompositorClock")) }?;
    let boost = unsafe { GetProcAddress(dcomp, s!("DCompositionBoostCompositorClock")) };
    // SAFETY: the signatures are dcomp.h's.
    Some(ClockFns {
        wait: unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, WaitForCompositorClock>(wait) },
        boost: boost.map(|b| unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, BoostCompositorClock>(b) }),
    })
}

/// A kernel handle the frame source waits on (the DXGI waitable, the compositor's wake event).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitHandle(pub HANDLE);
// SAFETY: kernel handles are process-wide; waiting on one from another thread is fine.
unsafe impl Send for WaitHandle {}

/// What [`Win32FrameSource::wait_with`] woke for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    Frame(FrameWake),
    /// The handle at this index (in the slice passed) was signalled.
    Handle(usize),
}

/// The meaning of a `DCompositionWaitForCompositorClock` result for `count` extra handles:
/// `WAIT_OBJECT_0 + i` is handle `i`, `WAIT_OBJECT_0 + count` the clock's tick.
pub fn clock_result(result: u32, count: usize) -> Option<ClockResult> {
    let i = result.wrapping_sub(WAIT_OBJECT_0.0) as usize;
    if result == WAIT_TIMEOUT.0 {
        Some(ClockResult::Timeout)
    } else if i < count {
        Some(ClockResult::Handle(i))
    } else if i == count {
        Some(ClockResult::Tick)
    } else {
        // An error status: the display is off (STATUS_GRAPHICS_PRESENT_OCCLUDED) or the call failed.
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockResult {
    Tick,
    Handle(usize),
    Timeout,
}

/// The DWM's refresh interval from `qpcRefreshPeriod`, or from `rateRefresh` when the period is
/// implausibly short (GPUI saw 29 µs).
pub fn interval_from_timing(qpc_period: u64, qpc_freq: i64, rate_num: u32, rate_den: u32) -> Option<Span> {
    let from_period = counts_to_nanos(qpc_period as i64, qpc_freq);
    if from_period >= EARLY_RETURN.as_nanos() as u64 {
        return Some(Span(from_period as i64));
    }
    (rate_num > 0 && rate_den > 0).then(|| Span((rate_den as u64 * 1_000_000_000 / rate_num as u64) as i64))
}

fn dwm_interval() -> Option<Span> {
    let mut info = DWM_TIMING_INFO { cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32, ..Default::default() };
    unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut info) }.ok()?;
    interval_from_timing(info.qpcRefreshPeriod, qpc_frequency(), info.rateRefresh.uiNumerator, info.rateRefresh.uiDenominator)
}

fn timeout_ms(timeout: Option<Span>) -> u32 {
    match timeout {
        None => INFINITE,
        Some(s) if s.0 <= 0 => 0,
        // Round up so a 0.4 ms timeout still waits.
        Some(s) => ((s.0 as u64).div_ceil(1_000_000)).min(INFINITE as u64 - 1) as u32,
    }
}

pub struct Win32FrameSource {
    clock: Option<ClockFns>,
    waitable: Option<WaitHandle>,
    interval: Span,
    interval_read: Instant,
    boosted: bool,
}

// SAFETY: the function pointers are plain code addresses; the waitable is a kernel handle.
unsafe impl Send for Win32FrameSource {}

impl Win32FrameSource {
    pub fn new() -> Self {
        Self {
            clock: load_clock(),
            waitable: None,
            interval: dwm_interval().unwrap_or(DEFAULT_INTERVAL),
            interval_read: Instant::now(),
            boosted: false,
        }
    }

    /// Whether the compositor clock (Windows 11) drives this source.
    pub fn has_compositor_clock(&self) -> bool {
        self.clock.is_some()
    }

    /// The swap chain's frame-latency waitable (`IDXGISwapChain2::GetFrameLatencyWaitableObject`
    /// through wgpu-hal), used when there is no compositor clock. The caller keeps it alive.
    pub fn set_waitable(&mut self, waitable: Option<WaitHandle>) {
        self.waitable = waitable;
    }

    fn sleep_frame(&self) -> Wake {
        std::thread::sleep(Duration::from_nanos(self.interval.0.max(0) as u64));
        Wake::Frame(FrameWake::Frame { at: qpc_to_time(qpc_now()), estimated: true })
    }

    fn frame_now(&self) -> Wake {
        Wake::Frame(FrameWake::Frame { at: qpc_to_time(qpc_now()), estimated: false })
    }

    /// The compositor's one multiplexed wait: the next frame, one of `handles`, or the timeout.
    /// Without the compositor clock the extra handles are not waited on together with the frame;
    /// they are checked (without waiting) before the frame wait.
    pub fn wait_with(&mut self, handles: &[WaitHandle], timeout: Option<Span>) -> Wake {
        let start = Instant::now();
        if let Some(clock) = self.clock {
            let raw: Vec<HANDLE> = handles.iter().map(|h| h.0).collect();
            let ptr = if raw.is_empty() { std::ptr::null() } else { raw.as_ptr() };
            let r = unsafe { (clock.wait)(raw.len() as u32, ptr, timeout_ms(timeout)) };
            return match clock_result(r, raw.len()) {
                Some(ClockResult::Handle(i)) => Wake::Handle(i),
                Some(ClockResult::Timeout) => Wake::Frame(FrameWake::Timeout),
                Some(ClockResult::Tick) if start.elapsed() >= EARLY_RETURN => self.frame_now(),
                _ => self.sleep_frame(),
            };
        }
        for (i, h) in handles.iter().enumerate() {
            if unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 {
                return Wake::Handle(i);
            }
        }
        if let Some(w) = self.waitable {
            let r = unsafe { WaitForSingleObject(w.0, timeout_ms(timeout)) };
            return if r == WAIT_OBJECT_0 { self.frame_now() } else { Wake::Frame(FrameWake::Timeout) };
        }
        if let Some(t) = timeout
            && t < self.interval
        {
            std::thread::sleep(Duration::from_nanos(t.0.max(0) as u64));
            return Wake::Frame(FrameWake::Timeout);
        }
        let ok = unsafe { DwmFlush() }.is_ok();
        if ok && start.elapsed() >= EARLY_RETURN { self.frame_now() } else { self.sleep_frame() }
    }
}

impl Default for Win32FrameSource {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameSource for Win32FrameSource {
    fn wait(&mut self, timeout: Option<Span>) -> FrameWake {
        match self.wait_with(&[], timeout) {
            Wake::Frame(f) => f,
            Wake::Handle(_) => FrameWake::Timeout,
        }
    }

    fn interval(&self) -> Span {
        self.interval
    }

    fn set_boost(&mut self, on: bool) {
        if on == self.boosted {
            return;
        }
        if let Some(boost) = self.clock.and_then(|c| c.boost) {
            // Boost requests are reference-counted by the OS: one call per state change.
            if unsafe { boost(BOOL::from(on)) }.is_ok() {
                self.boosted = on;
            }
        }
        // The refresh rate may change with the boost; re-read the interval now and then.
        if self.interval_read.elapsed() > Duration::from_millis(500) {
            self.interval = dwm_interval().unwrap_or(self.interval);
            self.interval_read = Instant::now();
        }
    }
}

impl Drop for Win32FrameSource {
    fn drop(&mut self) {
        self.set_boost(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_results() {
        assert_eq!(clock_result(0, 0), Some(ClockResult::Tick));
        assert_eq!(clock_result(0, 2), Some(ClockResult::Handle(0)));
        assert_eq!(clock_result(1, 2), Some(ClockResult::Handle(1)));
        assert_eq!(clock_result(2, 2), Some(ClockResult::Tick));
        assert_eq!(clock_result(WAIT_TIMEOUT.0, 2), Some(ClockResult::Timeout));
        assert_eq!(clock_result(0xC01E_0006, 0), None, "occluded");
    }

    #[test]
    fn intervals() {
        // 10 MHz QPC, 166_666 counts = 16.6666 ms.
        assert_eq!(interval_from_timing(166_666, 10_000_000, 60, 1), Some(Span(16_666_600)));
        // The implausible 60-count period falls back to the rational rate.
        assert_eq!(interval_from_timing(60, 10_000_000, 144, 1), Some(Span(6_944_444)));
        assert_eq!(interval_from_timing(60, 10_000_000, 0, 0), None);
    }

    #[test]
    fn timeouts_round_up() {
        assert_eq!(timeout_ms(None), INFINITE);
        assert_eq!(timeout_ms(Some(Span::from_micros(400))), 1);
        assert_eq!(timeout_ms(Some(Span::from_millis(5))), 5);
        assert_eq!(timeout_ms(Some(Span(-1))), 0);
    }
}
