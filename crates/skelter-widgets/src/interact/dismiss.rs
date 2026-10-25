//! `Dismissable`: an open floating surface (popup, menu, select list, tooltip) asks to close on
//! Escape and on a press outside it and its anchor (ARCH-WIDGETS §1.2). The kind owns `open` (a
//! prop) and decides; the unit only reports why.
//!
//! It reads the presses its host receives, inside `inside` or not (a backdrop the surface covers the
//! window with), and the presses outside the host that skelter-core routes to an open surface
//! (`Event::OutsidePress`, for a kind whose `light_dismiss` is on).

use super::{FrameEvent, InputCx, InputFrame, Intents, Mods, StateBits, Unit};
use skelter_base::geom::{Point, Rect};
use skelter_base::key::Key;

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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dismiss {
    pub reason: Reason,
    /// Where the outside press was (host-local).
    pub at: Option<Point>,
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
    /// The surface and what counts as inside it (its anchor, its open submenus), in the host's
    /// local coordinates (the pass's).
    pub inside: Vec<Rect>,
    pub on_outside: Outside,
    /// Escape closes it (the topmost surface only: one Escape closes one level).
    pub escape: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Dismissable;

/// Both edges inclusive (egui's `Rect::contains`).
fn contains(r: &Rect, p: Point) -> bool {
    p.x >= r.min.x && p.x <= r.max.x && p.y >= r.min.y && p.y <= r.max.y
}

impl Dismissable {
    /// Why `input` dismisses a surface configured by `p`, if it does.
    pub fn check(p: &DismissParams, input: &InputFrame) -> Option<Dismiss> {
        for e in &input.events {
            if let FrameEvent::Key { key: Key::Escape, pressed: true, modifiers, .. } = e
                && p.escape
                && !modifiers.any()
            {
                return Some(Dismiss { reason: Reason::Escape, at: None });
            }
        }
        if p.on_outside == Outside::Close {
            let pressed = if input.pointer.any_pressed() { input.pointer.interact_pos() } else { None };
            if let Some(pos) = input.outside_press.or(pressed)
                && !p.inside.iter().any(|r| contains(r, pos))
            {
                return Some(Dismiss { reason: Reason::OutsidePress, at: Some(pos) });
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

    fn claims(&self, p: &DismissParams, key: Key, m: Mods) -> bool {
        p.escape && key == Key::Escape && !m.any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::Sense;
    use crate::interact::tracker::testing::Rig;

    #[test]
    fn escape_and_outside_press() {
        let mut rig = Rig::new(Sense::CLICK);
        rig.size = skelter_base::geom::Size::new(400.0, 300.0);
        let p = DismissParams { inside: vec![Rect::new(10.0, 10.0, 110.0, 60.0)], on_outside: Outside::Close, escape: true };
        let mut u = Dismissable;
        let mut out = Intents::default();
        rig.down(50.0, 30.0, |cx| u.input(&p, cx, &mut out));
        rig.up(50.0, 30.0, |cx| u.input(&p, cx, &mut out));
        assert!(out.is_empty(), "a press inside");
        rig.down(200.0, 200.0, |cx| u.input(&p, cx, &mut out));
        assert_eq!(out.0, vec![Dismiss { reason: Reason::OutsidePress, at: Some(Point::new(200.0, 200.0)) }]);
        rig.up(200.0, 200.0, |_| {});
        rig.key(Key::Escape, |cx| u.input(&p, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Dismiss { reason: Reason::Escape, at: None }));
        let keep = DismissParams { on_outside: Outside::Ignore, escape: false, ..p };
        let mut out = Intents::default();
        rig.down(200.0, 200.0, |cx| u.input(&keep, cx, &mut out));
        rig.up(200.0, 200.0, |_| {});
        rig.key(Key::Escape, |cx| u.input(&keep, cx, &mut out));
        assert!(out.is_empty() && !u.claims(&keep, Key::Escape, Mods::NONE));
    }

    #[test]
    fn routed_outside_press() {
        // skelter-core's light-dismiss routing: a press elsewhere in the window, not on the host
        let p = DismissParams { inside: vec![Rect::new(0.0, 0.0, 100.0, 50.0)], on_outside: Outside::Close, escape: true };
        let input = InputFrame { outside_press: Some(Point::new(300.0, 10.0)), ..InputFrame::default() };
        assert_eq!(Dismissable::check(&p, &input), Some(Dismiss { reason: Reason::OutsidePress, at: Some(Point::new(300.0, 10.0)) }));
        // inside the anchor (a press on the select that opened it): not a dismiss
        let on_anchor = DismissParams { inside: vec![Rect::new(0.0, 0.0, 100.0, 50.0), Rect::new(250.0, 0.0, 350.0, 20.0)], ..p.clone() };
        assert_eq!(Dismissable::check(&on_anchor, &input), None);
        assert_eq!(Dismissable::check(&DismissParams { on_outside: Outside::Ignore, ..p }, &input), None);
    }
}
