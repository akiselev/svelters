//! Inline rich text (WIDGET-PARITY Q4, ported from svelters-egui `rich.rs`): a `label`'s paragraph
//! of runs, each with its own format, laid out as one presto-text [`RichText`] with a span per run
//! and painted with a colour per style index (so a hover or colour change never reshapes).
//!
//! The runs are the label's `runs` prop: a list of strings and records
//! `{text, b?, i?, code?, href?, color?}` (svelters-egui's plain run list, `rich::from_value`, plus
//! `color`, which stands in for a run node's computed `color`). Each record's flags give the tag
//! defaults: `b` 700, `i` italic, `code` the theme's monospace family; a run with an `href` field is
//! a link (the `link` colour, underlined while the pointer is on it: DESIGN R18). No markup is
//! parsed; unknown or mistyped fields are errors.
//!
//! Placed runs carry their span's style index (0: the base, `i + 1`: run `i`), which gives each
//! glyph run its colour and decorations at record time and maps a point on a run back to its link
//! ([`link_at`]).

use crate::paint::PaintCx;
use crate::shim::core::Value;
use crate::shim::text::{RichText, Span, TextLayout, TextStyle};
use presto_base::color::Color;
use presto_base::display::{Line, LineCap, Prim};
use presto_base::geom::{Point, Rect};
use std::sync::Arc;

/// The look of one run.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunFmt {
    /// `b`: weight 700 (or the label's, if heavier).
    pub bold: bool,
    pub italic: bool,
    /// `code`: the theme's monospace family.
    pub mono: bool,
    /// `None`: the label's text colour (or the link colour on a link).
    pub color: Option<Color>,
}

/// One section of the paragraph: `plain[start..end]` drawn with `fmt`.
#[derive(Clone, Debug, PartialEq)]
pub struct RunSeg {
    pub start: usize,
    pub end: usize,
    pub fmt: RunFmt,
    /// Its index in the `runs` prop (`None`: the label's own `text`, the first run).
    pub item: Option<usize>,
    /// A link run's target (`Some("")`: a link without one, which only signals).
    pub href: Option<String>,
}

impl RunSeg {
    pub fn is_link(&self) -> bool {
        self.href.is_some()
    }
}

/// A label's runs: its whole text and the sections over it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rich {
    pub plain: String,
    pub runs: Vec<RunSeg>,
}

impl Rich {
    /// Whether any run is a link.
    pub fn has_links(&self) -> bool {
        self.runs.iter().any(RunSeg::is_link)
    }
}

/// The paragraph of a label whose own text is `lead` (the first run, when not empty) followed by
/// `items` (the `runs` prop): strings, or records `{text, b, i, code, href, color}`.
pub fn from_value(lead: &str, items: &[Value]) -> Result<Rich, String> {
    let mut rich = Rich::default();
    let mut push = |text: &str, fmt: RunFmt, item: Option<usize>, href: Option<String>| {
        let start = rich.plain.len();
        rich.plain.push_str(text);
        rich.runs.push(RunSeg { start, end: rich.plain.len(), fmt, item, href });
    };
    if !lead.is_empty() {
        push(lead, RunFmt::default(), None, None);
    }
    for (k, it) in items.iter().enumerate() {
        match it {
            Value::Str(s) => push(s, RunFmt::default(), Some(k), None),
            Value::Record(fields) => {
                let (mut text, mut f, mut href) = (String::new(), RunFmt::default(), None);
                for (key, val) in fields {
                    match (&**key, val) {
                        ("text", Value::Str(s)) => text = s.to_string(),
                        ("b", Value::Bool(on)) => f.bold = *on,
                        ("i", Value::Bool(on)) => f.italic = *on,
                        ("code", Value::Bool(on)) => f.mono = *on,
                        ("href", Value::Str(s)) => href = Some(s.to_string()),
                        ("color", Value::Color(c)) => f.color = Some(*c),
                        (other, _) => return Err(format!("run {k}: unknown or mistyped field `{other}` (text, href: string; b, i, code: bool; color: colour)")),
                    }
                }
                push(&text, f, Some(k), href);
            }
            other => return Err(format!("run {k}: a string or {{text, b, i, code, href, color}}, not {other:?}")),
        }
    }
    Ok(rich)
}

/// The style of run `r` over the label's `base` (`mono`: the monospace family).
fn run_style(base: &TextStyle, mono: &str, r: &RunSeg) -> TextStyle {
    let mut s = base.clone();
    if r.fmt.bold {
        s.weight = s.weight.max(700);
    }
    if r.fmt.italic {
        s.italic = true;
    }
    if r.fmt.mono {
        s.family = Arc::from(mono);
    }
    s
}

