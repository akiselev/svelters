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

/// The time from the press to repeat `k` (1-based) under `p` (`None`: no repeat).
fn repeat_due(p: &PressParams, k: u32) -> Option<f64> {
    let (delay, interval) = p.repeat?;
    let (delay, interval) = (delay.max(0.0) as f64, interval.max(0.005) as f64);
    let mut t = delay;
    let mut step = interval;
    for i in 1..k {
        if p.accelerate && i % ACCEL_EVERY == 0 {
            step = (step / 2.0).max(interval / 5.0);
        }
        t += step;
    }
    Some(t)
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
    /// OW1b: when the press began (input time) and how many repeats it sent.
    down_at: f64,
    repeats: u32,
    /// The input time of the last pass and the next repeat's due time ([`Unit::next_frame`]).
    now: f64,
    next_due: Option<f64>,
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
        self.now = cx.now;
        if down && !self.down {
            out.push(Press::Down);
            self.down_at = cx.now;
            self.repeats = 0;
        } else if !down && self.down {
            out.push(if cx.enabled { Press::Up { inside: r.hovered } } else { Press::Cancel });
        }
        self.down = down;
        // OW1b: auto-repeat while held over the host (spin buttons; Qt pauses it outside)
        self.next_due = None;
        if down {
            while let Some(due) = repeat_due(p, self.repeats + 1) {
                if cx.now - self.down_at < due {
                    self.next_due = Some(self.down_at + due);
                    break;
                }
                self.repeats += 1;
                if r.contains_pointer {
                    out.push(Press::Repeat);
                }
            }
        }
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

    /// EV3: a focused clickable claims Space and Enter (unmodified).
    fn claims(&self, _p: &PressParams, key: egui::Key, m: egui::Modifiers) -> bool {
        !m.command && !m.alt && !m.shift && matches!(key, egui::Key::Space | egui::Key::Enter)
    }

    /// OW1b: the next auto-repeat while held.
    fn next_frame(&self) -> Option<std::time::Duration> {
        self.next_due.map(|t| std::time::Duration::from_secs_f64((t - self.now).max(0.0)))
    }
}
