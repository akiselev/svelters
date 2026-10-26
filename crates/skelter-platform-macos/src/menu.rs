//! The menu bar through muda (`platform.md` §7): `PlatformCx::set_menu_bar` builds it from a
//! [`MenuDesc`] (skelter-widgets' `MenuModel` converts into one) after the standard app menu, or the
//! app installs its own `muda::Menu` with [`set_menu_bar`]. Item activations reach
//! `AppHandler::menu_activated` with the item's id.

use crate::app::{self, Pending};
use muda::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use objc2::MainThreadMarker;
use skelter_platform::{MenuDesc, MenuNode};
use std::cell::RefCell;

thread_local! {
    static DEFAULT_MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
}

/// Routes muda's item events to the handler (muda may report them on any thread).
pub(crate) fn forward_events() {
    muda::MenuEvent::set_event_handler(Some(|e: muda::MenuEvent| {
        let id = e.id;
        dispatch2::DispatchQueue::main().exec_async(move || app::post(Pending::Menu(id)));
    }));
}

/// Makes `menu` the app's menu bar. Main thread.
pub fn set_menu_bar(menu: &Menu) {
    menu.init_for_nsapp();
    DEFAULT_MENU.with(|m| m.borrow_mut().take());
}

fn app_menu() -> muda::Result<Submenu> {
    Submenu::with_items(
        "App",
        true,
        &[
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ],
    )
}

/// The app menu (About, Services, Hide, Quit) and an Edit menu (AppKit adds Emoji & Symbols and
/// Dictation to a menu titled "Edit").
pub(crate) fn install_default(_mtm: MainThreadMarker) {
    let app_menu = app_menu();
    let edit_menu = Submenu::with_items(
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ],
    );
    let (Ok(app_menu), Ok(edit_menu)) = (app_menu, edit_menu) else {
        return;
    };
    let Ok(menu) = Menu::with_items(&[&app_menu, &edit_menu]) else {
        return;
    };
    menu.init_for_nsapp();
    DEFAULT_MENU.with(|m| *m.borrow_mut() = Some(menu));
}

/// Installs the menu bar `desc` describes, after the standard app menu; `None` leaves the app menu
/// alone.
pub(crate) fn install(desc: Option<&MenuDesc>) -> bool {
    let bar = Menu::new();
    let Ok(app) = app_menu() else { return false };
    if bar.append(&app).is_err() {
        return false;
    }
    for node in desc.map(|d| d.menus.as_slice()).unwrap_or_default() {
        if let Some(item) = build(node)
            && bar.append(item.as_ref()).is_err()
        {
            return false;
        }
    }
    bar.init_for_nsapp();
    DEFAULT_MENU.with(|m| *m.borrow_mut() = Some(bar));
    true
}

/// muda reads `&` as a mnemonic marker; captions arrive without markers.
fn caption(label: &str) -> String {
    label.replace('&', "&&")
}

/// An authored accelerator (`Primary+Shift+S`) as muda's; `None` when muda can't parse it.
pub(crate) fn accelerator(shortcut: &str) -> Option<muda::accelerator::Accelerator> {
    if shortcut.trim().is_empty() {
        return None;
    }
    let s: Vec<&str> = shortcut
        .split('+')
        .map(str::trim)
        .map(|p| {
            if p.eq_ignore_ascii_case("primary") {
                "Cmd"
            } else {
                p
            }
        })
        .collect();
    s.join("+").parse().ok()
}

fn build(node: &MenuNode) -> Option<Box<dyn IsMenuItem>> {
    Some(match node {
        MenuNode::Separator => Box::new(PredefinedMenuItem::separator()),
        MenuNode::Item {
            id,
            label,
            shortcut,
            enabled,
            checked,
        } => match checked {
            Some(c) => Box::new(CheckMenuItem::with_id(
                id.as_str(),
                caption(label),
                *enabled,
                *c,
                accelerator(shortcut),
            )),
            None => Box::new(MenuItem::with_id(
                id.as_str(),
                caption(label),
                *enabled,
                accelerator(shortcut),
            )),
        },
        MenuNode::Submenu {
            label,
            enabled,
            items,
        } => {
            let sub = Submenu::new(caption(label), *enabled);
            for child in items {
                if let Some(c) = build(child) {
                    sub.append(c.as_ref()).ok()?;
                }
            }
            Box::new(sub)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerators_from_authored_shortcuts() {
        assert_eq!(accelerator("Primary+Shift+S"), accelerator("Cmd+Shift+S"));
        assert!(accelerator("Primary+O").is_some());
        assert!(accelerator("Ctrl+Alt+F5").is_some());
        assert_eq!(accelerator(""), None);
        assert_eq!(accelerator("Shift+"), None);
        assert_eq!(caption("Save & Close"), "Save && Close");
    }
}
