//! VIS1: the default look, VS Code 1.133 Dark Modern / Light Modern, as measured on the installed
//! VS Code (`conformance/visual/vscode/<theme>/metrics.json`, plans/milestones/VIS1-visual-conformance.md).
//!
//! One token table per scheme ([`DARK`], [`LIGHT`]: the `--sv-*` roles of plans/research/DESIGN.md
//! §1.5, values from VS Code's resolved `--vscode-*` variables) drives both outputs:
//! - [`egui_style`]: egui's `Style` (DESIGN §1.6), which every stock widget and egui-native Rust
//!   code paints with;
//! - [`BASE_CSS`]: the per-kind default sheet (`@layer sv.base`, theme origin, `base.css`) for what
//!   egui's one `Visuals` cannot say per kind (a button's fill is not a checkbox's), reading the same
//!   tokens through `var(--sv-*)`.
//!
//! [`apply`] installs both on a backend and its context; the eframe runner (`run_desktop`,
//! `svl_backend.run`) applies it, an embedder (cadatomic) calls it or sets its own style. DS1's
//! `svelters_egui::theme` (one token map, typed getters, density, HC sets) supersedes this module and
//! takes these values; until then this is the VS Code base the visual conformance gate checks.
//!
//! Writer lane: VIS1 (then DS1).

use crate::backend::EguiBackend;
use egui::{Color32, CornerRadius, Margin, Shadow, Stroke, Theme, vec2};
use svelters_style::ColorScheme;

/// Dark Modern (`Default Dark Modern`, VS Code 1.133.0).
pub const DARK: &[(&str, &str)] = &[
    // surfaces and lines
    ("--sv-bg", "#181818"),             // sideBar.background
    ("--sv-surface", "#1F1F1F"),        // editor.background
    ("--sv-surface-raised", "#202020"), // editorWidget.background
    ("--sv-border", "#2B2B2B"),         // sideBar.border, panel.border
    ("--sv-border-widget", "#313131"),  // widget.border, notificationToast.border
    ("--sv-menu-border", "#454545"),    // menu.border
    ("--sv-shadow-color", "#0000005C"), // widget.shadow
    // text
    ("--sv-fg", "#CCCCCC"),          // foreground
    ("--sv-fg-strong", "#FFFFFF"),   // settings.headerForeground
    ("--sv-muted", "#9D9D9D"),       // descriptionForeground
    ("--sv-disabled-fg", "#CCCCCC80"),
    ("--sv-placeholder", "#989898"), // input.placeholderForeground
    ("--sv-link", "#4DAAFC"),
    // accent
    ("--sv-accent", "#0078D4"),       // button.background
    ("--sv-accent-hover", "#026EC1"), // button.hoverBackground
    ("--sv-accent-fg", "#FFFFFF"),
    ("--sv-button-border", "#FFFFFF1A"), // button.border
    // L3 component tokens the base sheet reads (the runtime's DEFAULT_TOKENS also set --sv-accent
    // and --sv-fg on egui until DS1, so the sheet does not read those names)
    ("--sv-button-bg", "#0078D4"),
    ("--sv-button-hover", "#026EC1"),
    ("--sv-button-fg", "#FFFFFF"),
    ("--sv-checkbox-fg", "#CCCCCC"),
    ("--sv-focus", "#0078D4"),           // focusBorder
    // controls
    ("--sv-button2-bg", "#00000000"),     // button.secondaryBackground
    ("--sv-button2-hover", "#2B2B2B"),    // button.secondaryHoverBackground
    ("--sv-button2-fg", "#CCCCCC"),       // button.secondaryForeground
    ("--sv-button2-border", "#CCCCCC26"), // button.secondaryBorder
    ("--sv-control-bg", "#313131"),       // input.background, dropdown.background
    ("--sv-control-border", "#3C3C3C"),   // input.border, dropdown.border, checkbox.border
    ("--sv-control-fg", "#CCCCCC"),       // input.foreground
    ("--sv-checkbox-bg", "#313131"),      // checkbox.background
    ("--sv-toggle-hover", "#5A5D5E80"),   // inputOption.hoverBackground
    ("--sv-toggle-on-bg", "#2489DB82"),   // inputOption.activeBackground
    ("--sv-toggle-on-border", "#2488DB"), // inputOption.activeBorder
    ("--sv-toggle-on-fg", "#FFFFFF"),     // inputOption.activeForeground
    ("--sv-toolbar-hover", "#5A5D5E50"),  // toolbar.hoverBackground
    ("--sv-invalid-border", "#BE1100"),   // inputValidation.errorBorder
    // lists, tabs, menus, scrollbars
    ("--sv-list-hover", "#2A2D2E"),
    ("--sv-list-sel", "#04395E"),
    ("--sv-list-sel-fg", "#FFFFFF"),
    ("--sv-list-sel-inactive", "#37373D"),
    ("--sv-list-focus-outline", "#0078D4"),
    ("--sv-selection", "#264F78"), // editor.selectionBackground
    ("--sv-caret", "#AEAFAD"),
    ("--sv-tab-bg", "#181818"),
    ("--sv-tab-active-bg", "#1F1F1F"),
    ("--sv-tab-fg", "#9D9D9D"),
    ("--sv-tab-active-fg", "#FFFFFF"),
    ("--sv-tab-active-marker", "#0078D4"),
    ("--sv-menu-bg", "#1F1F1F"),
    ("--sv-menu-sel", "#2A2D2E"), // the item under the pointer (menu-item-hover crop)
    ("--sv-menu-separator", "#454545"),
    ("--sv-hover-bg", "#202020"),     // editorHoverWidget.background
    ("--sv-hover-border", "#CCCCCC33"),
    ("--sv-notification-bg", "#1F1F1F"),
    ("--sv-scrollbar", "#79797966"),
    ("--sv-scrollbar-hover", "#646464B3"),
    ("--sv-scrollbar-active", "#BFBFBF66"),
    ("--sv-progress", "#0078D4"),
    ("--sv-sash-hover", "#0078D4"),
    ("--sv-danger", "#F85149"),
    ("--sv-error", "#F14C4C"),
    ("--sv-warning", "#CCA700"),
    ("--sv-info", "#59A4F9"),
];

