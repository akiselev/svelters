//! The rail shared by `slider` and `range-slider`: geometry (value ↔ position, linear or log,
//! horizontal or vertical, inverted) and the paint of its parts, with Fluent 2 / WinUI metrics on
//! the VS Code tokens (ARCH-WIDGETS Q4: VS Code has no slider): a 4 px track (`track`, radius 2),
//! the filled span (`fill`; `range` on a range slider) in `accent`, the rest (`rest`), a 20 px round
//! thumb (`thumb`: an `input-bg` disc with a 1 px boundary and an accent core that grows on hover
//! and shrinks while pressed, as WinUI's), and tick marks (`tick`, 1×4 px at 4 px from the track,
//! `muted`) with optional labels (`mark-label`).
//!
//! Each part paints from its `::part(name)` computed style over its UA table below, in the part's
//! own state (the host's `:disabled`/`:focus-visible` plus the part's `:hover`/`:active`/
//! `:dragging`). Positions are host-local points.

use crate::interact::{Scale, StateBits};
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::Styles;
use presto_base::color::{Color, Fill};
use presto_base::display::{BorderStyle, Prim, Quad};
use presto_base::geom::{Corners, Insets, Point, Rect};

/// Thumb diameter (WinUI `SliderHorizontalThumbWidth`).
pub const THUMB: f32 = 20.0;
/// Track thickness (WinUI `SliderTrackThemeHeight`).
pub const TRACK: f32 = 4.0;
/// Tick length and its gap from the track (WinUI `SliderOutsideTickBarThemeHeight`).
pub const TICK: f32 = 4.0;
pub const TICK_GAP: f32 = 4.0;
/// Space between the rail, the value box and the caption.
pub const GAP: f32 = 8.0;

pub const TRACK_UA: Ua = Ua(&[
    UaRule::base(&[("background-color", Token("muted")), ("border-radius", Px(2.0))]),
    UaRule::when(StateBits::DISABLED, &[("background-color", Token("disabled-fg"))]),
]);

pub const FILL_UA: Ua = Ua(&[
    UaRule::base(&[("background-color", Token("accent")), ("border-radius", Px(2.0))]),
    UaRule::when(StateBits::DISABLED, &[("background-color", Token("disabled-fg"))]),
]);

/// The part after the thumb (Qt `::add-page`): transparent over the track by default.
pub const REST_UA: Ua = Ua(&[UaRule::base(&[("border-radius", Px(2.0))])]);

pub const THUMB_UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("input-bg")),
        ("border-color", Token("control-border")),
        ("border-width", Token("stroke")),
        ("border-radius", Px(THUMB / 2.0)),
        ("color", Token("accent")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

pub const TICK_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("muted")), ("color", Token("muted")), ("font-size", Token("font-size-sm"))])]);

/// The thumb core's colour when no style gives one (egui's `Color32::GRAY`).
const CORE_FALLBACK: Color = Color::rgba(160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0, 1.0);

/// Where the ticks go (`ticks`, Qt `tickPosition`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ticks {
    #[default]
    None,
    /// Above (horizontal) or left (vertical).
    Before,
    After,
    Both,
}

impl Ticks {
    pub fn parse(s: &str) -> Ticks {
        match s {
            "before" | "above" | "left" => Ticks::Before,
            "after" | "below" | "right" => Ticks::After,
            "both" => Ticks::Both,
            _ => Ticks::None,
        }
    }
    fn before(self) -> bool {
        matches!(self, Ticks::Before | Ticks::Both)
    }
    fn after(self) -> bool {
        matches!(self, Ticks::After | Ticks::Both)
    }
    /// The extent the ticks add across the rail.
    pub fn extra(self, labels: bool) -> f32 {
        let one = TICK_GAP + TICK + if labels { 14.0 } else { 0.0 };
        (self.before() as u8 as f32 + self.after() as u8 as f32) * one
    }
}

/// A mark on the rail (`marks`: a value, optionally labelled).
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub value: f64,
    pub label: String,
}

