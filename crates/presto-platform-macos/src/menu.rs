//! The menu bar through muda (`platform.md` §7): the app builds a `muda::Menu` and installs it with
//! [`set_menu_bar`]; item activations reach [`MacAppHandler::menu`](crate::MacAppHandler::menu).

use crate::app::{self, Pending};
use muda::{Menu, PredefinedMenuItem, Submenu};
use objc2::MainThreadMarker;
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

/// The app menu (About, Services, Hide, Quit) and an Edit menu (AppKit adds Emoji & Symbols and
/// Dictation to a menu titled "Edit").
pub(crate) fn install_default(_mtm: MainThreadMarker) {
    let app_menu = Submenu::with_items(
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
    );
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