/// Light Modern (`Default Light Modern`, VS Code 1.133.0).
pub const LIGHT: &[(&str, &str)] = &[
    ("--sv-bg", "#F8F8F8"),
    ("--sv-surface", "#FFFFFF"),
    ("--sv-surface-raised", "#F8F8F8"),
    ("--sv-border", "#E5E5E5"),
    ("--sv-border-widget", "#E5E5E5"),
    ("--sv-menu-border", "#CECECE"),
    ("--sv-shadow-color", "#00000029"),
    ("--sv-fg", "#3B3B3B"),
    ("--sv-fg-strong", "#1F1F1F"),
    ("--sv-muted", "#616161"),
    ("--sv-disabled-fg", "#61616180"),
    ("--sv-placeholder", "#767676"),
    ("--sv-link", "#005FB8"),
    ("--sv-accent", "#005FB8"),
    ("--sv-accent-hover", "#0258A8"),
    ("--sv-accent-fg", "#FFFFFF"),
    ("--sv-button-border", "#0000001A"),
    ("--sv-button-bg", "#005FB8"),
    ("--sv-button-hover", "#0258A8"),
    ("--sv-button-fg", "#FFFFFF"),
    ("--sv-checkbox-fg", "#3B3B3B"),
    ("--sv-focus", "#005FB8"),
    ("--sv-button2-bg", "#E5E5E5"),
    ("--sv-button2-hover", "#CCCCCC"),
    ("--sv-button2-fg", "#3B3B3B"),
    ("--sv-button2-border", "#3B3B3B26"),
    ("--sv-control-bg", "#FFFFFF"),
    ("--sv-control-border", "#CECECE"),
    ("--sv-control-fg", "#3B3B3B"),
    ("--sv-checkbox-bg", "#F8F8F8"),
    ("--sv-toggle-hover", "#B8B8B850"),
    ("--sv-toggle-on-bg", "#BED6ED"),
    ("--sv-toggle-on-border", "#005FB8"),
    ("--sv-toggle-on-fg", "#000000"),
    ("--sv-toolbar-hover", "#B8B8B850"),
    ("--sv-invalid-border", "#BE1100"),
    ("--sv-list-hover", "#F2F2F2"),
    ("--sv-list-sel", "#E8E8E8"),
    ("--sv-list-sel-fg", "#000000"),
    ("--sv-list-sel-inactive", "#E4E6F1"),
    ("--sv-list-focus-outline", "#005FB8"),
    ("--sv-selection", "#ADD6FF"),
    ("--sv-caret", "#000000"),
    ("--sv-tab-bg", "#F8F8F8"),
    ("--sv-tab-active-bg", "#FFFFFF"),
    ("--sv-tab-fg", "#868686"),
    ("--sv-tab-active-fg", "#3B3B3B"),
    ("--sv-tab-active-marker", "#005FB8"),
    ("--sv-menu-bg", "#FFFFFF"),
    ("--sv-menu-sel", "#F2F2F2"),
    ("--sv-menu-separator", "#3B3B3B33"),
    ("--sv-hover-bg", "#F8F8F8"),
    ("--sv-hover-border", "#3B3B3B33"),
    ("--sv-notification-bg", "#FFFFFF"),
    ("--sv-scrollbar", "#64646466"),
    ("--sv-scrollbar-hover", "#646464B3"),
    ("--sv-scrollbar-active", "#00000099"),
    ("--sv-progress", "#005FB8"),
    ("--sv-sash-hover", "#005FB8"),
    ("--sv-danger", "#F85149"),
    ("--sv-error", "#E51400"),
    ("--sv-warning", "#BF8803"),
    ("--sv-info", "#0063D3"),
];

