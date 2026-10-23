//! FX1/OW4: the native per-platform policy (EGUI.md W1/W2/W6; WG3's pins, WG3w, WG3m).
//!
//! The user's decision W1 is native per-platform behaviour for text, selection and keys, with GTK4
//! and Qt both references on Linux (W2). [`Policy`] holds one row per platform ([`Native`]); svelters'
//! own widgets read it through their behaviour units, and the text fields through [`text`]
//! ([`TextPolicy`]: the TC0 rule sets, word stops and keys per platform). Around the remaining egui
//! wrappers:
//!
//! - Escape no longer drops keyboard focus ([`keep_focus_on_escape`]);
//! - after a button or check box ran: a press focuses it ([`after_toggle`]), drawn unfocused until
//!   focus leaves it ([`focus_visible`], GTK's `:focus-visible`).
//!
//! Which toolkit is followed is a per-context setting ([`set`], [`get`]); the default comes from the
//! platform ([`Native::for_os`]): GTK on Linux and the BSDs, the Windows row on Windows, the Mac row
//! on macOS (OW4, from WG3w's and WG3m's references).

// OW4: the text-field rules (TC0 rule sets per platform); FX1's `words.rs` and FX3's `undo.rs`
// moved to quire-text
pub mod text;

use egui::{Event, Id, Key, Ui};
pub use text::{TextPolicy, TextWords, Unfocused};

/// The toolkit whose behaviour the built-in leaves follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// GTK 4 (`GtkText`, `GtkButton`), reference `conformance/golden/linux/gtk-4.24.1.json`.
    Gtk,
    /// Qt 6 widgets (`QLineEdit`, `QPushButton`), reference `conformance/golden/linux/qt-6.12.0.json`.
    Qt,
    /// egui 0.37's own behaviour, unchanged.
    Egui,
    /// OW0 (ARCH-WIDGETS W6): Windows (Ctrl word motions with Qt-like stops, focus on click); its
    /// text fields follow WG3w's references (OW4). The default on Windows.
    Windows,
    /// OW0 (W6): macOS (Option word motions, no focus on click for buttons); its text fields follow
    /// WG3m's AppKit references (OW4). The default on macOS.
    Mac,
}

/// OW0: how a kinetic (momentum) scroll is produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kinetic {
    /// The OS sends momentum phases (macOS, Windows precision touchpads).
    Os,
    /// svelters decays the velocity itself (Linux touchpads without OS momentum).
    Own,
    /// None (mouse wheels).
    None,
}

/// OW0: the platform's scrollbar style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scrollbars {
    /// Thin bars over the content, shown while scrolling or hovered (VS Code, macOS, GTK).
    Overlay,
    /// Bars beside the content, always shown (Qt Fusion, classic Windows).
    Classic,
}

/// OW0: when Space (or Enter) activates a focused button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivateOn {
    Press,
    Release,
}

/// OW0 (FM1 QW11): the order of a dialog's affirmative and dismissive buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonOrder {
    /// OK then Cancel (Windows, KDE).
    OkCancel,
    /// Cancel then OK (GTK, macOS).
    CancelOk,
}