/// The paragraph as presto-text input: `base` for the whole text, a span per run (`spans[i]` is
/// `rich.runs[i]`, style index `i + 1`).
pub fn rich_text(rich: &Rich, base: &TextStyle, mono: &str) -> RichText {
    let spans = rich.runs.iter().map(|r| Span { range: r.start..r.end, style: run_style(base, mono, r) }).collect();
    RichText { text: rich.plain.clone(), spans, base: base.clone(), ..RichText::default() }
}

/// The colours runs fall back to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub text: Color,
    pub link: Color,
}

/// The colour of style index `style` (0: the base) of `rich`.
pub fn color(rich: Option<&Rich>, colors: Colors, style: u32) -> Color {
    let run = style.checked_sub(1).and_then(|i| rich?.runs.get(i as usize));
    match run {
        Some(r) => r.fmt.color.unwrap_or(if r.is_link() { colors.link } else { colors.text }),
        None => colors.text,
    }
}

/// The rect a placed run covers (its advance, from ascent to descent) with the layout at `pos`.
fn run_rect(pos: Point, origin: Point, ascent: f32, descent: f32, advance: f32) -> Rect {
    let o = pos + origin.to_vec();
    Rect::new(o.x, o.y - ascent, o.x + advance, o.y + descent)
}

/// The link run (index into `rich.runs`) whose glyphs are under `p`, with `layout` (laid out from
/// [`rich_text`] of `rich`) at `pos`.
pub fn link_at(rich: &Rich, layout: &TextLayout, pos: Point, p: Point) -> Option<usize> {
    layout.runs().iter().find_map(|pr| {
        let i = pr.style.checked_sub(1)? as usize;
        let hit = run_rect(pos, pr.origin, pr.run.ascent, pr.run.descent, pr.run.advance).contains(p);
        (hit && rich.runs.get(i)?.is_link()).then_some(i)
    })
}

/// `y` on the pixel grid for a horizontal stroke `w` points wide (an odd number of device pixels
/// centres on a pixel, as epaint's `round_center_to_pixel`).
fn center_to_pixel(y: f32, w: f32, scale: f32) -> f32 {
    let px = (w * scale).round() as i32;
    if px % 2 == 1 { ((y * scale).floor() + 0.5) / scale } else { (y * scale).round() / scale }
}