/// `marks`: values separated by commas, each optionally `value:label` (`"0:Low, 50, 100:High"`);
/// values that do not parse are skipped.
pub fn marks_of(s: &str) -> Vec<Mark> {
    s.split(',')
        .filter_map(|m| {
            let (v, label) = m.split_once(':').unwrap_or((m, ""));
            let value: f64 = v.trim().parse().ok()?;
            value.is_finite().then(|| Mark { value, label: label.trim().to_string() })
        })
        .collect()
}

fn from_min_max(a: Point, b: Point) -> Rect {
    Rect { min: a, max: b }
}

fn from_center_size(c: Point, w: f32, h: f32) -> Rect {
    Rect::new(c.x - w / 2.0, c.y - h / 2.0, c.x + w / 2.0, c.y + h / 2.0)
}

fn center(r: &Rect) -> Point {
    Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The rail's geometry in one pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rail {
    /// The rail's box (thumb travel plus half a thumb at each end, ticks included).
    pub rect: Rect,
    pub vertical: bool,
    /// Max at the start (horizontal: left; vertical: bottom, so max at the top is not inverted).
    pub inverted: bool,
    pub scale: Scale,
    pub ticks: Ticks,
    pub labels: bool,
}

impl Default for Rail {
    fn default() -> Self {
        Rail { rect: Rect::ZERO, vertical: false, inverted: false, scale: Scale::default(), ticks: Ticks::None, labels: false }
    }
}

impl Rail {
    /// The line the thumb's centre travels on.
    pub fn travel(&self) -> Rect {
        let r = self.rect;
        let h = THUMB / 2.0;
        let one = TICK_GAP + TICK + if self.labels { 14.0 } else { 0.0 };
        let (b, a) = (if self.ticks.before() { one } else { 0.0 }, if self.ticks.after() { one } else { 0.0 });
        if self.vertical {
            let x = (r.min.x + b + (r.width() - a - b) / 2.0).round();
            from_min_max(Point::new(x, r.min.y + h), Point::new(x, (r.max.y - h).max(r.min.y + h)))
        } else {
            let y = (r.min.y + b + (r.height() - a - b) / 2.0).round();
            from_min_max(Point::new(r.min.x + h, y), Point::new((r.max.x - h).max(r.min.x + h), y))
        }
    }

    /// Fraction 0..1 along the travel, from the start (left, or bottom).
    fn t(&self, v: f64) -> f32 {
        let t = self.scale.fraction(v) as f32;
        if self.inverted { 1.0 - t } else { t }
    }

    /// The thumb's centre at `v`.
    pub fn at(&self, v: f64) -> Point {
        let tr = self.travel();
        let t = self.t(v);
        if self.vertical { Point::new(tr.min.x, lerp(tr.max.y, tr.min.y, t)) } else { Point::new(lerp(tr.min.x, tr.max.x, t), tr.min.y) }
    }

    pub fn thumb(&self, v: f64) -> Rect {
        from_center_size(self.at(v), THUMB, THUMB)
    }

    /// The track's box.
    pub fn track(&self) -> Rect {
        let tr = self.travel();
        if self.vertical { from_center_size(center(&tr), TRACK, tr.height() + THUMB) } else { from_center_size(center(&tr), tr.width() + THUMB, TRACK) }
    }

    /// The track between values `a` and `b`.
    pub fn span(&self, a: f64, b: f64) -> Rect {
        let (pa, pb) = (self.at(a), self.at(b));
        let t = self.track();
        if self.vertical {
            from_min_max(Point::new(t.min.x, pa.y.min(pb.y)), Point::new(t.max.x, pa.y.max(pb.y)))
        } else {
            from_min_max(Point::new(pa.x.min(pb.x), t.min.y), Point::new(pa.x.max(pb.x), t.max.y))
        }
    }

    /// The track from its start to `v` (the fill).
    pub fn fill(&self, v: f64) -> Rect {
        let (t, c) = (self.track(), self.at(v));
        match (self.vertical, self.inverted) {
            (false, false) => from_min_max(t.min, Point::new(c.x, t.max.y)),
            (false, true) => from_min_max(Point::new(c.x, t.min.y), t.max),
            (true, false) => from_min_max(Point::new(t.min.x, c.y), t.max),
            (true, true) => from_min_max(t.min, Point::new(t.max.x, c.y)),
        }
    }

