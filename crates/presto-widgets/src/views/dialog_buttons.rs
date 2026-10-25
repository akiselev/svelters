//! `dialog-buttons` (ported from svelters-egui `views/dialog_buttons.rs`; QDialogButtonBox): a row
//! whose `button` children take their place by `dialog-role` in the platform's order
//! ([`ButtonOrder`], or the `order` prop). Help and reset come first on every platform, then the
//! children without a role, then the roles in the platform's order, stable within a role. Only layout
//! sees the order ([`DialogButtons::order_children`]); paint, hit testing and Tab order stay the
//! tree's. A click reports `clicked(role)` (and `help`); an accept or reject closes the enclosing
//! `modal` or `inner-window` with that result ([`DialogButtons::child_clicked`]).

use super::shim::{ButtonOrder, Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal};
use presto_base::accesskit;
use presto_base::change::ChangeClass;

impl ButtonOrder {
    /// The trailing roles in order (help and reset lead on every platform).
    pub fn roles(self) -> &'static [&'static str] {
        match self {
            ButtonOrder::Windows => &["accept", "action", "reject", "destructive", "apply"],
            ButtonOrder::Kde => &["accept", "action", "apply", "destructive", "reject"],
            ButtonOrder::Gnome => &["destructive", "action", "apply", "reject", "accept"],
            ButtonOrder::MacOs => &["destructive", "apply", "action", "reject", "accept"],
        }
    }
    /// The `order` prop's names (`platform` resolves through the policy).
    pub fn of(name: &str) -> Option<ButtonOrder> {
        Some(match name {
            "windows" => ButtonOrder::Windows,
            "kde" => ButtonOrder::Kde,
            "gnome" => ButtonOrder::Gnome,
            "macos" => ButtonOrder::MacOs,
            _ => return None,
        })
    }
}

/// A child of the row as the order sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonChild<'a> {
    pub is_button: bool,
    /// Its `dialog-role` prop (`auto` by default).
    pub dialog_role: &'a str,
    /// A `default` button (Enter).
    pub default_button: bool,
    /// A `cancel` button (Escape).
    pub cancel_button: bool,
}

/// The child's role: its `dialog-role`, `auto` resolved (`accept` for a default button, `reject` for
/// a cancel one, else `action`); `None` for a child that is not a button and has no role.
pub fn role_of(c: &ButtonChild<'_>) -> Option<&'static str> {
    const ROLES: &[&str] = &["accept", "reject", "destructive", "action", "apply", "reset", "help"];
    match c.dialog_role {
        "auto" | "" if !c.is_button => None,
        "auto" | "" if c.default_button => Some("accept"),
        "auto" | "" if c.cancel_button => Some("reject"),
        "auto" | "" => Some("action"),
        r => ROLES.iter().copied().find(|x| *x == r).or(Some("action")),
    }
}

/// The row's order setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrderSetting {
    /// The platform's (`Policy::button_order`).
    #[default]
    Platform,
    /// Tree order.
    None,
    Fixed(ButtonOrder),
}

/// The `dialog-buttons` row.
#[derive(Clone, Debug, Default)]
pub struct DialogButtons {
    pub order: OrderSetting,
}

static DIALOG_BUTTONS_SPEC: KindSpec = KindSpec {
    name: "dialog-buttons",
    props: &[prop("order", PropType::Enum(&["platform", "none", "windows", "kde", "gnome", "macos"]), ChangeClass::Layout)],
    signals: &[signal("clicked", true, false), signal("help", true, false)],
    slots: &[""],
    role: accesskit::Role::Group,
};

impl DialogButtons {
    /// The order this row uses on a platform whose order is `platform`.
    pub fn resolved(&self, platform: ButtonOrder) -> Option<ButtonOrder> {
        match self.order {
            OrderSetting::Platform => Some(platform),
            OrderSetting::None => None,
            OrderSetting::Fixed(o) => Some(o),
        }
    }

