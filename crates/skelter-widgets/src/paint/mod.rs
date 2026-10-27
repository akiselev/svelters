//! The paint primitives of skelter's built-in kinds (ARCH-WIDGETS §1.1), ported from svelters-egui
//! `paint/` onto display lists.
//!
//! Every primitive takes a [`StyleView`] (the computed style of a node or a part, with the kind's
//! user-agent defaults under it), a rect in node-local points and a [`PaintCx`], and pushes
//! [`Prim`](skelter_base::display::Prim)s. The look is CSS (the theme's `sv.base`/`sv.affordance` layers
//! and author rules) and, where no rule sets a property, the kind's [`Ua`] table (values from the
//! built-in VS Code theme's tokens).
//!
//! Nothing here needs a tree node: a foreign painter builds a [`StyleView`] from a theme
//! ([`StyleView::ua_only`]) or a computed style and calls the same functions with its own list.
//!
//! Paint order of one node (CSS 2.1 Appendix E, simplified): outer shadows, background, inset
//! shadows, border ([`paint_box`]), content ([`paint_text`], [`paint_icon`], [`paint_image`]),
//! then the outline ([`paint_outline`], the focus ring, last).
//!
//! `opacity` is not painted: it is a property (an effect node, architecture §4.3), applied by the
//! compositor. Files: this one holds the context and the style view; `boxes.rs` box and outline;
//! `text.rs` text; `parts.rs` paint parts laid out by CSS; `icon.rs`, `image.rs` the icon and image
//! primitives.

mod boxes;
pub mod icon;
mod image;
mod parts;
mod text;

pub use boxes::{BoxGeom, box_extra, box_geom, paint_box, paint_box_layers, paint_inset_shadows, paint_outline, paint_outline_radius};
pub use icon::{IconKey, content_icon, icon_size, icon_size_sized, is_registered, paint_icon, paint_icon_sized, paint_missing};
pub use image::{fit_rect, paint_image, paint_image_pixels};
pub use parts::{PartItem, layout_parts, measure_parts};
pub use text::{Align, Elide, elide, elide_with, fit_layout, measure_text, paint_runs, paint_text, text_layout, text_style};

use crate::interact::StateBits;
use crate::shim::core::{RecordCx, Resources};
use crate::shim::text::TextShaper;
use skelter_base::color::Color;
use skelter_base::cursor::CursorIcon;
use skelter_base::display::{DisplayList, Prim};
use skelter_base::geom::{Point, Rect};
use std::borrow::Cow;
use svelters_style::{ComputedStyle, ComputedValue, Shadow};

/// `inset 0 1px 2px rgba(0,0,0,.25)`: DESIGN R2's pressed shadow.
pub const PRESSED_INSET: &[Shadow] = &[Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: Some(svelters_style::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 }), inset: true }];

/// A theme shadow (an `elev-*` token) as a CSS one.
fn css_shadow(s: crate::theme::Shadow) -> Shadow {
    let c = s.color;
    Shadow { x: s.x, y: s.y, blur: s.blur, spread: s.spread, color: Some(svelters_style::Color { r: c.r, g: c.g, b: c.b, a: c.a }), inset: false }
}

/// A CSS colour as skelter's.
pub fn color_of(c: svelters_style::Color) -> Color {
    Color::rgba(c.r, c.g, c.b, c.a)
}

/// Where primitives paint: the display list, the resources runs and images register in, the text
/// shaper, and the device scale for snapping.
pub struct PaintCx<'a> {
    pub list: &'a mut DisplayList,
    pub res: &'a mut dyn Resources,
    pub text: &'a mut dyn TextShaper,
    /// Device pixels per point: box edges, borders and outlines snap to device pixels (§3.5).
    pub scale: f32,
}

