//! `Tooltipable`: the host's tooltip shows after the pointer rested on it for the wake-up delay
//! (`Policy::tooltip_delay_ms`, a per-node `tooltip-delay`), at once while another tooltip was
//! shown a moment ago (the follow-on window, Qt's fall-asleep timer), and also on a disabled host
//! (R9: the reason, WG6 QA5). A press or the pointer leaving hides it; `duration` hides it after a
//! time. Writer lane: OW3.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;

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

#[derive(Clone, Debug, Default)]
pub struct Tooltipable {
    /// When the pointer came to rest on the host.
    since: Option<f64>,
    /// When it showed.
    shown: Option<f64>,
    /// A press hid it: not again until the pointer leaves.
    pressed: bool,
    wait: Option<f64>,
}

impl Tooltipable {
    pub fn shown(&self) -> bool {
        self.shown.is_some()
    }

    /// One pass: `over` the pointer is on the host (also when disabled), `pressed` a pointer
    /// button went down this pass, at time `now`.
    pub fn step(&mut self, p: &TooltipParams, over: bool, pressed: bool, now: f64, out: &mut Intents<Tip>) {
        self.wait = None;
        let was = self.shown.is_some();
        if !p.has_tip || !over {
            self.since = None;
            self.pressed = false;
        } else if pressed {
            self.pressed = true;
        } else if self.since.is_none() {
            self.since = Some(now);
        }
        let mut show = false;
        if let Some(since) = self.since
            && !self.pressed
        {
            let due = if p.warm || was { since } else { since + p.delay };
            if now >= due {
                show = true;
            } else {
                self.wait = Some(due - now);
            }
        }
        if show && let Some(at) = self.shown
            && p.duration > 0.0
        {
            if now >= at + p.duration {
                show = false;
                self.pressed = true; // stays hidden until the pointer leaves
            } else {
                self.wait = Some(at + p.duration - now);
            }
        }
        match (was, show) {
            (false, true) => {
                self.shown = Some(now);
                if p.duration > 0.0 {
                    self.wait = Some(p.duration);
                }
                out.push(Tip::Show);
            }
            (true, false) => {
                self.shown = None;
                out.push(Tip::Hide);
            }
            _ => {}
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

    fn next_frame(&self) -> Option<std::time::Duration> {
        self.wait.map(|w| std::time::Duration::from_secs_f64(w.max(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
