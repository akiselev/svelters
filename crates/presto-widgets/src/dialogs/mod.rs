//! Dialog support: the platform order of dialog buttons ([`ButtonOrder`], [`order_buttons`]) and the
//! native file and message dialogs ([`native`]).
//!
//! The `dialog-buttons` kind is lane WIDGETS-VIEWS' (it lays its `button` children out); it asks
//! [`order_buttons`] for their layout order. The order follows QDialogButtonBox's layouts (FM1 QW11,
//! svelters-egui `native::ButtonOrder`): help and reset lead on every platform, then the buttons
//! without a role, then the platform's roles. Only layout sees the order: paint, hit testing and Tab
//! order stay the tree's. svelters-egui: `views/dialog_buttons.rs`, `native/mod.rs`.

pub mod native;

use crate::table::shim::Platform;

/// A dialog button's role (`dialog-role`; `auto` is resolved by [`DialogRole::resolve`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DialogRole {
    /// OK, Save, Open: accepts the dialog and closes it with `accept`.
    Accept,
    /// Cancel, Close: rejects the dialog and closes it with `reject`.
    Reject,
    /// Don't Save, Discard.
    Destructive,
    /// Apply: applies without closing.
    Apply,
    /// Any other button that acts without closing.
    Action,
    /// Restore Defaults, Reset.
    Reset,
    Help,
}

impl DialogRole {
    /// The `dialog-role` spelling (`auto` is not a role: resolve it first).
    pub fn of(name: &str) -> Option<DialogRole> {
        Some(match name {
            "accept" => DialogRole::Accept,
            "reject" => DialogRole::Reject,
            "destructive" => DialogRole::Destructive,
            "apply" => DialogRole::Apply,
            "action" => DialogRole::Action,
            "reset" => DialogRole::Reset,
            "help" => DialogRole::Help,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            DialogRole::Accept => "accept",
            DialogRole::Reject => "reject",
            DialogRole::Destructive => "destructive",
            DialogRole::Apply => "apply",
            DialogRole::Action => "action",
            DialogRole::Reset => "reset",
            DialogRole::Help => "help",
        }
    }

    /// A child's role from its `dialog-role` prop: `auto` is `accept` for a `default` button,
    /// `reject` for a `cancel` one and `action` for other buttons; a child that is not a button
    /// and has no explicit role has none.
    pub fn resolve(dialog_role: &str, is_button: bool, default_button: bool, cancel_button: bool) -> Option<DialogRole> {
        match dialog_role {
            "auto" | "" if !is_button => None,
            "auto" | "" if default_button => Some(DialogRole::Accept),
            "auto" | "" if cancel_button => Some(DialogRole::Reject),
            "auto" | "" => Some(DialogRole::Action),
            r => DialogRole::of(r),
        }
    }

    /// Whether a click on a button with this role closes the enclosing `modal` or `inner-window`
    /// (with the role's name as the result).
    pub fn closes(self) -> bool {
        matches!(self, DialogRole::Accept | DialogRole::Reject)
    }
}

/// A platform's order of the trailing dialog roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonOrder {
    /// `accept action reject destructive apply` (QDialogButtonBox WinLayout).
    Windows,
    /// `accept action apply destructive reject` (KdeLayout).
    Kde,
    /// `destructive action apply reject accept` (GnomeLayout).
    Gnome,
    /// `destructive apply action reject accept` (MacLayout).
    MacOs,
}

