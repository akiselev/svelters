//! `Expandable` (ARCH-WIDGETS §1.2 catalogue): a host that opens and closes something (a collapsing
//! header's body, tree rows). An activation (click, Space/Enter, AccessKit `Click`) toggles it;
//! Right/Left open and close it (the tree convention, VS Code and Qt); AccessKit `Expand`/`Collapse`
//! do the same. Owns `expanded` as the prop mirror; sets AccessKit `expanded` and the actions.

use super::{InputCx, Intents, Mods, Sense, StateBits, Unit};
use skelter_base::accesskit;
use skelter_base::key::Key;

/// What a kind configures on its [`Expandable`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExpandParams {
    /// Open now (the prop).
    pub expanded: bool,
    /// An activation of the host toggles it (off when only part of the host is the header: the
    /// kind toggles on its own clicks then, and the keys still work).
    pub on_click: bool,
}

/// What [`Expandable`] reports: the user opened (`true`) or closed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expand {
    Set(bool),
}

#[derive(Debug, Default)]
pub struct Expandable;

impl Unit for Expandable {
    type Params = ExpandParams;
    type Intent = Expand;
    const OWNS: StateBits = StateBits::EXPANDED;

    fn sense(&self, _p: &ExpandParams) -> Sense {
        Sense::CLICK
    }

    fn input(&mut self, p: &ExpandParams, cx: &mut InputCx<'_>, out: &mut Intents<Expand>) {
        let mut v = p.expanded;
        if cx.enabled {
            let keys = cx.focus.has && cx.input.modifiers.is_none();
            let want = if p.on_click && cx.resp.clicked {
                Some(!v)
            } else if cx.a11y_requested(accesskit::Action::Expand) || (keys && cx.input.key_pressed(Key::ArrowRight)) {
                Some(true)
            } else if cx.a11y_requested(accesskit::Action::Collapse) || (keys && cx.input.key_pressed(Key::ArrowLeft)) {
                Some(false)
            } else {
                None
            };
            if let Some(n) = want.filter(|n| *n != v) {
                out.push(Expand::Set(n));
                v = n;
            }
        }
        cx.set(StateBits::EXPANDED, v);
    }

    fn a11y(&self, p: &ExpandParams, node: &mut accesskit::Node) {
        node.set_expanded(p.expanded);
        node.add_action(if p.expanded { accesskit::Action::Collapse } else { accesskit::Action::Expand });
        node.add_action(accesskit::Action::Click);
    }

    fn claims(&self, _p: &ExpandParams, key: Key, m: Mods) -> bool {
        m.is_none() && matches!(key, Key::ArrowLeft | Key::ArrowRight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::tracker::testing::Rig;

    #[test]
    fn click_keys_and_accesskit() {
        let mut rig = Rig::new(Sense::CLICK);
        let mut u = Expandable;
        let mut out = Intents::default();
        let closed = ExpandParams { expanded: false, on_click: true };
        let open = ExpandParams { expanded: true, on_click: true };
        rig.down(5.0, 5.0, |cx| u.input(&closed, cx, &mut out));
        rig.up(5.0, 5.0, |cx| u.input(&closed, cx, &mut out));
        assert_eq!(out.0, vec![Expand::Set(true)]);
        assert!(rig.state.contains(StateBits::EXPANDED));
        rig.focus(true, |_| {});
        let mut out = Intents::default();
        rig.key(Key::ArrowLeft, |cx| u.input(&open, cx, &mut out));
        rig.key(Key::ArrowLeft, |cx| u.input(&closed, cx, &mut out));
        assert_eq!(out.0, vec![Expand::Set(false)], "Left closes; already closed: nothing");
        rig.a11y(accesskit::Action::Expand, None, |cx| u.input(&closed, cx, &mut out));
        assert_eq!(out.0.last(), Some(&Expand::Set(true)));
        // on_click off: an activation does nothing, the keys still work
        let mut out = Intents::default();
        let part = ExpandParams { expanded: false, on_click: false };
        rig.key(Key::Enter, |cx| u.input(&part, cx, &mut out));
        assert!(out.is_empty());
        rig.key(Key::ArrowRight, |cx| u.input(&part, cx, &mut out));
        assert_eq!(out.0, vec![Expand::Set(true)]);
        assert!(u.claims(&part, Key::ArrowRight, Mods::NONE));
        let mut node = accesskit::Node::new(accesskit::Role::Button);
        u.a11y(&open, &mut node);
        assert_eq!(node.is_expanded(), Some(true));
        assert!(node.supports_action(accesskit::Action::Collapse) && !node.supports_action(accesskit::Action::Expand));
    }
}
