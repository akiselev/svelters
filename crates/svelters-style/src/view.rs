//! [`StyleView`]: what paint reads (ARCH-WIDGETS §1.1, §3.4). A node's or part's computed style
//! with its running transitions applied, plus typed getters for the paint primitives: per-side
//! borders, corner radii, the outline, text settings, the cursor and the background gradient.
//!
//! A view with no transitions is `StyleView::new(&style)`; [`Transitions::view`](crate::Transitions::view)
//! gives one with the store's interpolated values overlaid.

use crate::computed::ComputedStyle;
use crate::values::{Color, ComputedValue, Content, LinearGradient, Shadow, Transition};
use smallvec::SmallVec;

/// A box side, in CSS order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

    fn name(self) -> &'static str {
        match self {
            Side::Top => "top",
            Side::Right => "right",
            Side::Bottom => "bottom",
            Side::Left => "left",
        }
    }
}

/// One side of the border, resolved for painting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderSide {
    /// In points; 0 when the style is `none` or `hidden`.
    pub width: f32,
    /// A [`LINE_STYLES`](crate::LINE_STYLES) keyword. Unset is `solid` (svelters: CS5g sheets set a
    /// width and a colour only).
    pub style: &'static str,
    /// `None`: not set (the primitive uses the text colour, as `currentcolor`).
    pub color: Option<Color>,
}

/// The outline, resolved for painting. It sits outside the border box by `offset`, never affects
/// layout or hit-testing, and grows paint bounds by `width + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    /// In points; 0 when the style is `none` (the initial value).
    pub width: f32,
    /// `none`, `auto` (painted as `solid`) or a line style.
    pub style: &'static str,
    /// `None`: not set (the primitive uses the text colour).
    pub color: Option<Color>,
    /// In points; may be negative (inside the border box).
    pub offset: f32,
}

/// Text settings beyond font and colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSettings {
    /// `none`, `uppercase`, `lowercase` or `capitalize`.
    pub transform: &'static str,
    /// Extra advance after each character, in points (`normal` = 0).
    pub letter_spacing: f32,
    /// `clip` or `ellipsis`.
    pub overflow: &'static str,
}

impl TextSettings {
    /// Applies `text-transform` to `text`.
    pub fn transform<'t>(&self, text: &'t str) -> std::borrow::Cow<'t, str> {
        match self.transform {
            "uppercase" => text.to_uppercase().into(),
            "lowercase" => text.to_lowercase().into(),
            "capitalize" => {
                let mut out = String::with_capacity(text.len());
                let mut start = true;
                for c in text.chars() {
                    if start && c.is_alphabetic() {
                        out.extend(c.to_uppercase());
                    } else {
                        out.push(c);
                    }
                    start = c.is_whitespace() || (start && !c.is_alphanumeric());
                }
                out.into()
            }
            _ => text.into(),
        }
    }
}

/// A computed style with overridden values (running transitions).
#[derive(Clone, Debug)]
pub struct StyleView<'a> {
    base: &'a ComputedStyle,
    /// `(property index, value)`, sorted by index.
    over: SmallVec<[(usize, ComputedValue); 4]>,
}

impl<'a> StyleView<'a> {
    /// The style as computed (no transitions).
    pub fn new(base: &'a ComputedStyle) -> StyleView<'a> {
        StyleView { base, over: SmallVec::new() }
    }

    pub(crate) fn with(base: &'a ComputedStyle, mut over: SmallVec<[(usize, ComputedValue); 4]>) -> StyleView<'a> {
        over.sort_by_key(|(i, _)| *i);
        StyleView { base, over }
    }

    /// The computed style under the overlay.
    pub fn base(&self) -> &'a ComputedStyle {
        self.base
    }

    /// Whether any value is a transition's (not the computed value).
    pub fn is_animated(&self) -> bool {
        !self.over.is_empty()
    }

    pub fn get_index(&self, index: usize) -> Option<&ComputedValue> {
        match self.over.binary_search_by_key(&index, |(i, _)| *i) {
            Ok(k) => Some(&self.over[k].1),
            Err(_) => self.base.get_index(index),
        }
    }

    pub fn get(&self, name: &str) -> Option<&ComputedValue> {
        self.base.table().index_of(name).and_then(|i| self.get_index(i))
    }

    pub fn color(&self, name: &str) -> Option<Color> {
        match self.get(name) {
            Some(ComputedValue::Color(c)) => Some(*c),
            _ => None,
        }
    }

    /// A length's px part (percentages are layout's).
    pub fn px(&self, name: &str) -> Option<f32> {
        match self.get(name) {
            Some(ComputedValue::Length(l)) => Some(l.px),
            _ => None,
        }
    }

    pub fn number(&self, name: &str) -> Option<f32> {
        match self.get(name) {
            Some(ComputedValue::Number(n)) => Some(*n),
            _ => None,
        }
    }

    pub fn keyword(&self, name: &str) -> Option<&'static str> {
        match self.get(name) {
            Some(ComputedValue::Keyword(k)) => Some(k),
            _ => None,
        }
    }

