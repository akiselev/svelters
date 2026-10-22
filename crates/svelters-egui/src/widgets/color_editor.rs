//! `color-editor`, a new kind (OW6; WIDGET-PARITY §2.20: the colour dialog's body, inline in a
//! property panel), and the body `color-picker` shows in its popup ([`ColorBody`]).
//!
//! Layout, top to bottom: the saturation/value square (`::part(square)`), the hue strip
//! (`::part(hue)`), the alpha strip over a checkerboard (`::part(alpha)`, unless `alpha` is
//! `opaque`), each with a `::part(handle)` (`:dragging`, `:focus-visible`); the fields
//! (`::part(fields)`: the hex field and, per `fields`, R/G/B and H/S/V channel boxes); the palette
//! (`::part(swatches)` of `::part(swatch-cell)`s, from `swatches`) and the user's
//! `custom-colors` with an add cell.
//!
//! HSV is in sRGB (what hex and the fields show); the editor keeps its HSV while the user edits,
//! so the hue of a grey survives. Each of square, hue, alpha and the channel boxes is a tab stop
//! (EV3, a `Slider`/`SpinButton` AccessKit node with SetValue, Increment, Decrement); arrows step
//! them (Shift ×10). The hex field is an AccessKit text field (SetValue) until OW1b's entry, then
//! OW4's text input, takes typing (milestone handoff). Writer lane: OW6.

use crate::paint::{self, Align, StyleView, TraceKind, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, StateBits, Widget, WidgetCx};
use egui::{Color32, Key, Modifiers, Rect, Stroke, Vec2, accesskit, pos2, vec2};

/// The host: no box inline; the gap between rows.
pub const UA: Ua = Ua(&[UaRule::base(&[("color", Token("fg")), ("font-size", Token("font-size")), ("gap", Px(6.0))])]);

/// `::part(square)`: the saturation/value area.
pub const SQUARE_UA: Ua = Ua(&[UaRule::base(&[("width", Px(200.0)), ("height", Px(140.0)), ("border-radius", Token("input-radius")), ("border-color", Token("control-border")), ("border-width", Token("stroke")), ("cursor", Keyword("crosshair"))])]);

/// `::part(hue)` and `::part(alpha)`: the strips.
pub const STRIP_UA: Ua = Ua(&[UaRule::base(&[("height", Px(12.0)), ("border-radius", Token("input-radius")), ("border-color", Token("control-border")), ("border-width", Token("stroke")), ("cursor", Keyword("pointer"))])]);

