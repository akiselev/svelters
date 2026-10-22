//! The kind → role table (E2E-INSPECTOR §1.3, TK2's table) and a node's accessible states.
//!
//! Roles are WAI-ARIA names in lower case, matching the AccessKit role egui gives the widget, so
//! `role=button[name="Apply"]` reads the same in tests, the inspector and a screen reader. Provider
//! kinds (`W::Ext`) and containers without a semantic role answer `generic`; a provider kind may
//! later name its role through `Widget::automation()` (RA1).

use svelters_egui::scene::{NodeRef, StateBits, W};

/// The role of `n` (lower-case ARIA name).
pub fn role_of(n: NodeRef<'_>) -> &'static str {
    match n.builtin() {
        W::Window { .. } => "window",
        W::Label { .. } => "label",
        W::Button { .. } => "button",
        W::Checkbox { .. } => "checkbox",
        W::TextEdit { .. } => "textbox",
        W::Slider { .. } | W::RangeSlider { .. } => "slider",
        W::DragValue { .. } => "spinbutton",
        W::ToggleButton { radio: true, .. } | W::RadioButton { .. } => "radio",
        W::ToggleButton { .. } => "button",
        W::ComboBox { .. } => "combobox",
        W::Separator { .. } => "separator",
        W::Icon { .. } => "img",
        W::Grid { .. } => "grid",
        W::CollapsingHeader { .. } => "button",
        W::Menu(_) => "menu",
        W::Hyperlink { .. } => "link",
        W::Switch { .. } => "switch",
        W::Image { .. } => "img",
        W::ProgressBar { .. } => "progressbar",
        W::Spinner { .. } => "progressbar",
        W::Canvas(_) => "canvas",
        W::Table(_) => "table",
        W::TableColumn { .. } => "columnheader",
        W::TabView { .. } => "tablist",
        W::Tab { .. } => "tab",
        W::VirtualList(_) => "list",
        W::Frame { .. } => "group",
        W::ToastHost(_) => "alert",
        // OW0: svelters' own widgets replace the built-in payloads behind the same tags
        W::Ext(_) => role_of_tag(n.tag()),
        _ => "generic",
    }
}

/// The role of a built-in tag (an own widget, `crate::widgets` in svelters-egui, is matched by the
/// tag it replaces).
fn role_of_tag(tag: &str) -> &'static str {
    match tag {
        "window" => "window",
        "label" => "label",
        "button" | "toggle-button" | "collapsing-header" => "button",
        "checkbox" => "checkbox",
        "text-edit" => "textbox",
        "slider" | "range-slider" => "slider",
        "drag-value" => "spinbutton",
        "radio-button" => "radio",
        "combo-box" => "combobox",
        "separator" => "separator",
        "icon" | "image" => "img",
        "grid" => "grid",
        "scroll-area" => "scrollarea",
        // OW2: own only
        "scrollbar" => "scrollbar",
        "menu-bar" | "menu" | "menu-button" | "action" => "menu",
        "hyperlink" => "link",
        "switch" => "switch",
        "progress-bar" | "spinner" => "progressbar",
        "canvas" => "canvas",
        "table" => "table",
        "table-column" => "columnheader",
        "tab-view" => "tablist",
        "tab" => "tab",
        "virtual-list" => "list",
        "frame" => "group",
        "toast-host" => "alert",
        _ => "generic",
    }
}

/// `n`'s states by name: the scene's state bits (`StateBits::names`) plus the ones a stock widget
/// holds as a prop (checked, selected, expanded, disabled).
pub fn states_of(n: NodeRef<'_>) -> Vec<&'static str> {
    let mut bits = n.state();
    match n.builtin() {
        W::Checkbox { checked: true, .. }
        | W::ToggleButton { checked: true, .. }
        | W::RadioButton { checked: true, .. }
        | W::Switch { active: true, .. } => bits = StateBits(bits.0 | StateBits::CHECKED.0),
        W::Button { selected: true, .. } | W::Icon { selected: true, .. } => {
            bits = StateBits(bits.0 | StateBits::SELECTED.0)
        }
        W::CollapsingHeader { open: true, .. } => bits = StateBits(bits.0 | StateBits::EXPANDED.0),
        _ => {}
    }
    if !n.common().enabled {
        bits = StateBits(bits.0 | StateBits::DISABLED.0);
    }
    bits.names()
}

/// Whether `n` has state `name` (`focus` and `focused` are the same, as are `disabled`/`enabled`
/// negated).
pub fn has_state(n: NodeRef<'_>, name: &str) -> bool {
    let states = states_of(n);
    match name {
        "focus" => states.contains(&"focused"),
        "enabled" => !states.contains(&"disabled"),
        "visible" => n.is_visible(),
        _ => states.contains(&name),
    }
}
