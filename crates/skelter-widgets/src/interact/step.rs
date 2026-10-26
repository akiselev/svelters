//! `Steppable` (ARCH-WIDGETS §1.2): a value that moves in steps, from the keyboard (Up/Down,
//! Left/Right on sliders, PageUp/PageDown, Home/End), the wheel, and AccessKit (`Increment`,
//! `Decrement`, `SetValue`). It owns no state bits; the kind applies its intents with
//! [`StepParams::apply`] (clamped, wrapped, rounded) and writes the prop.
//!
//! FM1 QW7's spec: a step is a commit point; PageUp/PageDown move `page` (ten steps unless the
//! kind sets it); the wheel steps once per line or page notch, and every 40 points of a touchpad
//! scroll, over the host, and only while it has focus under `wheel = Focused` (so scrolling a long
//! property grid does not change values).

use super::{FrameEvent, InputCx, Intents, Mods, StateBits, Unit};
use skelter_base::accesskit;
use skelter_base::key::Key;

/// When the wheel steps the value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wheel {
    /// While the pointer is over the host.
    Hover,
    /// While the pointer is over the host and the host has keyboard focus (FM1's default).
    #[default]
    Focused,
    /// Never.
    Off,
}

impl Wheel {
    /// `hover`, `focused` (default) or `off`.
    pub fn parse(s: &str) -> Wheel {
        match s {
            "hover" => Wheel::Hover,
            "off" | "none" => Wheel::Off,
            _ => Wheel::Focused,
        }
    }
}

/// Which arrow keys step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arrows {
    /// Up and Down (spin boxes: Left and Right move the caret).
    #[default]
    Vertical,
    /// All four (sliders): Right and Up increase.
    Both,
}

/// What a kind configures on its [`Steppable`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepParams {
    /// The current value (the prop, or the draft in commit mode).
    pub value: f64,
    /// Bounds; infinite for none.
    pub min: f64,
    pub max: f64,
    /// One step (Up/Down, a step button, a wheel notch); ≤ 0 is 1% of a finite range, else 1.
    pub step: f64,
    /// PageUp/PageDown, in value units; ≤ 0 is ten steps.
    pub page: f64,
    /// Stepping past one end goes to the other (Qt `wrapping`).
    pub wrap: bool,
    /// Qt AdaptiveDecimalStepType: a step is one decade below the value's magnitude (never below
    /// `step`).
    pub adaptive: bool,
    pub wheel: Wheel,
    pub arrows: Arrows,
    /// Keys and wheel act (false when read-only).
    pub active: bool,
    /// Rounding of the result: fixed decimals, else 10 places (`0.1 + 0.2` is `0.3`).
    pub decimals: Option<usize>,
}

impl Default for StepParams {
    fn default() -> Self {
        StepParams {
            value: 0.0,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 1.0,
            page: 0.0,
            wrap: false,
            adaptive: false,
            wheel: Wheel::Focused,
            arrows: Arrows::Vertical,
            active: true,
            decimals: None,
        }
    }
}

/// What [`Steppable`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// `n` steps (signed); `page`: `n` pages.
    By { n: i32, page: bool },
    /// To `min` (Home) or `max` (End).
    Home,
    End,
    /// AccessKit `SetValue`.
    Set(f64),
}

/// Every 40 points of a touchpad scroll step once (FM1).
pub const POINTS_PER_STEP: f32 = 40.0;

impl StepParams {
    /// One step at `v` (adaptive or not).
    pub fn step_at(&self, v: f64, up: bool) -> f64 {
        let base = if self.step > 0.0 && self.step.is_finite() {
            self.step
        } else if (self.max - self.min).is_finite() && self.max > self.min {
            (self.max - self.min) / 100.0
        } else {
            1.0
        };
        if !self.adaptive || !v.is_finite() || v == 0.0 {
            return base;
        }
        // one decade below the value; stepping down from a power of ten uses the lower decade
        let a = v.abs();
        let mut e = a.log10().floor();
        if !up && (a - 10f64.powf(e)).abs() < 1e-12 * a.max(1.0) {
            e -= 1.0;
        }
        10f64.powf(e - 1.0).max(base)
    }

