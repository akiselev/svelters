//! `Expandable` (ARCH-WIDGETS §1.2 catalogue): a host that opens and closes something (a
//! collapsing header's body, later tree rows). An activation (click, Space/Enter, AccessKit
//! `Click`) toggles it; Right/Left open and close it (the tree convention, VS Code and Qt);
//! AccessKit `Expand`/`Collapse` do the same. Owns `expanded` as the prop mirror; sets AccessKit
//! `expanded` and the actions.
//! Writer lane: OW1a2.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::accesskit;

/// What a kind configures on its [`Expandable`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExpandParams {
    /// Open now (the prop).
    pub expanded: bool,
    /// An activation of the host toggles it (off when only part of the host is the header: the
    /// widget toggles on its own clicks then, and the keys still work).
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

    fn sense(&self, _p: &ExpandParams) -> egui::Sense {
        egui::Sense::click()
    }

    fn input(&mut self, p: &ExpandParams, cx: &mut InputCx<'_>, out: &mut Intents<Expand>) {
        let mut v = p.expanded;
        if cx.enabled {
            let keys = cx.focus.has && cx.input.modifiers.is_none();
            let want = if p.on_click && cx.resp.clicked {
                Some(!v)
            } else if cx.a11y_requested(accesskit::Action::Expand) || (keys && cx.input.key_pressed(egui::Key::ArrowRight)) {
                Some(true)
            } else if cx.a11y_requested(accesskit::Action::Collapse) || (keys && cx.input.key_pressed(egui::Key::ArrowLeft)) {
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

    fn claims(&self, _p: &ExpandParams, key: egui::Key, m: egui::Modifiers) -> bool {
        m.is_none() && matches!(key, egui::Key::ArrowLeft | egui::Key::ArrowRight)
    }
}
