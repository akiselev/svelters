//! The UI phases of one frame for one window (architecture §2): style, layout, prepare, record,
//! property trees, layers, hit list, accessibility, and the [`Commit`] delta.

use crate::document::Document;
use presto_base::commit::Commit;
use presto_base::ids::WindowId;
use presto_base::time::Time;

impl Document {
    /// Runs the UI phases for `window` for a frame presenting at `target` and returns the commit, or
    /// `None` when nothing changed.
    pub fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        let _ = (window, target);
        None
    }
}