    /// The track from `v` to its end (the rest, Qt `::add-page`).
    pub fn rest(&self, v: f64) -> Rect {
        let (t, c) = (self.track(), self.at(v));
        match (self.vertical, self.inverted) {
            (false, false) => from_min_max(Point::new(c.x, t.min.y), t.max),
            (false, true) => from_min_max(t.min, Point::new(c.x, t.max.y)),
            (true, false) => from_min_max(t.min, Point::new(t.max.x, c.y)),
            (true, true) => from_min_max(Point::new(t.min.x, c.y), t.max),
        }
    }

    /// Paints the `track`, the `fill` (part `fill_part`: up to `v`, or the span `range`) and the
    /// `rest`, in the host's state `host`.
    pub fn paint_track(&self, pc: &mut PaintCx, styles: &Styles, parent: &StyleView, host: StateBits, v: f64, range: Option<(f64, f64)>, fill_part: &'static str) {
        let st = parent.part(styles.part_css("track"), TRACK_UA).in_state(host);
        paint::paint_box(pc, &st, self.track());
        let fill = match range {
            Some((a, b)) => self.span(a, b),
            None => self.fill(v),
        };
        let sf = parent.part(styles.part_css(fill_part), FILL_UA).in_state(host);
        paint::paint_box(pc, &sf, fill);
        let rest = match range {
            Some((_, b)) => self.rest(b),
            None => self.rest(v),
        };
        let sr = parent.part(styles.part_css("rest"), REST_UA).in_state(host);
        paint::paint_box(pc, &sr, rest);
    }

