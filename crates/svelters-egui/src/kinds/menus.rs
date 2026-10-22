//! WG2a menus (plans/milestones/WG2a-menus-popups.md): the X4 vocabulary of the GTK and Qt backends
//! (`menu-bar`, `menu`, `action`, `menu-separator`, `context-menu`) plus `menu-button`. Their
//! entries are appended to [`KINDS`](super::KINDS); painting and behaviour are `crate::menus`.
//!
//! The menu entries (`menu`, `action`, `menu-separator`, `context-menu`) derive from the abstract
//! `MenuEntry`, not `Widget`: they are never laid out by taffy and a box does not accept them.
//! `context-menu` and `popup` attach to any element ([`attaches_to`], spec `attaches_to`).

use super::{Def, KindDef, MethodDef, P, PropDef, SignalDef, Ty, input, prop, signal};

/// The children a menu container accepts (spec slot `accepts`).
const MENU_ITEMS: &[&str] = &["Menu", "Action", "MenuSeparator"];

static ENTRY_PROPS: &[PropDef] = &[
    prop(
        "enabled",
        P::Enabled,
        Ty::Bool,
        Def::Bool(true),
        "Can be picked when true; drawn greyed out otherwise.",
    ),
    prop(
        "visible",
        P::Visible,
        Ty::Bool,
        Def::Bool(true),
        "Shown in its menu when true.",
    ),
    prop(
        "tooltip",
        P::Tooltip,
        Ty::Str,
        Def::Str(""),
        "Hover text; empty for none.",
    ),
    prop(
        "accessible-name",
        P::AccessibleName,
        Ty::Str,
        Def::Str(""),
        "AccessKit name; the label by default.",
    ),
    prop(
        "anchor",
        P::Anchor,
        Ty::Str,
        Def::Str(""),
        "A name unique in its embedded root (AccessKit author id, `Embed::anchor_rect`).",
    ),
];

static MENU_PROPS: &[PropDef] = &[prop(
    "label",
    P::Text,
    Ty::Str,
    Def::Str(""),
    "The title: the menu-bar button, or the submenu row.",
)];

static ACTION_PROPS: &[PropDef] = &[
    prop("label", P::Text, Ty::Str, Def::Str(""), "The caption."),
    prop(
        "shortcut",
        P::Shortcut,
        Ty::Str,
        Def::Str(""),
        "Keyboard accelerator such as `Ctrl+O`, `Ctrl+Shift+S` or `F5`: shown right-aligned and active while the menu's owner (menu-bar, menu-button, context-menu) is drawn. Modifiers: `Ctrl`, `Shift`, `Alt`, `Cmd` (macOS command), `Primary` (Cmd on macOS, Ctrl elsewhere); keys are egui `Key` names. An unknown key is an error.",
    ),
    prop(
        "checkable",
        P::Checkable,
        Ty::Bool,
        Def::Bool(false),
        "A check item: picking it toggles `checked`.",
    ),
    input(
        "checked",
        P::Checked,
        Ty::Bool,
        Def::Bool(false),
        "Check or radio state; picking a check item toggles it, picking a radio item sets it.",
    ),
    prop(
        "group",
        P::Group,
        Ty::Str,
        Def::Str(""),
        "A radio item: the actions of one parent with the same group form an exclusive set; picking one checks it and unchecks the others (each announced).",
    ),
];

static ACTION_SIGNALS: &[SignalDef] = &[
    signal(
        "toggled",
        &[("checked", Ty::Bool)],
        "The user changed `checked` (a check item, or a radio item turned on or off by a pick); carries the new state.",
    ),
    signal(
        "activate",
        &[("checked", Ty::Bool)],
        "The user picked it (click, Enter, or its shortcut); carries `checked` after the pick.",
    ),
];

static MENU_BUTTON_PROPS: &[PropDef] = &[
    prop("label", P::Text, Ty::Str, Def::Str(""), "The caption."),
    input(
        "active",
        P::Active,
        Ty::Bool,
        Def::Bool(false),
        "The menu is open; the user opening or closing it changes it, setting it opens or closes it.",
    ),
];

static MENU_BUTTON_SIGNALS: &[SignalDef] = &[signal(
    "changed",
    &[("active", Ty::Bool)],
    "The user opened or closed the menu; carries `active`.",
)];

