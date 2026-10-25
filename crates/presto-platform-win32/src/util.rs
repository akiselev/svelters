//! Small Win32 helpers: the QPC clock presto's [`Time`] is read from, conversions from message
//! timestamps, wide strings and `LPARAM`/`WPARAM` unpacking.

use presto_base::time::{Clock, Time};
use std::sync::OnceLock;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::SystemInformation::GetTickCount;

struct QpcBase {
    epoch: i64,
    freq: i64,
}

fn qpc_base() -> &'static QpcBase {
    static BASE: OnceLock<QpcBase> = OnceLock::new();
    BASE.get_or_init(|| {
        let (mut epoch, mut freq) = (0i64, 0i64);
        // Both always succeed on Windows XP and later (documented).
        unsafe {
            let _ = QueryPerformanceFrequency(&mut freq);
            let _ = QueryPerformanceCounter(&mut epoch);
        }
        QpcBase { epoch, freq: freq.max(1) }
    })
}

/// The QPC frequency in counts per second.
pub fn qpc_frequency() -> i64 {
    qpc_base().freq
}

/// The current QPC count.
pub fn qpc_now() -> i64 {
    let mut now = 0i64;
    unsafe {
        let _ = QueryPerformanceCounter(&mut now);
    }
    now
}

/// A QPC count (`POINTER_INFO::PerformanceCount`, DXGI frame statistics, DComp frame times) on
/// presto's clock; counts before the process epoch clamp to zero.
pub fn qpc_to_time(count: i64) -> Time {
    let base = qpc_base();
    Time(counts_to_nanos(count - base.epoch, base.freq))
}

/// `counts / freq` seconds in nanoseconds, without overflow; negative counts give 0.
pub fn counts_to_nanos(counts: i64, freq: i64) -> u64 {
    if counts <= 0 || freq <= 0 {
        return 0;
    }
    ((counts as u128 * 1_000_000_000) / freq as u128) as u64
}

/// A message's `GetMessageTime` (milliseconds of the 32-bit tick count) on presto's clock: the
/// current time minus the message's age.
pub fn message_time_to_time(msg_ms: i32) -> Time {
    let age_ms = unsafe { GetTickCount() }.wrapping_sub(msg_ms as u32);
    let now = qpc_to_time(qpc_now());
    // A message older than ten seconds means the tick count wrapped or the time is bogus.
    if age_ms > 10_000 { now } else { Time(now.0.saturating_sub(age_ms as u64 * 1_000_000)) }
}

/// The real clock: QPC.
#[derive(Clone, Copy, Debug, Default)]
pub struct QpcClock;

impl Clock for QpcClock {
    fn now(&self) -> Time {
        qpc_to_time(qpc_now())
    }
}

/// A NUL-terminated UTF-16 copy of `s`.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn loword(v: usize) -> u16 {
    (v & 0xFFFF) as u16
}

pub fn hiword(v: usize) -> u16 {
    ((v >> 16) & 0xFFFF) as u16
}

/// The signed x and y packed in an `LPARAM` (`GET_X_LPARAM`, `GET_Y_LPARAM`).
pub fn lparam_point(lparam: isize) -> (i32, i32) {
    let v = lparam as usize;
    (loword(v) as i16 as i32, hiword(v) as i16 as i32)
}

/// The signed wheel delta of `WM_MOUSEWHEEL`/`WM_POINTERWHEEL` (`GET_WHEEL_DELTA_WPARAM`).
pub fn wheel_delta(wparam: usize) -> i16 {
    hiword(wparam) as i16
}

/// UTF-16 length of `s`.
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_to_nanos_scales_and_clamps() {
        assert_eq!(counts_to_nanos(10_000_000, 10_000_000), 1_000_000_000);
        assert_eq!(counts_to_nanos(1, 10_000_000), 100);
        assert_eq!(counts_to_nanos(-5, 10_000_000), 0);
        // No overflow for a year of 10 MHz counts.
        let year = 365 * 24 * 3600 * 10_000_000i64;
        assert_eq!(counts_to_nanos(year, 10_000_000), 365 * 24 * 3600 * 1_000_000_000u64);
    }

    #[test]
    fn lparam_points_are_signed() {
        let packed = ((-3i16 as u16 as usize) | ((-7i16 as u16 as usize) << 16)) as isize;
        assert_eq!(lparam_point(packed), (-3, -7));
        assert_eq!(wheel_delta(((-120i16 as u16 as usize) << 16) | 0x8), -120);
    }

    #[test]
    fn qpc_clock_is_monotonic() {
        let a = QpcClock.now();
        let b = QpcClock.now();
        assert!(b >= a);
    }
}