    /// The values the ticks go at: every `interval` from `min` (and at both ends), or the marks in
    /// range, with their labels.
    pub fn tick_values<'m>(&self, interval: f64, marks: &'m [Mark]) -> Vec<(f64, &'m str)> {
        let (lo, hi) = (self.scale.min.min(self.scale.max), self.scale.max.max(self.scale.min));
        let mut values: Vec<(f64, &str)> = Vec::new();
        if marks.is_empty() {
            if interval > 0.0 && interval.is_finite() && (hi - lo).is_finite() && (hi - lo) / interval <= 1000.0 {
                let mut v = lo;
                while v <= hi + interval * 1e-9 {
                    values.push((v, ""));
                    v += interval;
                }
            } else if (hi - lo).is_finite() {
                values.extend([(lo, ""), (hi, "")]);
            }
        } else {
            values.extend(marks.iter().filter(|m| m.value >= lo && m.value <= hi).map(|m| (m.value, m.label.as_str())));
        }
        values
    }

    /// Paints ticks every `interval` from `min` (and at both ends), and the marks.
    pub fn paint_ticks(&self, pc: &mut PaintCx, styles: &Styles, parent: &StyleView, host: StateBits, interval: f64, marks: &[Mark]) {
        if self.ticks == Ticks::None {
            return;
        }
        let s = parent.part(styles.part_css("tick"), TICK_UA).in_state(host);
        let ls = parent.part(styles.part_css("mark-label"), TICK_UA).in_state(host);
        let t = self.track();
        let w = pc.snap_width(1.0);
        for (v, label) in self.tick_values(interval, marks) {
            let c = self.at(v);
            for before in [true, false] {
                if (before && !self.ticks.before()) || (!before && !self.ticks.after()) {
                    continue;
                }
                let r = if self.vertical {
                    let x = if before { t.min.x - TICK_GAP - TICK } else { t.max.x + TICK_GAP };
                    Rect::new(x, c.y - w / 2.0, x + TICK, c.y + w / 2.0)
                } else {
                    let y = if before { t.min.y - TICK_GAP - TICK } else { t.max.y + TICK_GAP };
                    Rect::new(c.x - w / 2.0, y, c.x + w / 2.0, y + TICK)
                };
                let r = pc.snap_rect(r);
                paint::paint_box(pc, &s, r);
                if !label.is_empty() {
                    let g = paint::text_layout(pc.text, &ls, label, None);
                    let gs = g.size();
                    let at = if self.vertical {
                        let x = if before { r.min.x - 2.0 - gs.w } else { r.max.x + 2.0 };
                        Rect::new(x, c.y - gs.h / 2.0, x + gs.w, c.y + gs.h / 2.0)
                    } else {
                        let y = if before { r.min.y - gs.h } else { r.max.y };
                        Rect::new(c.x - gs.w / 2.0, y, c.x + gs.w / 2.0, y + gs.h)
                    };
                    paint::paint_text(pc, &ls, &g, at, Align::Center);
                }
            }
        }
    }

    /// Paints a thumb at `v` in state `bits` (the part's own and the host's) under part `name`.
    pub fn paint_thumb(&self, pc: &mut PaintCx, styles: &Styles, parent: &StyleView, name: &'static str, bits: StateBits, v: f64) {
        let r = self.thumb(v);
        let s = parent.part(styles.part_css(name), THUMB_UA).in_state(bits);
        paint::paint_box(pc, &s, r);
        // WinUI's core: 12 px, 14 hovered, 10 pressed
        let core = if bits.intersects(StateBits(StateBits::PRESSED.0 | StateBits::DRAGGING.0)) {
            10.0
        } else if bits.contains(StateBits::HOVERED) {
            14.0
        } else {
            12.0
        };
        let col = s.color("color").unwrap_or(CORE_FALLBACK);
        let cr = from_center_size(center(&r), core, core);
        pc.push(Prim::Quad(Quad { rect: cr, radii: Corners::all(core / 2.0), fill: Fill::Solid(col), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: BorderStyle::Solid }), cr);
        paint::paint_outline(pc, &s, r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rail(vertical: bool, inverted: bool) -> Rail {
        let rect = if vertical { Rect::new(0.0, 0.0, 24.0, 120.0) } else { Rect::new(0.0, 0.0, 120.0, 24.0) };
        Rail { rect, vertical, inverted, scale: Scale { min: 0.0, max: 100.0, log: false }, ..Rail::default() }
    }

    #[test]
    fn geometry_horizontal_vertical_inverted() {
        let r = rail(false, false);
        assert_eq!(r.travel(), Rect::new(10.0, 12.0, 110.0, 12.0));
        assert_eq!(r.at(50.0), Point::new(60.0, 12.0));
        assert_eq!(r.track(), Rect::new(0.0, 10.0, 120.0, 14.0));
        assert_eq!(r.fill(25.0), Rect::new(0.0, 10.0, 35.0, 14.0));
        assert_eq!(r.rest(25.0), Rect::new(35.0, 10.0, 120.0, 14.0));
        assert_eq!(r.thumb(0.0), Rect::new(0.0, 2.0, 20.0, 22.0));
        assert_eq!(rail(false, true).at(100.0), Point::new(10.0, 12.0), "inverted: max at the start");
        let v = rail(true, false);
        assert_eq!(v.at(100.0), Point::new(12.0, 10.0), "vertical: max at the top");
        assert_eq!(v.fill(0.0), Rect::new(10.0, 110.0, 14.0, 120.0));
        let mut t = rail(false, false);
        t.ticks = Ticks::Both;
        assert_eq!(t.travel().min.y, 12.0, "ticks on both sides keep the travel centred");
        t.ticks = Ticks::Before;
        assert_eq!(t.travel().min.y, 16.0);
        assert_eq!(Ticks::Both.extra(false), 16.0);
    }

    #[test]
    fn marks_and_tick_values() {
        assert_eq!(marks_of("0:Low, 50, x, 100:High"), vec![Mark { value: 0.0, label: "Low".into() }, Mark { value: 50.0, label: String::new() }, Mark { value: 100.0, label: "High".into() }]);
        let r = rail(false, false);
        assert_eq!(r.tick_values(25.0, &[]).len(), 5);
        assert_eq!(r.tick_values(0.0, &[]), vec![(0.0, ""), (100.0, "")], "no interval: both ends");
        let m = marks_of("10:a, 200:b");
        assert_eq!(r.tick_values(25.0, &m), vec![(10.0, "a")], "marks out of range are left out");
    }
}
