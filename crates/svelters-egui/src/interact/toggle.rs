//! `Toggleable` (ARCH-WIDGETS §1.2): a two- or three-state value toggled by an activation
//! (pointer click, Space/Enter on the focused host through egui's interaction, AccessKit
//! `Click`). Owns `checked` and `indeterminate` as the prop mirror of the widget's props (§2.1:
//! props are the truth; the unit reports `Toggled` and the widget writes the prop).
//!
//! Semantics (Qt and GTK): a click on an indeterminate box checks it; with `tristate` the user
//! cycles unchecked → checked → indeterminate → unchecked (WIDGET-PARITY §2.2). A `radio` toggle
//! only turns on (a click on a checked radio toggle keeps it checked).
//! Writer lane: OW1a.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::accesskit;

/// What a kind configures on its [`Toggleable`]: the current value (the props) and the rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToggleParams {
    pub checked: bool,
    pub indeterminate: bool,
    /// The user cycles through the indeterminate state too.
    pub tristate: bool,
    /// Activation only turns it on (radio toggle buttons, radio buttons).
    pub radio: bool,
}

/// What [`Toggleable`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    /// The user changed the value: the new `checked` and `indeterminate`.
    Toggled { checked: bool, indeterminate: bool },
}

impl ToggleParams {
    /// The value after one activation.
    pub fn next(&self) -> (bool, bool) {
        match (self.checked, self.indeterminate) {
            (_, true) => (!self.tristate, false),
            (false, false) => (true, false),
            (true, false) if self.radio => (true, false),
            (true, false) if self.tristate => (false, true),
            (true, false) => (false, false),
        }
    }
}

#[derive(Debug, Default)]
pub struct Toggleable;

impl Unit for Toggleable {
    type Params = ToggleParams;
    type Intent = Toggle;
    const OWNS: StateBits = StateBits(StateBits::CHECKED.0 | StateBits::INDETERMINATE.0);

    fn sense(&self, _p: &ToggleParams) -> egui::Sense {
        egui::Sense::click()
    }

    fn input(&mut self, p: &ToggleParams, cx: &mut InputCx<'_>, out: &mut Intents<Toggle>) {
        let mut v = (p.checked, p.indeterminate);
        // `resp.clicked` covers the pointer, Space/Enter on focus and AccessKit `Click` (egui
        // routes the action into the response); a disabled host never toggles
        if cx.enabled && cx.resp.clicked {
            let n = p.next();
            if n != v {
                out.push(Toggle::Toggled { checked: n.0, indeterminate: n.1 });
            }
            v = n;
        }
        // the prop mirror, already at the new value so this pass paints it (§2.3)
        cx.set(StateBits::CHECKED, v.0 && !v.1);
        cx.set(StateBits::INDETERMINATE, v.1);
    }

    fn a11y(&self, p: &ToggleParams, node: &mut accesskit::Node) {
        node.set_toggled(if p.indeterminate {
            accesskit::Toggled::Mixed
        } else if p.checked {
            accesskit::Toggled::True
        } else {
            accesskit::Toggled::False
        });
        node.add_action(accesskit::Action::Click);
    }
}

#[cfg(test)]
mod tests {
    use super::ToggleParams;

    #[test]
    fn ow1a_toggle_cycles() {
        let p = |checked, indeterminate, tristate, radio| ToggleParams { checked, indeterminate, tristate, radio };
        assert_eq!(p(false, false, false, false).next(), (true, false));
        assert_eq!(p(true, false, false, false).next(), (false, false));
        // a click on an indeterminate box checks it (Qt, GTK)
        assert_eq!(p(false, true, false, false).next(), (true, false));
        // tristate: unchecked → checked → partial → unchecked
        assert_eq!(p(true, false, true, false).next(), (false, true));
        assert_eq!(p(false, true, true, false).next(), (false, false));
        // radio only turns on
        assert_eq!(p(true, false, false, true).next(), (true, false));
    }
}