impl ButtonOrder {
    /// The trailing roles in order (help and reset lead on every platform).
    pub fn roles(self) -> &'static [DialogRole] {
        use DialogRole::*;
        match self {
            ButtonOrder::Windows => &[Accept, Action, Reject, Destructive, Apply],
            ButtonOrder::Kde => &[Accept, Action, Apply, Destructive, Reject],
            ButtonOrder::Gnome => &[Destructive, Action, Apply, Reject, Accept],
            ButtonOrder::MacOs => &[Destructive, Apply, Action, Reject, Accept],
        }
    }

    pub fn of_platform(p: Platform) -> ButtonOrder {
        match p {
            Platform::Windows => ButtonOrder::Windows,
            Platform::MacOs => ButtonOrder::MacOs,
            Platform::Gnome => ButtonOrder::Gnome,
            Platform::Kde => ButtonOrder::Kde,
        }
    }

    /// The `dialog-buttons` `order` prop: `platform` is `platform`'s order, `none` keeps the tree
    /// order (`Ok(None)`), an unknown name is an error.
    pub fn from_prop(name: &str, platform: Platform) -> Result<Option<ButtonOrder>, String> {
        Ok(Some(match name {
            "none" => return Ok(None),
            "platform" => ButtonOrder::of_platform(platform),
            "windows" => ButtonOrder::Windows,
            "kde" => ButtonOrder::Kde,
            "gnome" => ButtonOrder::Gnome,
            "macos" => ButtonOrder::MacOs,
            other => return Err(format!("unknown dialog-buttons order `{other}` (platform, none, windows, kde, gnome, macos)")),
        }))
    }

    /// Where a child with role `role` sorts: help, reset, no role, then the platform's roles.
    fn rank(self, role: Option<DialogRole>) -> usize {
        match role {
            Some(DialogRole::Help) => 0,
            Some(DialogRole::Reset) => 1,
            None => 2,
            Some(r) => 3 + self.roles().iter().position(|x| *x == r).unwrap_or(self.roles().len()),
        }
    }
}

/// The layout order of children with `roles` (tree order in, indices out): stable within a rank,
/// the tree order when `order` is `None`.
pub fn order_buttons(roles: &[Option<DialogRole>], order: Option<ButtonOrder>) -> Vec<usize> {
    let mut v: Vec<usize> = (0..roles.len()).collect();
    if let Some(o) = order {
        v.sort_by_key(|&k| o.rank(roles[k]));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::DialogRole::*;
    use super::*;

    fn names(roles: &[Option<DialogRole>], o: ButtonOrder) -> Vec<&'static str> {
        order_buttons(roles, Some(o)).into_iter().map(|k| roles[k].map(DialogRole::name).unwrap_or("-")).collect()
    }

    #[test]
    fn button_order_per_platform() {
        let roles = [Some(Accept), Some(Reject), Some(Help), Some(Destructive), Some(Apply)];
        assert_eq!(names(&roles, ButtonOrder::Windows), ["help", "accept", "reject", "destructive", "apply"]);
        assert_eq!(names(&roles, ButtonOrder::Kde), ["help", "accept", "apply", "destructive", "reject"]);
        assert_eq!(names(&roles, ButtonOrder::Gnome), ["help", "destructive", "apply", "reject", "accept"]);
        assert_eq!(names(&roles, ButtonOrder::MacOs), ["help", "destructive", "apply", "reject", "accept"]);
    }

    #[test]
    fn unroled_children_sit_after_help_and_reset_and_ties_keep_tree_order() {
        let roles = [Some(Action), None, Some(Reset), Some(Action), Some(Accept)];
        assert_eq!(order_buttons(&roles, Some(ButtonOrder::Gnome)), vec![2, 1, 0, 3, 4]);
        assert_eq!(order_buttons(&roles, None), vec![0, 1, 2, 3, 4], "order none keeps the tree");
    }

    #[test]
    fn auto_roles_and_order_props() {
        assert_eq!(DialogRole::resolve("auto", true, true, false), Some(Accept));
        assert_eq!(DialogRole::resolve("auto", true, false, true), Some(Reject));
        assert_eq!(DialogRole::resolve("auto", true, false, false), Some(Action));
        assert_eq!(DialogRole::resolve("auto", false, false, false), None);
        assert_eq!(DialogRole::resolve("help", false, false, false), Some(Help));
        assert!(Accept.closes() && Reject.closes() && !Apply.closes());
        assert_eq!(ButtonOrder::from_prop("platform", Platform::MacOs), Ok(Some(ButtonOrder::MacOs)));
        assert_eq!(ButtonOrder::from_prop("none", Platform::Windows), Ok(None));
        assert!(ButtonOrder::from_prop("bogus", Platform::Windows).is_err());
    }
}
