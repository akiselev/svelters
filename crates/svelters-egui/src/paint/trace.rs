//! PaintTrace (ARCH-WIDGETS §1.1, §3.7): a debug and test side table naming what each painted
//! shape is (`node`, `part`, kind), so the VIS1 gate and kittests measure "the thumb's background"
//! or "the focus ring" instead of guessing which `RectShape` in a rect is which.
//!
//! Primitives record into the `PaintCx`'s trace when it has one; scene nodes record into the
//! context's per-pass trace while [`enable`] has it on (debug builds only: release builds never
//! record). [`last`] returns the latest pass's entries.

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
    /// Boxes (`paint_box`) and outlines: the top-left corner radius.
    pub radius: f32,
    /// Boxes (`paint_box`): the padding, `[top, right, bottom, left]` (CSS order; VIS1c).
    pub padding: [f32; 4],
    /// The painter's opacity (`opacity` multiplied into the ui), which the colours above do not
    /// include.
    pub opacity: f32,
}

/// The traced shapes of one pass, in paint order.
#[derive(Clone, Debug, Default)]
pub struct PaintTrace {
    pub entries: Vec<TraceEntry>,
}

#[derive(Clone, Default)]
struct Store {
    on: bool,
    pass: u64,
    trace: PaintTrace,
}

fn store_id() -> egui::Id {
    egui::Id::unique("svelters-egui/paint-trace")
}

/// Turns the per-pass paint trace of `ctx`'s scene nodes on or off (debug builds; a no-op in
/// release builds). Tests and the VIS1 gate turn it on.
pub fn enable(ctx: &egui::Context, on: bool) {
    if cfg!(debug_assertions) {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Store>(store_id()).on = on);
    }
}

/// Whether scene nodes record their shapes.
pub fn enabled(ctx: &egui::Context) -> bool {
    cfg!(debug_assertions) && ctx.data(|d| d.get_temp::<Store>(store_id()).is_some_and(|s| s.on))
}

pub(crate) fn push(ctx: &egui::Context, e: TraceEntry) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Store>(store_id());
        if s.pass != pass {
            s.pass = pass;
            s.trace.entries.clear();
        }
        s.trace.entries.push(e);
    });
}

/// The shapes the latest traced pass painted.
pub fn last(ctx: &egui::Context) -> PaintTrace {
    ctx.data(|d| d.get_temp::<Store>(store_id()).map(|s| s.trace).unwrap_or_default())
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