/// `::part(handle)`: a ring on the square, a bar on the strips; R8 ring while focus-visible.
pub const HANDLE_UA: Ua = Ua(&[
    UaRule::base(&[
        ("width", Px(10.0)),
        ("border-color", Keyword("white")),
        ("border-width", Px(2.0)),
        ("box-shadow", Token("elev-1")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(2.0)),
        ("outline-offset", Px(1.0)),
    ]),
    UaRule::when(StateBits::DRAGGING, &[("width", Px(12.0))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

/// `::part(field)`: the hex field and the channel boxes, sunken fields (DESIGN R4, R8 for fields).
pub const FIELD_UA: Ua = Ua(&[
    UaRule::base(&[
        ("height", Token("control-h")),
        ("background-color", Token("control-bg")),
        ("color", Token("control-fg")),
        ("border-color", Token("control-border")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("input-radius")),
        ("padding-left", Px(4.0)),
        ("padding-right", Px(4.0)),
        ("cursor", Keyword("ew-resize")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::DRAGGING, &[("border-color", Token("focus"))]),
    UaRule::when(StateBits::INVALID, &[("border-color", Token("invalid-border"))]),
]);

/// `::part(swatch-cell)`: a palette cell (raised: R1's boundary; hover lights the border).
pub const CELL_UA: Ua = Ua(&[
    UaRule::base(&[("width", Px(16.0)), ("height", Px(16.0)), ("border-radius", Px(3.0)), ("border-color", Token("control-border")), ("border-width", Token("stroke")), ("cursor", Keyword("pointer")), ("gap", Px(4.0))]),
    UaRule::when(StateBits::HOVERED, &[("border-color", Token("focus"))]),
    UaRule::when(StateBits::SELECTED, &[("border-color", Token("fg")), ("border-width", Px(2.0))]),
]);

/// `fields` tokens.
pub const FIELDS: &[&str] = &["hex", "rgb", "hsv"];
/// `commit-mode` values of the colour kinds.
pub const COLOR_COMMIT_MODE: &[&str] = &["live", "confirm"];

/// A colour in sRGB HSV (each 0..1) with unmultiplied alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
    pub a: f32,
}

impl Hsv {
    pub fn from_srgba([r, g, b, a]: [u8; 4]) -> Hsv {
        let (h, s, v) = egui::ecolor::hsv_from_rgb([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]);
        Hsv { h, s, v, a: a as f32 / 255.0 }
    }

    pub fn to_srgba(self) -> [u8; 4] {
        let [r, g, b] = egui::ecolor::rgb_from_hsv((self.h.rem_euclid(1.0), self.s.clamp(0.0, 1.0), self.v.clamp(0.0, 1.0)));
        let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        [q(r), q(g), q(b), q(self.a)]
    }

    pub fn color(self) -> Color32 {
        let [r, g, b, a] = self.to_srgba();
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    fn opaque(self) -> Color32 {
        Hsv { a: 1.0, ..self }.color()
    }
}

/// Parses a CSS hex colour into unmultiplied sRGBA.
pub fn parse_color(s: &str) -> Result<[u8; 4], String> {
    Color32::from_hex(s.trim()).map(|c| c.to_srgba_unmultiplied()).map_err(|e| format!("`{s}` is not a hex colour (#rrggbb, #rrggbbaa, #rgb, #rgba): {e:?}"))
}

/// Unmultiplied sRGBA as CSS hex: `#rrggbb` when opaque, else `#rrggbbaa`.
pub fn color_hex([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
}

/// A colour prop: CSS hex, stored normalized (`#rrggbb`, `#rrggbbaa`); anything else is refused
/// on write (as the egui wrapper's `color-picker` did).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HexColor(pub String);

impl Default for HexColor {
    fn default() -> Self {
        HexColor("#000000".into())
    }
}

impl HexColor {
    pub fn rgba(&self) -> [u8; 4] {
        parse_color(&self.0).unwrap_or([0, 0, 0, 255])
    }
}

impl svelters_host::exports::JsType for HexColor {
    fn describe(b: &mut svelters_host::exports::__private::TypeBuilder) -> svelters_host::exports::__private::TypeRef {
        <String as svelters_host::exports::JsType>::describe(b)
    }
}

impl svelters_host::exports::FromJs for HexColor {
    fn from_js(v: svelters_host::exports::Value, cx: &svelters_host::exports::CallCx<'_>) -> Result<Self, svelters_host::exports::NativeError> {
        let s = <Option<String> as svelters_host::exports::FromJs>::from_js(v, cx)?.unwrap_or_else(|| "#000000".into());
        parse_color(&s).map(|c| HexColor(color_hex(c))).map_err(super::invalid)
    }
}

impl svelters_host::exports::IntoJs for HexColor {
    fn into_js(self, cx: &svelters_host::exports::CallCx<'_>) -> Result<svelters_host::exports::Value, svelters_host::exports::NativeError> {
        self.0.into_js(cx)
    }
}

/// The `alpha` prop: one of [`crate::kinds::pickers::ALPHA`]; anything else is refused on write
/// (as the egui wrapper did).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlphaMode(pub &'static str);

impl Default for AlphaMode {
    fn default() -> Self {
        AlphaMode("opaque")
    }
}

impl AlphaMode {
    /// The editor shows the alpha strip.
    pub fn shown(&self) -> bool {
        self.0 != "opaque"
    }
}

impl svelters_host::exports::JsType for AlphaMode {
    fn describe(b: &mut svelters_host::exports::__private::TypeBuilder) -> svelters_host::exports::__private::TypeRef {
        <String as svelters_host::exports::JsType>::describe(b)
    }
}

impl svelters_host::exports::FromJs for AlphaMode {
    fn from_js(v: svelters_host::exports::Value, cx: &svelters_host::exports::CallCx<'_>) -> Result<Self, svelters_host::exports::NativeError> {
        let all = crate::kinds::pickers::ALPHA;
        let s = <Option<String> as svelters_host::exports::FromJs>::from_js(v, cx)?.unwrap_or_else(|| "opaque".into());
        all.iter().find(|a| **a == s).map(|a| AlphaMode(a)).ok_or_else(|| super::invalid(format!("`{s}` is not one of {}", all.join(", "))))
    }
}

impl svelters_host::exports::IntoJs for AlphaMode {
    fn into_js(self, cx: &svelters_host::exports::CallCx<'_>) -> Result<svelters_host::exports::Value, svelters_host::exports::NativeError> {
        self.0.to_string().into_js(cx)
    }
}

/// What the body is configured with (the props `color-editor` and `color-picker` share).
#[derive(Clone, Debug)]
pub struct ColorCfg<'a> {
    pub alpha: bool,
    pub hex: bool,
    pub rgb: bool,
    pub hsv: bool,
    pub swatches: &'a [String],
    pub custom: &'a [String],
}

impl ColorCfg<'_> {
    pub fn fields_of(v: &str) -> (bool, bool, bool) {
        let has = |t: &str| v.split([' ', ',']).any(|x| x == t);
        (has("hex"), has("rgb"), has("hsv"))
    }
}

/// What one pass of the body did.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorOut {
    /// The colour changed (a drag frame, a step, a field, a swatch).
    Input(Hsv),
    /// A commit point: a drag released, a key step, a field set, a swatch picked.
    Commit,
    /// The add cell: append the current colour to `custom-colors`.
    AddCustom,
}

/// The focusable sub-controls (tab stops) of the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ctl {
    Square,
    Hue,
    Alpha,
    /// R, G, B, H, S, V.
    Chan(u8),
}

const CHANNELS: [(&str, &str, f32); 6] = [("R", "Red", 255.0), ("G", "Green", 255.0), ("B", "Blue", 255.0), ("H", "Hue", 360.0), ("S", "Saturation", 100.0), ("V", "Value", 100.0)];

/// The body's state between frames.
#[derive(Debug, Default)]
pub struct ColorBody {
    /// The colour being edited; none: from the value.
    hsv: Option<Hsv>,
    /// The value `hsv` was last synced with (a host write resets it).
    synced: Option<[u8; 4]>,
    /// The sub-control being dragged.
    dragging: Option<Ctl>,
    /// A channel drag's start: (pointer x, channel value).
    scrub_from: Option<(f32, f32)>,
}

/// One row of the layout.
struct Geom {
    square: Rect,
    hue: Rect,
    alpha: Option<Rect>,
    hex: Option<Rect>,
    chans: Vec<(u8, Rect)>,
    cells: Vec<Rect>,
    custom_cells: Vec<Rect>,
    add: Rect,
}

impl ColorBody {
    /// The colour shown: the edit, else the value.
    pub fn hsv(&mut self, value: [u8; 4]) -> Hsv {
        if self.synced != Some(value) {
            // a host write (or the first frame): the value wins, keeping a grey's hue
            let mut h = Hsv::from_srgba(value);
            if let Some(old) = self.hsv
                && (h.s == 0.0 || h.v == 0.0)
            {
                h.h = old.h;
                if h.v == 0.0 {
                    h.s = old.s;
                }
            }
            self.hsv = Some(h);
            self.synced = Some(value);
        }
        self.hsv.unwrap_or_else(|| Hsv::from_srgba(value))
    }

    /// Records that the owner wrote `rgba` as the new value (from this body's edit).
    pub fn edited(&mut self, h: Hsv) {
        self.hsv = Some(h);
        self.synced = Some(h.to_srgba());
    }

    fn sizes(host: &StyleView<'_>) -> (Vec2, f32, f32, f32, Vec2) {
        let (st, dark) = (host.state(), host.is_dark());
        let sq = StyleView::ua_only(SQUARE_UA, st, dark);
        let square = vec2(sq.px("width").unwrap_or(200.0), sq.px("height").unwrap_or(140.0));
        let strip = StyleView::ua_only(STRIP_UA, st, dark).px("height").unwrap_or(12.0);
        let field = StyleView::ua_only(FIELD_UA, st, dark).px("height").unwrap_or(22.0);
        let cv = StyleView::ua_only(CELL_UA, st, dark);
        let cell = vec2(cv.px("width").unwrap_or(16.0), cv.px("height").unwrap_or(16.0));
        (square, strip, field, cv.px("gap").unwrap_or(4.0), cell)
    }

    fn cells_per_row(w: f32, cell: Vec2, gap: f32) -> usize {
        (((w + gap) / (cell.x + gap)).floor() as usize).max(1)
    }

    fn geom(host: &StyleView<'_>, cfg: &ColorCfg<'_>, r: Rect) -> Geom {
        let (square, strip, field, cgap, cell) = ColorBody::sizes(host);
        let gap = super::parts::gap(host, 6.0);
        let w = r.width().max(square.x);
        let mut y = r.top();
        let mut take = |h: f32| {
            let rr = Rect::from_min_size(pos2(r.left(), y), vec2(w, h));
            y += h + gap;
            rr
        };
        let square = take(square.y);
        let hue = take(strip);
        let alpha = cfg.alpha.then(|| take(strip));
        let mut hex = None;
        let mut chans = Vec::new();
        let rows: Vec<Vec<u8>> = [cfg.rgb.then(|| vec![0u8, 1, 2]), cfg.hsv.then(|| vec![3u8, 4, 5])].into_iter().flatten().collect();
        if cfg.hex || !rows.is_empty() {
            let row = take(field);
            let hex_w = if cfg.hex { (w * 0.36).max(64.0) } else { 0.0 };
            if cfg.hex {
                hex = Some(Rect::from_min_size(row.min, vec2(if rows.is_empty() { w } else { hex_w }, field)));
            }
            for (k, row_chans) in rows.iter().enumerate() {
                let (x0, rr) = if k == 0 { (row.left() + if cfg.hex { hex_w + cgap } else { 0.0 }, row) } else { (r.left() + if cfg.hex { hex_w + cgap } else { 0.0 }, take(field)) };
                let cw = (row.right() - x0 - 2.0 * cgap) / 3.0;
                for (i, c) in row_chans.iter().enumerate() {
                    chans.push((*c, Rect::from_min_size(pos2(x0 + i as f32 * (cw + cgap), rr.top()), vec2(cw, field))));
                }
            }
        }
        let per = ColorBody::cells_per_row(w, cell, cgap);
        let grid = |n: usize, y0: f32| -> Vec<Rect> { (0..n).map(|k| Rect::from_min_size(pos2(r.left() + (k % per) as f32 * (cell.x + cgap), y0 + (k / per) as f32 * (cell.y + cgap)), cell)).collect() };
        let cells = grid(cfg.swatches.len(), y);
        let rows_of = |n: usize| n.div_ceil(per) as f32;
        let y2 = if cfg.swatches.is_empty() { y } else { y + rows_of(cfg.swatches.len()) * (cell.y + cgap) - cgap + gap };
        let mut custom_cells = grid(cfg.custom.len() + 1, y2);
        let add = custom_cells.pop().unwrap_or(Rect::NOTHING);
        Geom { square, hue, alpha, hex, chans, cells, custom_cells, add }
    }

    /// The size the body asks for (content box).
    pub fn measure(host: &StyleView<'_>, cfg: &ColorCfg<'_>) -> Vec2 {
        let big = Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 10_000.0));
        let g = ColorBody::geom(host, cfg, big);
        let bottom = g.add.bottom();
        vec2(g.square.width(), bottom)
    }

    /// The keys a focused sub-control takes.
    pub fn claims(key: Key, _m: Modifiers) -> bool {
        matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown)
    }

    fn channel(h: Hsv, c: u8) -> f32 {
        let [r, g, b, _] = h.to_srgba();
        match c {
            0 => r as f32,
            1 => g as f32,
            2 => b as f32,
            3 => (h.h * 360.0).round(),
            4 => (h.s * 100.0).round(),
            _ => (h.v * 100.0).round(),
        }
    }

    fn with_channel(h: Hsv, c: u8, v: f32) -> Hsv {
        let v = v.clamp(0.0, CHANNELS[c as usize].2);
        match c {
            0..=2 => {
                let mut rgba = h.to_srgba();
                rgba[c as usize] = v.round() as u8;
                let mut n = Hsv::from_srgba(rgba);
                if n.s == 0.0 {
                    n.h = h.h;
                }
                n
            }
            3 => Hsv { h: (v / 360.0).min(0.9999), ..h },
            4 => Hsv { s: v / 100.0, ..h },
            _ => Hsv { v: v / 100.0, ..h },
        }
    }

    /// Runs and paints the body in `r`; `id` salts its sub-controls, `a11y_parent` holds their
    /// nodes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>, ua: Ua, cfg: &ColorCfg<'_>, value: [u8; 4], r: Rect, a11y_parent: egui::Id) -> Vec<ColorOut> {
        let mut out = Vec::new();
        // the tab stops are recorded after painting (`host` borrows `cx` until then)
        let mut stops: Vec<egui::Response> = Vec::new();
        let host_v = cx.style(ua);
        let host = &host_v;
        let enabled = cx.enabled() && ui.is_enabled();
        let g = ColorBody::geom(host, cfg, r);
        let mut h = self.hsv(value);
        let before = h;
        let id = a11y_parent.with("color-editor");
        let dark = host.is_dark();
        let mut commit = false;
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(r).accessibility_parent(a11y_parent));
        let shift = ui.input(|i| i.modifiers.shift);
        let mul = if shift { 10.0 } else { 1.0 };

        // ---- square, hue, alpha: drag, keys, AccessKit ----
        let mut ctls = vec![(Ctl::Square, g.square, "Saturation and value"), (Ctl::Hue, g.hue, "Hue")];
        if let Some(a) = g.alpha {
            ctls.push((Ctl::Alpha, a, "Alpha"));
        }
        for (ctl, rect, label) in ctls.iter().copied() {
            let cid = id.with(label);
            let resp = ui.interact(rect, cid, if enabled { egui::Sense::click_and_drag() | egui::Sense::focusable_noninteractive() } else { egui::Sense::hover() });
            stops.push(resp.clone());
            if enabled && (resp.drag_started() || resp.is_pointer_button_down_on()) {
                self.dragging = Some(ctl);
            }
            if self.dragging == Some(ctl)
                && let Some(p) = ui.input(|i| i.pointer.interact_pos())
            {
                let fx = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                let fy = ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                match ctl {
                    Ctl::Square => {
                        h.s = fx;
                        h.v = 1.0 - fy;
                    }
                    Ctl::Hue => h.h = fx.min(0.9999),
                    Ctl::Alpha => h.a = fx,
                    Ctl::Chan(_) => {}
                }
                if !ui.input(|i| i.pointer.any_down()) {
                    self.dragging = None;
                    commit = true;
                }
            }
            if resp.has_focus() && enabled {
                let step = 0.01 * mul;
                ui.input(|i| {
                    for e in &i.events {
                        let egui::Event::Key { key, pressed: true, .. } = e else { continue };
                        let (dx, dy) = match key {
                            Key::ArrowLeft => (-step, 0.0),
                            Key::ArrowRight => (step, 0.0),
                            Key::ArrowUp => (0.0, step),
                            Key::ArrowDown => (0.0, -step),
                            Key::PageUp => (0.1, 0.1),
                            Key::PageDown => (-0.1, -0.1),
                            Key::Home => (-1.0, 0.0),
                            Key::End => (1.0, 0.0),
                            _ => continue,
                        };
                        match ctl {
                            Ctl::Square => {
                                h.s = (h.s + dx).clamp(0.0, 1.0);
                                h.v = (h.v + dy).clamp(0.0, 1.0);
                            }
                            Ctl::Hue => h.h = (h.h + dx + dy).clamp(0.0, 0.9999),
                            Ctl::Alpha => h.a = (h.a + dx + dy).clamp(0.0, 1.0),
                            Ctl::Chan(_) => {}
                        }
                        commit = true;
                    }
                });
            }
            // AccessKit: SetValue / Increment / Decrement on the slider (the square's value is its saturation)
            {
                for (action, data) in ak_requests(ui, cid) {
                    let d = match action {
                        accesskit::Action::Increment => 0.01,
                        accesskit::Action::Decrement => -0.01,
                        accesskit::Action::SetValue => {
                            if let Some(accesskit::ActionData::NumericValue(v)) = data {
                                let v = (v as f32 / 100.0).clamp(0.0, 1.0);
                                match ctl {
                                    Ctl::Square => h.s = v,
                                    Ctl::Hue => h.h = v.min(0.9999),
                                    Ctl::Alpha => h.a = v,
                                    Ctl::Chan(_) => {}
                                }
                                commit = true;
                            }
                            continue;
                        }
                        _ => continue,
                    };
                    match ctl {
                        Ctl::Square => h.s = (h.s + d).clamp(0.0, 1.0),
                        Ctl::Hue => h.h = (h.h + d).clamp(0.0, 0.9999),
                        Ctl::Alpha => h.a = (h.a + d).clamp(0.0, 1.0),
                        Ctl::Chan(_) => {}
                    }
                    commit = true;
                }
            }
            let pct = match ctl {
                Ctl::Square => h.s,
                Ctl::Hue => h.h,
                _ => h.a,
            } as f64
                * 100.0;
            let text = match ctl {
                Ctl::Square => format!("saturation {}%, value {}%", (h.s * 100.0).round(), (h.v * 100.0).round()),
                Ctl::Hue => format!("{}°", (h.h * 360.0).round()),
                _ => format!("{}%", (h.a * 100.0).round()),
            };
            ui.ctx().accesskit_node_builder(cid, |n| {
                n.set_role(accesskit::Role::Slider);
                n.set_label(label);
                n.set_numeric_value(pct.round());
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(100.0);
                n.set_numeric_value_step(1.0);
                n.set_value(text.clone());
                if enabled {
                    n.add_action(accesskit::Action::SetValue);
                    n.add_action(accesskit::Action::Increment);
                    n.add_action(accesskit::Action::Decrement);
                } else {
                    n.set_disabled();
                }
            });
            if resp.hovered() && enabled {
                let part = if ctl == Ctl::Square { SQUARE_UA } else { STRIP_UA };
                if let Some(c) = StyleView::ua_only(part, StateBits::NONE, dark).cursor() {
                    ui.ctx().set_cursor_icon(c);
                }
            }
            let mut hb = StateBits::NONE;
            hb.set(StateBits::DRAGGING, self.dragging == Some(ctl));
            hb.set(StateBits::FOCUS_VISIBLE, resp.has_focus() && !ui.input(|i| i.pointer.any_down()) && self.dragging.is_none());
            self.paint_ctl(ui, cx, host, ctl, rect, h, hb);
        }

        // ---- channel fields (scrub, keys, AccessKit SpinButton) ----
        for (c, rect) in g.chans.iter().copied() {
            let (short, label, max) = CHANNELS[c as usize];
            let cid = id.with(("chan", c));
            let resp = ui.interact(rect, cid, if enabled { egui::Sense::click_and_drag() | egui::Sense::focusable_noninteractive() } else { egui::Sense::hover() });
            stops.push(resp.clone());
            let cur = ColorBody::channel(h, c);
            if enabled && resp.drag_started() {
                self.dragging = Some(Ctl::Chan(c));
                self.scrub_from = resp.interact_pointer_pos().map(|p| (p.x, cur));
            }
            if self.dragging == Some(Ctl::Chan(c)) {
                if let (Some((x0, v0)), Some(p)) = (self.scrub_from, ui.input(|i| i.pointer.interact_pos())) {
                    h = ColorBody::with_channel(h, c, (v0 + ((p.x - x0) / 2.0).round() * mul).clamp(0.0, max));
                }
                if !ui.input(|i| i.pointer.any_down()) {
                    self.dragging = None;
                    self.scrub_from = None;
                    commit = true;
                }
            }
            let mut set = None;
            if resp.has_focus() && enabled {
                ui.input(|i| {
                    for e in &i.events {
                        let egui::Event::Key { key, pressed: true, .. } = e else { continue };
                        let v = ColorBody::channel(h, c);
                        set = match key {
                            Key::ArrowUp | Key::ArrowRight => Some(v + mul),
                            Key::ArrowDown | Key::ArrowLeft => Some(v - mul),
                            Key::PageUp => Some(v + 10.0),
                            Key::PageDown => Some(v - 10.0),
                            Key::Home => Some(0.0),
                            Key::End => Some(max),
                            _ => set,
                        };
                    }
                });
            }
            {
                for (action, data) in ak_requests(ui, cid) {
                    let v = ColorBody::channel(h, c);
                    set = match (action, &data) {
                        (accesskit::Action::Increment, _) => Some(v + 1.0),
                        (accesskit::Action::Decrement, _) => Some(v - 1.0),
                        (accesskit::Action::SetValue, Some(accesskit::ActionData::NumericValue(n))) => Some(*n as f32),
                        (accesskit::Action::SetValue, Some(accesskit::ActionData::Value(s))) => s.trim().parse::<f32>().ok(),
                        _ => set,
                    };
                }
            }
            if let Some(v) = set {
                h = ColorBody::with_channel(h, c, v);
                commit = true;
            }
            let shown = ColorBody::channel(h, c);
            ui.ctx().accesskit_node_builder(cid, |n| {
                n.set_role(accesskit::Role::SpinButton);
                n.set_label(label);
                n.set_numeric_value(shown as f64);
                n.set_min_numeric_value(0.0);
                n.set_max_numeric_value(max as f64);
                n.set_numeric_value_step(1.0);
                if enabled {
                    n.add_action(accesskit::Action::SetValue);
                    n.add_action(accesskit::Action::Increment);
                    n.add_action(accesskit::Action::Decrement);
                } else {
                    n.set_disabled();
                }
            });
            let mut fb = StateBits::NONE;
            fb.set(StateBits::DRAGGING, self.dragging == Some(Ctl::Chan(c)));
            fb.set(StateBits::FOCUS_VISIBLE, resp.has_focus() && self.dragging.is_none());
            fb.set(StateBits::DISABLED, !enabled);
            let fv = cx.part(host, "field", FIELD_UA).in_state(fb);
            if resp.hovered()
                && enabled
                && let Some(cur) = fv.cursor()
            {
                ui.ctx().set_cursor_icon(cur);
            }
            let mut pc = cx.paint_cx(ui);
            let mut fp = pc.part("field");
            let geom = paint::paint_box(&mut fp, &fv, rect);
            let label_v = cx.part(host, "field", FIELD_UA).in_state(fb);
            let lg = paint::text_galley(ui.ctx(), &label_v, short, None);
            paint::paint_text(&mut fp, &label_v, &lg, geom.content, Align::Start);
            let vg = paint::text_galley(ui.ctx(), &fv, &format!("{shown}"), None);
            paint::paint_text(&mut fp, &fv, &vg, geom.content, Align::End);
            paint::paint_outline(&mut fp, &fv, rect);
        }

        // ---- the hex field ----
        if let Some(rect) = g.hex {
            let cid = id.with("hex");
            let resp = ui.interact(rect, cid, if enabled { egui::Sense::click() | egui::Sense::focusable_noninteractive() } else { egui::Sense::hover() });
            stops.push(resp.clone());
            let mut invalid = false;
            {
                for (action, data) in ak_requests(ui, cid) {
                    if let (accesskit::Action::SetValue, Some(accesskit::ActionData::Value(s))) = (action, &data) {
                        match parse_color(s) {
                            Ok(mut c) => {
                                if !cfg.alpha {
                                    c[3] = 255;
                                }
                                h = Hsv::from_srgba(c);
                                commit = true;
                            }
                            Err(_) => invalid = true,
                        }
                    }
                }
            }
            let hex = color_hex(if cfg.alpha { h.to_srgba() } else { Hsv { a: 1.0, ..h }.to_srgba() });
            ui.ctx().accesskit_node_builder(cid, |n| {
                n.set_role(accesskit::Role::TextInput);
                n.set_label("Hex");
                n.set_value(hex.clone());
                if enabled {
                    n.add_action(accesskit::Action::SetValue);
                } else {
                    n.set_disabled();
                }
                if invalid {
                    n.set_invalid(accesskit::Invalid::True);
                }
            });
            let mut fb = StateBits::NONE;
            fb.set(StateBits::FOCUS_VISIBLE, resp.has_focus());
            fb.set(StateBits::INVALID, invalid);
            fb.set(StateBits::DISABLED, !enabled);
            let fv = cx.part(host, "field", FIELD_UA).in_state(fb);
            let mut pc = cx.paint_cx(ui);
            let mut fp = pc.part("field");
            let geom = paint::paint_box(&mut fp, &fv, rect);
            let gl = paint::text_galley(ui.ctx(), &fv, &hex, None);
            paint::paint_text(&mut fp, &fv, &gl, geom.content, Align::Start);
            paint::paint_outline(&mut fp, &fv, rect);
        }

        // ---- swatches, custom colours, add ----
        let cur = h.to_srgba();
        let lists = [(cfg.swatches, &g.cells, "swatch"), (cfg.custom, &g.custom_cells, "custom")];
        for (list, rects, kind) in lists {
            for (k, (c, rect)) in list.iter().zip(rects.iter()).enumerate() {
                let Ok(rgba) = parse_color(c) else { continue };
                let cid = id.with((kind, k));
                let resp = ui.interact(*rect, cid, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
                let ak = ui.input(|i| i.has_accesskit_action_request(cid, accesskit::Action::Click));
                if enabled && (resp.clicked() || ak) {
                    let mut rgba = rgba;
                    if !cfg.alpha {
                        rgba[3] = 255;
                    }
                    h = Hsv::from_srgba(rgba);
                    commit = true;
                }
                let mut cb = StateBits::NONE;
                cb.set(StateBits::HOVERED, resp.hovered());
                cb.set(StateBits::SELECTED, rgba == cur);
                self.paint_cell(ui, cx, host, *rect, Some(rgba), cb);
                let name = color_hex(rgba);
                let sel = rgba == cur;
                ui.ctx().accesskit_node_builder(cid, |n| {
                    n.set_role(accesskit::Role::Button);
                    n.set_label(name.clone());
                    n.set_selected(sel);
                    n.add_action(accesskit::Action::Click);
                });
            }
        }
        if cfg.custom.len() < 16 && g.add.is_positive() {
            let cid = id.with("add");
            let resp = ui.interact(g.add, cid, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
            let ak = ui.input(|i| i.has_accesskit_action_request(cid, accesskit::Action::Click));
            if enabled && (resp.clicked() || ak) {
                out.push(ColorOut::AddCustom);
            }
            let mut cb = StateBits::NONE;
            cb.set(StateBits::HOVERED, resp.hovered());
            self.paint_cell(ui, cx, host, g.add, None, cb);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, "Add to custom colours"));
        }

        for r in &stops {
            cx.record_part_focus(ui, r);
        }
        if h != before {
            self.hsv = Some(h);
            out.insert(0, ColorOut::Input(h));
            ui.ctx().request_repaint();
        }
        if commit {
            out.push(ColorOut::Commit);
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_ctl(&self, ui: &egui::Ui, cx: &WidgetCx<'_>, host: &StyleView<'_>, ctl: Ctl, rect: Rect, h: Hsv, hb: StateBits) {
        if !ui.is_rect_visible(rect) {
            return;
        }
        let (name, ua) = match ctl {
            Ctl::Square => ("square", SQUARE_UA),
            Ctl::Hue => ("hue", STRIP_UA),
            _ => ("alpha", STRIP_UA),
        };
        let v = cx.part(host, name, ua);
        let mut pc = cx.paint_cx(ui);
        let mut pp = pc.part(name);
        let inner = pp.snap_rect(rect);
        let mut mesh = egui::Mesh::default();
        match ctl {
            Ctl::Square => {
                // a 16×16 grid of exact sRGB HSV samples (bilinear between them)
                const N: usize = 16;
                for j in 0..=N {
                    for i in 0..=N {
                        let (fx, fy) = (i as f32 / N as f32, j as f32 / N as f32);
                        let c = Hsv { h: h.h, s: fx, v: 1.0 - fy, a: 1.0 }.color();
                        mesh.colored_vertex(pos2(inner.left() + fx * inner.width(), inner.top() + fy * inner.height()), c);
                    }
                }
                quads(&mut mesh, N, N);
            }
            Ctl::Hue => {
                const N: usize = 36;
                for i in 0..=N {
                    let fx = i as f32 / N as f32;
                    let c = Hsv { h: fx.min(0.9999), s: 1.0, v: 1.0, a: 1.0 }.color();
                    mesh.colored_vertex(pos2(inner.left() + fx * inner.width(), inner.top()), c);
                    mesh.colored_vertex(pos2(inner.left() + fx * inner.width(), inner.bottom()), c);
                }
                strip(&mut mesh, N);
            }
            _ => {
                checkerboard(&mut pp, inner);
                let c = h.opaque();
                let t = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 0);
                for (x, col) in [(inner.left(), t), (inner.right(), c)] {
                    mesh.colored_vertex(pos2(x, inner.top()), col);
                    mesh.colored_vertex(pos2(x, inner.bottom()), col);
                }
                strip(&mut mesh, 1);
            }
        }
        pp.painter.add(egui::Shape::mesh(mesh));
        pp.record(TraceKind::Path, inner, h.opaque(), Stroke::NONE, 0.0);
        // the border over the gradient (paint_box with no fill)
        paint::paint_box(&mut pp, &v, rect);
        drop(pc);
        // the handle
        let hv = cx.part(host, "handle", HANDLE_UA).in_state(hb);
        let hw = hv.px("width").unwrap_or(10.0);
        let ring = hv.color("border-color").unwrap_or(Color32::WHITE);
        let bw = hv.px("border-width").unwrap_or(2.0);
        let mut pc = cx.paint_cx(ui);
        let mut hp = pc.part("handle");
        let hr = match ctl {
            Ctl::Square => Rect::from_center_size(pos2(rect.left() + h.s * rect.width(), rect.top() + (1.0 - h.v) * rect.height()), Vec2::splat(hw)),
            Ctl::Hue => Rect::from_center_size(pos2(rect.left() + h.h * rect.width(), rect.center().y), vec2(hw * 0.5, rect.height() + 4.0)),
            _ => Rect::from_center_size(pos2(rect.left() + h.a * rect.width(), rect.center().y), vec2(hw * 0.5, rect.height() + 4.0)),
        };
        let fill = if ctl == Ctl::Hue { Hsv { s: 1.0, v: 1.0, a: 1.0, ..h }.color() } else { h.opaque() };
        if ctl == Ctl::Square {
            hp.painter.circle(hr.center(), hw / 2.0, fill, Stroke::new(bw, ring));
            hp.painter.circle_stroke(hr.center(), hw / 2.0 + bw / 2.0, Stroke::new(1.0, Color32::from_black_alpha(140)));
        } else {
            hp.painter.rect(hr, 2.0, fill, Stroke::new(bw, ring), egui::StrokeKind::Middle);
        }
        hp.record(TraceKind::Path, hr, fill, Stroke::new(bw, ring), 0.0);
        paint::paint_outline(&mut hp, &hv, hr);
    }

    fn paint_cell(&self, ui: &egui::Ui, cx: &WidgetCx<'_>, host: &StyleView<'_>, rect: Rect, rgba: Option<[u8; 4]>, bits: StateBits) {
        let v = cx.part(host, "swatch-cell", CELL_UA).in_state(bits);
        let mut pc = cx.paint_cx(ui);
        let mut cp = pc.part("swatch-cell");
        let r = cp.snap_rect(rect);
        match rgba {
            Some(c) => {
                if c[3] < 255 {
                    checkerboard(&mut cp, r);
                }
                let col = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                cp.painter.rect_filled(r, v.radii()[0], col);
                cp.record(TraceKind::Box, r, col, Stroke::NONE, 0.0);
            }
            None => {
                // the add cell: a plus
                let col = v.color("color").unwrap_or(Color32::GRAY);
                let s = Stroke::new(cp.snap_width(1.0), col);
                let c = r.center();
                let d = r.width() * 0.3;
                cp.painter.line_segment([pos2(c.x - d, c.y), pos2(c.x + d, c.y)], s);
                cp.painter.line_segment([pos2(c.x, c.y - d), pos2(c.x, c.y + d)], s);
                cp.record(TraceKind::Path, r, col, s, 0.0);
            }
        }
        paint::paint_box(&mut cp, &v, rect);
    }
}

