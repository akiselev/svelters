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
pub mod painters;
mod parts;
mod text;
pub mod trace;

pub use boxes::{BoxGeom, box_extra, box_geom, paint_box, paint_box_layers, paint_inset_shadows, paint_outline};
pub use icon::{IconKey, content_icon, icon_size, paint_icon};
pub use image::paint_image;
pub use painters::{Orientation, PaintArgs, PartInfo, PartPainter, register_painter};
pub use parts::{PartItem, layout_parts, measure_parts};
pub use text::{Align, Elide, elide, elide_with, measure_text, paint_text, text_galley};
pub use trace::{PaintTrace, TraceEntry, TraceKind};

use crate::provider::StateBits;
use egui::Color32;
use std::borrow::Cow;
use svelters_style::{ComputedStyle, ComputedValue, Shadow};

/// `inset 0 1px 2px rgba(0,0,0,.25)`: DESIGN R2's pressed shadow.
pub const PRESSED_INSET: &[Shadow] = &[Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Some(svelters_style::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 }), inset: true }];

/// An egui shadow (a theme's `elev-*` token) as a CSS one.
fn css_shadow(s: egui::Shadow) -> Shadow {
    let [r, g, b, a] = s.color.to_srgba_unmultiplied();
    Shadow {
        x: s.offset[0] as f32,
        y: s.offset[1] as f32,
        blur: s.blur as f32 / 2.0,
        spread: s.spread as f32,
        color: Some(svelters_style::Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }),
        inset: false,
    }
}

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
    /// The scene's per-pass trace ([`trace::enable`]), when `trace` is `None`: debug builds only.
    sink: Option<egui::Context>,
    /// The host's tag, for part painters ([`PartInfo::tag`]).
    pub tag: &'a str,
    /// What a part painter selected in CSS gets for the part being painted (set by the widget).
    pub args: PaintArgs<'a>,
}

