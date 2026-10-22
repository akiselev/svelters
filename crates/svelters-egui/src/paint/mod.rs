//! OW0 (ARCH-WIDGETS §1.1): the paint primitives of svelters' own widgets.
//!
//! Every primitive takes a [`StyleView`] (the computed style of a node or a part, with the
//! widget's user-agent defaults under it), a rect in points and a [`PaintCx`], and emits epaint
//! shapes. Primitives never read `egui::Style` or `Visuals`: the look is CSS (the theme's
//! `sv.base`/`sv.affordance` layers and author rules) and, where no rule sets a property, the
//! widget's [`Ua`] table (the UA sheet: values from the built-in VS Code theme's tokens).
//!
//! Nothing here needs a scene node: a foreign painter (cadatomic's `ChromePainter` for berth, the
//! OW-RUST immediate-mode layer) builds a [`StyleView`] from a theme ([`StyleView::ua_only`]) or a
//! computed style and calls the same functions with its own `Painter` and rect.
//!
//! Paint order of one node (CSS 2.1 Appendix E, simplified): outer shadows, background, inset
//! shadows, border ([`paint_box`]), content ([`paint_text`], [`paint_icon`], [`paint_image`]),
//! then the outline ([`paint_outline`], the focus ring, last).
//!
//! Files: this one holds the contexts and the style view; `boxes.rs` box and outline; `text.rs`
//! text; `icon.rs`, `image.rs` the icon and image primitives; `trace.rs` the debug paint trace.
//! Writer lane: OW (OW0; OW-CSS wires transitions behind [`StyleView`]).

mod boxes;
mod icon;
mod image;
mod text;
mod trace;

pub use boxes::{BoxGeom, box_extra, box_geom, paint_box, paint_outline};
pub use icon::{IconKey, icon_size, paint_icon};
pub use image::paint_image;
pub use text::{Align, measure_text, paint_text, text_galley};
pub use trace::{PaintTrace, TraceEntry, TraceKind};

use crate::provider::StateBits;
use egui::Color32;
use svelters_style::{ComputedStyle, ComputedValue, Shadow};

/// Where primitives paint: the painter (its clip), the physical pixel scale for snapping, and the
/// optional debug trace that names what each shape is ([`PaintTrace`], VIS1).
pub struct PaintCx<'a> {
    pub painter: &'a egui::Painter,
    /// `pixels_per_point`: box edges, borders and outlines snap to physical pixels (§3.5).
    pub ppp: f32,
    /// Tests and VIS1: records `(target, part)` for each shape painted.
    pub trace: Option<&'a mut PaintTrace>,
    /// What the shapes belong to in the trace: the node (its `NodeId::to_raw()`, 0 for none) and
    /// the part (`None`: the node itself).
    pub target: (u64, Option<&'static str>),
}

impl<'a> PaintCx<'a> {
    /// Painting into `painter` at its context's scale, untraced.
    pub fn new(painter: &'a egui::Painter) -> PaintCx<'a> {
        PaintCx { painter, ppp: painter.pixels_per_point(), trace: None, target: (0, None) }
    }

    /// The same painter and trace for part `part` of the current target.
    pub fn part(&mut self, part: &'static str) -> PaintCx<'_> {
        PaintCx { painter: self.painter, ppp: self.ppp, trace: self.trace.as_deref_mut(), target: (self.target.0, Some(part)) }
    }

    pub(crate) fn record(&mut self, kind: TraceKind, rect: egui::Rect, fill: Color32, stroke: egui::Stroke, font_size: f32) {
        if let Some(t) = self.trace.as_deref_mut() {
            t.entries.push(TraceEntry { node: self.target.0, part: self.target.1, kind, rect, fill, stroke, font_size });
        }
    }

    /// `v` points rounded to whole physical pixels, at least one if `v > 0` (borders, outlines,
    /// carets: §3.5).
    pub fn snap_width(&self, v: f32) -> f32 {
        if v <= 0.0 {
            return 0.0;
        }
        (v * self.ppp).floor().max(1.0) / self.ppp
    }

    /// `r` with its edges on the physical pixel grid.
    pub fn snap_rect(&self, r: egui::Rect) -> egui::Rect {
        use egui::emath::GuiRounding as _;
        r.round_to_pixels(self.ppp)
    }
}

// ---- the user-agent table ----

/// A user-agent value: a theme token (resolved against the built-in VS Code theme of the scheme),
/// or a literal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UaValue {
    /// A theme token by name (`button-bg`, `--sv-focus`).
    Token(&'static str),
    Px(f32),
    Color(Color32),
    Number(f32),
    Keyword(&'static str),
}

/// One UA rule: applies when the state has every bit of `when` and none of `unless`.
#[derive(Clone, Copy, Debug)]
pub struct UaRule {
    pub when: StateBits,
    pub unless: StateBits,
    pub decls: &'static [(&'static str, UaValue)],
}

impl UaRule {
    /// Always applies.
    pub const fn base(decls: &'static [(&'static str, UaValue)]) -> UaRule {
        UaRule { when: StateBits::NONE, unless: StateBits::NONE, decls }
    }
    /// Applies in state `when`.
    pub const fn when(when: StateBits, decls: &'static [(&'static str, UaValue)]) -> UaRule {
        UaRule { when, unless: StateBits::NONE, decls }
    }
    /// Applies in state `when` but not in `unless`.
    pub const fn when_not(when: StateBits, unless: StateBits, decls: &'static [(&'static str, UaValue)]) -> UaRule {
        UaRule { when, unless, decls }
    }
}

