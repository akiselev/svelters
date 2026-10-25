//! `Draggable` (ARCH-WIDGETS §1.2): a plain pointer drag past a threshold, for sashes, scrollbar
//! thumbs and drag sources (`DragToValue` is the numeric variant). Owns `dragging`. Escape cancels
//! while dragging.

use super::{InputCx, Intents, Mods, Sense, StateBits, Unit};
use presto_base::geom::{Point, Vec2};
use presto_base::key::Key;

/// Which way the drag moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    #[default]
    Both,
    X,
    Y,
}

/// What a kind configures on its [`Draggable`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragParams {
    /// Points the pointer moves before the drag starts; `None`: the tracker's own decision (its
    /// click vs drag threshold). Per platform: GTK 8, Qt 10, Windows 4.
    pub threshold: Option<f32>,
    pub axis: Axis,
}

impl Default for DragParams {
    fn default() -> Self {
        DragParams { threshold: None, axis: Axis::Both }
    }
}

/// What [`Draggable`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    Start { origin: Point },
    Move { delta: Vec2, total: Vec2 },
    End,
    Cancel,
}

#[derive(Debug, Default)]
pub struct Draggable {
    origin: Option<Point>,
    started: bool,
    cancelled: bool,
    total: Vec2,
}

impl Draggable {
    pub fn dragging(&self) -> bool {
        self.started && !self.cancelled
    }
}

fn on_axis(v: Vec2, a: Axis) -> Vec2 {
    match a {
        Axis::Both => v,
        Axis::X => Vec2::new(v.x, 0.0),
        Axis::Y => Vec2::new(0.0, v.y),
    }
}

impl Unit for Draggable {
    type Params = DragParams;
    type Intent = Drag;
    const OWNS: StateBits = StateBits::DRAGGING;

    fn sense(&self, _p: &DragParams) -> Sense {
        Sense::DRAG
    }

    fn input(&mut self, p: &DragParams, cx: &mut InputCx<'_>, out: &mut Intents<Drag>) {
        let r = *cx.resp;
        let held = cx.enabled && (r.down_on || r.dragged);
        if held && self.origin.is_none() {
            self.origin = r.interact_pointer_pos;
            self.total = Vec2::ZERO;
            self.started = false;
            self.cancelled = false;
        }
        if let (true, Some(origin)) = (held, self.origin) {
            let pos = r.interact_pointer_pos.unwrap_or(origin);
            let total = on_axis(pos - origin, p.axis);
            if !self.started {
                let past = match p.threshold {
                    Some(t) => total.length() >= t,
                    None => r.dragged,
                };
                if past {
                    self.started = true;
                    out.push(Drag::Start { origin });
                }
            }
            if self.started && !self.cancelled {
                if cx.input.key_pressed(Key::Escape) {
                    self.cancelled = true;
                    out.push(Drag::Cancel);
                } else if total != self.total {
                    out.push(Drag::Move { delta: total - self.total, total });
                    self.total = total;
                }
            }
        }
        if !held && self.origin.is_some() {
            if self.started && !self.cancelled {
                out.push(Drag::End);
            }
            self.origin = None;
            self.started = false;
            self.cancelled = false;
        }
        cx.set(StateBits::DRAGGING, self.dragging());
    }

    fn claims(&self, _p: &DragParams, key: Key, _m: Mods) -> bool {
        self.dragging() && key == Key::Escape
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::tracker::testing::Rig;

    #[test]
    fn threshold_axis_moves_and_end() {
        let mut rig = Rig::new(Sense::DRAG);
        let mut d = Draggable::default();
        let p = DragParams { threshold: Some(4.0), axis: Axis::X };
        let mut out = Intents::default();
        rig.down(10.0, 5.0, |cx| d.input(&p, cx, &mut out));
        assert!(out.is_empty(), "not past the threshold yet");
        rig.move_to(12.0, 9.0, |cx| d.input(&p, cx, &mut out));
        assert!(out.is_empty(), "only x counts on the X axis");
        rig.move_to(15.0, 9.0, |cx| d.input(&p, cx, &mut out));
        assert_eq!(out.0, vec![Drag::Start { origin: Point::new(10.0, 5.0) }, Drag::Move { delta: Vec2::new(5.0, 0.0), total: Vec2::new(5.0, 0.0) }]);
        assert!(d.dragging() && rig.state.contains(StateBits::DRAGGING));
        assert!(d.claims(&p, Key::Escape, Mods::NONE));
        rig.up(15.0, 9.0, |cx| d.input(&p, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Drag::End));
        assert!(!rig.state.contains(StateBits::DRAGGING));
    }

    #[test]
    fn escape_cancels_a_drag_below_the_click_distance() {
        let p = DragParams { threshold: Some(4.0), axis: Axis::Both };
        // the tracker drags (past its click distance): Escape ends its drag, the unit sees an End
        let mut rig = Rig::new(Sense::DRAG);
        let mut d = Draggable::default();
        let mut out = Intents::default();
        rig.down(10.0, 5.0, |cx| d.input(&p, cx, &mut out));
        rig.move_to(30.0, 5.0, |cx| d.input(&p, cx, &mut out));
        rig.key(Key::Escape, |cx| d.input(&p, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Drag::End));
        // the unit started it within the click distance (the press is held): Escape cancels
        let mut rig = Rig::new(Sense::CLICK_AND_DRAG);
        let mut d = Draggable::default();
        let mut out = Intents::default();
        rig.down(10.0, 5.0, |cx| d.input(&p, cx, &mut out));
        rig.move_to(15.0, 5.0, |cx| d.input(&p, cx, &mut out));
        assert_eq!(out.0.first(), Some(&Drag::Start { origin: Point::new(10.0, 5.0) }));
        rig.key(Key::Escape, |cx| d.input(&p, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Drag::Cancel));
        rig.up(15.0, 5.0, |cx| d.input(&p, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Drag::Cancel), "no End after a cancel");
        assert!(!d.dragging());
    }
}
