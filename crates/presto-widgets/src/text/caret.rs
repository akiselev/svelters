//! The caret as a compositor fast commit (architecture §7.3): the focused field is the caret's
//! owner and writes its window-space rect with [`FastCommit::Caret`]; the compositor draws and
//! blinks it, so a caret move or a blink costs no UI phases. Every edit and motion sends the rect
//! again, which restarts the blink with the caret shown (all platforms do). The same rect is the
//! IME candidate area of the next [`Commit`](presto_base::commit::Commit).
//!
//! The blink cycle and its timeout are the platform's ([`TextPolicy::caret_blink_ms`],
//! [`TextPolicy::caret_blink_timeout_ms`]); `FastCommit::Caret` does not carry them yet, so
//! [`blink_visible`] states the rule the compositor applies (a request in the lane record).
//!
//! [`TextPolicy::caret_blink_ms`]: super::policy::TextPolicy::caret_blink_ms
//! [`TextPolicy::caret_blink_timeout_ms`]: super::policy::TextPolicy::caret_blink_timeout_ms

use presto_base::commit::FastCommit;
use presto_base::geom::Rect;
use presto_base::ids::{Revision, WindowId};
use presto_base::time::{Span, Time};

/// What the field last sent, to send only changes and restarts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CaretState {
    sent: Option<Rect>,
    /// An edit or motion since the last send: send again even if the rect is the same.
    restart: bool,
}

impl CaretState {
    /// The caret was moved by input or text changed under it: the blink restarts.
    pub fn restart(&mut self) {
        self.restart = true;
    }

    /// The fast commit for caret rect `rect` (`None`: hidden — unfocused, a selection, or an IME
    /// that hides it), when it differs from what was sent or a restart is due.
    pub fn update(&mut self, window: WindowId, rect: Option<Rect>, revision: Revision) -> Option<FastCommit> {
        if rect == self.sent && !(self.restart && rect.is_some()) {
            self.restart = false;
            return None;
        }
        self.sent = rect;
        self.restart = false;
        Some(FastCommit::Caret { window, rect, revision })
    }

    pub fn shown(&self) -> Option<Rect> {
        self.sent
    }
}

/// Whether a blinking caret shows at `now`, `since` the last restart: on for the first half of each
/// `cycle_ms`, then off; always on with no blink (`cycle_ms` 0) and once `timeout_ms` passed
/// without input (GTK stops blinking with the caret shown).
pub fn blink_visible(since: Time, now: Time, cycle_ms: u32, timeout_ms: Option<u32>) -> bool {
    if cycle_ms == 0 {
        return true;
    }
    let ms = (now - since).as_millis_f64().max(0.0);
    if timeout_ms.is_some_and(|t| ms >= t as f64) {
        return true;
    }
    (ms % cycle_ms as f64) < cycle_ms as f64 / 2.0
}

/// The next time [`blink_visible`] changes after `now`, if it ever does.
pub fn next_blink(since: Time, now: Time, cycle_ms: u32, timeout_ms: Option<u32>) -> Option<Time> {
    if cycle_ms == 0 {
        return None;
    }
    let half = cycle_ms as f64 / 2.0;
    let ms = (now - since).as_millis_f64().max(0.0);
    let next = ((ms / half).floor() + 1.0) * half;
    match timeout_ms {
        Some(t) if ms >= t as f64 => None,
        // the last toggle is the one that ends shown at the timeout
        Some(t) if next >= t as f64 => Some(since + Span::from_millis(t as i64)),
        _ => Some(since + Span::from_secs_f64(next / 1000.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_fast_commits_only_on_change_or_restart() {
        let w = WindowId(1);
        let r = Some(Rect::new(10.0, 0.0, 11.0, 16.0));
        let mut c = CaretState::default();
        assert!(matches!(c.update(w, r, Revision(1)), Some(FastCommit::Caret { rect, .. }) if rect == r));
        assert_eq!(c.update(w, r, Revision(1)), None);
        c.restart();
        assert!(c.update(w, r, Revision(2)).is_some(), "typing at the same place restarts the blink");
        assert!(matches!(c.update(w, None, Revision(3)), Some(FastCommit::Caret { rect: None, .. })));
        c.restart();
        assert_eq!(c.update(w, None, Revision(3)), None, "a hidden caret has nothing to restart");
    }

    #[test]
    fn blink_cycle_and_timeout() {
        let t0 = Time(0);
        let at = |ms: i64| t0 + Span::from_millis(ms);
        assert!(blink_visible(t0, at(0), 1200, Some(10_000)));
        assert!(!blink_visible(t0, at(700), 1200, Some(10_000)));
        assert!(blink_visible(t0, at(1250), 1200, Some(10_000)));
        assert!(blink_visible(t0, at(10_300), 1200, Some(10_000)), "shown after the timeout");
        assert_eq!(next_blink(t0, at(100), 1200, None), Some(at(600)));
        assert_eq!(next_blink(t0, at(9_700), 1200, Some(10_000)), Some(at(10_000)));
        assert_eq!(next_blink(t0, at(10_000), 1200, Some(10_000)), None);
        assert!(blink_visible(t0, at(5_000), 0, None));
    }
}
