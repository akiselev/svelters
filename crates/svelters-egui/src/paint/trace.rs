//! PaintTrace (ARCH-WIDGETS §1.1, §3.7): a debug and test side table naming what each painted
//! shape is (`node`, `part`, kind), so the VIS1 gate and kittests measure "the thumb's background"
//! or "the focus ring" instead of guessing which `RectShape` in a rect is which.
//!
//! Primitives record into the `PaintCx`'s trace when it has one. The scene's per-pass trace (on
//! in debug builds and tests only) is OW0b.

use egui::{Color32, Rect, Stroke};

/// What a traced shape is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceKind {
    /// Background and border (`paint_box`).
    Box,
    /// A galley (`paint_text`): `fill` is its colour, `font_size` its size.
    Text,
    Icon,
    Image,
    /// The outline (`paint_outline`): the focus ring when it is a `:focus-visible` rule.
    Outline,
    /// OW1a: a path part (a check mark, a radio dot, a switch knob, a spinner arc, a bar's fill).
    Path,
}

/// One traced shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceEntry {
    /// The node (`NodeId::to_raw()`), 0 outside the scene.
    pub node: u64,
    /// The part (`::part(name)`), `None` for the node itself.
    pub part: Option<&'static str>,
    pub kind: TraceKind,
    pub rect: Rect,
    /// Background (box), text or tint colour.
    pub fill: Color32,
    /// Border (box) or outline stroke.
    pub stroke: Stroke,
    /// Text and icons: the font size.
    pub font_size: f32,
}

/// The traced shapes of one pass, in paint order.
#[derive(Clone, Debug, Default)]
pub struct PaintTrace {
    pub entries: Vec<TraceEntry>,
}

impl PaintTrace {
    /// The shapes of node `node` (`part`: `None` for the node itself).
    pub fn of(&self, node: u64, part: Option<&str>) -> impl Iterator<Item = &TraceEntry> + '_ {
        let part = part.map(str::to_string);
        self.entries.iter().filter(move |e| e.node == node && e.part.map(str::to_string) == part)
    }

    /// The first shape of `kind` of node `node`'s `part`.
    pub fn find(&self, node: u64, part: Option<&str>, kind: TraceKind) -> Option<&TraceEntry> {
        self.of(node, part).find(|e| e.kind == kind)
    }
}