/// What a [`Native`] does, one switch per behaviour WG3 measured. `#[non_exhaustive]`: an app
/// changes a field through [`override_policy`] (WIDGET-PARITY §4.3, the `styleHint` equivalent).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Policy {
    /// OW4: the text fields' rules (undo, words, paste, PRIMARY, keys).
    pub text: TextPolicy,
    /// Word movement without Shift on a selection collapses it to its end (forward) or start
    /// (backward) without moving (GTK); otherwise it moves from the caret.
    pub word_move_collapses: bool,
    /// Keyboard focus (Tab, Shift+Tab) into a single-line field selects all of it, caret at the end.
    pub select_on_keyboard_focus: bool,
    /// Programmatic focus (the `focus` method) selects all too (GTK's `grab_focus`; Qt's
    /// `setFocus(OtherFocusReason)` does not).
    pub select_on_programmatic_focus: bool,
    /// Only one field holds a selection: a field that selects collapses the previous owner's
    /// selection to its start (GTK on X11, the PRIMARY selection changing hands).
    pub one_selection_owner: bool,
    /// Pressing a button or check box gives it keyboard focus.
    pub focus_on_click: bool,
    /// Escape leaves keyboard focus where it is (egui drops it).
    pub escape_keeps_focus: bool,
    // ---- OW0: the behaviour units' fields (ARCH-WIDGETS §1.2) ----
    /// Points the pointer moves before a press becomes a drag (GTK `gtk-dnd-drag-threshold` 8,
    /// Qt `startDragDistance` 10, Windows `SM_CXDRAG` 4; WG3s pin 10 measures).
    pub drag_threshold: f32,
    /// The longest gap between the clicks of a double click.
    pub double_click_ms: u32,
    /// Hover time before a tooltip shows.
    pub tooltip_delay_ms: u32,
    /// A full caret blink cycle; 0 for no blink.
    pub caret_blink_ms: u32,
    /// OW5a1b: a secondary click on an unselected list or tree row selects it before its context
    /// menu opens (Windows, GTK, Qt, VS Code). AppKit outlines the row and keeps the selection.
    pub context_click_selects: bool,
    /// OW5a1b: how long a drag hovers over a collapsed tree row (dropping `into` it) before the
    /// row expands (Qt `autoExpandDelay`, GTK's and AppKit's spring-loading; 0: never).
    pub drag_expand_ms: u32,
    pub kinetic: Kinetic,
    pub scrollbars: Scrollbars,
    /// When Space activates a focused button.
    pub space_activates_on: ActivateOn,
    pub button_order: ButtonOrder,
    /// OW3: hover time on a submenu row before the submenu opens, and on another row before an
    /// open submenu closes (Qt `SH_Menu_SubMenuPopupDelay`, GTK `gtk-menu-popup-delay` 225,
    /// Windows `MenuShowDelay` 400).
    pub submenu_delay_ms: u32,
    /// OW3: after a tooltip hid, how long another one shows at once (Qt's fall-asleep timer, 2 s).
    pub tooltip_follow_on_ms: u32,
    /// OW3: a lone Alt press and release highlights the menu bar's first menu (Windows, KDE);
    /// F10 does it everywhere.
    pub alt_focuses_menu_bar: bool,
    /// U2 E7: the platform `@media (-sv-platform: macos | windows | linux)` sees; `None`: the
    /// context's OS (`egui::Context::os`). Overriding it emulates another platform's sheets.
    pub platform: Option<svelters_style::Platform>,
    /// OW2: scroll gestures and sashes (`interact::Scrollable`, `interact::Resizable`).
    pub scroll: ScrollPolicy,
}

/// OW2 (ARCH-WIDGETS §1.2, §3.3): how scrollables and sashes behave on a platform, beside
/// [`Policy::kinetic`] and [`Policy::scrollbars`]. Sizes and colours are CSS (WIDGET-PARITY P9);
/// this is timing and gesture behaviour only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollPolicy {
    /// What a primary press on the track (beside the thumb) does.
    pub track_click: TrackClick,
    /// Shift+press on the track does the other [`TrackClick`] (GTK).
    pub shift_track_click_inverts: bool,
    /// Points one arrow key (or one scroll-bar arrow step) scrolls.
    pub line: f32,
    /// Overlay bars: how long they stay after the last scroll or hover, then fade out over
    /// [`ScrollPolicy::bar_fade`] seconds (VS Code: 0.5 s, then 0.8 s).
    pub bar_hold: f32,
    pub bar_fade: f32,
    /// R11: hovering a sash shows its hover bar and grip after this many seconds (VS Code 0.3 s).
    pub sash_hover_delay: f32,
}

/// A primary press on the track (Qt `SH_ScrollBar_LeftClickAbsolutePosition`, GTK
/// `gtk-primary-button-warps-slider`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackClick {
    /// Scrolls one page towards the press, repeating while held.
    Page,
    /// Moves the thumb's centre to the press and drags it from there.
    Jump,
}