impl<'a> PaintCx<'a> {
    /// Painting into `painter` at its context's scale, untraced.
    pub fn new(painter: &'a egui::Painter) -> PaintCx<'a> {
        PaintCx { painter, ppp: painter.pixels_per_point(), trace: None, target: (0, None), sink: None, tag: "", args: PaintArgs::None }
    }

    /// Painting for scene node `node` (`NodeId::to_raw()`): traced into the context's per-pass
    /// trace when [`trace::enable`] turned it on (debug builds).
    pub fn for_node(painter: &'a egui::Painter, node: u64) -> PaintCx<'a> {
        let mut p = PaintCx::new(painter);
        p.target = (node, None);
        if trace::enabled(painter.ctx()) {
            p.sink = Some(painter.ctx().clone());
        }
        p
    }

    /// The same painter and trace for part `part` of the current target.
    pub fn part(&mut self, part: &'static str) -> PaintCx<'_> {
        PaintCx { painter: self.painter, ppp: self.ppp, trace: self.trace.as_deref_mut(), target: (self.target.0, Some(part)), sink: self.sink.clone(), tag: self.tag, args: PaintArgs::None }
    }

    /// Calls the part painter `p` for the current target in `rect`.
    pub(crate) fn run_painter(&mut self, p: &dyn PartPainter, s: &StyleView, rect: egui::Rect) {
        let (tag, part, args) = (self.tag, self.target.1, self.args);
        let info = PartInfo::new(tag, part, s.state(), self.ppp);
        p.paint(self, s, rect, &info, &args);
    }

    pub(crate) fn record(&mut self, kind: TraceKind, rect: egui::Rect, fill: Color32, stroke: egui::Stroke, font_size: f32) {
        let e = TraceEntry { node: self.target.0, part: self.target.1, kind, rect, fill, stroke, font_size };
        if let Some(t) = self.trace.as_deref_mut() {
            t.entries.push(e);
        } else if let Some(ctx) = &self.sink {
            trace::push(ctx, e);
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
    /// A theme token by name (`button-bg`, `--sv-focus`, `elev-1` for `box-shadow`).
    Token(&'static str),
    Px(f32),
    Color(Color32),
    Number(f32),
    Keyword(&'static str),
    /// A `box-shadow` list (empty: `none`).
    Shadows(&'static [Shadow]),
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

    fn find_own(&self, name: &str, state: StateBits) -> Option<UaValue> {
        self.0.iter().rev().filter(|r| state.contains(r.when) && !state.intersects(r.unless)).find_map(|r| r.decls.iter().find(|(n, _)| *n == name).map(|(_, v)| *v))
    }
}

/// The built-in theme a UA token resolves against: VS Code's Dark Modern or Light Modern.
fn ua_theme(dark: bool) -> &'static crate::theme::Theme {
    static DARK: std::sync::OnceLock<crate::theme::Theme> = std::sync::OnceLock::new();
    static LIGHT: std::sync::OnceLock<crate::theme::Theme> = std::sync::OnceLock::new();
    if dark { DARK.get_or_init(crate::theme::Theme::dark_modern) } else { LIGHT.get_or_init(crate::theme::Theme::light_modern) }
}

/// The properties a part (or child) inherits from its host when nothing sets them on it.
const INHERITED: &[&str] = &["color", "font-size", "font-weight", "font-family", "font-style", "line-height", "letter-spacing", "text-align", "text-transform", "white-space", "cursor", "visibility"];

// ---- the style view ----

/// Where a property's value came from.
#[derive(Clone, Copy)]
enum Got<'a> {
    Css(&'a ComputedValue),
    Ua(UaValue, bool),
}

/// What primitives read: one node's (or part's) computed style, the widget's UA table under it,
/// and the state the UA rules match. Copy, borrowed: build one per paint.
///
/// Resolution of a property: the computed style, then the UA table, then (inherited properties
/// only) the parent view ([`StyleView::with_parent`]: a part's host), then [`Ua::ROOT`].
///
/// OW-CSS: running CSS transitions apply here. `over` holds the node's (or part's) transition
/// samples for this frame ([`svelters_style::Transitions::sample`], kept on the scene), which
/// override the computed values.
#[derive(Clone, Copy)]
pub struct StyleView<'a> {
    css: Option<&'a ComputedStyle>,
    /// Running transitions' values at this frame, sorted by property index (empty: none run).
    over: &'a [(usize, ComputedValue)],
    ua: Ua,
    state: StateBits,
    dark: bool,
    weight: Option<u16>,
    parent: Option<&'a StyleView<'a>>,
}

impl std::fmt::Debug for StyleView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StyleView").field("css", &self.css.is_some()).field("animated", &!self.over.is_empty()).field("state", &self.state).field("dark", &self.dark).field("parent", &self.parent.is_some()).finish()
    }
}

impl<'a> StyleView<'a> {
    /// `css` (the cascade's computed style, `None` when no sheet reaches the node) over `ua`, in
    /// `state`; `dark` picks the scheme UA tokens resolve in.
    pub fn new(css: Option<&'a ComputedStyle>, ua: Ua, state: StateBits, dark: bool) -> StyleView<'a> {
        StyleView { css, over: &[], ua, state, dark, weight: None, parent: None }
    }

    /// The UA table alone (no cascade): a foreign painter's styled control (OW-RUST, berth's
    /// `ChromePainter`).
    pub fn ua_only(ua: Ua, state: StateBits, dark: bool) -> StyleView<'static> {
        StyleView { css: None, over: &[], ua, state, dark, weight: None, parent: None }
    }

    /// A part's view: its own computed style (`::part(name)`, `None` until a rule reaches it) and
    /// UA table, inheriting from this view (its host). The part shares the host's state.
    pub fn part(&'a self, css: Option<&'a ComputedStyle>, ua: Ua) -> StyleView<'a> {
        StyleView { css, over: &[], ua, state: self.state, dark: self.dark, weight: None, parent: Some(self) }
    }

    /// With running transitions' values over the computed style (OW-CSS: the scene's sample of
    /// the node or part this frame, sorted by property index).
    pub fn with_overlay(mut self, over: &'a [(usize, ComputedValue)]) -> StyleView<'a> {
        self.over = over;
        self
    }

    /// The engine's typed view of the CSS layer (transitions applied), for what the UA table
    /// doesn't carry: per-side borders, radii, gradients, painters, `content`.
    pub fn engine(&self) -> Option<svelters_style::StyleView<'a>> {
        self.css.map(|c| svelters_style::StyleView::with_overlay(c, self.over))
    }

    /// Whether a running transition overrides any value.
    pub fn is_animated(&self) -> bool {
        !self.over.is_empty()
    }

    /// With `parent` to inherit from.
    pub fn with_parent(mut self, parent: &'a StyleView<'a>) -> StyleView<'a> {
        self.parent = Some(parent);
        self
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
        self.css.and_then(|c| svelters_style::overlay_get(c, self.over, name))
    }

    fn get(&self, name: &str) -> Option<Got<'a>> {
        if let Some(v) = self.css(name) {
            return Some(Got::Css(v));
        }
        if let Some(v) = self.ua.find_own(name, self.state) {
            return Some(Got::Ua(v, self.dark));
        }
        if let Some(p) = self.parent
            && INHERITED.contains(&name)
        {
            return p.get(name);
        }
        Ua::ROOT.find_own(name, self.state).map(|v| Got::Ua(v, self.dark))
    }

    fn ua_px_of(v: UaValue, dark: bool) -> Option<f32> {
        match v {
            UaValue::Token(t) => ua_theme(dark).px(t),
            UaValue::Px(p) | UaValue::Number(p) => Some(p),
            _ => None,
        }
    }

    /// A colour property (`background-color`, `color`, `border-top-color`, `outline-color`).
    pub fn color(&self, name: &str) -> Option<Color32> {
        match self.get(name)? {
            Got::Css(ComputedValue::Color(k)) => {
                let [r, g, b, a] = k.rgba8();
                Some(Color32::from_rgba_unmultiplied(r, g, b, a))
            }
            Got::Css(_) => None,
            Got::Ua(UaValue::Token(t), dark) => ua_theme(dark).color(t),
            Got::Ua(UaValue::Color(c), _) => Some(c),
            Got::Ua(..) => None,
        }
    }

    /// A length in points (`border-radius`, `padding-left`, `font-size`, `outline-offset`).
    pub fn px(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Got::Css(ComputedValue::Length(l)) => Some(l.px),
            Got::Css(ComputedValue::Number(n)) => Some(*n),
            Got::Css(_) => None,
            Got::Ua(v, dark) => Self::ua_px_of(v, dark),
        }
    }

