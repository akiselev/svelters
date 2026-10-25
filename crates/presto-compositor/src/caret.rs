//! The caret blink the compositor runs for a window (the rule of presto-widgets'
//! `text::caret::blink_visible`): visible for the first half of each cycle, counted from the last
//! `FastCommit::Caret`, and steady (visible) once the timeout has passed.

use presto_base::time::{Span, Time};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaretBlink {
    /// One on + off period.
    pub cycle: Duration,
    /// Blinking stops (caret steady) this long after the last caret change; `None` = never.
    pub timeout: Option<Duration>,
}

impl CaretBlink {
    /// GTK: `gtk-cursor-blink-time` 1200 ms, `gtk-cursor-blink-timeout` 10 s.
    pub const GTK: CaretBlink = CaretBlink { cycle: Duration::from_millis(1200), timeout: Some(Duration::from_secs(10)) };
    /// Qt: `QStyleHints::cursorFlashTime` 1000 ms.
    pub const QT: CaretBlink = CaretBlink { cycle: Duration::from_millis(1000), timeout: None };
    /// Windows: `GetCaretBlinkTime` 530 ms per phase.
    pub const WINDOWS: CaretBlink = CaretBlink { cycle: Duration::from_millis(1060), timeout: None };
    /// macOS: 1060 ms.
    pub const MACOS: CaretBlink = CaretBlink { cycle: Duration::from_millis(1060), timeout: None };

    /// This OS's default (GTK's on Linux and the BSDs).
    pub fn platform_default() -> Self {
        if cfg!(windows) {
            Self::WINDOWS
        } else if cfg!(target_os = "macos") {
            Self::MACOS
        } else {
            Self::GTK
        }
    }

    fn half(&self) -> i64 {
        (self.cycle.as_nanos() as i64 / 2).max(1)
    }

    fn timeout_span(&self) -> Option<Span> {
        self.timeout.map(|t| Span(t.as_nanos() as i64))
    }

    /// Whether the caret shows `elapsed` after the last caret change.
    pub fn visible(&self, elapsed: Span) -> bool {
        if elapsed.0 < 0 || self.timeout_span().is_some_and(|t| elapsed >= t) {
            return true;
        }
        let cycle = (self.half() * 2).max(1);
        elapsed.0 % cycle < self.half()
    }

    /// The first time after `now` at which [`Self::visible`] may change, for a caret changed at
    /// `since`; `None` once it is steady.
    pub fn next_change(&self, since: Time, now: Time) -> Option<Time> {
        let elapsed = (now - since).0.max(0);
        let timeout = self.timeout_span();
        if timeout.is_some_and(|t| elapsed >= t.0) {
            return None;
        }
        let half = self.half();
        let boundary = since + Span((elapsed / half + 1) * half);
        Some(match timeout {
            Some(t) => boundary.min(since + t),
            None => boundary,
        })
    }
}