const GTK: Policy = Policy {
    text: text::GTK,
    word_move_collapses: true,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: true,
    one_selection_owner: true,
    focus_on_click: true,
    escape_keeps_focus: true,
    drag_threshold: 8.0,
    double_click_ms: 400,
    tooltip_delay_ms: 500,
    caret_blink_ms: 1200,
    context_click_selects: true,
    drag_expand_ms: 500,
    kinetic: Kinetic::Own,
    scrollbars: Scrollbars::Overlay,
    space_activates_on: ActivateOn::Release,
    button_order: ButtonOrder::CancelOk,
    submenu_delay_ms: 225,
    tooltip_follow_on_ms: 1000,
    alt_focuses_menu_bar: false,
    platform: Some(svelters_style::Platform::Linux),
    scroll: ScrollPolicy {
        track_click: TrackClick::Jump,
        shift_track_click_inverts: true,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
};

const QT: Policy = Policy {
    text: text::QT,
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: false,
    one_selection_owner: false,
    focus_on_click: true,
    escape_keeps_focus: true,
    drag_threshold: 10.0,
    double_click_ms: 400,
    tooltip_delay_ms: 700,
    caret_blink_ms: 1000,
    context_click_selects: true,
    drag_expand_ms: 500,
    kinetic: Kinetic::None,
    scrollbars: Scrollbars::Classic,
    space_activates_on: ActivateOn::Release,
    button_order: ButtonOrder::OkCancel,
    submenu_delay_ms: 225,
    tooltip_follow_on_ms: 2000,
    alt_focuses_menu_bar: true,
    platform: Some(svelters_style::Platform::Linux),
    scroll: ScrollPolicy {
        track_click: TrackClick::Page,
        shift_track_click_inverts: false,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
};

const WINDOWS: Policy = Policy {
    text: text::WINDOWS,
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: false,
    one_selection_owner: false,
    focus_on_click: true,
    escape_keeps_focus: true,
    drag_threshold: 4.0,
    double_click_ms: 500,
    tooltip_delay_ms: 400,
    caret_blink_ms: 1060,
    context_click_selects: true,
    drag_expand_ms: 500,
    kinetic: Kinetic::Os,
    scrollbars: Scrollbars::Overlay,
    space_activates_on: ActivateOn::Release,
    button_order: ButtonOrder::OkCancel,
    submenu_delay_ms: 400,
    tooltip_follow_on_ms: 2000,
    alt_focuses_menu_bar: true,
    platform: Some(svelters_style::Platform::Windows),
    scroll: ScrollPolicy {
        track_click: TrackClick::Page,
        shift_track_click_inverts: false,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
};

const MAC: Policy = Policy {
    text: text::MAC,
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: true,
    one_selection_owner: false,
    focus_on_click: false,
    escape_keeps_focus: true,
    drag_threshold: 3.0,
    double_click_ms: 500,
    tooltip_delay_ms: 1000,
    caret_blink_ms: 1060,
    context_click_selects: false,
    drag_expand_ms: 500,
    kinetic: Kinetic::Os,
    scrollbars: Scrollbars::Overlay,
    space_activates_on: ActivateOn::Release,
    button_order: ButtonOrder::CancelOk,
    submenu_delay_ms: 0,
    tooltip_follow_on_ms: 1000,
    alt_focuses_menu_bar: false,
    platform: Some(svelters_style::Platform::Macos),
    scroll: ScrollPolicy {
        track_click: TrackClick::Page,
        shift_track_click_inverts: false,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
};

const EGUI: Policy = Policy {
    text: text::egui(egui::os::OperatingSystem::Nix),
    word_move_collapses: false,
    select_on_keyboard_focus: false,
    select_on_programmatic_focus: false,
    one_selection_owner: false,
    focus_on_click: false,
    escape_keeps_focus: false,
    drag_threshold: 6.0,
    double_click_ms: 300,
    tooltip_delay_ms: 500,
    caret_blink_ms: 1000,
    context_click_selects: true,
    drag_expand_ms: 500,
    kinetic: Kinetic::None,
    scrollbars: Scrollbars::Overlay,
    space_activates_on: ActivateOn::Press,
    button_order: ButtonOrder::OkCancel,
    submenu_delay_ms: 200,
    tooltip_follow_on_ms: 1000,
    alt_focuses_menu_bar: false,
    platform: None,
    scroll: ScrollPolicy {
        track_click: TrackClick::Jump,
        shift_track_click_inverts: false,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
};

impl Native {
    /// The per-platform table: Linux (and the BSDs) follow GTK; the others keep egui's behaviour
    /// until WG3 has their references.
    pub fn for_os(os: egui::os::OperatingSystem) -> Native {
        match os {
            egui::os::OperatingSystem::Nix => Native::Gtk,
            // OW4: WG3w and WG3m pinned the Windows and macOS rows (text fields first)
            egui::os::OperatingSystem::Windows => Native::Windows,
            egui::os::OperatingSystem::Mac => Native::Mac,
            _ => Native::Egui,
        }
    }

    pub fn policy(self) -> &'static Policy {
        match self {
            Native::Gtk => &GTK,
            Native::Qt => &QT,
            Native::Egui => &EGUI,
            Native::Windows => &WINDOWS,
            Native::Mac => &MAC,
        }
    }
}

fn setting_id() -> Id {
    Id::unique("svelters-egui/native")
}

pub(crate) fn owner_id() -> Id {
    Id::unique("svelters-egui/native/selection-owner")
}

/// Makes the leaves of `ctx` follow `native` (default: [`Native::for_os`] of `ctx.os()`).
pub fn set(ctx: &egui::Context, native: Native) {
    ctx.data_mut(|d| d.insert_temp(setting_id(), native));
}

/// The toolkit the leaves of `ctx` follow.
pub fn get(ctx: &egui::Context) -> Native {
    ctx.data(|d| d.get_temp(setting_id())).unwrap_or_else(|| Native::for_os(ctx.os()))
}

fn override_id() -> Id {
    Id::unique("svelters-egui/native/policy-override")
}

/// OW0 (WIDGET-PARITY §4.3): changes fields of `ctx`'s policy (the `styleHint` equivalent):
/// `override_policy(ctx, |p| p.tooltip_delay_ms = 400)`. The change applies on top of whichever
/// [`Native`] is set, now and after a later [`set`]. `EmbedConfig::with_policy` calls it.
pub fn override_policy(ctx: &egui::Context, f: fn(&mut Policy)) {
    ctx.data_mut(|d| d.insert_temp(override_id(), PolicyOverride(f)));
}

/// Drops [`override_policy`]'s change.
pub fn clear_policy_override(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<PolicyOverride>(override_id()));
}

#[derive(Clone, Copy)]
struct PolicyOverride(fn(&mut Policy));

/// The policy `ctx`'s widgets follow: its [`Native`]'s row with the app's override applied.
pub fn policy_of(ctx: &egui::Context) -> Policy {
    let native = get(ctx);
    let mut p = *native.policy();
    if native == Native::Egui {
        // egui's own text keys follow the OS (Cmd on macOS, the CUA chords on Windows)
        p.text = text::egui(ctx.os());
    }
    if let Some(PolicyOverride(f)) = ctx.data(|d| d.get_temp::<PolicyOverride>(override_id())) {
        f(&mut p);
    }
    p
}

fn policy(ui: &Ui) -> Policy {
    policy_of(ui.ctx())
}

fn tab_pass_id() -> Id {
    Id::unique("svelters-egui/native/tab-pass")
}

/// Notes a pass whose input holds Tab or Shift+Tab, for [`tab_moved_focus`].
pub(crate) fn note_tab(ui: &Ui) {
    let tab = ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.any() || modifiers.shift_only())));
    if tab {
        let pass = ui.ctx().cumulative_pass_nr();
        ui.data_mut(|d| d.insert_temp(tab_pass_id(), pass));
    }
}

