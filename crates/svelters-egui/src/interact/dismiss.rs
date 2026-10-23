//! `Dismissable`: an open floating surface (popup, menu, select list, tooltip) asks to close on
//! Escape and on a press outside it and its anchor (ARCH-WIDGETS §1.2). The widget owns `open` (a
//! prop) and decides; the unit only reports why. Writer lane: OW3.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::{Pos2, Rect};

/// Why a surface should close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    Escape,
    /// A pointer press outside the surface and its anchor.
    OutsidePress,
    /// Keyboard focus left it.
    FocusOut,
    /// Its anchor is no longer drawn.
    AnchorHidden,
    /// The app (`open = false`, `popdown()`).
    Programmatic,
}

/// What [`Dismissable`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dismiss {
    pub reason: Reason,
    /// Where the outside press was.
    pub at: Option<Pos2>,
}

/// A press outside: close, or ignore it (`autohide = false`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outside {
    #[default]
    Close,
    Ignore,
}

/// What [`Dismissable`] is given each pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DismissParams {
    /// The surface and what counts as inside it (its anchor, its open submenus), screen rects.
    pub inside: Vec<Rect>,
    pub on_outside: Outside,
    /// Escape closes it (the topmost surface only: one Escape closes one level).
    pub escape: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Dismissable;

impl Dismissable {
    /// Why `input` dismisses a surface configured by `p`, if it does.
    pub fn check(p: &DismissParams, input: &egui::InputState) -> Option<Dismiss> {
        for e in &input.events {
            match e {
                egui::Event::Key { key: egui::Key::Escape, pressed: true, modifiers, .. } if p.escape && !modifiers.any() => {
                    return Some(Dismiss { reason: Reason::Escape, at: None });
                }
                egui::Event::PointerButton { pos, pressed: true, .. } if p.on_outside == Outside::Close && !p.inside.iter().any(|r| r.contains(*pos)) => {
                    return Some(Dismiss { reason: Reason::OutsidePress, at: Some(*pos) });
                }
                _ => {}
            }
        }
        None
    }
}

impl Unit for Dismissable {
    type Params = DismissParams;
    type Intent = Dismiss;
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &DismissParams, cx: &mut InputCx<'_>, out: &mut Intents<Dismiss>) {
        if let Some(d) = Dismissable::check(p, cx.input) {
            out.push(d);
        }
    }

    fn claims(&self, p: &DismissParams, key: egui::Key, m: egui::Modifiers) -> bool {
        p.escape && key == egui::Key::Escape && !m.any()
    }
}