    /// The page in value units.
    pub fn page_size(&self, v: f64, up: bool) -> f64 {
        if self.page > 0.0 && self.page.is_finite() { self.page } else { 10.0 * self.step_at(v, up) }
    }

    /// Rounds `v` as the kind shows it.
    pub fn round(&self, v: f64) -> f64 {
        let d = self.decimals.unwrap_or(10).min(15) as i32;
        let f = 10f64.powi(d);
        let r = (v * f).round() / f;
        if r.is_finite() { r } else { v }
    }

    /// Clamps `v` into the bounds (a NaN bound is no bound).
    pub fn clamp(&self, v: f64) -> f64 {
        let mut v = v;
        if !self.min.is_nan() && v < self.min {
            v = self.min;
        }
        if !self.max.is_nan() && v > self.max {
            v = self.max;
        }
        v
    }

    /// The value after `s`, from `v`: rounded, then wrapped (from an end to the other) or clamped.
    pub fn apply(&self, v: f64, s: Step) -> f64 {
        let next = match s {
            Step::Home if self.min.is_finite() => self.min,
            Step::End if self.max.is_finite() => self.max,
            Step::Home | Step::End => v,
            Step::Set(x) => x,
            Step::By { n, page } => {
                let up = n > 0;
                let mut x = v;
                for _ in 0..n.unsigned_abs().min(10_000) {
                    let d = if page { self.page_size(x, up) } else { self.step_at(x, up) };
                    x = self.round(if up { x + d } else { x - d });
                }
                if self.wrap && self.min.is_finite() && self.max.is_finite() {
                    // Qt: past an end from that end goes to the other end; otherwise to the end
                    if x > self.max {
                        x = if v >= self.max { self.min } else { self.max };
                    } else if x < self.min {
                        x = if v <= self.min { self.max } else { self.min };
                    }
                }
                x
            }
        };
        self.clamp(self.round(next))
    }

    /// Whether stepping up (or down) from `v` can change it (a step button's `:disabled`).
    pub fn can_step(&self, v: f64, up: bool) -> bool {
        self.wrap || if up { v < self.max } else { v > self.min }
    }
}

#[derive(Debug, Default)]
pub struct Steppable {
    /// Touchpad scroll not yet a whole step.
    acc: f32,
    /// This pass's wheel input was taken (the kind keeps it from enclosing scroll areas).
    took_wheel: bool,
}

impl Steppable {
    /// The wheel stepped this pass: the kind should consume the scroll.
    pub fn took_wheel(&self) -> bool {
        self.took_wheel
    }
}

/// The notch count of a wheel step: rounded, and at least one for a partial notch.
fn notches(d: f32) -> i32 {
    if d == 0.0 {
        0
    } else if d.abs() < 1.0 {
        d.signum() as i32
    } else {
        d.round() as i32
    }
}

