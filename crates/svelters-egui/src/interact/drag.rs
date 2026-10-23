//! `Draggable` (ARCH-WIDGETS §1.2, OW1b): a plain pointer drag past a threshold, for sashes,
//! scrollbar thumbs and drag sources (OW2, OW5 use it; `DragToValue` is the numeric variant).
//! Owns `dragging`. Escape cancels while dragging.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::{Key, Pos2, Vec2};

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
    /// Points the pointer moves before the drag starts; `None`: egui's own decision (its click vs
    /// drag threshold). Per platform (WG3s pin 10): GTK 8, Qt 10, Windows 4.
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
    Start { origin: Pos2 },
    Move { delta: Vec2, total: Vec2 },
    End,
    Cancel,
}

#[derive(Debug, Default)]
pub struct Draggable {
    origin: Option<Pos2>,
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

    fn sense(&self, _p: &DragParams) -> egui::Sense {
        egui::Sense::drag()
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

    fn claims(&self, _p: &DragParams, key: Key, _m: egui::Modifiers) -> bool {
        self.dragging() && key == Key::Escape
    }
}
