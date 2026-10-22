//! `button`, svelters' own (OW0, ARCH-WIDGETS §6.1): box, text, icon; Hoverable, Focusable,
//! Pressable; paint parts `icon` and `label` (WIDGET-PARITY §2.1), laid out by CSS (P3). Replaces the `egui::Button` wrapper of `leaf.rs` behind the own-widget switch.
//!
//! Look (CSS, theme `sv.base`): `button`, `:hover`, `:active`, `:selected`, `:focus-visible`
//! (the focus ring is the outline), `:default`, `:disabled`. Where no rule sets a property the UA
//! table [`UA`] does, from the built-in VS Code theme's tokens. Signals as the built-in row:
//! `clicked`, `double-clicked`, `secondary-clicked`. `default`/`cancel` stay EV3's (stored in
//! `Common`; Enter or Escape in the focus scope activate it through `focus::take_activation`).

use crate::interact::{Focusable, Hoverable, Press, PressParams, Pressable};
use crate::paint::{self, Align, IconKey, PaintCx, PartItem, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};

/// The button's UA sheet (ARCH-WIDGETS §1.1), with DESIGN's affordance rules on by default:
/// VS Code's text button metrics (§1.3: padding 4/8, 16px lines, 1px border, radius 4: 26px tall;
/// 12px text as VIS1 measured), the theme's secondary-button roles, and
/// - R1 raised: `button-bg` fill (`button2-bg` with the affordance overlay, never transparent),
///   `button-border-color` boundary (`control-border`), radius 4, the 1px lip (`elev-1`);
/// - R2 pressed sinks: the lip goes, the fill steps to `button-bg-active` (`toolbar-active`), an
///   inset `0 1px 2px rgba(0,0,0,.25)`, at once;
/// - R3 hover: `button-bg-hover` in the same frame, lip kept;
/// - R8 focus ring: keyboard-only (`:focus-visible`), 2px `focus` at offset 1 (standalone control);
/// - R9 disabled: opacity .4, still hoverable (tooltip with the reason), `not-allowed` cursor;
/// - R10 cursor: `pointer`.
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("button-bg")),
        ("color", Token("button-fg")),
        ("border-color", Token("button-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("button-radius")),
        ("box-shadow", Token("elev-1")),
        ("padding-left", Token("button-pad-x")),
        ("padding-right", Token("button-pad-x")),
        ("padding-top", Token("button-pad-y")),
        ("padding-bottom", Token("button-pad-y")),
        ("font-size", Token("font-size-sm")),
        ("line-height", Px(16.0)),
        ("text-align", Keyword("center")),
        ("justify-content", Keyword("center")),
        ("gap", Px(4.0)),
        ("cursor", Keyword("pointer")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(2.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("button-bg-hover"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("button-bg-active"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("button-bg-active")), ("box-shadow", Shadows(paint::PRESSED_INSET))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed")), ("box-shadow", Shadows(&[]))]),
]);

/// A push button.
#[svelters::widget(namespace = "sv", replaces = "button", caps = FOCUSABLE | PAINTS_BOX)]
pub struct Button {
    /// The caption.
    #[prop(measure)]
    text: String,
    /// Drawn as selected (a toggle look); clicking does not change it.
    #[prop]
    selected: bool,
    /// Draws the background and border (otherwise only while hovered, pressed or selected).
    #[prop(default = true)]
    frame: bool,
    /// No vertical padding.
    #[prop(measure)]
    small: bool,
    /// CSS font weight of the caption (a prop the runtime sets beats CSS).
    #[prop(measure, default = 400)]
    font_weight: i64,
    /// An icon before the caption: an icon key (`name` in the `icons` family, or `family:name`).
    /// Its size, place and visibility are CSS on `::part(icon)` (P3).
    #[prop(measure)]
    icon: String,
    /// Held down, it activates again and again (`clicked` after 300 ms, then every 100 ms;
    /// QAbstractButton `autoRepeat`).
    #[prop]
    repeat: bool,
    /// Pressed and released (or Space, Enter, AccessKit `Click`; Enter/Escape as default/cancel).
    #[event]
    clicked: Event<()>,
    #[event]
    double_clicked: Event<()>,
    #[event]
    secondary_clicked: Event<()>,
    /// The primary button went down on it (QAbstractButton `pressed`).
    #[event]
    press: Event<()>,
    /// It came up again, over the button or not (`released`).
    #[event]
    release: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior(params = press_params)]
    pressable: Pressable,
    /// The icon (paint part): `width`/`height`, `margin`, `order`, `display`, `color`,
    /// `content: icon(<key>)`.
    #[part(name = "icon")]
    icon_part: Part,
    /// The caption (paint part): `color`, `font-*`, `margin`, `order`, `display`.
    #[part(name = "label")]
    label_part: Part,
}

/// `::part(icon)`'s UA sheet: a 16px glyph (codicon size).
pub const ICON_UA: Ua = Ua(&[UaRule::base(&[("font-size", Px(16.0))])]);

impl Button {
    fn press_params(&self) -> PressParams {
        PressParams { repeat: self.repeat.then_some((0.3, 0.1)), ..PressParams::default() }
    }

    fn look(&self) -> Look<'_> {
        Look { text: &self.text, icon: &self.icon, frame: self.frame, small: self.small }
    }
}

/// What a button shows, from its props: the kind's and `imm::Button`'s (OW-RUST) input to
/// [`measure`] and [`paint`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Look<'a> {
    pub text: &'a str,
    /// An icon key (`name` in the `icons` family, or `family:name`); empty for none.
    pub icon: &'a str,
    pub frame: bool,
    pub small: bool,
}

