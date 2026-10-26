//! The computed style skelter's phases read: the visual properties recording and the compositor need,
//! the text properties measuring needs, each a plain field. Layout properties go straight into the
//! node's `taffy::Style`. Built by the cascade (`style/mod.rs`) from svelters-style's computed values.

use skelter_base::color::{Color, Fill};
use skelter_base::cursor::CursorIcon;
use skelter_base::display::BorderStyle;
use skelter_base::geom::{Affine, Corners, Insets, Point, Vec2};
use smallvec::SmallVec;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

impl Overflow {
    /// Clips its content.
    pub fn clips(self) -> bool {
        !matches!(self, Overflow::Visible)
    }
    /// Is a scroll container on this axis.
    pub fn scrolls(self) -> bool {
        matches!(self, Overflow::Scroll | Overflow::Auto)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShadow {
    pub offset: Vec2,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

/// One CSS `transition` entry for a property skelter animates.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionSpec {
    /// The CSS property name (`opacity`, `transform`, `all`).
    pub property: Arc<str>,
    pub duration: skelter_base::time::Span,
    pub delay: skelter_base::time::Span,
    pub easing: skelter_base::anim::Easing,
}

/// The computed style of one node. Shared (`Arc`) between nodes with equal styles.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedBox {
    // text (inherited)
    pub color: Color,
    pub font_family: Arc<str>,
    pub font_size: f32,
    pub font_weight: u16,
    pub italic: bool,
    /// A multiple of the font size; `None` = the font's normal.
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
    /// `white-space` allows wrapping.
    pub wrap: bool,
    pub text_align: TextAlign,
    pub cursor: Option<CursorIcon>,
    /// `visibility: hidden` (inherited): laid out, not drawn, not hit.
    pub visibility_hidden: bool,
    /// `pointer-events: none` (inherited).
    pub pointer_events_none: bool,
    // box (not inherited)
    pub background: Fill,
    pub border_widths: Insets,
    pub border_color: Color,
    pub border_style: BorderStyle,
    pub radii: Corners,
    pub shadows: SmallVec<[BoxShadow; 1]>,
    pub opacity: f32,
    pub transform: Affine,
    /// `transform-origin` as fractions of the border box (CSS default: the centre).
    pub transform_origin: Point,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub z_index: Option<i32>,
    /// `will-change: transform | opacity`: give the node its own layer.
    pub will_change: bool,
    pub outline_width: f32,
    pub outline_color: Color,
    pub outline_offset: f32,
    pub transitions: SmallVec<[TransitionSpec; 1]>,
}

impl Default for ComputedBox {
    fn default() -> Self {
        ComputedBox {
            color: Color::BLACK,
            font_family: Arc::from("system-ui, sans-serif"),
            font_size: 13.0,
            font_weight: 400,
            italic: false,
            line_height: None,
            letter_spacing: 0.0,
            wrap: true,
            text_align: TextAlign::Start,
            cursor: None,
            visibility_hidden: false,
            pointer_events_none: false,
            background: Fill::Solid(Color::TRANSPARENT),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: BorderStyle::Solid,
            radii: Corners::ZERO,
            shadows: SmallVec::new(),
            opacity: 1.0,
            transform: Affine::IDENTITY,
            transform_origin: Point::new(0.5, 0.5),
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            z_index: None,
            will_change: false,
            outline_width: 0.0,
            outline_color: Color::TRANSPARENT,
            outline_offset: 0.0,
            transitions: SmallVec::new(),
        }
    }
}

impl ComputedBox {
    /// Paints something of its own (background, border, shadow or outline).
    pub fn has_box_paint(&self) -> bool {
        let bg = match &self.background {
            Fill::Solid(c) => c.a > 0.0,
            Fill::Linear { .. } => true,
        };
        let border = self.border_color.a > 0.0
            && (self.border_widths.top > 0.0 || self.border_widths.right > 0.0 || self.border_widths.bottom > 0.0 || self.border_widths.left > 0.0);
        bg || border || !self.shadows.is_empty() || (self.outline_width > 0.0 && self.outline_color.a > 0.0)
    }
    /// The text properties as layout's measure spec.
    pub fn text_spec<'a>(&'a self, text: &'a str) -> skelter_base::text::TextSpec<'a> {
        skelter_base::text::TextSpec {
            text,
            family: &self.font_family,
            size: self.font_size,
            weight: self.font_weight,
            italic: self.italic,
            line_height: self.line_height,
            letter_spacing: self.letter_spacing,
            wrap: self.wrap,
        }
    }
    /// The inherited properties of `self` over the defaults (what a child starts from).
    pub fn inherited(&self) -> ComputedBox {
        ComputedBox {
            color: self.color,
            font_family: self.font_family.clone(),
            font_size: self.font_size,
            font_weight: self.font_weight,
            italic: self.italic,
            line_height: self.line_height,
            letter_spacing: self.letter_spacing,
            wrap: self.wrap,
            text_align: self.text_align,
            cursor: self.cursor,
            visibility_hidden: self.visibility_hidden,
            pointer_events_none: self.pointer_events_none,
            ..ComputedBox::default()
        }
    }
}