/// A widget's user-agent sheet: what a property is when no CSS rule sets it, by state. Later rules
/// win (as in a sheet). Every table falls back to [`Ua::ROOT`] (text colour and font).
#[derive(Clone, Copy, Debug)]
pub struct Ua(pub &'static [UaRule]);

impl Ua {
    pub const NONE: Ua = Ua(&[]);
    /// What every widget inherits: the foreground colour and the body font.
    pub const ROOT: Ua = Ua(&[
        UaRule::base(&[("color", UaValue::Token("fg")), ("font-size", UaValue::Token("font-size")), ("font-weight", UaValue::Number(400.0))]),
        UaRule::when(StateBits::DISABLED, &[("color", UaValue::Token("disabled-fg"))]),
    ]);

    fn find(&self, name: &str, state: StateBits) -> Option<UaValue> {
        let hit = |rules: &[UaRule]| {
            rules.iter().rev().filter(|r| state.contains(r.when) && !state.intersects(r.unless)).find_map(|r| r.decls.iter().find(|(n, _)| *n == name).map(|(_, v)| *v))
        };
        hit(self.0).or_else(|| hit(Ua::ROOT.0))
    }
}

/// The built-in theme a UA token resolves against: VS Code's Dark Modern or Light Modern.
fn ua_theme(dark: bool) -> &'static crate::theme::Theme {
    static DARK: std::sync::OnceLock<crate::theme::Theme> = std::sync::OnceLock::new();
    static LIGHT: std::sync::OnceLock<crate::theme::Theme> = std::sync::OnceLock::new();
    if dark { DARK.get_or_init(crate::theme::Theme::dark_modern) } else { LIGHT.get_or_init(crate::theme::Theme::light_modern) }
}

// ---- the style view ----

/// What primitives read: one node's (or part's) computed style, the widget's UA table under it,
/// and the state the UA rules match. Copy, borrowed: build one per paint.
///
/// DS2/OW-CSS hook: running transitions will be applied here (the view interpolates a property
/// between the painted and the computed value at paint time); today it reads the computed value.
#[derive(Clone, Copy)]
pub struct StyleView<'a> {
    css: Option<&'a ComputedStyle>,
    ua: Ua,
    state: StateBits,
    dark: bool,
    weight: Option<u16>,
}

impl std::fmt::Debug for StyleView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StyleView").field("css", &self.css.is_some()).field("state", &self.state).field("dark", &self.dark).finish()
    }
}

impl<'a> StyleView<'a> {
    /// `css` (the cascade's computed style, `None` when no sheet reaches the node) over `ua`, in
    /// `state`; `dark` picks the scheme UA tokens resolve in.
    pub fn new(css: Option<&'a ComputedStyle>, ua: Ua, state: StateBits, dark: bool) -> StyleView<'a> {
        StyleView { css, ua, state, dark, weight: None }
    }

    /// The UA table alone (no cascade): a foreign painter's styled control (OW-RUST, berth's
    /// `ChromePainter`).
    pub fn ua_only(ua: Ua, state: StateBits, dark: bool) -> StyleView<'static> {
        StyleView { css: None, ua, state, dark, weight: None }
    }

