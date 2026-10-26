//! `Tooltipable`: the host's tooltip shows after the pointer rested on it for the wake-up delay
//! (`Policy::tooltip_delay_ms`, a per-node `tooltip-delay`), at once while another tooltip was
//! shown a moment ago (the follow-on window, Qt's fall-asleep timer), and also on a disabled host
//! (R9: the reason). A press or the pointer leaving hides it; `duration` hides it after a time.

use super::{InputCx, Intents, StateBits, Unit};

/// What [`Tooltipable`] is given each pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipParams {
    /// The host has a tooltip (empty text: the unit stays idle).
    pub has_tip: bool,
    /// Seconds of rest before it shows.
    pub delay: f64,
    /// Another tooltip hid within the follow-on window: show at once.
    pub warm: bool,
    /// Seconds it stays (0: while hovered).
    pub duration: f64,
}

impl Default for TooltipParams {
    fn default() -> Self {
        TooltipParams { has_tip: true, delay: 0.5, warm: false, duration: 0.0 }
    }
}

/// What [`Tooltipable`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip {
    Show,
    Hide,
}

/// The egui-style unit over [`crate::views::tooltip::TooltipTimer`], the one tooltip timer (the
/// `title` tooltip and the `tooltip` surface step it too): seconds in, [`Tip`] intents out.
#[derive(Clone, Debug, Default)]
pub struct Tooltipable {
    timer: crate::views::tooltip::TooltipTimer,
    wait: Option<f64>,
}

fn at(secs: f64) -> skelter_base::time::Time {
    skelter_base::time::Time((secs.max(0.0) * 1e9) as u64)
}

fn span(secs: f64) -> skelter_base::time::Span {
    skelter_base::time::Span::from_secs_f64(secs.max(0.0))
}

impl Tooltipable {
    pub fn shown(&self) -> bool {
        self.timer.shown()
    }

    /// One pass: `over` the pointer is on the host (also when disabled), `pressed` a pointer
    /// button went down this pass, at time `now`.
    pub fn step(&mut self, p: &TooltipParams, over: bool, pressed: bool, now: f64, out: &mut Intents<Tip>) {
        use crate::views::tooltip::{Tip as T, TipInput, TooltipWarmth};
        let input = TipInput { over: over && p.has_tip, pressed, ..TipInput::default() };
        // warm: as if a tip hid just now (zero follow-on window)
        let mut warmth = if p.warm { TooltipWarmth::hidden_at(at(now)) } else { TooltipWarmth::default() };
        let r = self.timer.step(input, at(now), span(p.delay), span(p.duration), skelter_base::time::Span::ZERO, &mut warmth);
        self.wait = self.timer.wake().map(|w| (w.0 as f64 / 1e9 - now).max(0.0));
        match r {
            Some(T::Show) => out.push(Tip::Show),
            Some(T::Hide) => out.push(Tip::Hide),
            None => {}
        }
    }
}

impl Unit for Tooltipable {
    type Params = TooltipParams;
    type Intent = Tip;
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &TooltipParams, cx: &mut InputCx<'_>, out: &mut Intents<Tip>) {
        let over = cx.resp.hovered || (!cx.enabled && cx.resp.contains_pointer);
        let pressed = cx.input.pointer.any_pressed();
        self.step(p, over, pressed, cx.now, out);
    }

    fn next_frame(&self) -> Option<f64> {
        self.wait.map(|w| w.max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::Sense;
    use crate::interact::tracker::testing::Rig;

    #[test]
    fn delay_follow_on_press_and_duration() {
        let p = TooltipParams { delay: 0.5, ..TooltipParams::default() };
        let mut t = Tooltipable::default();
        let mut out = Intents::default();
        t.step(&p, true, false, 0.0, &mut out);
        assert!(out.is_empty() && t.next_frame().is_some());
        t.step(&p, true, false, 0.6, &mut out);
        assert_eq!(out.0, vec![Tip::Show]);
        t.step(&p, true, true, 0.7, &mut out);
        assert_eq!(out.0, vec![Tip::Show, Tip::Hide], "a press hides it");
        t.step(&p, true, false, 2.0, &mut out);
        assert_eq!(out.len(), 2, "not again until the pointer leaves");
        t.step(&p, false, false, 2.1, &mut out);
        // warm: at once
        let warm = TooltipParams { warm: true, ..p };
        t.step(&warm, true, false, 2.2, &mut out);
        assert_eq!(out.0.last(), Some(&Tip::Show));
        // duration
        let mut d = Tooltipable::default();
        let mut o = Intents::default();
        let dp = TooltipParams { delay: 0.0, duration: 1.0, ..p };
        d.step(&dp, true, false, 0.0, &mut o);
        d.step(&dp, true, false, 1.5, &mut o);
        assert_eq!(o.0, vec![Tip::Show, Tip::Hide]);
        // disabled-host and empty text are the caller's (`over`, `has_tip`)
        let mut e = Tooltipable::default();
        let mut eo = Intents::default();
        e.step(&TooltipParams { has_tip: false, ..dp }, true, false, 0.0, &mut eo);
        assert!(eo.is_empty());
    }

    #[test]
    fn a_disabled_host_shows_its_tip() {
        let mut rig = Rig::new(Sense::HOVER);
        rig.enabled = false;
        let mut t = Tooltipable::default();
        let p = TooltipParams { delay: 0.5, ..TooltipParams::default() };
        let mut out = Intents::default();
        rig.hover(true, |cx| t.input(&p, cx, &mut out));
        assert!(out.is_empty());
        rig.advance(600);
        rig.timer(|cx| t.input(&p, cx, &mut out));
        assert_eq!(out.0, vec![Tip::Show], "R9: disabled controls keep their tooltip");
        rig.hover(false, |cx| t.input(&p, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Tip::Hide));
    }
}