static POPUP_METHODS: &[MethodDef] = &[
    MethodDef {
        name: "popup",
        doc: "Opens it (a context menu below its owner).",
    },
    MethodDef {
        name: "popdown",
        doc: "Closes it.",
    },
];

pub(super) const MENU_ENTRY: KindDef = KindDef {
    id: "MenuEntry",
    tag: None,
    native: "egui menu entry",
    base: None,
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: ENTRY_PROPS,
    signals: &[],
    methods: &[],
    doc: "Common properties of the menu entries (`menu`, `action`, `menu-separator`, `context-menu`): drawn inside a menu, never laid out by taffy.",
};

pub(super) const MENU_BAR: KindDef = KindDef {
    id: "MenuBar",
    tag: Some("menu-bar"),
    native: "egui::MenuBar",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("items"),
    text_content: None,
    present: None,
    props: &[],
    signals: &[],
    methods: &[],
    doc: "A horizontal menu bar of `menu` children (and plain `action`s). One interact row high; it takes no width of its own, so it fills a column (align-items stretch) or needs `flex-grow` in a row. The shortcuts of its actions are active while it is drawn.",
};

pub(super) const MENU: KindDef = KindDef {
    id: "Menu",
    tag: Some("menu"),
    native: "egui::containers::menu::{MenuButton, SubMenuButton}",
    base: Some("MenuEntry"),
    toplevel: false,
    slot: Some("items"),
    text_content: Some("label"),
    present: None,
    props: MENU_PROPS,
    signals: &[],
    methods: &[],
    doc: "A menu: a menu-bar button, or a submenu row inside another menu. Inside a `menu-button` its items are the button's menu (its label is unused).",
};

pub(super) const ACTION: KindDef = KindDef {
    id: "Action",
    tag: Some("action"),
    native: "egui::Button (menu item)",
    base: Some("MenuEntry"),
    toplevel: false,
    slot: None,
    text_content: Some("label"),
    present: None,
    props: ACTION_PROPS,
    signals: ACTION_SIGNALS,
    methods: &[],
    doc: "A menu item: plain, a check item (`checkable`) or a radio item (`group`), with an optional `shortcut`. AccessKit role Button, CheckBox or RadioButton.",
};

pub(super) const MENU_SEPARATOR: KindDef = KindDef {
    id: "MenuSeparator",
    tag: Some("menu-separator"),
    native: "egui::Separator",
    base: Some("MenuEntry"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: &[],
    signals: &[],
    methods: &[],
    doc: "A line between menu items (a vertical one in a menu bar).",
};

pub(super) const CONTEXT_MENU: KindDef = KindDef {
    id: "ContextMenu",
    tag: Some("context-menu"),
    native: "egui::Popup (menu)",
    base: Some("MenuEntry"),
    toplevel: false,
    slot: Some("items"),
    text_content: None,
    present: None,
    props: &[],
    signals: &[],
    methods: POPUP_METHODS,
    doc: "A menu that attaches to the element it is declared in and opens at the pointer on a secondary click on it (the innermost owner wins). `popup` opens it below its owner. The shortcuts of its actions are active while the owner is drawn.",
};

pub(super) const MENU_BUTTON: KindDef = KindDef {
    id: "MenuButton",
    tag: Some("menu-button"),
    native: "egui::containers::menu::MenuButton",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("items"),
    text_content: Some("label"),
    present: None,
    props: MENU_BUTTON_PROPS,
    signals: MENU_BUTTON_SIGNALS,
    methods: POPUP_METHODS,
    doc: "A button that opens a menu of its children (`action`, `menu`, `menu-separator`; a `menu` child's items are shown inline, as GTK's menu-button takes a menu). The shortcuts of its actions are active while it is drawn.",
};

/// The kinds a slot of kind `id` accepts (spec `children.slots.*.accepts`): `Widget` unless a
/// family says otherwise.
pub fn slot_accepts(id: &str) -> &'static [&'static str] {
    match id {
        "MenuBar" | "Menu" | "ContextMenu" | "MenuButton" => MENU_ITEMS,
        _ => &["Widget"],
    }
}

/// What kind `id` attaches to when its parent has no slot for it (spec `attaches_to`): it is then
/// inserted into the element itself (`scene/tree.rs` accepts it under any node).
pub fn attaches_to(id: &str) -> Option<&'static str> {
    match id {
        "ContextMenu" | "Popup" => Some("Widget"),
        _ => None,
    }
}