impl Unit for Steppable {
    type Params = StepParams;
    type Intent = Step;
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &StepParams, cx: &mut InputCx<'_>, out: &mut Intents<Step>) {
        self.took_wheel = false;
        if !cx.enabled || !p.active {
            self.acc = 0.0;
            return;
        }
        let focused = cx.resp.has_focus;
        for e in &cx.input.events {
            match e {
                FrameEvent::Key { key, pressed: true, modifiers: m, .. } if focused && !m.command && !m.alt && !m.ctrl => {
                    let both = p.arrows == Arrows::Both;
                    match key {
                        Key::ArrowUp => out.push(Step::By { n: 1, page: false }),
                        Key::ArrowDown => out.push(Step::By { n: -1, page: false }),
                        Key::ArrowRight if both => out.push(Step::By { n: 1, page: false }),
                        Key::ArrowLeft if both => out.push(Step::By { n: -1, page: false }),
                        Key::PageUp => out.push(Step::By { n: 1, page: true }),
                        Key::PageDown => out.push(Step::By { n: -1, page: true }),
                        Key::Home if both || p.min.is_finite() => out.push(Step::Home),
                        Key::End if both || p.max.is_finite() => out.push(Step::End),
                        _ => {}
                    }
                }
                FrameEvent::Wheel { delta, .. } => {
                    let over = cx.resp.contains_pointer;
                    let wants = match p.wheel {
                        Wheel::Hover => over,
                        Wheel::Focused => over && focused,
                        Wheel::Off => false,
                    };
                    if !wants {
                        continue;
                    }
                    // positive y is the wheel turned away from the user (a scroll up): increase
                    let n = match cx.input.wheel_notches {
                        Some(v) => notches(if v.y != 0.0 { v.y } else { v.x }),
                        None => {
                            self.acc += if delta.y != 0.0 { delta.y } else { delta.x };
                            let n = (self.acc / POINTS_PER_STEP).trunc();
                            self.acc -= n * POINTS_PER_STEP;
                            n as i32
                        }
                    };
                    self.took_wheel = true;
                    if n != 0 {
                        out.push(Step::By { n, page: false });
                    }
                }
                _ => {}
            }
        }
        for r in cx.a11y {
            match r.action {
                accesskit::Action::Increment => out.push(Step::By { n: 1, page: false }),
                accesskit::Action::Decrement => out.push(Step::By { n: -1, page: false }),
                accesskit::Action::SetValue => {
                    if let Some(accesskit::ActionData::NumericValue(v)) = &r.data {
                        out.push(Step::Set(*v));
                    }
                }
                _ => {}
            }
        }
    }

    fn a11y(&self, p: &StepParams, node: &mut accesskit::Node) {
        if p.value.is_finite() {
            node.set_numeric_value(p.value);
        }
        if p.min.is_finite() {
            node.set_min_numeric_value(p.min);
        }
        if p.max.is_finite() {
            node.set_max_numeric_value(p.max);
        }
        node.set_numeric_value_step(p.step_at(p.value, true));
        node.set_numeric_value_jump(p.page_size(p.value, true));
        node.add_action(accesskit::Action::SetValue);
        if p.can_step(p.value, true) {
            node.add_action(accesskit::Action::Increment);
        }
        if p.can_step(p.value, false) {
            node.add_action(accesskit::Action::Decrement);
        }
    }

    /// EV3: Up/Down (Left/Right too on sliders), PageUp/PageDown, Home/End.
    fn claims(&self, p: &StepParams, key: Key, m: Mods) -> bool {
        if m.command || m.alt || m.ctrl {
            return false;
        }
        match key {
            Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown => true,
            Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End => p.arrows == Arrows::Both,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::Sense;
    use crate::interact::tracker::testing::Rig;
    use skelter_base::event::ScrollDelta;
    use skelter_base::geom::Vec2;

    fn p() -> StepParams {
        StepParams { min: 0.0, max: 10.0, step: 0.5, ..StepParams::default() }
    }

    #[test]
    fn steps_clamp_wrap_and_round() {
        let p = p();
        assert_eq!(p.apply(3.0, Step::By { n: 1, page: true }), 8.0, "a page is ten steps");
        assert_eq!(p.apply(8.0, Step::By { n: 1, page: true }), 10.0, "clamped");
        assert_eq!(p.apply(3.0, Step::Home), 0.0);
        let w = StepParams { wrap: true, ..p };
        assert_eq!(w.apply(10.0, Step::By { n: 1, page: false }), 0.0, "from max past max: min");
        assert_eq!(w.apply(9.8, Step::By { n: 1, page: false }), 10.0, "to max first");
        assert_eq!(w.apply(0.0, Step::By { n: -1, page: false }), 10.0);
        let f = StepParams { step: 0.1, ..p };
        assert_eq!(f.apply(0.2, Step::By { n: 1, page: false }), 0.3, "rounded");
        assert!(!p.can_step(10.0, true) && p.can_step(10.0, false) && w.can_step(10.0, true));
    }

    #[test]
    fn adaptive_steps_follow_the_magnitude() {
        let a = StepParams { adaptive: true, step: 0.01, ..StepParams::default() };
        assert_eq!(a.step_at(1234.0, true), 100.0);
        assert_eq!(a.step_at(5.0, true), 0.1);
        assert_eq!(a.step_at(100.0, false), 1.0, "down from a power of ten uses the lower decade");
        assert_eq!(a.step_at(0.0, true), 0.01, "never below step");
    }

    #[test]
    fn keys_step_only_while_focused() {
        let mut rig = Rig::new(Sense::FOCUSABLE);
        let mut u = Steppable::default();
        let both = StepParams { arrows: Arrows::Both, ..p() };
        let mut out = Intents::default();
        rig.key(Key::ArrowUp, |cx| u.input(&both, cx, &mut out));
        assert!(out.is_empty(), "not focused");
        rig.focus(true, |_| {});
        for k in [Key::ArrowUp, Key::ArrowLeft, Key::PageUp, Key::End] {
            rig.key(k, |cx| u.input(&both, cx, &mut out));
        }
        assert_eq!(out.0, vec![Step::By { n: 1, page: false }, Step::By { n: -1, page: false }, Step::By { n: 1, page: true }, Step::End]);
        let mut out = Intents::default();
        rig.key(Key::ArrowLeft, |cx| u.input(&p(), cx, &mut out));
        assert!(out.is_empty(), "Left/Right only with `Arrows::Both`");
        assert!(u.claims(&both, Key::ArrowLeft, Mods::NONE) && !u.claims(&p(), Key::ArrowLeft, Mods::NONE));
        let ctrl = Mods { ctrl: true, command: true, ..Mods::NONE };
        assert!(!u.claims(&both, Key::ArrowUp, ctrl));
    }

    #[test]
    fn wheel_notches_and_touchpad_points() {
        let mut rig = Rig::new(Sense::FOCUSABLE);
        let mut u = Steppable::default();
        let hover = StepParams { wheel: Wheel::Hover, ..p() };
        let mut out = Intents::default();
        rig.hover(true, |_| {});
        rig.wheel(ScrollDelta::Lines(Vec2::new(0.0, 2.0)), false, |cx| u.input(&hover, cx, &mut out));
        assert_eq!(out.0, vec![Step::By { n: 2, page: false }]);
        assert!(u.took_wheel());
        rig.wheel(ScrollDelta::Lines(Vec2::new(0.0, -0.3)), false, |cx| u.input(&hover, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Step::By { n: -1, page: false }), "a partial notch is one step");
        let mut out = Intents::default();
        rig.wheel(ScrollDelta::Pixels(Vec2::new(0.0, 30.0)), true, |cx| u.input(&hover, cx, &mut out));
        assert!(out.is_empty() && u.took_wheel(), "30 points: not a step yet, but taken");
        rig.wheel(ScrollDelta::Pixels(Vec2::new(0.0, 15.0)), true, |cx| u.input(&hover, cx, &mut out));
        assert_eq!(out.0, vec![Step::By { n: 1, page: false }], "45 points: one step");
        // `focused`: only with focus
        let mut out = Intents::default();
        rig.wheel(ScrollDelta::Lines(Vec2::new(0.0, 1.0)), false, |cx| u.input(&p(), cx, &mut out));
        assert!(out.is_empty() && !u.took_wheel());
    }

    #[test]
    fn accesskit_increment_and_set_value() {
        let mut rig = Rig::new(Sense::FOCUSABLE);
        let mut u = Steppable::default();
        let mut out = Intents::default();
        rig.a11y(accesskit::Action::Increment, None, |cx| u.input(&p(), cx, &mut out));
        rig.a11y(accesskit::Action::SetValue, Some(accesskit::ActionData::NumericValue(4.0)), |cx| u.input(&p(), cx, &mut out));
        assert_eq!(out.0, vec![Step::By { n: 1, page: false }, Step::Set(4.0)]);
        let mut node = accesskit::Node::new(accesskit::Role::Slider);
        u.a11y(&StepParams { value: 10.0, ..p() }, &mut node);
        assert_eq!((node.numeric_value(), node.max_numeric_value(), node.numeric_value_step()), (Some(10.0), Some(10.0), Some(0.5)));
        assert!(!node.supports_action(accesskit::Action::Increment) && node.supports_action(accesskit::Action::Decrement), "at max");
    }
}
