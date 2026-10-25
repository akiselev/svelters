//! Stand-ins for presto-core and presto-text APIs that lane WIDGETS-TIER1 needs before those crates
//! have them (`plans/cloud/presto-b2/BRIEF.md`: "the smallest thing you need, in your own crate,
//! behind a module named `shim`"). The consolidation replaces each item with the real API:
//!
//! | Item | Replaced by |
//! |---|---|
//! | [`Value`] | `presto_core::Value` (signal arguments and prop values) |
//! | [`Emit`] | the `Dispatched` JS-facing event list of `presto_core::EventCx` (signal id, node, args) |
//! | [`TextStyle`], [`TextPainter`] | `presto_text::RichText` + `TextLayout`, recorded through `presto_core::RecordCx` |
//! | [`Platform`] | presto-core's platform policy (button order, modifiers; svelters-egui `native::Policy`) |
//! | [`fill`], [`frame`], [`line`] | WIDGETS-BASIC's `paint` helpers (box, border, line from computed style) |
//!
//! The tier-1 kinds keep their state machines, geometry and recording in plain structs over these
//! types, so the `Kind` impls are thin glue once `presto_core::Kind` exists.

use presto_base::color::{Color, Fill};
use presto_base::display::{BorderStyle, DisplayList, Line, LineCap, Prim, Quad};
use presto_base::geom::{Corners, Insets, Point, Rect, Size};

/// A prop value or signal argument (`presto_core::Value`'s variants that tier-1 kinds use).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    List(Vec<Value>),
}

/// A JS-facing event a kind raises: the signal's spec name and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Emit {
    pub signal: &'static str,
    pub args: Vec<Value>,
}

impl Emit {
    pub fn new(signal: &'static str, args: Vec<Value>) -> Emit {
        Emit { signal, args }
    }
}

/// Horizontal placement of a text in its box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HAlign {
    #[default]
    Start,
    Center,
    End,
}

/// The style of one text a tier-1 kind records (a label, a cell, a caption).
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// CSS `font-family` list.
    pub family: String,
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
    pub color: Color,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle { family: "system-ui".into(), size: 13.0, weight: 400, italic: false, color: Color::BLACK }
    }
}

impl TextStyle {
    pub fn with_color(&self, color: Color) -> TextStyle {
        TextStyle { color, ..self.clone() }
    }
    pub fn with_weight(&self, weight: u16) -> TextStyle {
        TextStyle { weight, ..self.clone() }
    }
}

/// Shapes and records single-line text (presto-text's `TextSystem::layout` plus `RecordCx`'s run
/// registration, which turns a `PlacedRun` into a `Prim::Glyphs` with a `RunId`).
pub trait TextPainter {
    /// The size of `text` on one line.
    fn measure(&mut self, text: &str, style: &TextStyle) -> Size;
    /// Records `text` into `list`, vertically centred in `rect`, placed by `align`, clipped to
    /// `rect`; `elide` ends an overflowing text with an ellipsis.
    fn draw(&mut self, list: &mut DisplayList, text: &str, style: &TextStyle, rect: Rect, align: HAlign, elide: bool);
}

/// The platform conventions tier-1 kinds follow (part of svelters-egui's `native::Policy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    /// GNOME and other GTK desktops.
    Gnome,
    Kde,
}

impl Platform {
    /// The platform this binary runs on (Linux defaults to GNOME's conventions).
    pub fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Gnome
        }
    }
    pub fn is_mac(self) -> bool {
        self == Platform::MacOs
    }
}

/// Records a filled rounded rect (nothing for a transparent colour).
pub fn fill(list: &mut DisplayList, rect: Rect, color: Color, radius: f32) {
    if color.a <= 0.0 || rect.is_empty() {
        return;
    }
    let q = Quad { rect, radii: Corners::all(radius), fill: Fill::Solid(color), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: BorderStyle::Solid };
    list.push(Prim::Quad(q), rect);
}

/// Records a rounded rect with a fill and a border of `width` inside it.
pub fn frame(list: &mut DisplayList, rect: Rect, fill: Color, border: Color, width: f32, radius: f32) {
    if rect.is_empty() {
        return;
    }
    let q = Quad { rect, radii: Corners::all(radius), fill: Fill::Solid(fill), border_widths: Insets::all(width), border_color: border, border_style: BorderStyle::Solid };
    list.push(Prim::Quad(q), rect);
}

/// Records a straight line.
pub fn line(list: &mut DisplayList, from: Point, to: Point, width: f32, color: Color) {
    if color.a <= 0.0 {
        return;
    }
    let ink = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y)).inflate(width / 2.0);
    list.push(Prim::Line(Line { from, to, width, color, cap: LineCap::Butt, dash: None, wavy: false }), ink);
}

#[cfg(test)]
pub(crate) mod testing {
    //! A deterministic [`TextPainter`] for tests: every char advances `size * 0.5`, the line is
    //! `size * 1.25` high, and a draw records one `Prim::Glyphs` whose `RunId` indexes `drawn`.

    use super::*;
    use presto_base::display::Glyphs;
    use presto_base::ids::RunId;

    #[derive(Default)]
    pub struct FixedText {
        /// Every text drawn: the text (after eliding), its rect and alignment.
        pub drawn: Vec<(String, Rect, HAlign)>,
    }

    impl FixedText {
        pub fn advance(style: &TextStyle) -> f32 {
            style.size * 0.5
        }
        pub fn texts(&self) -> Vec<&str> {
            self.drawn.iter().map(|d| d.0.as_str()).collect()
        }
    }

    impl TextPainter for FixedText {
        fn measure(&mut self, text: &str, style: &TextStyle) -> Size {
            Size::new(text.chars().count() as f32 * Self::advance(style), style.size * 1.25)
        }
        fn draw(&mut self, list: &mut DisplayList, text: &str, style: &TextStyle, rect: Rect, align: HAlign, elide: bool) {
            let adv = Self::advance(style);
            let mut t: String = text.to_string();
            if elide && t.chars().count() as f32 * adv > rect.width() {
                let fit = ((rect.width() / adv).floor() as usize).saturating_sub(1);
                t = t.chars().take(fit).collect::<String>() + "…";
            }
            let w = t.chars().count() as f32 * adv;
            let x = match align {
                HAlign::Start => rect.min.x,
                HAlign::Center => rect.min.x + (rect.width() - w) / 2.0,
                HAlign::End => rect.max.x - w,
            };
            let origin = Point::new(x, rect.min.y + (rect.height() + style.size) / 2.0);
            let id = RunId(self.drawn.len() as u64);
            list.push(Prim::Glyphs(Glyphs { run: id, origin, color: style.color }), rect);
            self.drawn.push((t, rect, align));
        }
    }
}