impl<'a> PaintCx<'a> {
    pub fn new(list: &'a mut DisplayList, res: &'a mut dyn Resources, text: &'a mut dyn TextShaper, scale: f32) -> PaintCx<'a> {
        PaintCx { list, res, text, scale: if scale > 0.0 { scale } else { 1.0 } }
    }

    /// Painting into a record context's list.
    pub fn of<'b>(cx: &'b mut RecordCx<'_>) -> PaintCx<'b> {
        let scale = cx.scale;
        PaintCx::new(&mut *cx.list, &mut *cx.res, &mut *cx.text, scale)
    }

    /// Pushes `p` with its ink bounds.
    pub fn push(&mut self, p: Prim, ink: Rect) {
        self.list.push(p, ink);
    }

    /// `v` points rounded to whole device pixels, at least one if `v > 0` (borders, outlines, carets).
    pub fn snap_width(&self, v: f32) -> f32 {
        if v <= 0.0 {
            return 0.0;
        }
        (v * self.scale).floor().max(1.0) / self.scale
    }

    /// `r` with its edges on the device pixel grid.
    pub fn snap_rect(&self, r: Rect) -> Rect {
        let f = |v: f32| (v * self.scale).round() / self.scale;
        Rect { min: Point::new(f(r.min.x), f(r.min.y)), max: Point::new(f(r.max.x), f(r.max.y)) }
    }

    /// `p` on the device pixel grid.
    pub fn snap_point(&self, p: Point) -> Point {
        Point::new((p.x * self.scale).round() / self.scale, (p.y * self.scale).round() / self.scale)
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
    Color(Color),
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
pub fn ua_theme(dark: bool) -> &'static crate::theme::Theme {
    static DARK: std::sync::OnceLock<crate::theme::Theme> = std::sync::OnceLock::new();
    static LIGHT: std::sync::OnceLock<crate::theme::Theme> = std::sync::OnceLock::new();
    if dark { DARK.get_or_init(crate::theme::Theme::dark_modern) } else { LIGHT.get_or_init(crate::theme::Theme::light_modern) }
}

/// A colour token of the built-in theme of the scheme (what UA tables resolve), for paint-only
/// pieces that have no part style yet (the selection and caret of a value box).
pub fn ua_token(dark: bool, name: &str) -> Option<Color> {
    ua_theme(dark).color(name)
}

// ---- UA tokens through the cascade (SV67) ----

/// A token's computed text parsed as each type a UA table reads it as.
#[derive(Clone, Copy)]
struct ParsedToken {
    color: Option<Color>,
    px: Option<f32>,
    shadow: Option<crate::theme::Shadow>,
}

thread_local! {
    /// Parsed token values by the address of their shared text: every node that inherits a token
    /// unchanged shares one `Arc`, so a document parses each distinct value once. The cache holds
    /// a clone of each `Arc`, so an address is never reused while its entry lives.
    static PARSED: std::cell::RefCell<std::collections::HashMap<usize, (std::sync::Arc<str>, ParsedToken)>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// The token names read while [`record_token_reads`] runs.
    static READS: std::cell::RefCell<Option<Vec<&'static str>>> = const { std::cell::RefCell::new(None) };
    /// `--sv-<name>` for a bare token name, without an allocation per lookup.
    static FULL: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The most distinct token values the parse cache keeps before it starts over.
const PARSED_MAX: usize = 4096;

fn parsed_token(v: &std::sync::Arc<str>) -> ParsedToken {
    let key = std::sync::Arc::as_ptr(v) as *const u8 as usize;
    PARSED.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((_, p)) = c.get(&key) {
            return *p;
        }
        let p = ParsedToken { color: crate::theme::parse_color(v), px: crate::theme::parse_px(v), shadow: crate::theme::parse_shadow(v) };
        if c.len() >= PARSED_MAX {
            c.clear();
        }
        c.insert(key, (v.clone(), p));
        p
    })
}

/// The custom property a UA token reads: `--sv-<name>`, or `name` when it is one already.
fn custom_name_of(name: &str) -> std::borrow::Cow<'_, str> {
    if name.starts_with("--") { name.into() } else { format!("--sv-{name}").into() }
}

/// Runs `f` and returns, with its result, the UA token names the style views read during it, as
/// custom-property names (`--sv-button-bg`), sorted and deduplicated. A kind keeps them to answer
/// which custom-property changes reach its look (SV69). Calls nest: an inner call's reads count for
/// the outer one too.
pub fn record_token_reads<R>(f: impl FnOnce() -> R) -> (R, Vec<String>) {
    let outer = READS.with(|r| r.borrow_mut().replace(Vec::new()));
    let out = f();
    let mine = READS.with(|r| r.borrow_mut().take()).unwrap_or_default();
    if let Some(mut o) = outer {
        o.extend_from_slice(&mine);
        READS.with(|r| *r.borrow_mut() = Some(o));
    }
    let mut names: Vec<String> = mine.iter().map(|n| custom_name_of(n).into_owned()).collect();
    names.sort_unstable();
    names.dedup();
    (out, names)
}