impl Look<'_> {
    fn icon_key(&self) -> Option<IconKey> {
        (!self.icon.is_empty()).then(|| match self.icon.split_once(':') {
            Some((f, n)) => IconKey::new(f, n),
            None => IconKey::new("icons", self.icon),
        })
    }

    /// The parts' content sizes: (icon key and size, label galley).
    fn content(&self, ctx: &egui::Context, icon: &StyleView, label: &StyleView) -> (Option<(IconKey, egui::Vec2)>, Option<std::sync::Arc<egui::Galley>>) {
        let key = self.icon_key().and_then(|k| paint::content_icon(icon, &k));
        let icon = key.map(|k| {
            let size = paint::icon_size(ctx, icon, &k);
            (k, size)
        });
        let label = (!self.text.is_empty()).then(|| paint::text_galley(ctx, label, self.text, None));
        (icon, label)
    }
}

/// The button's size under `host` (its style at rest, with the `font-weight` prop) and its parts'
/// views (`::part(icon)` over [`ICON_UA`], `::part(label)`).
pub(crate) fn measure(ctx: &egui::Context, host: &StyleView, icon_v: &StyleView, label_v: &StyleView, look: &Look) -> egui::Vec2 {
    let (icon, label) = look.content(ctx, icon_v, label_v);
    let mut items = Vec::new();
    if let Some((_, size)) = &icon {
        items.push(PartItem { name: "icon", style: icon_v, content: *size });
    }
    if let Some(g) = &label {
        items.push(PartItem { name: "label", style: label_v, content: line_size(label_v, g) });
    }
    let mut size = paint::measure_parts(host, &items);
    if items.is_empty() {
        // an empty button keeps one line's height
        size.y = host.px("line-height").unwrap_or(host.font_size());
    }
    let mut extra = paint::box_extra(host);
    if look.small {
        let [_, _, pt, pb] = host.sides("padding");
        extra.y -= pt + pb;
    }
    size + extra
}

