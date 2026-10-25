//! The drag source half: a primary press on a source node that moves past the drag threshold
//! starts a drag ([`SourceIntent::Begin`]: the host then calls [`DragSession::begin`] with the
//! node's payload). A release before the threshold stays a click for the node's own widget.
//! The source owns `:dragging` until the session reports its `drag-end`.
//!
//! [`DragSession::begin`]: super::DragSession::begin

use presto_base::cursor::CursorIcon;
use presto_base::event::{MouseButton, PointerEvent, PointerPhase};
use presto_base::geom::Point;

/// Points the pointer moves before a press becomes a drag (svelters-egui's pinned
/// `Policy::drag_threshold`; GTK uses 8, Qt 10, Windows `SM_CXDRAG` 4).
pub const DRAG_THRESHOLD: f32 = 10.0;

/// What [`DragSource::pointer`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SourceIntent {
    /// A drag starts at `at` (window points): begin a session with the node's payload.
    Begin { at: Point },
}

/// A source node's drag state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DragSource {
    press: Option<(u32, Point)>,
    dragging: bool,
    hovered: bool,
}

impl DragSource {
    /// A pointer event reaching the source (window points). `enabled` false drops any press.
    pub fn pointer(&mut self, ev: &PointerEvent, threshold: f32, enabled: bool) -> Option<SourceIntent> {
        if !enabled {
            self.press = None;
            return None;
        }
        match ev.phase {
            PointerPhase::Enter => self.hovered = true,
            PointerPhase::Leave => self.hovered = false,
            PointerPhase::Down if ev.button == Some(MouseButton::Primary) && !self.dragging => {
                self.press = Some((ev.pointer, ev.pos));
                self.hovered = true;
            }
            PointerPhase::Move => {
                let (id, start) = self.press?;
                if id == ev.pointer && (ev.pos - start).length() >= threshold {
                    self.press = None;
                    self.dragging = true;
                    return Some(SourceIntent::Begin { at: ev.pos });
                }
            }
            PointerPhase::Up | PointerPhase::Cancel => self.press = None,
            _ => {}
        }
        None
    }

    /// The drag this source started ended (the session delivered `drag-end`).
    pub fn ended(&mut self) {
        self.dragging = false;
    }

    /// `:dragging`.
    pub fn dragging(&self) -> bool {
        self.dragging
    }

    /// `grab` over an idle source (the session picks the cursor during a drag).
    pub fn cursor(&self) -> Option<CursorIcon> {
        (self.hovered && !self.dragging).then_some(CursorIcon::Grab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::event::{Modifiers, PointerType};
    use presto_base::geom::Vec2;

    fn ev(phase: PointerPhase, x: f32) -> PointerEvent {
        let button = matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary);
        PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button, buttons: 1, pos: Point::new(x, 0.0), modifiers: Modifiers::default(), pressure: 0.0, tilt: Vec2::ZERO, history: Default::default() }
    }

    #[test]
    fn a_drag_starts_past_the_threshold_only() {
        let mut s = DragSource::default();
        assert_eq!(s.pointer(&ev(PointerPhase::Down, 0.0), DRAG_THRESHOLD, true), None);
        assert_eq!(s.pointer(&ev(PointerPhase::Move, 5.0), DRAG_THRESHOLD, true), None);
        assert_eq!(s.cursor(), Some(CursorIcon::Grab));
        assert_eq!(s.pointer(&ev(PointerPhase::Move, 12.0), DRAG_THRESHOLD, true), Some(SourceIntent::Begin { at: Point::new(12.0, 0.0) }));
        assert!(s.dragging());
        assert_eq!(s.cursor(), None);
        assert_eq!(s.pointer(&ev(PointerPhase::Move, 30.0), DRAG_THRESHOLD, true), None, "begins once");
        s.ended();
        assert!(!s.dragging());
    }

    #[test]
    fn a_click_is_not_a_drag() {
        let mut s = DragSource::default();
        s.pointer(&ev(PointerPhase::Down, 0.0), DRAG_THRESHOLD, true);
        s.pointer(&ev(PointerPhase::Up, 3.0), DRAG_THRESHOLD, true);
        assert_eq!(s.pointer(&ev(PointerPhase::Move, 40.0), DRAG_THRESHOLD, true), None);
        s.pointer(&ev(PointerPhase::Down, 0.0), DRAG_THRESHOLD, false);
        assert_eq!(s.pointer(&ev(PointerPhase::Move, 40.0), DRAG_THRESHOLD, true), None, "disabled hosts don't drag");
    }
}
