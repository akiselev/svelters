//! [`Theme::egui_style`]: the theme as an egui `Style` (DESIGN.md §1.6), so stock egui widgets
//! and egui-native Rust code paint the same tokens as the Svelte cascade.

use super::{Theme, contrast_ratio};
use egui::style::{ScrollStyle, WidgetVisuals};
use egui::{Color32, CornerRadius, CursorIcon, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2};

fn radius(px: f32) -> CornerRadius {
    CornerRadius::same(px.round().clamp(0.0, 255.0) as u8)
}

fn margin(px: f32) -> Margin {
    Margin::same(px.round().clamp(-128.0, 127.0) as i8)
}

impl Theme {
    /// The egui `Style` of this theme: Visuals, Spacing, text sizes and the tooltip delay, after
    /// DESIGN.md §1.6. The families stay the context's (the embedder registers Inter and a
    /// monospace with `fonts.rs`); the sizes are the theme's.
    pub fn egui_style(&self) -> egui::Style {
        let c = self.colors();
        let k = self.components();
        let m = self.metrics();
        let e = self.elevation();
        let mut style = egui::Style::default();
        let mut v = if self.is_dark() { egui::Visuals::dark() } else { egui::Visuals::light() };

        v.dark_mode = self.is_dark();
        v.override_text_color = None;
        v.weak_text_color = Some(c.muted);
        v.panel_fill = k.panel_bg;
        v.window_fill = k.menu_bg;
        v.window_stroke = Stroke::new(m.stroke, k.menu_border_color);
        v.window_corner_radius = radius(m.window_radius);
        v.menu_corner_radius = radius(m.menu_radius);
        v.window_shadow = e.window;
        v.popup_shadow = e.popup;
        v.extreme_bg_color = k.input_bg;
        v.text_edit_bg_color = Some(k.input_bg);
        v.faint_bg_color = c.list_hover;
        v.code_bg_color = c.button2_hover;
        v.hyperlink_color = c.link;
        v.warn_fg_color = c.warning;
        v.error_fg_color = c.error;
        // the selected text (and selected button) colour: whichever of the strong foreground and
        // the content surface reads better on the selection
        let on_sel = if contrast_ratio(c.fg_strong, c.selection) >= contrast_ratio(c.surface, c.selection) { c.fg_strong } else { c.surface };
        v.selection.bg_fill = c.selection;
        v.selection.stroke = Stroke::new(1.0, on_sel);
        v.text_cursor.stroke = Stroke::new(2.0, c.caret);

        let w = |bg: Color32, weak: Color32, border: Color32, fg: Color32| WidgetVisuals {
            bg_fill: bg,
            weak_bg_fill: weak,
            bg_stroke: Stroke::new(m.stroke, border),
            corner_radius: radius(m.button_radius),
            fg_stroke: Stroke::new(1.0, fg),
            // VS Code does not grow widgets on hover
            expansion: 0.0,
        };
        v.widgets.noninteractive = w(c.bg, c.bg, c.border, c.fg);
        v.widgets.inactive = w(k.checkbox_bg, k.button_bg, k.button_border_color, k.button_fg);
        v.widgets.hovered = w(k.button_bg_hover, k.button_bg_hover, k.button_border_color, k.button_fg);
        v.widgets.active = w(k.button_bg_active, k.button_bg_active, k.button_border_color, k.button_fg);
        v.widgets.open = v.widgets.hovered;

        v.interact_cursor = Some(CursorIcon::PointingHand);
        v.disabled_alpha = 0.4;
        v.indent_has_left_vline = true;
        v.button_frame = true;
        v.collapsing_header_frame = false;
        v.slider_trailing_fill = true;
        v.striped = false;
        style.visuals = v;

        let s = &mut style.spacing;
        s.button_padding = Vec2::new(m.button_pad_x, m.button_pad_y);
        s.interact_size = Vec2::new(40.0, m.control_h);
        s.item_spacing = Vec2::new(m.space_2, m.space_1);
        s.indent = m.indent;
        s.icon_width = 18.0;
        s.icon_width_inner = 10.0;
        s.icon_spacing = m.space_2;
        s.menu_margin = margin(m.space_1);
        s.window_margin = margin(m.space_2);
        s.scroll = ScrollStyle { bar_width: m.scrollbar_w, ..ScrollStyle::floating() };
        s.combo_height = 200.0;
        s.tooltip_width = 400.0;
        s.menu_width = 220.0;

        let prop = |px: f32| FontId::new(px, FontFamily::Proportional);
        style.text_styles = [
            (TextStyle::Small, prop(m.font_size_xs)),
            (TextStyle::Body, prop(m.font_size)),
            (TextStyle::Button, prop(m.font_size)),
            (TextStyle::Heading, prop(m.font_size_h3)),
            (TextStyle::Monospace, FontId::new(m.font_size, FontFamily::Monospace)),
        ]
        .into();
        style.interaction.tooltip_delay = 0.35;
        // OW1a3 (cadatomic): app-chrome labels are not selectable (no stray selections, no
        // AccessKit text runs per label); a label opts in (`selectable`, a read-only TextInput)
        style.interaction.selectable_labels = false;
        style
    }

    /// Makes this theme the context's: its Style for its scheme, and that scheme selected. Rust
    /// code without a backend (an egui-only panel, a test) calls it directly;
    /// [`EguiBackend::set_theme`](crate::EguiBackend::set_theme) calls it for its context.
    pub fn apply_to(&self, ctx: &egui::Context) {
        let t = if self.is_dark() { egui::Theme::Dark } else { egui::Theme::Light };
        let mut style = self.egui_style();
        // text styles the app added (`TextStyle::Name`, cadatomic's `caption`) stay
        for (ts, id) in ctx.style_of(t).text_styles.iter() {
            style.text_styles.entry(ts.clone()).or_insert_with(|| id.clone());
        }
        ctx.set_style_of(t, style);
        ctx.set_theme(t);
    }
}