/// A UA token's value on a node: its computed custom property `--sv-<name>` (what the token sheet
/// and any override in the cascade gave it), parsed once per distinct text; `None` when the node
/// has no such property (no token sheet reaches it), and the caller falls back to the theme.
fn css_token(css: Option<&ComputedStyle>, name: &'static str) -> Option<ParsedToken> {
    READS.with(|r| {
        if let Some(v) = r.borrow_mut().as_mut() {
            v.push(name);
        }
    });
    let css = css?;
    let v = if name.starts_with("--") {
        css.custom_shared(name)
    } else {
        FULL.with(|f| {
            let mut f = f.borrow_mut();
            f.clear();
            f.push_str("--sv-");
            f.push_str(name);
            css.custom_shared(&f)
        })
    }?;
    Some(parsed_token(v))
}

/// The properties a part (or child) inherits from its host when nothing sets them on it.
const INHERITED: &[&str] = &["color", "font-size", "font-weight", "font-family", "font-style", "line-height", "letter-spacing", "text-align", "text-transform", "white-space", "cursor", "visibility"];

// ---- the style view ----

/// Where a property's value came from.
#[derive(Clone, Copy)]
enum Got<'a> {
    Css(&'a ComputedValue),
    /// A UA value, the theme it falls back to, and the computed style its tokens read first
    /// (the view's whose table gave it).
    Ua(UaValue, &'a crate::theme::Theme, Option<&'a ComputedStyle>),
}