    /// A number (`opacity`).
    pub fn number(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Got::Css(ComputedValue::Number(n)) => Some(*n),
            Got::Css(ComputedValue::Int(n)) => Some(*n as f32),
            Got::Css(_) => None,
            Got::Ua(v, dark) => Self::ua_px_of(v, dark),
        }
    }

    /// A keyword or raw value (`text-align`, `outline-style`, `content`, `background-image`).
    pub fn keyword(&self, name: &str) -> Option<&'a str> {
        match self.get(name)? {
            Got::Css(ComputedValue::Keyword(k)) => Some(k),
            Got::Css(ComputedValue::Raw(r)) => Some(r),
            Got::Css(ComputedValue::String(s)) => Some(s),
            Got::Css(_) => None,
            Got::Ua(UaValue::Keyword(k), _) => Some(k),
            Got::Ua(..) => None,
        }
    }

    /// `font-weight` (1–1000): the kind's prop, else CSS, else the UA table, else inherited, else 400.
    pub fn font_weight(&self) -> i64 {
        if let Some(w) = self.weight {
            return w as i64;
        }
        match self.css("font-weight") {
            Some(ComputedValue::FontWeight(w)) => return *w as i64,
            Some(ComputedValue::Number(n)) => return *n as i64,
            _ => {}
        }
        if let Some(v) = self.ua.find_own("font-weight", self.state).and_then(|v| Self::ua_px_of(v, self.dark)) {
            return v as i64;
        }
        match self.parent {
            Some(p) => p.font_weight(),
            None => 400,
        }
    }

    /// `font-size` in points (the UA root: the theme's body size).
    pub fn font_size(&self) -> f32 {
        self.px("font-size").unwrap_or(13.0)
    }

    /// The `box-shadow` list: CSS's, else the UA table's (an `elev-*` token or a literal list).
    pub fn shadows(&self) -> Option<Cow<'a, [Shadow]>> {
        if let Some(ComputedValue::Shadows(l)) = self.css("box-shadow") {
            return (!l.is_empty()).then(|| Cow::Borrowed(&**l));
        }
        match self.ua.find_own("box-shadow", self.state)? {
            UaValue::Shadows(l) if !l.is_empty() => Some(Cow::Borrowed(l)),
            UaValue::Token(t) => {
                let s = ua_theme(self.dark).shadow(t)?;
                (s != egui::Shadow::NONE).then(|| Cow::Owned(vec![css_shadow(s)]))
            }
            _ => None,
        }
    }

    /// `cursor` as egui's icon (DESIGN R10), if set.
    pub fn cursor(&self) -> Option<egui::CursorIcon> {
        use egui::CursorIcon as C;
        Some(match self.keyword("cursor")? {
            "pointer" => C::PointingHand,
            "text" => C::Text,
            "not-allowed" => C::NotAllowed,
            "grab" => C::Grab,
            "grabbing" => C::Grabbing,
            "col-resize" => C::ResizeColumn,
            "row-resize" => C::ResizeRow,
            "crosshair" => C::Crosshair,
            "move" => C::Move,
            "wait" => C::Wait,
            "progress" => C::Progress,
            "help" => C::Help,
            "default" | "auto" => C::Default,
            _ => return None,
        })
    }

    /// `opacity` (1 when unset).
    pub fn opacity(&self) -> f32 {
        self.number("opacity").unwrap_or(1.0).clamp(0.0, 1.0)
    }

    /// `display: none`.
    pub fn is_hidden(&self) -> bool {
        self.keyword("display") == Some("none")
    }

    /// The corner radii [top-left, top-right, bottom-right, bottom-left]: each
    /// `border-{corner}-radius`, else `border-radius`.
    pub fn radii(&self) -> [f32; 4] {
        let one = self.px("border-radius").unwrap_or(0.0);
        ["border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius"].map(|n| match self.css(n) {
            Some(ComputedValue::Length(l)) => l.px.max(0.0),
            _ => one.max(0.0),
        })
    }

    /// The border sides [left, right, top, bottom] as (width, colour): `border-{side}-width` and
    /// `-color`, else `border-width` and `border-color`. A `none`/`hidden` style gives width 0 (an
    /// unset style counts as solid, as in the engine's view).
    pub fn border_sides(&self) -> [(f32, Color32); 4] {
        const NAMES: [(&str, &str); 4] = [("border-left-style", "border-left-color"), ("border-right-style", "border-right-color"), ("border-top-style", "border-top-color"), ("border-bottom-style", "border-bottom-color")];
        let w = self.sides("border-width");
        let one = self.color("border-color");
        let mut out = [(0.0, Color32::TRANSPARENT); 4];
        for (k, (style, color)) in NAMES.iter().enumerate() {
            let hidden = matches!(self.keyword(style), Some("none" | "hidden"));
            out[k] = (if hidden { 0.0 } else { w[k] }, self.color(color).or(one).unwrap_or(Color32::TRANSPARENT));
        }
        out
    }

    /// One side group (`padding`, `border-width`, `margin`) as [left, right, top, bottom]: the
    /// longhands (`padding-left`, `border-left-width`), else the shorthand.
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
                _ => all.map(|a| a[k]).or(one).or_else(|| self.ua.find_own(n, self.state).and_then(|v| Self::ua_px_of(v, self.dark))).unwrap_or(0.0),
            };
        }
        out
    }
}