    /// `box-shadow` layers (empty: none).
    pub fn shadows(&self) -> &[Shadow] {
        match self.get("box-shadow") {
            Some(ComputedValue::Shadows(s)) => s,
            _ => &[],
        }
    }

    /// `opacity`, 1 when unset.
    pub fn opacity(&self) -> f32 {
        self.number("opacity").unwrap_or(1.0).clamp(0.0, 1.0)
    }

    /// `background-color`.
    pub fn background(&self) -> Option<Color> {
        self.color("background-color")
    }

    /// `background-image: linear-gradient(...)`.
    pub fn background_image(&self) -> Option<&LinearGradient> {
        match self.get("background-image") {
            Some(ComputedValue::Image(Some(g))) => Some(g),
            _ => None,
        }
    }

    /// `background-image: paint(name)`: the Rust part painter to draw the background with.
    pub fn background_painter(&self) -> Option<&str> {
        match self.get("background-image") {
            Some(ComputedValue::Painter(n)) => Some(n),
            _ => None,
        }
    }

    /// `content` of a glyph part (P2); unset is `None` (the widget's default glyph, if any).
    pub fn content(&self) -> Option<&Content> {
        match self.get("content") {
            Some(ComputedValue::Content(c)) => Some(c),
            _ => None,
        }
    }

    /// One border side: its longhands, else the uniform `border-width`/`border-color`.
    pub fn border_side(&self, side: Side) -> BorderSide {
        let n = side.name();
        let style = self.keyword(&format!("border-{n}-style")).unwrap_or("solid");
        let width = self.px(&format!("border-{n}-width")).or_else(|| self.px("border-width")).unwrap_or(0.0);
        let color = self.color(&format!("border-{n}-color")).or_else(|| self.color("border-color"));
        let width = if matches!(style, "none" | "hidden") { 0.0 } else { width.max(0.0) };
        BorderSide { width, style, color }
    }

    /// All four sides, top, right, bottom, left.
    pub fn border(&self) -> [BorderSide; 4] {
        Side::ALL.map(|s| self.border_side(s))
    }

    /// Whether every side has the same width, style and colour (one `RectShape` stroke).
    pub fn border_is_uniform(&self) -> bool {
        let b = self.border();
        b.iter().all(|s| *s == b[0])
    }

    /// Corner radii: top-left, top-right, bottom-right, bottom-left (corner longhands, else
    /// `border-radius`, else 0).
    pub fn radii(&self) -> [f32; 4] {
        let uniform = self.px("border-radius").unwrap_or(0.0);
        ["border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius"].map(|n| self.px(n).unwrap_or(uniform).max(0.0))
    }

    pub fn outline(&self) -> Outline {
        let style = self.keyword("outline-style").unwrap_or("none");
        let width = if style == "none" { 0.0 } else { self.px("outline-width").unwrap_or(3.0).max(0.0) };
        Outline { width, style, color: self.color("outline-color"), offset: self.px("outline-offset").unwrap_or(0.0) }
    }

    pub fn text(&self) -> TextSettings {
        TextSettings {
            transform: self.keyword("text-transform").unwrap_or("none"),
            letter_spacing: self.px("letter-spacing").unwrap_or(0.0),
            overflow: self.keyword("text-overflow").unwrap_or("clip"),
        }
    }

    /// `cursor` (`auto` and unset: `None`, the widget's default).
    pub fn cursor(&self) -> Option<&'static str> {
        self.keyword("cursor").filter(|c| *c != "auto")
    }

    /// The `transition` list.
    pub fn transitions(&self) -> &[Transition] {
        match self.base.get("transition") {
            Some(ComputedValue::Transitions(t)) => t,
            _ => &[],
        }
    }
}
