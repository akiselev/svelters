//! `Pressable`: press, release, click (pointer, Space/Enter on the focused host through egui's
//! interaction, and AccessKit `Click`). Owns `pressed` (`:active`).
//!
//! OW0a follows egui's click semantics (what `egui::Button` did, so every button test holds);
//! the platform rows (Space activates on release on GTK/Qt, pointer thresholds of WG3s pin 10,
//! Escape cancels a held press) are OW1a's. `repeat` (with Qt's `accelerated`) is OW1b's.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::accesskit;

/// When a press activates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActivateOn {
    /// On release inside (buttons).
    #[default]
    Release,
    /// On press (menu bar items, tabs).
    Press,
}

/// What a kind configures on its [`Pressable`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PressParams {
    pub activate_on: ActivateOn,
    /// `(delay, interval)` in seconds: repeat while held (OW1b: spin buttons, scroll arrows).
    pub repeat: Option<(f32, f32)>,
    /// OW1b (Qt `accelerated`): the repeat interval halves after every [`ACCEL_EVERY`] repeats,
    /// down to a fifth of `interval`.
    pub accelerate: bool,
}

/// OW1b: repeats per halving of the interval when [`PressParams::accelerate`] is on.
pub const ACCEL_EVERY: u32 = 8;

/// The time from the press to the first repeat under `p` (`None`: no repeat).
fn repeat_due(p: &PressParams, _k: u32) -> Option<f64> {
    p.repeat.map(|(delay, _)| delay.max(0.0) as f64)
}

/// The interval after `sent` repeats: `interval`, halved every [`ACCEL_EVERY`] repeats under
/// `accelerate`, down to a fifth.
fn repeat_interval(p: &PressParams, sent: u32) -> f64 {
    let base = p.repeat.map_or(0.1, |(_, i)| (i as f64).max(0.01));
    if !p.accelerate {
        return base;
    }
    (base / 2f64.powi((sent / ACCEL_EVERY).min(8) as i32)).max(base / 5.0)
}

/// What [`Pressable`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// The primary button went down on the host.
    Down,
    /// It came up; `inside`: over the host.
    Up { inside: bool },
    /// The press ended without activating (capture lost, Escape, disabled while held).
    Cancel,
    /// Activated: `count` 1 for a click, 2 for a double click (sent after the first one's
    /// `Click { count: 1 }`, as the DOM), `button` which one, `mods` the modifiers.
    Click { count: u8, button: egui::PointerButton, mods: egui::Modifiers },
    /// Held long enough for another step (`repeat`).
    Repeat,
}

impl Press {
    /// A primary single click (or Space/Enter/AccessKit activation).
    pub fn is_click(&self) -> bool {
        matches!(self, Press::Click { count: 1, button: egui::PointerButton::Primary, .. })
    }
}

#[derive(Debug, Default)]
pub struct Pressable {
    down: bool,
    /// When the press began and when it last repeated (input time, seconds).
    down_at: f64,
    last_repeat: f64,
    /// The interval while a repeating press is held.
    repeat_every: Option<f64>,
    /// OW1b: repeats sent by this press (`accelerate`).
    repeats: u32,
}

impl Pressable {
    pub fn is_down(&self) -> bool {
        self.down
    }
}

impl Unit for Pressable {
    type Params = PressParams;
    type Intent = Press;
    const OWNS: StateBits = StateBits::PRESSED;

    fn sense(&self, _p: &PressParams) -> egui::Sense {
        egui::Sense::click()
    }

    fn input(&mut self, p: &PressParams, cx: &mut InputCx<'_>, out: &mut Intents<Press>) {
        let r = cx.resp;
        let down = cx.enabled && r.down_on;
        if down && !self.down {
            out.push(Press::Down);
            self.down_at = cx.now;
            self.last_repeat = cx.now;
            self.repeats = 0;
        } else if !down && self.down {
            out.push(if cx.enabled { Press::Up { inside: r.hovered } } else { Press::Cancel });
        }
        // `repeat`: held over the host, a step after `delay`, then every `interval` (QAbstractButton
        // autoRepeat); the widget treats each `Repeat` as an activation. OW1b: with `accelerate`
        // the interval shortens as the press is held (Qt `QAbstractSpinBox::accelerated`).
        self.repeat_every = None;
        if down && p.repeat.is_some() {
            let interval = repeat_interval(p, self.repeats);
            self.repeat_every = Some(interval);
            let next = if self.repeats == 0 { self.down_at + repeat_due(p, 1).unwrap_or(0.0) } else { self.last_repeat + interval };
            if r.hovered && cx.now >= next {
                out.push(Press::Repeat);
                self.last_repeat = cx.now;
                self.repeats += 1;
            }
        }
        self.down = down;
        cx.set(StateBits::PRESSED, down);
        if !cx.enabled {
            return;
        }
        let mods = cx.input.modifiers;
        if r.clicked {
            out.push(Press::Click { count: 1, button: egui::PointerButton::Primary, mods });
        }
        if r.double_clicked {
            out.push(Press::Click { count: 2, button: egui::PointerButton::Primary, mods });
        }
        if r.secondary_clicked {
            out.push(Press::Click { count: 1, button: egui::PointerButton::Secondary, mods });
        }
        if r.middle_clicked {
            out.push(Press::Click { count: 1, button: egui::PointerButton::Middle, mods });
        }
    }

    fn a11y(&self, _p: &PressParams, node: &mut accesskit::Node) {
        node.add_action(accesskit::Action::Click);
    }

    fn next_frame(&self) -> Option<std::time::Duration> {
        self.repeat_every.map(std::time::Duration::from_secs_f64)
    }

    /// EV3: a focused clickable claims Space and Enter (unmodified).
    fn claims(&self, _p: &PressParams, key: egui::Key, m: egui::Modifiers) -> bool {
        !m.command && !m.alt && !m.shift && matches!(key, egui::Key::Space | egui::Key::Enter)
    }
}

#[cfg(test)]
mod ow1b_tests {
    use super::*;

    #[test]
    fn ow1b_accelerated_repeat_shortens_the_interval() {
        let p = PressParams { repeat: Some((0.5, 0.1)), accelerate: true, ..PressParams::default() };
        assert_eq!(repeat_due(&p, 1), Some(0.5));
        assert_eq!(repeat_interval(&p, 0), 0.1);
        assert_eq!(repeat_interval(&p, ACCEL_EVERY), 0.05);
        assert!((repeat_interval(&p, 10 * ACCEL_EVERY) - 0.02).abs() < 1e-12, "at most five times faster");
        let q = PressParams { accelerate: false, ..p };
        assert_eq!(repeat_interval(&q, 100), 0.1);
    }
}
