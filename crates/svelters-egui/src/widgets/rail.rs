//! OW1b: the rail shared by `slider` and `range-slider`: geometry (value ↔ position, linear or
//! log, horizontal or vertical, inverted) and the paint of its parts, with Fluent 2 / WinUI
//! metrics on the VS Code tokens (ARCH-WIDGETS Q4: VS Code has no slider): a 4 px track
//! (`track`, radius 2), the filled span (`fill`; `range` on a range slider) in `accent`, the rest
//! (`rest`), a 20 px round thumb (`thumb`: an `input-bg` disc with a 1 px boundary and an accent
//! core that grows on hover and shrinks while pressed, as WinUI's), and tick marks (`tick`, 1×4 px
//! at 4 px from the track, `muted`) with optional labels (`mark-label`).
//!
//! Until OW-CSS computes part styles, each part paints from its UA table below in the part's own
//! state (the host's `:disabled`/`:focus-visible` plus the part's `:hover`/`:active`/`:dragging`).

use crate::interact::Scale;
use crate::paint::{self, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use egui::{Pos2, Rect, pos2, vec2};

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

/// `marks` as given: numbers, or `{ value, label }` objects.
pub fn marks_of(v: &serde_json::Value) -> Vec<Mark> {
    let one = |m: &serde_json::Value| match m {
        serde_json::Value::Number(n) => n.as_f64().map(|value| Mark { value, label: String::new() }),
        serde_json::Value::Object(o) => o.get("value").and_then(|x| x.as_f64()).map(|value| Mark { value, label: o.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string() }),
        _ => None,
    };
    match v {
        serde_json::Value::Array(a) => a.iter().filter_map(one).filter(|m| m.value.is_finite()).collect(),
        _ => Vec::new(),
    }
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
        Rail { rect: Rect::NOTHING, vertical: false, inverted: false, scale: Scale::default(), ticks: Ticks::None, labels: false }
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
            let x = (r.left() + b + (r.width() - a - b) / 2.0).round();
            Rect::from_min_max(pos2(x, r.top() + h), pos2(x, (r.bottom() - h).max(r.top() + h)))
        } else {
            let y = (r.top() + b + (r.height() - a - b) / 2.0).round();
            Rect::from_min_max(pos2(r.left() + h, y), pos2((r.right() - h).max(r.left() + h), y))
        }
    }

    /// Fraction 0..1 along the travel, from the start (left, or bottom).
    fn t(&self, v: f64) -> f32 {
        let t = self.scale.fraction(v) as f32;
        if self.inverted { 1.0 - t } else { t }
    }

    /// The thumb's centre at `v`.
    pub fn at(&self, v: f64) -> Pos2 {
        let tr = self.travel();
        let t = self.t(v);
        if self.vertical { pos2(tr.left(), egui::lerp(tr.bottom()..=tr.top(), t)) } else { pos2(egui::lerp(tr.left()..=tr.right(), t), tr.top()) }
    }

    pub fn thumb(&self, v: f64) -> Rect {
        Rect::from_center_size(self.at(v), vec2(THUMB, THUMB))
    }

    /// The track's box.
    pub fn track(&self) -> Rect {
        let tr = self.travel();
        if self.vertical { Rect::from_center_size(tr.center(), vec2(TRACK, tr.height() + THUMB)) } else { Rect::from_center_size(tr.center(), vec2(tr.width() + THUMB, TRACK)) }
    }

    /// The track between values `a` and `b`.
    pub fn span(&self, a: f64, b: f64) -> Rect {
        let (pa, pb) = (self.at(a), self.at(b));
        let t = self.track();
        if self.vertical {
            Rect::from_min_max(pos2(t.left(), pa.y.min(pb.y)), pos2(t.right(), pa.y.max(pb.y)))
        } else {
            Rect::from_min_max(pos2(pa.x.min(pb.x), t.top()), pos2(pa.x.max(pb.x), t.bottom()))
        }
    }

    /// The track from its start to `v` (the fill).
    pub fn fill(&self, v: f64) -> Rect {
        let (t, c) = (self.track(), self.at(v));
        match (self.vertical, self.inverted) {
            (false, false) => Rect::from_min_max(t.min, pos2(c.x, t.max.y)),
            (false, true) => Rect::from_min_max(pos2(c.x, t.min.y), t.max),
            (true, false) => Rect::from_min_max(pos2(t.min.x, c.y), t.max),
            (true, true) => Rect::from_min_max(t.min, pos2(t.max.x, c.y)),
        }
    }

    /// The track from `v` to its end (the rest, Qt `::add-page`).
    pub fn rest(&self, v: f64) -> Rect {
        let (t, c) = (self.track(), self.at(v));
        match (self.vertical, self.inverted) {
            (false, false) => Rect::from_min_max(pos2(c.x, t.min.y), t.max),
            (false, true) => Rect::from_min_max(t.min, pos2(c.x, t.max.y)),
            (true, false) => Rect::from_min_max(t.min, pos2(t.max.x, c.y)),
            (true, true) => Rect::from_min_max(pos2(t.min.x, c.y), t.max),
        }
    }

    /// Paints the `track`, the `fill` (part `fill_part`: up to `v`, or the span `range`) and the
    /// `rest`.
    pub fn paint_track(&self, pc: &mut PaintCx, host: StateBits, dark: bool, v: f64, range: Option<(f64, f64)>, fill_part: &'static str) {
        let st = StyleView::ua_only(TRACK_UA, host, dark);
        paint::paint_box(&mut pc.part("track"), &st, self.track());
        let fill = match range {
            Some((a, b)) => self.span(a, b),
            None => self.fill(v),
        };
        let sf = StyleView::ua_only(FILL_UA, host, dark);
        paint::paint_box(&mut pc.part(fill_part), &sf, fill);
        let rest = match range {
            Some((_, b)) => self.rest(b),
            None => self.rest(v),
        };
        let sr = StyleView::ua_only(REST_UA, host, dark);
        paint::paint_box(&mut pc.part("rest"), &sr, rest);
    }

    /// Paints ticks every `interval` from `min` (and at both ends), and the marks.
    pub fn paint_ticks(&self, pc: &mut PaintCx, host: StateBits, dark: bool, interval: f64, marks: &[Mark]) {
        if self.ticks == Ticks::None {
            return;
        }
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
        let s = StyleView::ua_only(TICK_UA, host, dark);
        let t = self.track();
        let w = pc.snap_width(1.0);
        let ctx = pc.painter.ctx().clone();
        for (v, label) in values {
            let c = self.at(v);
            for before in [true, false] {
                if (before && !self.ticks.before()) || (!before && !self.ticks.after()) {
                    continue;
                }
                let r = if self.vertical {
                    let x = if before { t.left() - TICK_GAP - TICK } else { t.right() + TICK_GAP };
                    Rect::from_min_size(pos2(x, c.y - w / 2.0), vec2(TICK, w))
                } else {
                    let y = if before { t.top() - TICK_GAP - TICK } else { t.bottom() + TICK_GAP };
                    Rect::from_min_size(pos2(c.x - w / 2.0, y), vec2(w, TICK))
                };
                let r = pc.snap_rect(r);
                paint::paint_box(&mut pc.part("tick"), &s, r);
                if !label.is_empty() {
                    let g = paint::text_galley(&ctx, &s, label, None);
                    let at = if self.vertical {
                        let x = if before { r.left() - 2.0 - g.size().x } else { r.right() + 2.0 };
                        Rect::from_min_size(pos2(x, c.y - g.size().y / 2.0), g.size())
                    } else {
                        let y = if before { r.top() - g.size().y } else { r.bottom() };
                        Rect::from_min_size(pos2(c.x - g.size().x / 2.0, y), g.size())
                    };
                    paint::paint_text(&mut pc.part("mark-label"), &s, &g, at, paint::Align::Center);
                }
            }
        }
    }

    /// Paints a thumb at `v` in state `bits` (the part's own and the host's) under part `name`.
    pub fn paint_thumb(&self, pc: &mut PaintCx, name: &'static str, bits: StateBits, dark: bool, v: f64) {
        let r = self.thumb(v);
        let s = StyleView::ua_only(THUMB_UA, bits, dark);
        let mut tc = pc.part(name);
        paint::paint_box(&mut tc, &s, r);
        // WinUI's core: 12 px, 14 hovered, 10 pressed
        let core = if bits.intersects(StateBits(StateBits::PRESSED.0 | StateBits::DRAGGING.0)) {
            10.0
        } else if bits.contains(StateBits::HOVERED) {
            14.0
        } else {
            12.0
        };
        let col = s.color("color").unwrap_or(egui::Color32::GRAY);
        tc.painter.circle_filled(r.center(), core / 2.0, col);
        paint::paint_outline(&mut tc, &s, r);
    }
}