/// What primitives read: one node's (or part's) computed style, the widget's UA table under it,
/// and the state the UA rules match. Copy, borrowed: build one per paint.
///
/// Resolution of a property: the computed style, then the UA table, then (inherited properties
/// only) the parent view ([`StyleView::with_parent`]: a part's host), then [`Ua::ROOT`].
///
/// A UA token (`Token("button-bg")`) resolves through the cascade first (SV67): the computed
/// `--sv-button-bg` of the view's style (a part without a style of its own reads its host's), so a
/// token sheet and any override in it reach the UA defaults too. Only where no such property is
/// computed (a document without a token sheet, a foreign painter) does it read the typed theme.
///
/// Running CSS transitions apply here: `over` holds the node's (or part's) samples for this frame,
/// which override the computed values.
#[derive(Clone, Copy)]
pub struct StyleView<'a> {
    css: Option<&'a ComputedStyle>,
    /// Running transitions' values at this frame, sorted by property index (empty: none run).
    over: &'a [(usize, ComputedValue)],
    ua: Ua,
    state: StateBits,
    dark: bool,
    /// The theme UA tokens resolve in: the document's (`Theme` service, [`StyleView::with_theme`]),
    /// else the built-in one of the scheme ([`ua_theme`]).
    theme: Option<&'a crate::theme::Theme>,
    weight: Option<u16>,
    parent: Option<&'a StyleView<'a>>,
    /// The parent node's computed style, for a host view (a part compares with `parent`).
    inherited: Option<&'a ComputedStyle>,
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
        StyleView { css, over: &[], ua, state, dark, theme: None, weight: None, parent: None, inherited: None }
    }

    /// With the document's theme (its `Theme` service) for UA tokens and the scheme; `None` keeps
    /// the built-in theme of the scheme.
    pub fn with_theme(mut self, theme: Option<&'a crate::theme::Theme>) -> StyleView<'a> {
        if let Some(t) = theme {
            self.dark = t.is_dark();
        }
        self.theme = theme;
        self
    }

    /// The theme UA tokens resolve in.
    pub fn theme(&self) -> &'a crate::theme::Theme {
        self.theme.unwrap_or_else(|| ua_theme(self.dark))
    }

    /// The UA table alone (no cascade): a foreign painter's styled control.
    pub fn ua_only(ua: Ua, state: StateBits, dark: bool) -> StyleView<'static> {
        StyleView { css: None, over: &[], ua, state, dark, theme: None, weight: None, parent: None, inherited: None }
    }

    /// A part's view: its own computed style (`::part(name)`, `None` until a rule reaches it) and
    /// UA table, inheriting from this view (its host). The part shares the host's state.
    pub fn part(&'a self, css: Option<&'a ComputedStyle>, ua: Ua) -> StyleView<'a> {
        StyleView { css, over: &[], ua, state: self.state, dark: self.dark, theme: self.theme, weight: None, parent: Some(self), inherited: None }
    }

    /// With running transitions' values over the computed style (the node's or part's sample this
    /// frame, sorted by property index).
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

    /// With the parent node's computed style (`Styles::inherited`): an inherited property whose
    /// value equals the parent's was only inherited, so the view's UA declaration beats it (CSS: a
    /// declaration on the element, the UA's included, beats inheritance).
    pub fn with_inherited(mut self, parent: Option<&'a ComputedStyle>) -> StyleView<'a> {
        self.inherited = parent;
        self
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

    /// The computed style UA tokens read: the view's own, else the nearest parent view's.
    fn token_css(&self) -> Option<&'a ComputedStyle> {
        match self.css {
            Some(c) => Some(c),
            None => self.parent.and_then(|p| p.token_css()),
        }
    }

    fn get(&self, name: &str) -> Option<Got<'a>> {
        if let Some(v) = self.css(name) {
            // a computed style always holds the inherited properties (skelter-core computes every
            // part from its host, every node from its parent): one the view only inherited (the
            // same value as its parent's) yields to the view's own UA declaration, as in CSS (a
            // switch knob's `:checked` colour, an icon's 16px, a hyperlink's link colour); a rule
            // that sets another value wins as usual
            let inherited_only = INHERITED.contains(&name)
                && match (self.parent, self.inherited) {
                    (Some(p), _) => p.css(name) == Some(v),
                    (None, Some(c)) => svelters_style::overlay_get(c, &[], name) == Some(v),
                    (None, None) => false,
                };
            if !inherited_only {
                return Some(Got::Css(v));
            }
            if let Some(u) = self.ua.find_own(name, self.state) {
                return Some(Got::Ua(u, self.theme(), self.token_css()));
            }
            return Some(Got::Css(v));
        }
        if let Some(v) = self.ua.find_own(name, self.state) {
            return Some(Got::Ua(v, self.theme(), self.token_css()));
        }
        if let Some(p) = self.parent
            && INHERITED.contains(&name)
        {
            return p.get(name);
        }
        Ua::ROOT.find_own(name, self.state).map(|v| Got::Ua(v, self.theme(), self.token_css()))
    }

    fn ua_px_of(v: UaValue, theme: &crate::theme::Theme, css: Option<&ComputedStyle>) -> Option<f32> {
        match v {
            UaValue::Token(t) => css_token(css, t).and_then(|p| p.px).or_else(|| theme.px(t)),
            UaValue::Px(p) | UaValue::Number(p) => Some(p),
            _ => None,
        }
    }

    /// A colour property (`background-color`, `color`, `border-top-color`, `outline-color`).
    pub fn color(&self, name: &str) -> Option<Color> {
        match self.get(name)? {
            Got::Css(ComputedValue::Color(k)) => Some(color_of(*k)),
            Got::Css(_) => None,
            Got::Ua(UaValue::Token(t), theme, css) => css_token(css, t).and_then(|p| p.color).or_else(|| theme.color(t)),
            Got::Ua(UaValue::Color(c), ..) => Some(c),
            Got::Ua(..) => None,
        }
    }

    /// A length in points (`border-radius`, `padding-left`, `font-size`, `outline-offset`).
    pub fn px(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Got::Css(ComputedValue::Length(l)) => Some(l.px),
            Got::Css(ComputedValue::Number(n)) => Some(*n),
            Got::Css(_) => None,
            Got::Ua(v, theme, css) => Self::ua_px_of(v, theme, css),
        }
    }

    /// A number (`opacity`).
    pub fn number(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Got::Css(ComputedValue::Number(n)) => Some(*n),
            Got::Css(ComputedValue::Int(n)) => Some(*n as f32),
            Got::Css(_) => None,
            Got::Ua(v, theme, css) => Self::ua_px_of(v, theme, css),
        }
    }

    /// A keyword or raw value (`text-align`, `outline-style`, `content`, `background-image`).
    pub fn keyword(&self, name: &str) -> Option<&'a str> {
        match self.get(name)? {
            Got::Css(ComputedValue::Keyword(k)) => Some(k),
            Got::Css(ComputedValue::Raw(r)) => Some(r),
            Got::Css(ComputedValue::String(s)) => Some(s),
            Got::Css(_) => None,
            Got::Ua(UaValue::Keyword(k), ..) => Some(k),
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
        if let Some(v) = self.ua.find_own("font-weight", self.state).and_then(|v| Self::ua_px_of(v, self.theme(), self.token_css())) {
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
            return (!l.is_empty()).then_some(Cow::Borrowed(&**l));
        }
        match self.ua.find_own("box-shadow", self.state)? {
            UaValue::Shadows(l) if !l.is_empty() => Some(Cow::Borrowed(l)),
            UaValue::Token(t) => {
                let s = css_token(self.token_css(), t).and_then(|p| p.shadow).or_else(|| self.theme().shadow(t))?;
                (s != crate::theme::Shadow::NONE).then(|| Cow::Owned(vec![css_shadow(s)]))
            }
            _ => None,
        }
    }

    /// `cursor` as skelter's icon (DESIGN R10), if set.
    pub fn cursor(&self) -> Option<CursorIcon> {
        use CursorIcon as C;
        Some(match self.keyword("cursor")? {
            "pointer" => C::Pointer,
            "text" => C::Text,
            "not-allowed" => C::NotAllowed,
            "grab" => C::Grab,
            "grabbing" => C::Grabbing,
            "col-resize" => C::ColResize,
            "row-resize" => C::RowResize,
            "ew-resize" => C::EwResize,
            "ns-resize" => C::NsResize,
            "crosshair" => C::Crosshair,
            "move" => C::Move,
            "wait" => C::Wait,
            "progress" => C::Progress,
            "help" => C::Help,
            "copy" => C::Copy,
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
    pub fn border_sides(&self) -> [(f32, Color); 4] {
        const NAMES: [(&str, &str); 4] = [("border-left-style", "border-left-color"), ("border-right-style", "border-right-color"), ("border-top-style", "border-top-color"), ("border-bottom-style", "border-bottom-color")];
        let w = self.sides("border-width");
        let one = self.color("border-color");
        let mut out = [(0.0, Color::TRANSPARENT); 4];
        for (k, (style, color)) in NAMES.iter().enumerate() {
            let hidden = matches!(self.keyword(style), Some("none" | "hidden"));
            out[k] = (if hidden { 0.0 } else { w[k] }, self.color(color).or(one).unwrap_or(Color::TRANSPARENT));
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
                _ => all.map(|a| a[k]).or(one).or_else(|| self.ua.find_own(n, self.state).and_then(|v| Self::ua_px_of(v, self.theme(), self.token_css()))).unwrap_or(0.0),
            };
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::css;

    /// A kind's (or part's) own UA declaration beats a value it only inherited; a declared value
    /// still beats the UA table (POL1-STATES).
    #[test]
    fn a_ua_declaration_beats_an_inherited_value() {
        const LINKED: Ua = Ua(&[UaRule::base(&[("color", UaValue::Token("link")), ("font-size", UaValue::Px(16.0))])]);
        let link = ua_token(true, "link").unwrap();
        let parent = css("color: #102030; font-size: 13px");
        let inherited = css("color: #102030; font-size: 13px");
        let declared = css("color: #405060; font-size: 13px");
        let parent_color = StyleView::new(Some(&parent), Ua::NONE, StateBits::NONE, true).color("color").unwrap();

        // a host view: only inherited, so the UA wins; without the parent, CSS wins (as before)
        let host = StyleView::new(Some(&inherited), LINKED, StateBits::NONE, true).with_inherited(Some(&parent));
        assert_eq!(host.color("color"), Some(link));
        assert_eq!(host.px("font-size"), Some(16.0));
        assert_eq!(StyleView::new(Some(&inherited), LINKED, StateBits::NONE, true).color("color"), Some(parent_color));
        // declared on the node: CSS wins
        let own = StyleView::new(Some(&declared), LINKED, StateBits::NONE, true).with_inherited(Some(&parent));
        assert_ne!(own.color("color"), Some(link));
        assert_ne!(own.color("color"), Some(parent_color));
        // a property the UA table doesn't declare keeps the inherited value
        assert_eq!(StyleView::new(Some(&inherited), Ua::NONE, StateBits::NONE, true).with_inherited(Some(&parent)).color("color"), Some(parent_color));

        // a part: compared with its host view
        let host = StyleView::new(Some(&parent), Ua::NONE, StateBits::NONE, true);
        assert_eq!(host.part(Some(&inherited), LINKED).color("color"), Some(link));
        assert_ne!(host.part(Some(&declared), LINKED).color("color"), Some(link));
    }
}
