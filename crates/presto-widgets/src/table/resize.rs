//! Column resizing from the header's edges (svelters-egui's `Resizable` unit on each
//! `::part(resize)`, R11): a press within the edge's hit zone starts a drag of that column's
//! width; moves resize it live (clamped to the column's `min-width`..`max-width`), the release
//! commits, Escape (or a lost pointer) cancels back to the width before. Hovering an edge shows the
//! `col-resize` cursor at once and the hover bar after [`HOVER_DELAY`].

use presto_base::time::{Span, Time};

/// Hover time on an edge before its hover bar shows (`ScrollPolicy::sash_hover_delay`).
pub const HOVER_DELAY: Span = Span::from_millis(300);

/// What a resize did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResizeIntent {
    /// The width while dragging (draw it; the column's prop is written at the commit).
    Live { data: usize, width: f32 },
    /// The release: the width to write into the column (`column-resized`).
    Commit { data: usize, width: f32 },
    /// The drag ended without a change (Escape, a cancelled pointer).
    Cancel { data: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Drag {
    data: usize,
    start_x: f32,
    start_w: f32,
    min: f32,
    max: f32,
    live: f32,
}

/// The resize state of one table's header edges.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnResize {
    drag: Option<Drag>,
    /// The edge under the pointer (data column) and since when.
    hover: Option<(usize, Time)>,
}

impl ColumnResize {
    /// A press on column `data`'s edge at `x`; its width now and limits.
    pub fn press(&mut self, data: usize, x: f32, width: f32, min: f32, max: f32) {
        self.drag = Some(Drag { data, start_x: x, start_w: width, min, max: max.max(min), live: width.clamp(min, max.max(min)) });
    }

    /// The pointer moved to `x` (pressed or not); `edge` is the edge under it, at time `now`.
    pub fn moved(&mut self, x: f32, edge: Option<usize>, now: Time) -> Option<ResizeIntent> {
        match (&mut self.hover, edge) {
            (Some((d, _)), Some(e)) if *d == e => {}
            (h, e) => *h = e.map(|e| (e, now)),
        }
        let d = self.drag.as_mut()?;
        let w = (d.start_w + x - d.start_x).clamp(d.min, d.max);
        if w == d.live {
            return None;
        }
        d.live = w;
        Some(ResizeIntent::Live { data: d.data, width: w })
    }

    /// The release: commits a changed width.
    pub fn release(&mut self) -> Option<ResizeIntent> {
        let d = self.drag.take()?;
        Some(if d.live != d.start_w { ResizeIntent::Commit { data: d.data, width: d.live } } else { ResizeIntent::Cancel { data: d.data } })
    }

    /// Escape or a cancelled pointer: back to the width before the drag.
    pub fn cancel(&mut self) -> Option<ResizeIntent> {
        self.drag.take().map(|d| ResizeIntent::Cancel { data: d.data })
    }

    /// The pointer left the table.
    pub fn leave(&mut self) {
        self.hover = None;
    }

    /// The live width of column `data` while it is dragged.
    pub fn live(&self, data: usize) -> Option<f32> {
        self.drag.filter(|d| d.data == data).map(|d| d.live)
    }

    pub fn dragging(&self) -> Option<usize> {
        self.drag.map(|d| d.data)
    }

    /// The edge under the pointer (the cursor is `col-resize` over it).
    pub fn hovered(&self) -> Option<usize> {
        self.hover.map(|h| h.0)
    }

    /// Whether column `data`'s hover bar shows at `now`: while dragged, or after hovering
    /// [`HOVER_DELAY`].
    pub fn bar_shown(&self, data: usize, now: Time) -> bool {
        self.dragging() == Some(data) || self.hover.is_some_and(|(d, t)| d == data && now - t >= HOVER_DELAY)
    }

    /// When the hover bar of the hovered edge turns on, if it is still waiting (a timer for the
    /// kind to ask a frame at).
    pub fn next_wakeup(&self, now: Time) -> Option<Time> {
        let (d, t) = self.hover?;
        (self.dragging() != Some(d) && now - t < HOVER_DELAY).then_some(t + HOVER_DELAY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Span = Span::from_millis(1);

    #[test]
    fn drag_clamps_to_the_limits_and_commits_on_release() {
        let mut r = ColumnResize::default();
        r.press(2, 100.0, 80.0, 16.0, 200.0);
        assert_eq!(r.moved(120.0, Some(2), Time::ZERO), Some(ResizeIntent::Live { data: 2, width: 100.0 }));
        assert_eq!(r.moved(0.0, Some(2), Time::ZERO), Some(ResizeIntent::Live { data: 2, width: 16.0 }), "clamped to the minimum");
        assert_eq!(r.moved(-50.0, Some(2), Time::ZERO), None, "no change at the minimum");
        assert_eq!(r.live(2), Some(16.0));
        assert_eq!(r.moved(500.0, None, Time::ZERO), Some(ResizeIntent::Live { data: 2, width: 200.0 }), "and the maximum");
        assert_eq!(r.release(), Some(ResizeIntent::Commit { data: 2, width: 200.0 }));
        assert_eq!(r.live(2), None);
        r.press(0, 10.0, 50.0, 16.0, f32::INFINITY);
        r.moved(30.0, None, Time::ZERO);
        assert_eq!(r.cancel(), Some(ResizeIntent::Cancel { data: 0 }));
        r.press(0, 10.0, 50.0, 16.0, f32::INFINITY);
        assert_eq!(r.release(), Some(ResizeIntent::Cancel { data: 0 }), "a release without a move changes nothing");
    }

    #[test]
    fn hover_bar_shows_after_the_delay() {
        let mut r = ColumnResize::default();
        let t0 = Time(1_000_000_000);
        r.moved(50.0, Some(1), t0);
        assert_eq!(r.hovered(), Some(1));
        assert!(!r.bar_shown(1, t0 + MS));
        assert_eq!(r.next_wakeup(t0), Some(t0 + HOVER_DELAY));
        r.moved(51.0, Some(1), t0 + Span::from_millis(100));
        assert!(r.bar_shown(1, t0 + HOVER_DELAY), "the delay counts from the first hover");
        r.moved(80.0, None, t0 + HOVER_DELAY);
        assert!(!r.bar_shown(1, t0 + HOVER_DELAY));
    }
}
