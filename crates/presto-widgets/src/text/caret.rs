//! The caret as a compositor fast commit (architecture §7.3): the focused field is the caret's
//! owner and writes its window-space rect with [`FastCommit::Caret`]; the compositor draws and
//! blinks it, so a caret move or a blink costs no UI phases. Every edit and motion sends the rect
//! again, which restarts the blink with the caret shown (all platforms do). The same rect is the
//! IME candidate area of the next [`Commit`](presto_base::commit::Commit).
//!
//! The blink cycle and its timeout are the platform's ([`TextPolicy::caret_blink_ms`],
//! [`TextPolicy::caret_blink_timeout_ms`]); the app gives them to the compositor per window
//! (`presto_compositor::CompositorHandle::set_caret_blink` with a `CaretBlink`).
//!
//! [`TextPolicy::caret_blink_ms`]: super::policy::TextPolicy::caret_blink_ms
//! [`TextPolicy::caret_blink_timeout_ms`]: super::policy::TextPolicy::caret_blink_timeout_ms

use presto_base::commit::FastCommit;
use presto_base::geom::Rect;
use presto_base::ids::{Revision, WindowId};

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
}