/// The per-kind default sheet (theme origin, `@layer sv.base`): author `<style>` and cadatomic's
/// `@layer cad` sheets beat it.
pub const BASE_CSS: &str = include_str!("vscode_look/base.css");

/// Metrics (`normal` density, DESIGN §1.3, measured): control height, button padding, radii, fonts.
pub mod metrics {
    /// Text buttons, inputs, selects: 26 px (`padding: 4px 8px; line-height: 16px; border: 1px`).
    pub const CONTROL_H: f32 = 26.0;
    pub const BUTTON_PAD: [f32; 2] = [8.0, 4.0];
    /// `cornerRadius.small` (buttons, inputs, selects, toasts).
    pub const RADIUS: u8 = 4;
    /// Checkbox box 18x18 (radius 3).
    pub const CHECKBOX: f32 = 18.0;
    /// Menus and the select's drop-down: `cornerRadius.large`.
    pub const RADIUS_MENU: u8 = 8;
    /// Dialogs: `cornerRadius.xLarge`.
    pub const RADIUS_DIALOG: u8 = 12;
    /// UI font sizes: body 13, buttons 12 (`.monaco-text-button`), small 11.
    pub const FONT: f32 = 13.0;
    pub const FONT_BUTTON: f32 = 12.0;
    pub const FONT_SMALL: f32 = 11.0;
    /// List and tree rows.
    pub const ROW_H: f32 = 22.0;
    /// Scrollbars of lists (VS Code: 10 px in lists, 14 px in editors).
    pub const SCROLLBAR_W: f32 = 10.0;
}

/// The token table of `scheme`.
pub fn tokens(scheme: ColorScheme) -> &'static [(&'static str, &'static str)] {
    match scheme {
        ColorScheme::Dark => DARK,
        ColorScheme::Light => LIGHT,
    }
}