/// Paints the button at `rect` under `host` (its style in this pass's state) and its parts'
/// views: box (when framed), icon and label laid out by CSS (P3), the focus ring last. The kind
/// and `imm::Button` (OW-RUST) call it.
pub(crate) fn paint(pc: &mut PaintCx<'_>, ctx: &egui::Context, host: &StyleView, icon_v: &StyleView, label_v: &StyleView, rect: egui::Rect, look: &Look) {
    let framed = look.frame || host.state().intersects(StateBits(StateBits::HOVERED.0 | StateBits::PRESSED.0 | StateBits::SELECTED.0));
    let geom = if framed { paint::paint_box(pc, host, rect) } else { paint::box_geom(host, rect) };
    let content = if look.small { geom.padding } else { geom.content };
    let (icon, label) = look.content(ctx, icon_v, label_v);
    let mut items = Vec::new();
    if let Some((_, size)) = &icon {
        items.push(PartItem { name: "icon", style: icon_v, content: *size });
    }
    if let Some(g) = &label {
        items.push(PartItem { name: "label", style: label_v, content: line_size(label_v, g) });
    }
    let rects = paint::layout_parts(host, content, &items);
    for (item, r) in items.iter().zip(rects) {
        let Some(r) = r else { continue };
        match (item.name, &icon, &label) {
            ("icon", Some((k, _)), _) => paint::paint_icon(&mut pc.part("icon"), item.style, k, r),
            ("label", _, Some(g)) => paint::paint_text(&mut pc.part("label"), item.style, g, r, Align::Center),
            _ => {}
        }
    }
    paint::paint_outline(pc, host, rect);
}

/// The height of a single label line under `s` (`line-height`, else the galley's).
fn line_size(s: &StyleView, g: &egui::Galley) -> egui::Vec2 {
    egui::vec2(g.size().x, s.px("line-height").filter(|l| *l > 0.0).unwrap_or(g.size().y))
}

impl Widget for Button {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let host = cx.style(UA).with_font_weight(Some(self.font_weight));
        let icon_v = host.part(cx.part_css("icon"), ICON_UA);
        let label_v = host.part(cx.part_css("label"), Ua::NONE);
        measure(cx.ctx, &host, &icon_v, &label_v, &self.look())
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        // the prop mirror (§2.3): set before the units run, so one restyle covers both
        cx.set_state(StateBits::SELECTED, self.selected);
        let default = cx.node().common().default_button;
        cx.set_state(StateBits::DEFAULT, default);
        let resp = cx.interact(ui, self.units_sense());
        let fired = self.run_units(cx, ui, &resp);
        // EV3: Enter or Escape on the scope's default or cancel button
        let mut clicks = (usize::from(crate::focus::take_activation(ui, cx.id())), 0, 0);
        let (mut press, mut release) = (false, false);
        for p in fired.pressable.iter() {
            match p {
                Press::Click { count: 1, button: egui::PointerButton::Primary, .. } | Press::Repeat => clicks.0 += 1,
                Press::Click { count: 2, button: egui::PointerButton::Primary, .. } => clicks.1 += 1,
                Press::Click { button: egui::PointerButton::Secondary, .. } => clicks.2 += 1,
                Press::Down => press = true,
                Press::Up { .. } | Press::Cancel => release = true,
                _ => {}
            }
        }
        // EV1: each is dispatched before a default action (a button has none)
        if press {
            cx.dispatch(&self.press, ());
        }
        if release {
            cx.dispatch(&self.release, ());
        }
        for _ in 0..clicks.0.min(1) {
            cx.dispatch(&self.clicked, ());
        }
        for _ in 0..clicks.1.min(1) {
            cx.dispatch(&self.double_clicked, ());
        }
        for _ in 0..clicks.2.min(1) {
            cx.dispatch(&self.secondary_clicked, ());
        }
        let host = cx.style(UA).with_font_weight(Some(self.font_weight));
        // R10: the cursor tells the verb (R9: `not-allowed` on a disabled button)
        if host.state().contains(StateBits::HOVERED)
            && let Some(c) = host.cursor()
        {
            ui.ctx().set_cursor_icon(c);
        }
        // R9: disabled keeps its shape at `opacity` .4
        if host.opacity() < 1.0 {
            ui.multiply_opacity(host.opacity());
        }
        if ui.is_rect_visible(cx.rect()) {
            let icon_v = cx.part(&host, "icon", ICON_UA);
            let label_v = cx.part(&host, "label", Ua::NONE);
            paint(&mut cx.paint_cx(ui), ui.ctx(), &host, &icon_v, &label_v, cx.rect(), &self.look());
        }
        let (enabled, selected, text) = (cx.enabled(), self.selected, &self.text);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_role(egui::accesskit::Role::Button);
        self.units_a11y(node);
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}
