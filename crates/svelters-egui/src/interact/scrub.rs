//! `DragToValue` (scrub; ARCH-WIDGETS §1.2, OW1b): a pointer drag that sets a number. Two modes:
//! **relative** (`drag-value`: the value moves by `speed` per point dragged, Shift ×0.1 for fine
//! control, Ctrl/Cmd ×10 for coarse) and **absolute** (a slider's track: the value is the one under
//! the pointer, with a grab offset when the press lands on the thumb so it does not jump).
//!
//! Intents: `Start` when the drag begins, `Scrub(v)` for each new value, `Commit(v)` on release
//! when the value differs from where the drag began, `End` on every release, `Cancel { restore }`
//! when Escape ends it (the widget puts `restore` back; no commit follows). The widget decides
//! whether a `Scrub` writes the prop (live) or a draft (commit mode, FM1 QW7). Owns `pressed`
//! (`:active`) and `dragging` (`:dragging`).

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::{Key, Rect};

/// A slider's value ↔ position mapping (`scale="linear" | "log"`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scale {
    pub min: f64,
    pub max: f64,
    /// Logarithmic (both ends > 0; otherwise linear).
    pub log: bool,
}

impl Scale {
    fn is_log(&self) -> bool {
        self.log && self.min > 0.0 && self.max > 0.0
    }

    /// Where `v` is along the scale, 0 at `min` and 1 at `max`.
    pub fn fraction(&self, v: f64) -> f64 {
        let (a, b) = (self.min, self.max);
        if !(b - a).is_finite() || b == a || !v.is_finite() {
            return 0.0;
        }
        let t = if self.is_log() { (v.max(f64::MIN_POSITIVE) / a).ln() / (b / a).ln() } else { (v - a) / (b - a) };
        t.clamp(0.0, 1.0)
    }

    /// The value at fraction `t`.
    pub fn value(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let (a, b) = (self.min, self.max);
        if self.is_log() { a * (b / a).powf(t) } else { a + t * (b - a) }
    }
}

/// How the drag maps to the value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrubMode {
    /// Value change per point dragged (right and up increase).
    Relative { speed: f64 },
    /// The value under the pointer along `track` (the thumb's travel); `thumb`: the thumb's rect,
    /// where a press grabs without jumping.
    Absolute { track: Rect, thumb: Rect, vertical: bool, inverted: bool, scale: Scale },
}

/// What a kind configures on its [`DragToValue`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrubParams {
    /// The value now (the prop or the draft).
    pub value: f64,
    pub min: f64,
    pub max: f64,
    /// Snaps to `min + k·step` (≤ 0: continuous).
    pub step: f64,
    pub mode: ScrubMode,
    /// Drags at all (false when read-only or disabled).
    pub active: bool,
    /// Rounding: fixed decimals, else none.
    pub decimals: Option<usize>,
}

impl Default for ScrubParams {
    fn default() -> Self {
        ScrubParams {
            value: 0.0,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 0.0,
            mode: ScrubMode::Relative { speed: 1.0 },
            active: true,
            decimals: None,
        }
    }
}

impl ScrubParams {
    /// `v` snapped to the step grid, rounded and clamped.
    pub fn snap(&self, v: f64) -> f64 {
        let mut v = v;
        if self.step > 0.0 && self.step.is_finite() {
            let base = if self.min.is_finite() { self.min } else { 0.0 };
            v = base + ((v - base) / self.step).round() * self.step;
            v = (v * 1e10).round() / 1e10;
        }
        if let Some(d) = self.decimals {
            let f = 10f64.powi(d.min(15) as i32);
            v = (v * f).round() / f;
        }
        if !self.min.is_nan() && v < self.min {
            v = self.min;
        }
        if !self.max.is_nan() && v > self.max {
            v = self.max;
        }
        v
    }
}

/// What [`DragToValue`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scrub {
    Start,
    Scrub(f64),
    Commit(f64),
    End,
    Cancel { restore: f64 },
}

#[derive(Debug, Default)]
pub struct DragToValue {
    dragging: bool,
    cancelled: bool,
    /// The value when the drag began.
    from: f64,
    /// The unrounded value (relative mode), so slow drags accumulate.
    precise: f64,
    /// The last value sent.
    last: f64,
    /// Absolute mode: pointer offset from the thumb's centre at the press, along the axis.
    grab: f32,
}

impl DragToValue {
    pub fn dragging(&self) -> bool {
        self.dragging && !self.cancelled
    }

