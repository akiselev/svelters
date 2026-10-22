//! `Pressable`: press, release, click (pointer, Space/Enter on the focused host through egui's
//! interaction, and AccessKit `Click`). Owns `pressed` (`:active`).
//!
//! OW0a follows egui's click semantics (what `egui::Button` did, so every button test holds);
//! the platform rows (Space activates on release on GTK/Qt, pointer thresholds of WG3s pin 10,
//! `repeat` for spin and scroll arrows, Escape cancels a held press) are OW1a/OW1b's.

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

    fn input(&mut self, _p: &PressParams, cx: &mut InputCx<'_>, out: &mut Intents<Press>) {
        let r = cx.resp;
        let down = cx.enabled && r.down_on;
        if down && !self.down {
            out.push(Press::Down);
        } else if !down && self.down {
            out.push(if cx.enabled { Press::Up { inside: r.hovered } } else { Press::Cancel });
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

    /// EV3: a focused clickable claims Space and Enter (unmodified).
    fn claims(&self, _p: &PressParams, key: egui::Key, m: egui::Modifiers) -> bool {
        !m.command && !m.alt && !m.shift && matches!(key, egui::Key::Space | egui::Key::Enter)
    }
}