    /// With a `font-weight` that beats CSS (a kind's `font-weight` prop, which the runtime set).
    pub fn with_font_weight(mut self, w: Option<i64>) -> StyleView<'a> {
        self.weight = w.map(|w| w.clamp(1, 1000) as u16);
        self
    }

    /// The same view in another state (a part's own state, a forced state in a test).
    pub fn in_state(mut self, state: StateBits) -> StyleView<'a> {
        self.state = state;
        self
    }

    pub fn state(&self) -> StateBits {
        self.state
    }
    pub fn computed(&self) -> Option<&'a ComputedStyle> {
        self.css
    }
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    fn css(&self, name: &str) -> Option<&'a ComputedValue> {
        self.css.and_then(|c| c.get(name))
    }

    fn ua_color(&self, name: &str) -> Option<Color32> {
        match self.ua.find(name, self.state)? {
            UaValue::Token(t) => ua_theme(self.dark).color(t),
            UaValue::Color(c) => Some(c),
            _ => None,
        }
    }

    fn ua_px(&self, name: &str) -> Option<f32> {
        match self.ua.find(name, self.state)? {
            UaValue::Token(t) => ua_theme(self.dark).px(t),
            UaValue::Px(p) | UaValue::Number(p) => Some(p),
            _ => None,
        }
    }

    /// A colour property (`background-color`, `color`, `border-top-color`, `outline-color`).
    pub fn color(&self, name: &str) -> Option<Color32> {
        match self.css(name) {
            Some(ComputedValue::Color(k)) => {
                let [r, g, b, a] = k.rgba8();
                Some(Color32::from_rgba_unmultiplied(r, g, b, a))
            }
            _ => self.ua_color(name),
        }
    }

    /// A length in points (`border-radius`, `padding-left`, `font-size`, `outline-offset`).
    pub fn px(&self, name: &str) -> Option<f32> {
        match self.css(name) {
            Some(ComputedValue::Length(l)) => Some(l.px),
            Some(ComputedValue::Number(n)) => Some(*n),
            _ => self.ua_px(name),
        }
    }

    /// A number (`opacity`).
    pub fn number(&self, name: &str) -> Option<f32> {
        match self.css(name) {
            Some(ComputedValue::Number(n)) => Some(*n),
            Some(ComputedValue::Int(n)) => Some(*n as f32),
            _ => self.ua_px(name),
        }
    }

    /// A keyword (`text-align`, `outline-style`, `white-space`).
    pub fn keyword(&self, name: &str) -> Option<&'a str> {
        match self.css(name) {
            Some(ComputedValue::Keyword(k)) => Some(k),
            Some(ComputedValue::Raw(r)) => Some(r),
            Some(ComputedValue::String(s)) => Some(s),
            _ => match self.ua.find(name, self.state)? {
                UaValue::Keyword(k) => Some(k),
                _ => None,
            },
        }
    }

    /// `font-weight` (1–1000): the kind's prop, else CSS, else 400.
    pub fn font_weight(&self) -> i64 {
        if let Some(w) = self.weight {
            return w as i64;
        }
        match self.css("font-weight") {
            Some(ComputedValue::FontWeight(w)) => *w as i64,
            Some(ComputedValue::Number(n)) => *n as i64,
            _ => self.ua_px("font-weight").map_or(400, |w| w as i64),
        }
    }

    /// `font-size` in points (the UA root: the theme's body size).
    pub fn font_size(&self) -> f32 {
        self.px("font-size").unwrap_or(13.0)
    }

    /// The `box-shadow` list (CSS only; the UA tables set none).
    pub fn shadows(&self) -> Option<&'a [Shadow]> {
        self.css.and_then(crate::style::shadow::shadows)
    }

    /// One side group (`padding`, `border-width`) as [left, right, top, bottom]: the longhands
    /// (`padding-left`, `border-left-width`), else the shorthand.
    pub fn sides(&self, group: &str) -> [f32; 4] {
        let names: [String; 4] = match group {
            "border-width" => ["border-left-width", "border-right-width", "border-top-width", "border-bottom-width"].map(String::from),
            g => ["left", "right", "top", "bottom"].map(|s| format!("{g}-{s}")),
        };
        let all = match self.css(group) {
            Some(ComputedValue::Insets([t, r, b, l])) => Some([l.px, r.px, t.px, b.px]),
            _ => None,
        };
        let one = self.px(group);
        let mut out = [0.0; 4];
        for (k, n) in names.iter().enumerate() {
            out[k] = match self.css(n) {
                Some(ComputedValue::Length(l)) => l.px,
                _ => all.map(|a| a[k]).or(one).or_else(|| self.ua_px(n)).unwrap_or(0.0),
            };
        }
        out
    }
}