    fn at_pointer(p: &ScrubParams, pos: egui::Pos2, grab: f32) -> f64 {
        let ScrubMode::Absolute { track, vertical, inverted, scale, .. } = p.mode else { return p.value };
        let (lo, hi, at) = if vertical { (track.bottom(), track.top(), pos.y + grab) } else { (track.left(), track.right(), pos.x + grab) };
        let span = hi - lo;
        let mut t = if span.abs() < f32::EPSILON { 0.0 } else { ((at - lo) / span) as f64 };
        if inverted {
            t = 1.0 - t;
        }
        scale.value(t)
    }
}

impl Unit for DragToValue {
    type Params = ScrubParams;
    type Intent = Scrub;
    const OWNS: StateBits = StateBits(StateBits::PRESSED.0 | StateBits::DRAGGING.0);

    fn sense(&self, _p: &ScrubParams) -> egui::Sense {
        egui::Sense::click_and_drag()
    }

    fn input(&mut self, p: &ScrubParams, cx: &mut InputCx<'_>, out: &mut Intents<Scrub>) {
        let r = *cx.resp;
        let active = cx.enabled && p.active;
        let absolute = matches!(p.mode, ScrubMode::Absolute { .. });
        // a relative drag starts past egui's drag threshold (a click edits the number instead);
        // an absolute one at the press (a click on the track jumps)
        let held = if absolute { r.down_on || r.clicked } else { r.dragged || r.drag_started };
        if active && held && !self.dragging {
            self.dragging = true;
            self.cancelled = false;
            self.from = p.value;
            self.precise = p.value;
            self.last = p.value;
            self.grab = 0.0;
            if let (ScrubMode::Absolute { thumb, vertical, .. }, Some(pos)) = (p.mode, r.interact_pointer_pos) {
                if thumb.contains(pos) {
                    self.grab = if vertical { thumb.center().y - pos.y } else { thumb.center().x - pos.x };
                }
            }
            out.push(Scrub::Start);
        }
        if self.dragging && !self.cancelled {
            if cx.input.key_pressed(Key::Escape) {
                self.cancelled = true;
                out.push(Scrub::Cancel { restore: self.from });
            } else if active {
                let next = match p.mode {
                    ScrubMode::Absolute { .. } => r.interact_pointer_pos.map(|pos| p.snap(Self::at_pointer(p, pos, self.grab))),
                    ScrubMode::Relative { speed } => {
                        let d = r.drag_delta.x - r.drag_delta.y;
                        let m = cx.input.modifiers;
                        let k = if m.shift { 0.1 } else if m.command || m.ctrl { 10.0 } else { 1.0 };
                        if d != 0.0 {
                            self.precise += d as f64 * speed * k;
                            self.precise = self.precise.clamp(p.min.min(p.max).max(f64::MIN), p.max.max(p.min).min(f64::MAX));
                        }
                        Some(p.snap(self.precise))
                    }
                };
                if let Some(v) = next
                    && v.is_finite()
                    && v != self.last
                {
                    self.last = v;
                    out.push(Scrub::Scrub(v));
                }
            }
        }
        let released = if absolute { !r.down_on } else { !(r.dragged || r.drag_started) };
        if self.dragging && released {
            if !self.cancelled && self.last != self.from {
                out.push(Scrub::Commit(self.last));
            }
            out.push(Scrub::End);
            self.dragging = false;
            self.cancelled = false;
        }
        cx.set(StateBits::PRESSED, active && r.down_on);
        cx.set(StateBits::DRAGGING, self.dragging);
    }

    /// Escape cancels a drag (it is the drag's, not the scope's cancel button).
    fn claims(&self, _p: &ScrubParams, key: Key, _m: egui::Modifiers) -> bool {
        self.dragging && key == Key::Escape
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ow1b_log_scale_and_snapping() {
        let s = Scale { min: 1.0, max: 1000.0, log: true };
        assert!((s.fraction(10.0) - 1.0 / 3.0).abs() < 1e-9);
        assert!((s.value(2.0 / 3.0) - 100.0).abs() < 1e-9);
        let lin = Scale { min: -1.0, max: 1.0, log: true }; // a non-positive end: linear
        assert_eq!(lin.fraction(0.0), 0.5);
        let p = ScrubParams { min: 0.0, max: 10.0, step: 0.5, ..ScrubParams::default() };
        assert_eq!(p.snap(3.3), 3.5);
        assert_eq!(p.snap(42.0), 10.0);
    }
}
