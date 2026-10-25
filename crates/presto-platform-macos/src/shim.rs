//! The frame-source interface lane PLATFORM is still writing in `presto-platform` (`frame` module,
//! b1 brief item 5). Until it lands, this backend's [`MacFrameSource`](crate::MacFrameSource)
//! implements this stand-in; the consolidation switches it to `presto_platform`'s trait.

use presto_base::time::{Span, Time};

/// One display refresh, as a frame source reports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VsyncTick {
    /// When the source fired.
    pub now: Time,
    /// When the frame being prepared will reach the display.
    pub target: Time,
    pub interval: Span,
}

/// A per-window vsync source the compositor drives (architecture §10). Created on the main thread,
/// then used from the compositor thread.
pub trait FrameSource: Send {
    /// Start or stop ticks; a stopped source costs nothing.
    fn set_active(&self, active: bool);
}