/// Whether focus gained in this pass came from Tab or Shift+Tab: egui moves it in the pass of the
/// key (Tab) or the next one (Shift+Tab). Other focus changes (a restore after a viewport change,
/// assistive technology) do not select.
pub(crate) fn tab_moved_focus(ui: &Ui) -> bool {
    let now = ui.ctx().cumulative_pass_nr();
    ui.data(|d| d.get_temp::<u64>(tab_pass_id())).is_some_and(|p| p <= now && now - p <= 2)
}

fn plain_escape(ui: &Ui) -> bool {
    ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Key { key: Key::Escape, pressed: true, modifiers, .. } if !modifiers.any())))
}

/// When this pass's plain Escape took keyboard focus from widget `id` (egui's `Memory` drops focus
/// on it), gives it back. Called before a `TextEdit` runs (else it collapses its selection for want
/// of focus) and after the other leaves (their id is known only then).
pub(crate) fn keep_focus_on_escape(ui: &Ui, id: Id) {
    if !policy(ui).escape_keeps_focus {
        return;
    }
    if plain_escape(ui) && ui.memory(|m| m.focused().is_none() && m.had_focus_last_frame(id)) {
        ui.memory_mut(|m| m.request_focus(id));
    }
}

fn pointer_focus_id() -> Id {
    Id::unique("svelters-egui/native/pointer-focus")
}

