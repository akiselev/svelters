//! Lane WIDGETS-VIEWS' kinds (`plans/cloud/presto-b2/LANE-widgets-views.md`): scrolling, panels and
//! splitters, virtual lists and trees, and the floating surfaces that live in a window's overlay stack
//! (architecture §14): popups, menus, selects, tooltips, toasts, modals and inner windows; plus the
//! status bar and the dialog button row.
//!
//! Each kind implements the crate's kind interface ([`crate::shim::core::Kind`]) over plain logic the
//! tests drive without a tree (latching, row recycling, placement, menu navigation); [`register_all`]
//! puts them on presto-core through [`crate::host`]. [`shim`] holds the stand-ins for APIs other crates
//! do not have yet.

pub mod common;
pub mod dialog_buttons;
pub mod inner_window;
pub mod menu;
pub mod menu_model;
pub mod modal;
pub mod overlay;
pub mod panel;
pub mod placement;
pub mod popup;
pub mod scroll;
pub mod select;
pub mod shim;
pub mod split;
pub mod status_bar;
pub mod toast;
pub mod tooltip;
pub mod tree;
pub mod virtual_list;

use presto_core::{KindRegistry, RegistryError};

/// Registers every view kind with presto-core.
pub fn register_all(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    use crate::host::register;
    register::<scroll::ScrollArea>(reg)?;
    register::<scroll::Scrollbar>(reg)?;
    register::<panel::Panel>(reg)?;
    register::<split::SplitView>(reg)?;
    register::<virtual_list::VirtualList>(reg)?;
    register::<tree::TreeView>(reg)?;
    register::<popup::Popup>(reg)?;
    register::<menu::MenuBar>(reg)?;
    register::<menu::ContextMenu>(reg)?;
    register::<menu::MenuKind>(reg)?;
    register::<menu::MenuItemKind>(reg)?;
    register::<menu::MenuSeparatorKind>(reg)?;
    register::<select::Select>(reg)?;
    register::<tooltip::Tooltip>(reg)?;
    register::<toast::ToastHost>(reg)?;
    register::<modal::Modal>(reg)?;
    register::<inner_window::InnerWindow>(reg)?;
    register::<status_bar::StatusBar>(reg)?;
    register::<dialog_buttons::DialogButtons>(reg)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_view_kind_registers_without_collisions() {
        let mut reg = presto_core::KindRegistry::new();
        super::register_all(&mut reg).expect("view kinds register");
        assert_eq!(reg.len(), 19);
        assert_eq!(reg.by_name("scroll-area").expect("scroll-area").spec.role, presto_base::accesskit::Role::ScrollView);
    }
}
