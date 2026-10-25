//! The platform policy: one switch per behaviour the WG3 references measured (focus on click, drag
//! thresholds, double-click time, tooltip delays, Space activation, dialog button order, scroll and sash
//! timing). Ported from svelters-egui `native/mod.rs` (FX1, OW0); the text rows (word rules, selection
//! on focus) are TEXT-INPUT's, kept here as plain flags so one table serves every kind.
//!
//! skelter picks the row per OS ([`Native::for_os`]); an app overrides fields (`#[non_exhaustive]`:
//! start from [`Native::policy`] and change fields, the `styleHint` equivalent).

pub use super::press::ActivateOn;

/// The toolkit whose behaviour the built-in kinds follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// GTK 4, reference `conformance/golden/linux/gtk-4.24.1.json`.
    Gtk,
    /// Qt 6 widgets, reference `conformance/golden/linux/qt-6.12.0.json`.
    Qt,
    /// egui 0.37's own behaviour (what svelters-egui's kittest suites assert).
    Egui,
    /// Windows, from the platform guidelines until WG3w's references pin it.
    Windows,
    /// macOS, from the platform guidelines until a WG3m lane pins it.
    Mac,
}

/// The OS family a policy row stands for (`@media (-sv-platform: …)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    Macos,
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

/// OW0 (FM1 QW11; OW7b): a platform's dialog button order (`dialog-buttons`). Help and reset come
/// first on every platform, then the children that are not role buttons, then the roles in
/// [`ButtonOrder::roles`] order.
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
    pub fn roles(self) -> &'static [&'static str] {
        match self {
            ButtonOrder::Windows => &["accept", "action", "reject", "destructive", "apply"],
            ButtonOrder::Kde => &["accept", "action", "apply", "destructive", "reject"],
            ButtonOrder::Gnome => &["destructive", "action", "apply", "reject", "accept"],
            ButtonOrder::MacOs => &["destructive", "apply", "action", "reject", "accept"],
        }
    }

    /// `dialog-buttons` `order` names (`platform` resolves through the policy).
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

/// What a [`Native`] does, one switch per behaviour WG3 measured.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Policy {
    /// Word movement without Shift on a selection collapses it to its end (forward) or start
    /// (backward) without moving (GTK); otherwise it moves from the caret.
    pub word_move_collapses: bool,
    /// Keyboard focus (Tab, Shift+Tab) into a single-line field selects all of it, caret at the end.
    pub select_on_keyboard_focus: bool,
    /// Programmatic focus (the `focus` method) selects all too (GTK's `grab_focus`; Qt's
    /// `setFocus(OtherFocusReason)` does not).
    pub select_on_programmatic_focus: bool,
    /// An unfocused field keeps its selection (GTK); otherwise it collapses to the caret (Qt, egui).
    pub keep_selection_unfocused: bool,
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
    /// OW7b (WIDGET-PARITY §2.1, Qt `autoDefault`): in a dialog scope the focused push button is the
    /// default: Enter activates it and it shows `:default` (Windows, KDE, GTK). Off (macOS), Enter
    /// goes to the scope's `default` button whatever is focused; Space still presses the focused one.
    pub auto_default: bool,
    /// U2 E7: the platform `@media (-sv-platform: macos | windows | linux)` sees; `None`: the
    /// the OS skelter runs on. Overriding it emulates another platform's sheets.
    pub platform: Option<Platform>,
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
    word_move_collapses: true,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: true,
    keep_selection_unfocused: true,
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
    button_order: ButtonOrder::Gnome,
    submenu_delay_ms: 225,
    tooltip_follow_on_ms: 1000,
    auto_default: true,
    alt_focuses_menu_bar: false,
    platform: Some(Platform::Linux),
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
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: false,
    keep_selection_unfocused: false,
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
    button_order: ButtonOrder::Kde,
    submenu_delay_ms: 225,
    tooltip_follow_on_ms: 2000,
    auto_default: true,
    alt_focuses_menu_bar: true,
    platform: Some(Platform::Linux),
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
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: false,
    keep_selection_unfocused: false,
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
    button_order: ButtonOrder::Windows,
    submenu_delay_ms: 400,
    tooltip_follow_on_ms: 2000,
    auto_default: true,
    alt_focuses_menu_bar: true,
    platform: Some(Platform::Windows),
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
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: true,
    keep_selection_unfocused: true,
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
    button_order: ButtonOrder::MacOs,
    submenu_delay_ms: 0,
    tooltip_follow_on_ms: 1000,
    auto_default: false,
    alt_focuses_menu_bar: false,
    platform: Some(Platform::Macos),
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
    word_move_collapses: false,
    select_on_keyboard_focus: false,
    select_on_programmatic_focus: false,
    keep_selection_unfocused: false,
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
    button_order: ButtonOrder::Windows,
    submenu_delay_ms: 200,
    tooltip_follow_on_ms: 1000,
    auto_default: true,
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
    /// The per-OS row: Linux and the BSDs follow GTK, Windows and macOS their guideline rows.
    pub fn for_os(os: Platform) -> Native {
        match os {
            Platform::Linux => Native::Gtk,
            Platform::Windows => Native::Windows,
            Platform::Macos => Native::Mac,
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

impl Default for Policy {
    /// egui's row: what the ported kittest assertions expect.
    fn default() -> Self {
        EGUI
    }
}