/// Before a button or check box runs in `ui` (its own child `Ui`): GTK's `:focus-visible`. A
/// widget focused by a press is drawn as it would be unfocused (egui draws a focused widget as
/// pressed), until focus leaves it; keyboard focus is drawn as egui draws it.
pub(crate) fn focus_visible(ui: &mut Ui) {
    if !policy(ui).focus_on_click {
        return;
    }
    // the id the button or check box takes (both use `next_auto_id`, egui 0.37)
    let id = ui.next_auto_id();
    let by_pointer = ui.data(|d| d.get_temp::<Id>(pointer_focus_id())) == Some(id);
    if !by_pointer || !ui.memory(|m| m.has_focus(id)) {
        return;
    }
    // egui styles a widget by its response of the previous pass
    let Some(prev) = ui.ctx().read_response(id) else { return };
    if prev.is_pointer_button_down_on() || prev.clicked() {
        return;
    }
    let w = &mut ui.visuals_mut().widgets;
    w.active = if prev.hovered() || prev.highlighted() { w.hovered } else { w.inactive };
}

/// After a button or check box ran: a press focuses it (GTK `focus-on-click`, Qt `StrongFocus`).
pub(crate) fn after_toggle(ui: &Ui, resp: &egui::Response) {
    if !policy(ui).focus_on_click {
        return;
    }
    let focused = ui.memory(|m| m.has_focus(resp.id));
    if resp.is_pointer_button_down_on() {
        if !focused {
            resp.request_focus();
        }
        ui.data_mut(|d| d.insert_temp(pointer_focus_id(), resp.id));
    } else if !focused && ui.data(|d| d.get_temp::<Id>(pointer_focus_id())) == Some(resp.id) {
        ui.data_mut(|d| d.remove::<Id>(pointer_focus_id()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx1_platform_table() {
        use egui::os::OperatingSystem as Os;
        assert_eq!(Native::for_os(Os::Nix), Native::Gtk);
        assert_eq!(Native::for_os(Os::Windows), Native::Windows);
        assert_eq!(Native::for_os(Os::Mac), Native::Mac);
        // where GTK and Qt differ, the switches say so
        assert_ne!(GTK.text.words, QT.text.words);
        assert!(GTK.text.unfocused == Unfocused::Keep && QT.text.unfocused == Unfocused::Collapse);
        assert!(GTK.focus_on_click && QT.focus_on_click && GTK.escape_keeps_focus && QT.escape_keeps_focus);
    }
}