/// The decorations of `layout` (laid out from `text`) at `pos`, as epaint draws them: an underline
/// at the bottom of each run whose style has `underline` (or `lit(style)`), a strikethrough at its
/// middle, 1 point wide in the run's `color(style)`.
pub fn paint_decorations(cx: &mut PaintCx, text: &RichText, layout: &TextLayout, pos: Point, color: &dyn Fn(u32) -> Color, lit: &dyn Fn(u32) -> bool) {
    let w = cx.snap_width(1.0);
    for pr in layout.runs() {
        if pr.run.glyphs.is_empty() || pr.run.advance <= 0.0 {
            continue;
        }
        let style = match pr.style.checked_sub(1) {
            Some(i) => text.spans.get(i as usize).map_or(&text.base, |s| &s.style),
            None => &text.base,
        };
        let c = color(pr.style);
        if c.a <= 0.0 {
            continue;
        }
        let r = run_rect(pos, pr.origin, pr.run.ascent, pr.run.descent, pr.run.advance);
        let mut ys = Vec::new();
        if style.underline || lit(pr.style) {
            ys.push(r.max.y);
        }
        if style.strikethrough {
            ys.push((r.min.y + r.max.y) / 2.0);
        }
        for y in ys {
            let y = center_to_pixel(y, w, cx.scale);
            let line = Line { from: Point::new(r.min.x, y), to: Point::new(r.max.x, y), width: w, color: c, cap: LineCap::Butt, dash: None, wavy: false };
            cx.push(Prim::Line(line), Rect::new(r.min.x, y - w / 2.0, r.max.x, y + w / 2.0));
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! A span-aware monospace shaper (the metrics of [`MonoShaper`], one placed run per span per
    //! line with the span's style index, `…` on the last line when `ellipsis` cuts it) and record
    //! and prepare helpers over it, for the rich label tests.

    use crate::interact::StateBits;
    use crate::shim::core::{Kind, PrepareCx, RecordCx, Styles};
    use crate::shim::testing::{MemResources, MonoShaper};
    use crate::shim::text::{PlacedRun, RichText, TextLayout, TextShaper};
    use presto_base::display::DisplayList;
    use presto_base::geom::{Point, Size};
    use presto_base::ids::FontId;
    use presto_base::text::{AaMode, Glyph, GlyphRun, HintMode};
    use std::sync::Arc;

    #[derive(Default)]
    pub struct SpanShaper;

    impl TextShaper for SpanShaper {
        fn layout(&mut self, t: &RichText, max_width: Option<f32>) -> Arc<TextLayout> {
            let size = t.base.size;
            let adv = MonoShaper::advance(size);
            let line_h = t.base.line_height.unwrap_or((size * 1.25).round());
            let per_line = max_width.map(|w| ((w / adv).floor() as usize).max(1));
            let mut lines: Vec<Vec<(usize, char)>> = Vec::new();
            let mut at = 0;
            for para in t.text.split('\n') {
                let chars: Vec<(usize, char)> = para.char_indices().map(|(b, c)| (at + b, c)).collect();
                at += para.len() + 1;
                match per_line {
                    Some(n) if chars.len() > n => lines.extend(chars.chunks(n).map(<[_]>::to_vec)),
                    _ => lines.push(chars),
                }
            }
            if let Some(m) = t.max_lines {
                let m = m.max(1) as usize;
                if lines.len() > m {
                    lines.truncate(m);
                    if t.ellipsis
                        && let Some(last) = lines.last_mut().and_then(|l| l.last_mut())
                    {
                        last.1 = '…';
                    }
                }
            }
            let style_of = |b: usize| t.spans.iter().position(|s| s.range.contains(&b)).map_or(0, |i| i as u32 + 1);
            let run = |glyphs: Vec<Glyph>, size: f32, advance: f32| GlyphRun { font: FontId(0), size, variations: 0, glyphs, ascent: size * 0.8, descent: size * 0.2, advance, hint: HintMode::Vertical, aa: AaMode::Grayscale, embolden: 0.0, skew_deg: 0.0 };
            let mut runs = Vec::new();
            let mut width: f32 = 0.0;
            for (i, line) in lines.iter().enumerate() {
                let baseline = i as f32 * line_h + (line_h - size) / 2.0 + size * 0.8;
                if line.is_empty() {
                    runs.push(PlacedRun { run: Arc::new(run(Vec::new(), size, 0.0)), origin: Point::new(0.0, baseline), style: 0 });
                    continue;
                }
                let (mut x, mut k) = (0.0, 0);
                while k < line.len() {
                    let st = style_of(line[k].0);
                    let mut j = k;
                    while j < line.len() && style_of(line[j].0) == st {
                        j += 1;
                    }
                    let sz = if st == 0 { size } else { t.spans[st as usize - 1].style.size };
                    let a = MonoShaper::advance(sz);
                    let glyphs: Vec<Glyph> = line[k..j].iter().enumerate().map(|(n, (_, c))| Glyph { id: *c as u32, x: n as f32 * a, y: 0.0 }).collect();
                    let advance = glyphs.len() as f32 * a;
                    runs.push(PlacedRun { run: Arc::new(run(glyphs, sz, advance)), origin: Point::new(x, baseline), style: st });
                    x += advance;
                    k = j;
                }
                width = width.max(x);
            }
            Arc::new(TextLayout { size: Size::new(width, line_h * lines.len() as f32), runs, lines: lines.len() as u32 })
        }
    }

    /// `testkit::record` with the span-aware shaper.
    pub fn record(k: &dyn Kind, size: Size, state: StateBits) -> (DisplayList, MemResources) {
        let mut list = DisplayList::new();
        let mut res = MemResources::default();
        let mut text = SpanShaper;
        let mut cx = RecordCx { size, style: Styles { computed: None, parts: &[], children: &[], state, dark: true }, list: &mut list, text: &mut text, res: &mut res, scale: 1.0 };
        k.record(&mut cx);
        (list, res)
    }

    /// Runs the kind's `prepare` at `size` (what presto-core does after layout).
    pub fn prepare(k: &mut dyn Kind, size: Size, state: StateBits) {
        let mut text = SpanShaper;
        let mut cx = PrepareCx { size, style: Styles { computed: None, parts: &[], children: &[], state, dark: true }, text: &mut text };
        k.prepare(&mut cx);
    }
}

#[cfg(test)]
mod tests {
    use super::testing::SpanShaper;
    use super::*;
    use crate::shim::text::TextShaper;

    fn rec(fields: &[(&str, Value)]) -> Value {
        Value::Record(fields.iter().map(|(k, v)| (Arc::from(*k), v.clone())).collect())
    }

    #[test]
    fn a_plain_run_list() {
        let r = from_value("", &[Value::str("a"), rec(&[("text", Value::str("b")), ("b", Value::Bool(true)), ("href", Value::str("x"))])]).unwrap();
        assert_eq!(r.plain, "ab");
        assert_eq!((r.runs[1].fmt.bold, r.runs[1].href.as_deref()), (true, Some("x")));
        assert_eq!((r.runs[0].item, r.runs[1].item), (Some(0), Some(1)));
        assert!(from_value("", &[rec(&[("color", Value::str("red"))])]).is_err(), "unknown or mistyped fields are errors, not markup");
        assert!(from_value("", &[rec(&[("u", Value::Bool(true))])]).is_err());
        assert!(from_value("", &[Value::I64(3)]).is_err());
    }

    #[test]
    fn the_label_text_leads() {
        let r = from_value("Hello ", &[rec(&[("text", Value::str("world")), ("i", Value::Bool(true)), ("color", Value::Color(Color::WHITE))])]).unwrap();
        assert_eq!(r.plain, "Hello world");
        assert_eq!((r.runs[0].start, r.runs[0].end, r.runs[0].item), (0, 6, None));
        assert_eq!((r.runs[1].start, r.runs[1].end), (6, 11));
        assert!(r.runs[1].fmt.italic && r.runs[1].fmt.color == Some(Color::WHITE));
        assert!(!r.has_links());
    }

    #[test]
    fn a_span_per_run_with_the_tag_defaults() {
        let r = from_value("", &[rec(&[("text", Value::str("b")), ("b", Value::Bool(true))]), rec(&[("text", Value::str("i")), ("i", Value::Bool(true))]), rec(&[("text", Value::str("c")), ("code", Value::Bool(true))])]).unwrap();
        let base = TextStyle { weight: 400, ..TextStyle::default() };
        let t = rich_text(&r, &base, "mono-family");
        assert_eq!(t.spans.len(), 3);
        assert_eq!(t.spans[0].range, 0..1);
        assert_eq!(t.spans[0].style.weight, 700);
        assert!(t.spans[1].style.italic && t.spans[1].style.weight == 400);
        assert_eq!(&*t.spans[2].style.family, "mono-family");
        assert_eq!(t.base, base);
    }

    #[test]
    fn colours_by_style_index() {
        let r = from_value("x", &[rec(&[("text", Value::str("y")), ("href", Value::str(""))]), rec(&[("text", Value::str("z")), ("color", Value::Color(Color::WHITE))])]).unwrap();
        let c = Colors { text: Color::BLACK, link: Color::rgba(0.0, 0.0, 1.0, 1.0) };
        assert_eq!(color(Some(&r), c, 0), Color::BLACK, "the base");
        assert_eq!(color(Some(&r), c, 1), Color::BLACK, "the label's own text");
        assert_eq!(color(Some(&r), c, 2), c.link, "a link");
        assert_eq!(color(Some(&r), c, 3), Color::WHITE, "its own colour");
        assert_eq!(color(None, c, 0), Color::BLACK);
    }

    #[test]
    fn a_point_on_a_link_run_finds_it() {
        let r = from_value("see ", &[rec(&[("text", Value::str("docs")), ("href", Value::str("https://x"))])]).unwrap();
        let t = rich_text(&r, &TextStyle::default(), "mono");
        let l = SpanShaper.layout(&t, None);
        assert_eq!(l.runs().iter().map(|p| p.style).collect::<Vec<_>>(), [1, 2], "a placed run per span");
        let adv = crate::shim::testing::MonoShaper::advance(13.0);
        let y = l.runs()[0].origin.y - 1.0;
        let pos = Point::new(10.0, 0.0);
        assert_eq!(link_at(&r, &l, pos, Point::new(10.0 + 5.0 * adv, y)), Some(1));
        assert_eq!(link_at(&r, &l, pos, Point::new(10.0 + 1.0 * adv, y)), None, "the plain run");
        assert_eq!(link_at(&r, &l, pos, Point::new(10.0 + 9.0 * adv, y)), None, "past the text");
    }

    #[test]
    fn pixel_centred_strokes() {
        assert_eq!(center_to_pixel(10.2, 1.0, 1.0), 10.5);
        assert_eq!(center_to_pixel(10.6, 2.0, 1.0), 11.0);
    }
}