    /// The children's layout order (indices into `children`): help and reset first, then the
    /// children without a role, then the roles in the order's sequence, stable within each.
    pub fn order_children(&self, platform: ButtonOrder, children: &[ButtonChild<'_>]) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..children.len()).collect();
        let Some(o) = self.resolved(platform) else { return idx };
        let rank = |i: &usize| -> usize {
            match role_of(&children[*i]) {
                Some("help") => 0,
                Some("reset") => 1,
                None => 2,
                Some(r) => 3 + o.roles().iter().position(|x| *x == r).unwrap_or(o.roles().len()),
            }
        };
        idx.sort_by_key(rank); // stable
        idx
    }

    /// A child button was clicked (its own `clicked`, or Enter/Escape through the dialog scope):
    /// emits `clicked(role)` and `help`, and returns the result an accept or reject closes the
    /// enclosing `modal` or `inner-window` with.
    pub fn child_clicked(&self, cx: &mut EventCx<'_>, child: &ButtonChild<'_>) -> Option<&'static str> {
        let role = role_of(child)?;
        cx.emit("clicked", Value::Str(role.into()));
        if role == "help" {
            cx.emit("help", Value::Null);
        }
        matches!(role, "accept" | "reject").then_some(role)
    }
}

impl Kind for DialogButtons {
    fn spec() -> &'static KindSpec {
        &DIALOG_BUTTONS_SPEC
    }

    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "order" => {
                self.order = match value.as_str().unwrap_or("platform") {
                    "platform" => OrderSetting::Platform,
                    "none" => OrderSetting::None,
                    n => OrderSetting::Fixed(ButtonOrder::of(n).ok_or(PropError::Invalid("order: platform, none, windows, kde, gnome or macos"))?),
                };
                Ok(ChangeClass::Layout)
            }
            _ => Err(PropError::Unknown),
        }
    }

    /// A flex row: no look of its own (the buttons draw themselves).
    fn record(&self, _cx: &mut RecordCx<'_>) {}

    fn event(&mut self, _cx: &mut EventCx<'_>, _ev: &Event) -> Handled {
        Handled::No
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Group);
    }
}

#[cfg(test)]
mod tests {
    use super::super::shim::{Os, Policy};
    use super::*;
    use presto_base::geom::Size;
    use presto_base::time::Time;

    fn b(role: &'static str) -> ButtonChild<'static> {
        ButtonChild { is_button: true, dialog_role: role, default_button: false, cancel_button: false }
    }

    fn roles(order: ButtonOrder, kids: &[ButtonChild<'_>]) -> Vec<&'static str> {
        DialogButtons::default().order_children(order, kids).into_iter().map(|i| role_of(&kids[i]).unwrap_or("-")).collect()
    }

    #[test]
    fn button_order_per_platform() {
        let ok = ButtonChild { default_button: true, ..b("auto") };
        let cancel = ButtonChild { cancel_button: true, ..b("auto") };
        let kids = [ok, cancel, b("apply"), b("help"), b("destructive")];
        assert_eq!(roles(Policy::for_os(Os::Windows).button_order, &kids), ["help", "accept", "reject", "destructive", "apply"]);
        assert_eq!(roles(Policy::for_os(Os::MacOs).button_order, &kids), ["help", "destructive", "apply", "reject", "accept"]);
        assert_eq!(roles(Policy::for_os(Os::Linux).button_order, &kids), ["help", "destructive", "apply", "reject", "accept"]);
        assert_eq!(roles(ButtonOrder::Kde, &kids), ["help", "accept", "apply", "destructive", "reject"]);
        // a child that is not a button keeps its place before the roles
        let label = ButtonChild { is_button: false, ..b("auto") };
        assert_eq!(roles(ButtonOrder::Gnome, &[ok, label, cancel]), ["-", "reject", "accept"]);
        // `order="none"`: the tree's order
        let row = DialogButtons { order: OrderSetting::None };
        assert_eq!(row.order_children(ButtonOrder::MacOs, &kids), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn accept_and_reject_close_with_their_role() {
        let pol = Policy::for_os(Os::Windows);
        let row = DialogButtons::default();
        let mut cx = EventCx::new(Time(0), Size::ZERO, &pol);
        assert_eq!(row.child_clicked(&mut cx, &ButtonChild { default_button: true, ..b("auto") }), Some("accept"));
        assert_eq!(row.child_clicked(&mut cx, &b("help")), None);
        assert_eq!(cx.emitted("clicked").count(), 2);
        assert_eq!(cx.emitted("help").count(), 1);
    }
}
