//! Rich text input: a string, a base style and byte-range spans over it.
//!
//! A [`TextStyle`] splits into what shaping depends on (family, size, weight, slant, spacing,
//! features, line height) and [`TextStyle::paint`], an opaque style id the caller resolves to a colour
//! and decorations at paint time. Shaping caches key on the first part only, so a hover tint or a
//! theme switch changes the paint ids and never reshapes.

use std::hash::{Hash, Hasher};
use std::ops::Range;

/// Horizontal alignment of each line within the layout width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    /// Left for LTR paragraphs, right for RTL ones.
    #[default]
    Start,
    End,
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// A CSS `font-family` list (`"Inter, system-ui, sans-serif"`). Empty = the store's default family.
    pub family: String,
    /// Points.
    pub size: f32,
    /// CSS weight, 1..=1000.
    pub weight: u16,
    pub italic: bool,
    /// CSS `line-height` as a multiple of the size; `None` = the font's normal line height.
    pub line_height: Option<f32>,
    /// Extra advance after each letter, points.
    pub letter_spacing: f32,
    /// Extra advance after each word separator, points.
    pub word_spacing: f32,
    /// CSS `font-feature-settings` (`"\"tnum\" 1, \"liga\" 0"`); empty = the font's defaults.
    pub features: String,
    /// CSS `font-variation-settings` (`"\"wght\" 450"`); empty = none.
    pub variations: String,
    /// Wrap at soft break opportunities (CSS `text-wrap-mode: wrap`); `false` keeps the text on
    /// its hard lines.
    pub wrap: bool,
    /// The caller's paint style (colour, decorations): copied to each [`crate::PlacedRun`] and never
    /// part of a shaping key.
    pub paint: u32,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: String::new(),
            size: 13.0,
            weight: 400,
            italic: false,
            line_height: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            features: String::new(),
            variations: String::new(),
            wrap: true,
            paint: 0,
        }
    }
}

impl TextStyle {
    /// Hashes what shaping and line breaking depend on (everything but [`Self::paint`]).
    pub(crate) fn hash_shaping<H: Hasher>(&self, h: &mut H) {
        self.family.hash(h);
        self.size.to_bits().hash(h);
        self.weight.hash(h);
        self.italic.hash(h);
        self.line_height.map(f32::to_bits).hash(h);
        self.letter_spacing.to_bits().hash(h);
        self.word_spacing.to_bits().hash(h);
        self.features.hash(h);
        self.variations.hash(h);
        self.wrap.hash(h);
    }
}

/// A style over `range` (bytes of [`RichText::text`], on char boundaries). Later spans win where
/// spans overlap.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: TextStyle,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RichText {
    pub text: String,
    pub spans: Vec<Span>,
    pub base: TextStyle,
    pub align: Align,
    /// End the last visible line with "…" when the text is cut (by `max_lines`, or by the width when
    /// the text does not wrap).
    pub ellipsis: bool,
    pub max_lines: Option<u32>,
    /// Tab stop interval as a multiple of the width of a space in the base style (CSS `tab-size`);
    /// `None` = 8.
    pub tab_size: Option<f32>,
}

impl RichText {
    /// Plain text in one style.
    pub fn plain(text: impl Into<String>, style: TextStyle) -> Self {
        RichText { text: text.into(), base: style, ..Default::default() }
    }

    /// The style in effect at each span index: 0 is the base, `i + 1` is `spans[i]`.
    pub(crate) fn style_at(&self, index: u32) -> &TextStyle {
        match index {
            0 => &self.base,
            i => self.spans.get(i as usize - 1).map(|s| &s.style).unwrap_or(&self.base),
        }
    }

    /// The shaping key: text, span ranges and shaping styles, alignment-independent.
    pub(crate) fn shaping_hash(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut h);
        self.base.hash_shaping(&mut h);
        for s in &self.spans {
            s.range.hash(&mut h);
            s.style.hash_shaping(&mut h);
        }
        self.tab_size.map(f32::to_bits).hash(&mut h);
        h.finish()
    }

    /// Hash of the paint ids (the part of a placed layout that changes without reshaping).
    pub(crate) fn paint_hash(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.base.paint.hash(&mut h);
        for s in &self.spans {
            s.style.paint.hash(&mut h);
        }
        h.finish()
    }
}
