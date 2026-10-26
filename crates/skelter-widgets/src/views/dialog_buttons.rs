//! `dialog-buttons` (ported from svelters-egui `views/dialog_buttons.rs`; QDialogButtonBox): a row
//! whose `button` children take their place by `dialog-role` in the platform's order
//! ([`ButtonOrder`], or the `order` prop). Help and reset come first on every platform, then the
//! children without a role, then the roles in the platform's order, stable within a role. Only layout
//! sees the order ([`DialogButtons::order_children`]); paint, hit testing and Tab order stay the
//! tree's. A click reports `clicked(role)` (and `help`); an accept or reject closes the enclosing
//! `modal` or `inner-window` with that result ([`DialogButtons::child_clicked`]).

use super::common;
use crate::interact::ButtonOrder;
use crate::kind_props;
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use skelter_base::accesskit;

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

kind_props! {
    /// The row's props.
    pub struct DialogButtonsProps;
    /// The button order: the platform's, the tree's (`none`), or a named platform's.
    order "order": String = Enum("platform") ; Enum(&["platform", "none", "windows", "kde", "gnome", "macos"]), Layout;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("clicked", &[("role", ValueType::Str)], "A child button was clicked; its resolved dialog role."),
    SignalSpec::new("help", &[], "The help button was clicked."),
];

pub static SPEC: KindSpec = KindSpec {
    tag: "dialog-buttons",
    props: DialogButtonsProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Group,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// The `dialog-buttons` row.
#[derive(Clone, Debug, Default)]
pub struct DialogButtons {
    pub props: DialogButtonsProps,
    pub order: OrderSetting,
}

impl OrderSetting {
    /// The `order` prop's value as a setting (an unknown name is the platform's).
    pub fn parse(s: &str) -> OrderSetting {
        match s {
            "none" => OrderSetting::None,
            n => ButtonOrder::of(n).map_or(OrderSetting::Platform, OrderSetting::Fixed),
        }
    }
}

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
        common::emit(cx, &SPEC, "clicked", vec![Value::str(role)]);
        if role == "help" {
            common::emit(cx, &SPEC, "help", Vec::new());
        }
        matches!(role, "accept" | "reject").then_some(role)
    }
}

impl Kind for DialogButtons {
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.props.set(prop, v)?;
        self.order = OrderSetting::parse(&self.props.order);
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    /// A flex row: no look of its own (the buttons draw themselves).
    fn record(&self, _cx: &mut RecordCx) {}

    fn event(&mut self, _cx: &mut EventCx, _ev: &Event) -> Handled {
        Handled::No
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Group);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::Driver;
    use crate::interact::{Native, Platform, StateBits};
    use skelter_base::geom::Size;

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
        assert_eq!(roles(Native::for_os(Platform::Windows).policy().button_order, &kids), ["help", "accept", "reject", "destructive", "apply"]);
        assert_eq!(roles(Native::for_os(Platform::Macos).policy().button_order, &kids), ["help", "destructive", "apply", "reject", "accept"]);
        assert_eq!(roles(Native::for_os(Platform::Linux).policy().button_order, &kids), ["help", "destructive", "apply", "reject", "accept"]);
        assert_eq!(roles(ButtonOrder::Kde, &kids), ["help", "accept", "apply", "destructive", "reject"]);
        // a child that is not a button keeps its place before the roles
        let label = ButtonChild { is_button: false, ..b("auto") };
        assert_eq!(roles(ButtonOrder::Gnome, &[ok, label, cancel]), ["-", "reject", "accept"]);
        // `order="none"`: the tree's order
        let row = DialogButtons { order: OrderSetting::None, ..Default::default() };
        assert_eq!(row.order_children(ButtonOrder::MacOs, &kids), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn accept_and_reject_close_with_their_role() {
        let mut d = Driver::new(DialogButtons::default(), Size::ZERO);
        let policy = d.policy;
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(d.now, Size::ZERO, &mut state, &policy);
        assert_eq!(d.kind.child_clicked(&mut cx, &ButtonChild { default_button: true, ..b("auto") }), Some("accept"));
        assert_eq!(d.kind.child_clicked(&mut cx, &b("help")), None);
        let names: Vec<&str> = cx.emitted.iter().map(|e| SPEC.signals[e.signal.0 as usize].name).collect();
        assert_eq!(names, ["clicked", "clicked", "help"]);
        assert_eq!(cx.emitted[0].args, vec![Value::str("accept")]);
        // the order prop
        d.set("order", Value::str("macos"));
        assert_eq!(d.kind.order, OrderSetting::Fixed(ButtonOrder::MacOs));
        d.set("order", Value::str("none"));
        assert_eq!(d.kind.order, OrderSetting::None);
        assert!(d.kind.set_prop(d.prop_id("order"), &Value::str("beos")).is_err(), "not one of the names");
    }
}
