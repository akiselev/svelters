//! Windows frame sources. The compositor clock (`DCompositionWaitForCompositorClock`, Windows 11,
//! looked up at run time so Windows 10 still loads) ticks with DWM's composition; `DwmFlush` is
//! the fallback. Both return early while a monitor sleeps, so a wait shorter than 1 ms sleeps one
//! interval instead and reports an estimated tick (GPUI's `gpui_windows/src/vsync.rs` rule).
//! Targets come from DWM's timing info: the next vblank after now, on presto's clock.

use super::{sleep, FrameSource, FrameWait};
use crate::clock;
use presto_base::time::{Span, Time};
use windows::core::{s, w};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmFlush, DwmGetCompositionTimingInfo, DWM_TIMING_INFO};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

const DEFAULT_INTERVAL: Span = Span(16_666_667);
const EARLY: Span = Span::from_millis(1);
const WAIT_TIMEOUT: u32 = 0x102;
const WAIT_FAILED: u32 = 0xFFFF_FFFF;

type WaitForCompositorClock = unsafe extern "system" fn(count: u32, handles: *const isize, timeout_ms: u32) -> u32;

pub struct DwmSource {
    compositor_clock: Option<WaitForCompositorClock>,
    qpc_freq: i64,
}

fn qpc_now() -> i64 {
    let mut c = 0i64;
    // SAFETY: plain query into a local.
    let _ = unsafe { QueryPerformanceCounter(&mut c) };
    c
}

impl DwmSource {
    pub fn new() -> Self {
        let mut freq = 0i64;
        // SAFETY: plain query into a local.
        let _ = unsafe { QueryPerformanceFrequency(&mut freq) };
        // SAFETY: loading a system DLL and looking up an export by its documented name; the
        // transmute matches the documented signature (UINT count, const HANDLE*, DWORD) -> DWORD.
        let compositor_clock = unsafe {
            LoadLibraryW(w!("dcomp.dll"))
                .ok()
                .and_then(|m| GetProcAddress(m, s!("DCompositionWaitForCompositorClock")))
                .map(|f| std::mem::transmute::<unsafe extern "system" fn() -> isize, WaitForCompositorClock>(f))
        };
        DwmSource { compositor_clock, qpc_freq: freq.max(1) }
    }

    fn qpc_to_ns(&self, q: i64) -> u64 {
        ((q as i128 * 1_000_000_000) / self.qpc_freq as i128) as u64
    }

    fn timing(&self) -> Option<DWM_TIMING_INFO> {
        let mut info = DWM_TIMING_INFO { cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32, ..Default::default() };
        // SAFETY: a null HWND asks for the desktop's timing; `info` is sized for the struct.
        unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut info) }.ok()?;
        Some(info)
    }

    /// The next vblank after now on presto's clock, and the refresh interval.
    fn next_vblank(&self) -> (Time, Span, bool) {
        let Some(info) = self.timing() else { return (clock::now() + DEFAULT_INTERVAL, DEFAULT_INTERVAL, true) };
        let mut period = Span(self.qpc_to_ns(info.qpcRefreshPeriod as i64) as i64);
        // Some drivers report a bogus period (29 µs was seen); the refresh rate is right then.
        if period < EARLY {
            let r = info.rateRefresh;
            period = if r.uiNumerator > 0 {
                Span((1_000_000_000i64 * r.uiDenominator as i64) / r.uiNumerator as i64)
            } else {
                DEFAULT_INTERVAL
            };
        }
        let now_q = qpc_now();
        let vblank_ns = self.qpc_to_ns(info.qpcVBlank as i64);
        let now_ns = self.qpc_to_ns(now_q);
        let mut next = vblank_ns;
        let p = period.0.max(1) as u64;
        if next <= now_ns {
            next += ((now_ns - next) / p + 1) * p;
        }
        (clock::time_from_os(next, now_ns), period, false)
    }
}

impl Default for DwmSource {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameSource for DwmSource {
    fn name(&self) -> &'static str {
        if self.compositor_clock.is_some() { "dcomp-compositor-clock" } else { "dwm-flush" }
    }

    fn wait(&self, timeout: Span) -> FrameWait {
        let start = clock::now();
        let ticked = match self.compositor_clock {
            Some(f) => {
                let ms = timeout.as_millis_f64().ceil().clamp(0.0, u32::MAX as f64 - 1.0) as u32;
                // SAFETY: no handles (count 0, null array): wait for the compositor clock only.
                match unsafe { f(0, std::ptr::null(), ms) } {
                    WAIT_TIMEOUT => return FrameWait::TimedOut,
                    WAIT_FAILED => false,
                    _ => true,
                }
            }
            // SAFETY: plain blocking call.
            None => unsafe { DwmFlush() }.is_ok(),
        };
        let (target, interval, mut estimated) = self.next_vblank();
        if !ticked || clock::now() - start < EARLY {
            sleep(interval);
            estimated = true;
        }
        FrameWait::Vsync { target, interval, estimated }
    }

    fn interval(&self) -> Option<Span> {
        Some(self.next_vblank().1)
    }
}