/// `#RRGGBB` or `#RRGGBBAA` (the token tables' form).
pub fn hex(s: &str) -> Option<Color32> {
    let h = s.strip_prefix('#')?;
    let b = |k: usize| u8::from_str_radix(h.get(k..k + 2)?, 16).ok();
    match h.len() {
        6 => Some(Color32::from_rgb(b(0)?, b(2)?, b(4)?)),
        8 => Some(Color32::from_rgba_unmultiplied(b(0)?, b(2)?, b(4)?, b(6)?)),
        _ => None,
    }
}

/// Token `name` of `scheme` as a colour (transparent if missing: the tables are tested complete).
pub fn color(scheme: ColorScheme, name: &str) -> Color32 {
    tokens(scheme).iter().find(|(k, _)| *k == name).and_then(|(_, v)| hex(v)).unwrap_or(Color32::TRANSPARENT)
}

/// egui's `Style` for `scheme` (DESIGN §1.6 with the measured values).
pub fn egui_style(scheme: ColorScheme) -> egui::Style {
    use egui::{FontFamily, FontId, TextStyle};
    let c = |n: &str| color(scheme, n);
    let dark = scheme == ColorScheme::Dark;
    let mut s = egui::Style::default();
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };

    let radius = CornerRadius::same(metrics::RADIUS);
    let w = &mut v.widgets;
    // noninteractive: labels, separators, frames
    w.noninteractive.bg_fill = c("--sv-bg");
    w.noninteractive.weak_bg_fill = c("--sv-bg");
    w.noninteractive.bg_stroke = Stroke::new(1.0, c("--sv-border"));
    w.noninteractive.fg_stroke = Stroke::new(1.0, c("--sv-fg"));
    w.noninteractive.corner_radius = radius;
    w.noninteractive.expansion = 0.0;
    // buttons fill with weak_bg_fill (the secondary button), checkboxes, radios, switches and
    // slider rails with bg_fill (the field fill)
    w.inactive.weak_bg_fill = c("--sv-button2-bg");
    w.inactive.bg_fill = c("--sv-checkbox-bg");
    w.inactive.bg_stroke = Stroke::new(1.0, c("--sv-control-border"));
    w.inactive.fg_stroke = Stroke::new(1.0, c("--sv-fg"));
    w.inactive.corner_radius = radius;
    w.inactive.expansion = 0.0;
    // VS Code only changes a button's fill under the pointer; fields keep theirs
    w.hovered = w.inactive;
    w.hovered.weak_bg_fill = c("--sv-button2-hover");
    // pressed: VS Code has no :active look; the pointer is over the button, so it shows hover
    w.active = w.hovered;
    w.open = w.hovered;

    v.selection.bg_fill = c("--sv-list-sel");
    v.selection.stroke = Stroke::new(1.0, c("--sv-list-sel-fg"));
    v.hyperlink_color = c("--sv-link");
    v.faint_bg_color = c("--sv-list-hover");
    v.extreme_bg_color = c("--sv-control-bg");
    v.text_edit_bg_color = Some(c("--sv-control-bg"));
    v.code_bg_color = c("--sv-button2-hover");
    v.warn_fg_color = c("--sv-warning");
    v.error_fg_color = c("--sv-invalid-border");
    v.weak_text_color = Some(c("--sv-muted"));
    v.panel_fill = c("--sv-bg");
    v.window_fill = c("--sv-surface-raised");
    v.window_stroke = Stroke::new(1.0, c("--sv-border-widget"));
    v.window_corner_radius = CornerRadius::same(metrics::RADIUS_DIALOG);
    v.menu_corner_radius = CornerRadius::same(metrics::RADIUS_MENU);
    // dialogs: 0 0 20px rgba(0,0,0,.15); menus, hovers, drop-downs: 0 0 12px rgba(0,0,0,.14)
    v.window_shadow = Shadow { offset: [0, 0], blur: 20, spread: 0, color: Color32::from_black_alpha(38) };
    v.popup_shadow = Shadow { offset: [0, 0], blur: 12, spread: 0, color: Color32::from_black_alpha(36) };
    v.text_cursor.stroke = Stroke::new(1.0, c("--sv-caret"));
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.disabled_alpha = 0.4;
    v.collapsing_header_frame = false;
    v.striped = false;
    v.slider_trailing_fill = true;
    v.indent_has_left_vline = true;
    s.visuals = v;

    let sp = &mut s.spacing;
    sp.button_padding = vec2(metrics::BUTTON_PAD[0], metrics::BUTTON_PAD[1]);
    sp.interact_size = vec2(40.0, metrics::CONTROL_H);
    sp.item_spacing = vec2(8.0, 4.0);
    sp.indent = 8.0;
    sp.icon_width = metrics::CHECKBOX;
    sp.icon_width_inner = 10.0;
    sp.icon_spacing = 8.0;
    sp.menu_margin = Margin::same(4);
    sp.window_margin = Margin::same(8);
    sp.combo_height = 200.0;
    sp.tooltip_width = 400.0;
    sp.menu_width = 220.0;
    sp.scroll = egui::style::ScrollStyle::floating();
    sp.scroll.bar_width = metrics::SCROLLBAR_W;
    sp.scroll.floating_width = metrics::SCROLLBAR_W;
    sp.scroll.foreground_color = false;
    sp.scroll.dormant_background_opacity = 0.0;

    s.text_styles = [
        (TextStyle::Body, FontId::new(metrics::FONT, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(metrics::FONT_BUTTON, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(metrics::FONT_SMALL, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(metrics::FONT, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(metrics::FONT, FontFamily::Monospace)),
    ]
    .into();
    s.interaction.tooltip_delay = 0.35;
    s
}

/// Installs the look: egui's style for both themes on `ctx`, the token tables (`:root` per
/// `prefers-color-scheme`) and the base sheet on `backend`.
pub fn apply(backend: &EguiBackend, ctx: &egui::Context) {
    ctx.set_style_of(Theme::Dark, egui_style(ColorScheme::Dark));
    ctx.set_style_of(Theme::Light, egui_style(ColorScheme::Light));
    apply_css(backend);
}

/// The CSS half of [`apply`] (tokens and base sheet), for an embedder that sets egui's style itself.
/// The sheets are the backend's (names `@tokens:sv-look:*`, theme origin): they survive the
/// runtime's "clear every sheet" when a bundle starts, and do not replace the app's `set_tokens`.
pub fn apply_css(backend: &EguiBackend) {
    for (scheme, name) in [(ColorScheme::Dark, "@tokens:sv-look:dark"), (ColorScheme::Light, "@tokens:sv-look:light")] {
        let css = crate::style::tokens_css(Some(scheme), tokens(scheme)).expect("the token tables are valid CSS");
        backend.add_sheet(name, &css);
    }
    backend.add_sheet("@tokens:sv-look:base", BASE_CSS);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_name_the_same_tokens_and_parse() {
        assert_eq!(DARK.len(), LIGHT.len());
        for ((a, va), (b, vb)) in DARK.iter().zip(LIGHT) {
            assert_eq!(a, b, "same order in both tables");
            assert!(hex(va).is_some() && hex(vb).is_some(), "{a}: {va} / {vb}");
        }
    }

    #[test]
    fn base_sheet_parses_cleanly() {
        let b = EguiBackend::new();
        let mut d = b.add_sheet("@tokens:sv-look:base", BASE_CSS);
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            d.extend(b.add_sheet("@tokens:sv-look:t", &crate::style::tokens_css(Some(scheme), tokens(scheme)).unwrap()));
        }
        assert!(d.is_empty(), "{d:#?}");
    }

    #[test]
    fn base_sheet_reads_only_known_tokens() {
        let mut rest = BASE_CSS;
        while let Some(k) = rest.find("var(--sv-") {
            let name: String = rest[k + 4..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            assert!(DARK.iter().any(|(n, _)| *n == name), "base.css reads {name}, which no table defines");
            rest = &rest[k + 4..];
        }
    }
}