/// This pass's AccessKit requests for node `id` (Increment, Decrement, SetValue, Click).
fn ak_requests(ui: &egui::Ui, id: egui::Id) -> Vec<(accesskit::Action, Option<accesskit::ActionData>)> {
    use accesskit::Action as A;
    ui.input(|i| [A::Increment, A::Decrement, A::SetValue, A::Click].iter().flat_map(|a| i.accesskit_action_requests(id, *a).map(|r| (r.action, r.data.clone()))).collect())
}

/// Triangles for a (w+1)×(h+1) vertex grid, row-major.
fn quads(mesh: &mut egui::Mesh, w: usize, h: usize) {
    let row = (w + 1) as u32;
    for j in 0..h as u32 {
        for i in 0..w as u32 {
            let a = j * row + i;
            mesh.add_triangle(a, a + 1, a + row);
            mesh.add_triangle(a + 1, a + row + 1, a + row);
        }
    }
}

/// Triangles for a strip of `n` segments, vertices as (top, bottom) pairs.
fn strip(mesh: &mut egui::Mesh, n: usize) {
    for i in 0..n as u32 {
        let a = 2 * i;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
}

/// The checkerboard under alpha (4px squares, VS Code's greys).
fn checkerboard(cx: &mut paint::PaintCx<'_>, r: Rect) {
    let s = (r.height() / 2.0).clamp(3.0, 6.0);
    let (light, dark) = (Color32::from_gray(204), Color32::from_gray(128));
    cx.painter.rect_filled(r, 0.0, light);
    let (nx, ny) = ((r.width() / s).ceil() as usize, (r.height() / s).ceil() as usize);
    for j in 0..ny {
        for i in (j % 2..nx).step_by(2) {
            let q = Rect::from_min_size(pos2(r.left() + i as f32 * s, r.top() + j as f32 * s), Vec2::splat(s)).intersect(r);
            cx.painter.rect_filled(q, 0.0, dark);
        }
    }
    cx.record(TraceKind::Box, r, light, Stroke::NONE, 0.0);
}

/// An inline colour editor.
// the `#[part]` fields name the kind's `::part()`s; nothing reads them (paint parts)
#[allow(dead_code)]
#[svelters::widget(namespace = "sv", replaces = "color-editor", caps = PAINTS_BOX)]
pub struct ColorEditor {
    /// The colour (`bind:value`), CSS hex.
    #[prop(bind)]
    value: HexColor,
    #[prop(measure)]
    alpha: AlphaMode,
    #[prop(default = "hex rgb", measure)]
    fields: String,
    #[prop(measure)]
    swatches: Vec<String>,
    #[prop(bind, measure)]
    custom_colors: Vec<String>,
    /// The colour changed (each drag frame, step, field or swatch).
    #[event]
    changed: Event<String>,
    /// A commit point (drag released, a step, a field set, a swatch).
    #[event]
    commit: Event<String>,
    #[state]
    body: ColorBody,
    #[part]
    square: crate::provider::Part,
    #[part]
    hue: crate::provider::Part,
    #[part(name = "alpha")]
    alpha_part: crate::provider::Part,
    #[part]
    handle: crate::provider::Part,
    #[part(name = "fields")]
    fields_part: crate::provider::Part,
    #[part]
    field: crate::provider::Part,
    #[part(name = "swatches")]
    swatches_part: crate::provider::Part,
    #[part(name = "swatch-cell")]
    swatch_cell: crate::provider::Part,
}

/// Runs a colour body for an owner (`color-editor`, `color-picker`'s popup) and applies what it
/// did: `changed` per edit (EV1 can prevent it), `commit` at commit points (`live` mode), the add
/// cell. `confirm`: edits stay in the body until the owner applies them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_body(
    body: &mut ColorBody,
    ui: &mut egui::Ui,
    cx: &mut WidgetCx<'_>,
    ua: Ua,
    cfg: &ColorCfg<'_>,
    value: &mut HexColor,
    custom: &mut Vec<String>,
    r: Rect,
    a11y_parent: egui::Id,
    live: bool,
    changed: &Event<String>,
    commit: &Event<String>,
) -> bool {
    let mut committed = false;
    let outs = body.ui(ui, cx, ua, cfg, value.rgba(), r, a11y_parent);
    for o in outs {
        match o {
            ColorOut::Input(mut h) if live => {
                if !cfg.alpha {
                    h.a = 1.0;
                }
                let hex = color_hex(h.to_srgba());
                if hex != value.0 && !cx.dispatch(changed, hex.clone()).prevented() {
                    body.edited(h);
                    cx.set(value, HexColor(hex));
                }
            }
            ColorOut::Input(_) => {}
            ColorOut::Commit if live => {
                cx.dispatch(commit, value.0.clone());
                committed = true;
            }
            ColorOut::Commit => {}
            ColorOut::AddCustom => {
                let mut l = custom.clone();
                let hex = color_hex(body.hsv(value.rgba()).to_srgba());
                if !l.contains(&hex) {
                    l.push(hex);
                    cx.set(custom, l);
                    cx.request_measure();
                }
            }
        }
    }
    committed
}

