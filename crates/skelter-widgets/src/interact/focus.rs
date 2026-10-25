//! `Focusable`: keyboard focus (`:focus`) and `:focus-visible`, with the platform's
//! focus-on-click (FX1: GTK `focus-on-click`, Qt `StrongFocus`).
//!
//! `:focus-visible` is set when focus was gained by keyboard or program, never by a pointer press
//! (FX1, the CSS rule): a button clicked into focus shows no ring until Tab moves focus.
//! Tab order and scopes stay EV3's (`focus::record`); this unit only reads and asks.

use super::{FocusReason, InputCx, Intents, Sense, StateBits, Unit};
use skelter_base::accesskit;

/// What a kind configures on its [`Focusable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusParams {
    /// In the Tab order.
    pub tab_stop: bool,
    /// A press focuses the host; `None`: the platform's (`Policy::focus_on_click`).
    pub focus_on_click: Option<bool>,
}

impl Default for FocusParams {
    fn default() -> Self {
        FocusParams { tab_stop: true, focus_on_click: None }
    }
}

/// What [`Focusable`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Gained(FocusReason),
    Lost,
}

#[derive(Debug, Default)]
pub struct Focusable {
    focused: bool,
    visible: bool,
    /// A press asked for focus: the focus that follows came from the pointer.
    by_pointer: bool,
}

impl Focusable {
    pub fn focused(&self) -> bool {
        self.focused
    }
    pub fn focus_visible(&self) -> bool {
        self.focused && self.visible
    }
}

impl Unit for Focusable {
    type Params = FocusParams;
    type Intent = Focus;
    const OWNS: StateBits = StateBits(StateBits::FOCUSED.0 | StateBits::FOCUS_VISIBLE.0);

    fn sense(&self, p: &FocusParams) -> Sense {
        if p.tab_stop { Sense::FOCUSABLE } else { Sense::HOVER }
    }

    fn input(&mut self, p: &FocusParams, cx: &mut InputCx<'_>, out: &mut Intents<Focus>) {
        let r = cx.resp;
        let on_click = p.focus_on_click.unwrap_or(cx.policy.focus_on_click);
        let pressed_here = cx.enabled && r.down_on;
        if on_click && pressed_here && !cx.focus.has {
            cx.focus.request();
            self.by_pointer = true;
        }
        let has = cx.focus.has && cx.enabled;
        if has && !self.focused {
            let reason = if self.by_pointer || pressed_here || r.clicked || cx.focus.pointer_pressed {
                FocusReason::Pointer
            } else if cx.focus.key_pressed {
                FocusReason::Keyboard
            } else {
                FocusReason::Program
            };
            self.visible = reason != FocusReason::Pointer;
            out.push(Focus::Gained(reason));
        } else if !has && self.focused {
            self.visible = false;
            out.push(Focus::Lost);
        }
        if !has && !cx.focus.requested() {
            self.by_pointer = false;
        }
        self.focused = has;
        cx.set(StateBits::FOCUSED, has);
        cx.set(StateBits::FOCUS_VISIBLE, has && self.visible);
    }

    fn a11y(&self, _p: &FocusParams, node: &mut accesskit::Node) {
        node.add_action(accesskit::Action::Focus);
    }
}