impl Widget for ColorEditor {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> Vec2 {
        let s = cx.style(UA);
        let (hex, rgb, hsv) = ColorCfg::fields_of(&self.fields);
        let cfg = ColorCfg { alpha: self.alpha.shown(), hex, rgb, hsv, swatches: &self.swatches, custom: &self.custom_colors };
        ColorBody::measure(&s, &cfg) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = cx.interact(ui, egui::Sense::hover());
        let opacity = cx.style(UA).opacity();
        if opacity < 1.0 {
            ui.multiply_opacity(opacity);
        }
        let geom = cx.paint_box(ui, UA);
        let (hex, rgb, hsv) = ColorCfg::fields_of(&self.fields);
        // `cx.set` finds a prop by its field's address: `value` and `custom-colors` stay in place
        let swatches = std::mem::take(&mut self.swatches);
        let custom = self.custom_colors.clone();
        let cfg = ColorCfg { alpha: self.alpha.shown(), hex, rgb, hsv, swatches: &swatches, custom: &custom };
        run_body(&mut self.body, ui, cx, UA, &cfg, &mut self.value, &mut self.custom_colors, geom.content, resp.id, true, &self.changed, &self.commit);
        self.swatches = swatches;
        let enabled = cx.enabled();
        let text = self.value.0.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, enabled, &text));
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Group);
        node.set_value(self.value.0.clone());
    }

    fn consume_key(&self, key: Key, m: Modifiers) -> bool {
        ColorBody::claims(key, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_round_trips_srgb_and_keeps_grey_hues() {
        for c in [[255, 0, 0, 255], [51, 102, 204, 255], [0, 0, 0, 128], [255, 255, 255, 255], [12, 200, 77, 3]] {
            assert_eq!(Hsv::from_srgba(c).to_srgba(), c);
        }
        let mut b = ColorBody::default();
        let h = b.hsv([255, 0, 0, 255]);
        b.edited(Hsv { h: 0.5, ..h });
        // a host write of a grey keeps the edit's hue
        let g = b.hsv([128, 128, 128, 255]);
        assert_eq!(g.h, 0.5);
        assert_eq!(color_hex([255, 0, 0, 255]), "#ff0000");
        assert_eq!(color_hex([0, 255, 0, 128]), "#00ff0080");
        assert_eq!(ColorCfg::fields_of("hex hsv"), (true, false, true));
        assert_eq!(ColorBody::with_channel(Hsv::from_srgba([255, 0, 0, 255]), 1, 255.0).to_srgba(), [255, 255, 0, 255]);
    }
}
