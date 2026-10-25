//! The UI thread's text system: shapes [`RichText`] with parley, breaks and places it into
//! [`TextLayout`]s, and caches every step that does not depend on the paint styles.
//!
//! **Pipeline.** `layout(text, max_width)`:
//! 1. **Shape** (cached): parley's ranged builder over the text, one brush per span index (0 = base,
//!    `i + 1` = `spans[i]`), so paint ids stay out of the shaped result. Tabs get an in-flow inline
//!    box one full tab stop wide, which makes line breaking reserve room for them (parley gives a
//!    tab no width, parley#302).
//! 2. **Break and align** at `max_width` (`break_all_lines` + `align`, no reshaping).
//! 3. **Place** (cached per width): walk each line's clusters in visual order, apply tab stops
//!    (advance to the next multiple of the tab interval from the line's left edge), cut the last
//!    visible line for `max_lines`/ellipsis, and group clusters into [`GlyphRun`]s with paint ids
//!    looked up from the span index. The ellipsis is a "…" shaped once per base style, so it never
//!    reshapes the paragraph (parley#304).
//!
//! **Caches** (architecture §9.3):
//! - shaped paragraphs, keyed by the text's shaping hash ([`RichText`] minus paint ids) and the
//!   font-set generation. An entry used in the current or the previous generation
//!   ([`TextSystem::end_frame`]) is never evicted; older entries go in LRU order once the estimated
//!   bytes exceed the budget ([`TextSystem::set_cache_budget`]);
//! - per shaped paragraph: the last break (width and alignment), its content widths, and the last
//!   three placed layouts keyed by (width, alignment, ellipsis, max lines, paint ids);
//! - the symbol fallback memo: per codepoint of the Common-script symbol blocks (arrows, math,
//!   technical, box drawing, shapes, dingbats), the first family of the per-OS symbol chain that
//!   maps it, appended to the font stack of a text that uses it; fontique itself memoises fallback
//!   per (script, locale). Cleared when the font generation changes.
//!
//! Layout runs at scale 1 in points, unquantised: device-pixel snapping of baselines is the
//! renderer's (it knows the scale).

use crate::fonts::FontStore;
use crate::layout::{ClusterBox, LineInfo, PlacedRun, TextLayout};
use crate::style::{Align, RichText, TextStyle};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontFeatures, FontStyle, FontVariations,
    FontWeight, InlineBox, InlineBoxKind, LayoutContext, LineHeight, RangedBuilder, StyleProperty,
    TextWrapMode,
};
use presto_base::geom::{Point, Size};
use presto_base::text::{AaMode, Glyph, GlyphRun, HintMode, TextMeasure, TextSpec};
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

/// Default shaped-paragraph cache budget (estimated bytes) for entries not used in the last two
/// generations.
pub const DEFAULT_CACHE_BUDGET: usize = 16 << 20;

/// Inline boxes standing in for tabs carry ids from here up (id = base + byte offset).
const TAB_BOX_BASE: u64 = 1 << 48;

/// The default tab interval in spaces (CSS `tab-size: 8`).
const DEFAULT_TAB_SIZE: f32 = 8.0;

/// Placed layouts kept per shaped paragraph.
const PLACED_PER_ENTRY: usize = 3;

#[cfg(target_os = "windows")]
const SYMBOL_CHAIN: &[&str] = &["Segoe UI Symbol", "Segoe UI Emoji", "Cambria Math"];
#[cfg(target_os = "macos")]
const SYMBOL_CHAIN: &[&str] = &["Apple Symbols", "Apple Color Emoji", "STIX Two Math"];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const SYMBOL_CHAIN: &[&str] = &["Noto Sans Symbols 2", "Noto Color Emoji", "Noto Sans Math"];

#[derive(Clone, Copy, PartialEq, Debug)]
struct PlacedKey {
    width: Option<u32>,
    align: Align,
    ellipsis: bool,
    max_lines: Option<u32>,
    paint: u64,
}

struct Shaped {
    layout: parley::Layout<u32>,
    bytes: usize,
    last_used: u64,
    broken: Option<(Option<u32>, Align)>,
    widths: Option<(f32, f32)>,
    placed: Vec<(PlacedKey, Arc<TextLayout>)>,
}

/// Per base style: the width of a space (tab stops) and the shaped ellipsis.
struct Aux {
    space: f32,
    ellipsis: Option<(Arc<GlyphRun>, f32)>,
}

/// Cache counters, for tests and the dev overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextStats {
    pub shaped: u64,
    pub shape_hits: u64,
    pub placed: u64,
    pub place_hits: u64,
    pub evicted: u64,
    pub entries: usize,
    pub bytes: usize,
}

pub struct TextSystem {
    fonts: Arc<FontStore>,
    fcx: FontContext,
    lcx: LayoutContext<u32>,
    shaped: HashMap<u64, Shaped>,
    aux: HashMap<u64, Arc<Aux>>,
    symbols: HashMap<char, Option<&'static str>>,
    font_generation: u64,
    generation: u64,
    bytes: usize,
    budget: usize,
    stats: TextStats,
}

impl TextSystem {
    pub fn new(fonts: Arc<FontStore>) -> Self {
        let fcx = FontContext { collection: fonts.collection(), source_cache: Default::default() };
        TextSystem {
            font_generation: fonts.generation(),
            fonts,
            fcx,
            lcx: LayoutContext::new(),
            shaped: HashMap::new(),
            aux: HashMap::new(),
            symbols: HashMap::new(),
            generation: 0,
            bytes: 0,
            budget: DEFAULT_CACHE_BUDGET,
            stats: TextStats::default(),
        }
    }

    pub fn fonts(&self) -> &Arc<FontStore> {
        &self.fonts
    }

    pub fn set_cache_budget(&mut self, bytes: usize) {
        self.budget = bytes;
        self.evict();
    }

    pub fn stats(&self) -> TextStats {
        TextStats { entries: self.shaped.len(), bytes: self.bytes, ..self.stats }
    }

    /// Ends a cache generation (call once per UI frame): entries unused in this and the previous
    /// generation become evictable.
    pub fn end_frame(&mut self) {
        self.generation += 1;
        self.evict();
    }

    fn evict(&mut self) {
        if self.bytes <= self.budget {
            return;
        }
        let keep_from = self.generation.saturating_sub(1);
        let mut old: Vec<(u64, u64)> =
            self.shaped.iter().filter(|(_, e)| e.last_used < keep_from).map(|(k, e)| (e.last_used, *k)).collect();
        old.sort_unstable();
        for (_, k) in old {
            if self.bytes <= self.budget {
                break;
            }
            if let Some(e) = self.shaped.remove(&k) {
                self.bytes -= e.bytes;
                self.stats.evicted += 1;
            }
        }
    }

    fn check_fonts(&mut self) {
        let g = self.fonts.generation();
        if g != self.font_generation {
            self.font_generation = g;
            self.fcx.collection = self.fonts.collection();
            self.shaped.clear();
            self.aux.clear();
            self.symbols.clear();
            self.bytes = 0;
        }
    }

    /// Lays out `text` at `max_width` (`None` = no wrapping width).
    pub fn layout(&mut self, text: &RichText, max_width: Option<f32>) -> Arc<TextLayout> {
        self.check_fonts();
        let key = text.shaping_hash();
        let pk = PlacedKey {
            width: max_width.map(f32::to_bits),
            align: text.align,
            ellipsis: text.ellipsis,
            max_lines: text.max_lines,
            paint: text.paint_hash(),
        };
        let generation = self.generation;
        if let Some(e) = self.shaped.get_mut(&key) {
            e.last_used = generation;
            if let Some((_, l)) = e.placed.iter().find(|(k, _)| *k == pk) {
                self.stats.place_hits += 1;
                return l.clone();
            }
        }
        let aux = (text.ellipsis || text.text.contains('\t')).then(|| self.aux_for(&text.base));
        self.ensure_shaped(key, text, aux.as_deref());
        let entry = self.shaped.get_mut(&key).expect("shaped above");
        let bk = (pk.width, text.align);
        if entry.broken != Some(bk) {
            entry.layout.break_all_lines(max_width);
            entry.layout.align(alignment(text.align), AlignmentOptions::default());
            entry.broken = Some(bk);
        }
        let placed = Arc::new(place(&entry.layout, text, &self.fonts, aux.as_deref(), max_width));
        if entry.placed.len() >= PLACED_PER_ENTRY {
            entry.placed.remove(0);
        }
        entry.placed.push((pk, placed.clone()));
        self.stats.placed += 1;
        placed
    }

    /// (min-content, max-content) widths of `text`.
    pub fn content_widths(&mut self, text: &RichText) -> (f32, f32) {
        self.check_fonts();
        let key = text.shaping_hash();
        let aux = text.text.contains('\t').then(|| self.aux_for(&text.base));
        self.ensure_shaped(key, text, aux.as_deref());
        let entry = self.shaped.get_mut(&key).expect("shaped above");
        if let Some(w) = entry.widths {
            return w;
        }
        let cw = entry.layout.calculate_content_widths();
        let w = (cw.min, cw.max.max(cw.min));
        entry.widths = Some(w);
        w
    }

    fn ensure_shaped(&mut self, key: u64, text: &RichText, aux: Option<&Aux>) {
        if let Some(e) = self.shaped.get_mut(&key) {
            e.last_used = self.generation;
            self.stats.shape_hits += 1;
            return;
        }
        let extra = self.symbol_families(&text.text);
        let tab = aux.map(|a| a.space * text.tab_size.unwrap_or(DEFAULT_TAB_SIZE)).unwrap_or(0.0);
        let mut layout = parley::Layout::new();
        let default_family = self.fonts.default_family().to_string();
        let mut b = self.lcx.ranged_builder(&mut self.fcx, &text.text, 1.0, false);
        push_style(&mut b, &text.base, 0, None, &default_family, &extra);
        for (i, span) in text.spans.iter().enumerate() {
            let range = span.range.start.min(text.text.len())..span.range.end.min(text.text.len());
            push_style(&mut b, &span.style, i as u32 + 1, Some(range), &default_family, &extra);
        }
        if tab > 0.0 {
            for (i, _) in text.text.match_indices('\t') {
                b.push_inline_box(InlineBox {
                    id: TAB_BOX_BASE + i as u64,
                    kind: InlineBoxKind::InFlow,
                    index: i,
                    width: tab,
                    height: 0.0,
                    baseline: None,
                    vertical_align: Default::default(),
                });
            }
        }
        b.build_into(&mut layout, &text.text);
        let bytes = estimate_bytes(text);
        self.bytes += bytes;
        self.stats.shaped += 1;
        self.shaped.insert(
            key,
            Shaped { layout, bytes, last_used: self.generation, broken: None, widths: None, placed: Vec::new() },
        );
        self.evict();
    }

    /// Families of the symbol chain to append for the Common-script symbols in `text`.
    fn symbol_families(&mut self, text: &str) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for ch in text.chars().filter(|c| is_common_symbol(*c)) {
            let fonts = &self.fonts;
            let found = *self
                .symbols
                .entry(ch)
                .or_insert_with(|| SYMBOL_CHAIN.iter().copied().find(|f| fonts.family_has_char(f, ch)));
            if let Some(f) = found {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
        out
    }

    fn aux_for(&mut self, base: &TextStyle) -> Arc<Aux> {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        base.hash_shaping(&mut h);
        let key = std::hash::Hasher::finish(&h);
        if let Some(a) = self.aux.get(&key) {
            return a.clone();
        }
        let space = {
            let rt = RichText::plain(" ", base.clone());
            let l = self.shape_plain(&rt);
            l.width()
        };
        let ellipsis = {
            let rt = RichText::plain("\u{2026}", base.clone());
            let mut l = self.shape_plain(&rt);
            l.break_all_lines(None);
            let tl = place(&l, &rt, &self.fonts, None, None);
            tl.runs.first().map(|r| (r.run.clone(), r.run.advance))
        };
        let aux = Arc::new(Aux { space, ellipsis });
        self.aux.insert(key, aux.clone());
        aux
    }

    fn shape_plain(&mut self, text: &RichText) -> parley::Layout<u32> {
        let default_family = self.fonts.default_family().to_string();
        let mut layout = parley::Layout::new();
        let mut b = self.lcx.ranged_builder(&mut self.fcx, &text.text, 1.0, false);
        push_style(&mut b, &text.base, 0, None, &default_family, &[]);
        b.build_into(&mut layout, &text.text);
        layout.break_all_lines(None);
        layout
    }
}


fn alignment(a: Align) -> Alignment {
    match a {
        Align::Start => Alignment::Start,
        Align::End => Alignment::End,
        Align::Left => Alignment::Left,
        Align::Center => Alignment::Center,
        Align::Right => Alignment::Right,
        Align::Justify => Alignment::Justify,
    }
}

/// A rough size of a shaped paragraph: parley keeps per-char analysis, per-cluster and per-glyph
/// records, about 64 bytes per text byte for UI text, plus a fixed header.
fn estimate_bytes(text: &RichText) -> usize {
    512 + text.text.len() * 64 + text.spans.len() * 128
}

fn push_style(
    b: &mut RangedBuilder<'_, u32>,
    s: &TextStyle,
    brush: u32,
    range: Option<Range<usize>>,
    default_family: &str,
    extra: &[&str],
) {
    let base = if s.family.trim().is_empty() { default_family } else { s.family.as_str() };
    let family = if extra.is_empty() {
        base.to_string()
    } else {
        let mut f = base.to_string();
        for e in extra {
            f.push_str(", \"");
            f.push_str(e);
            f.push('"');
        }
        f
    };
    let props: [StyleProperty<'_, u32>; 10] = [
        StyleProperty::FontFamily(FontFamily::Source(Cow::Owned(family))),
        StyleProperty::FontSize(s.size),
        StyleProperty::FontWeight(FontWeight::new(s.weight as f32)),
        StyleProperty::FontStyle(if s.italic { FontStyle::Italic } else { FontStyle::Normal }),
        StyleProperty::LineHeight(match s.line_height {
            Some(m) => LineHeight::FontSizeRelative(m),
            None => LineHeight::MetricsRelative(1.0),
        }),
        StyleProperty::LetterSpacing(s.letter_spacing),
        StyleProperty::WordSpacing(s.word_spacing),
        StyleProperty::FontFeatures(FontFeatures::Source(Cow::Owned(s.features.clone()))),
        StyleProperty::FontVariations(FontVariations::Source(Cow::Owned(s.variations.clone()))),
        StyleProperty::TextWrapMode(if s.wrap { TextWrapMode::Wrap } else { TextWrapMode::NoWrap }),
    ];
    for p in props {
        match &range {
            None => b.push_default(p),
            Some(r) => b.push(p, r.clone()),
        }
    }
    match &range {
        None => b.push_default(StyleProperty::Brush(brush)),
        Some(r) => b.push(StyleProperty::Brush(brush), r.clone()),
    }
}

/// The Common-script symbol blocks the fallback memo covers.
fn is_common_symbol(c: char) -> bool {
    matches!(c as u32,
        0x2100..=0x214F   // letterlike symbols
        | 0x2190..=0x23FF // arrows, mathematical operators, miscellaneous technical (⌘ ⌥ ⏎)
        | 0x2400..=0x243F // control pictures
        | 0x2460..=0x27BF // enclosed alphanumerics, box drawing, blocks, shapes, misc symbols, dingbats (✓)
        | 0x27C0..=0x2BFF // math and arrow supplements, misc symbols and arrows
    )
}

/// The identity of one glyph run: clusters with equal keys and adjacent positions share a run.
#[derive(Clone, Copy, PartialEq)]
struct RunKey {
    font: presto_base::ids::FontId,
    size: u32,
    variations: u64,
    embolden: bool,
    skew: i8,
    brush: u32,
}

struct RawCluster {
    range: Range<usize>,
    advance: f32,
    rtl: bool,
    newline: bool,
    tab: bool,
    space: bool,
    glyphs: Vec<parley::layout::Glyph>,
    key: RunKey,
    ascent: f32,
    descent: f32,
}

/// Turns a broken parley layout into a [`TextLayout`].
fn place(
    layout: &parley::Layout<u32>,
    text: &RichText,
    fonts: &FontStore,
    aux: Option<&Aux>,
    max_width: Option<f32>,
) -> TextLayout {
    let rtl = layout.is_rtl();
    let tab = aux.map(|a| a.space * text.tab_size.unwrap_or(DEFAULT_TAB_SIZE)).unwrap_or(0.0);
    let container = max_width.unwrap_or(layout.width());
    let line_count = layout.len();
    let visible = text.max_lines.map(|m| (m.max(1) as usize).min(line_count)).unwrap_or(line_count);
    let mut out = TextLayout { text_len: text.text.len(), rtl, ..Default::default() };
    let dropped = visible < line_count;
    out.truncated = dropped;

    for (li, line) in layout.lines().enumerate().take(visible) {
        let m = *line.metrics();
        let mut raw: Vec<RawCluster> = Vec::new();
        for run in line.runs() {
            let font = fonts.intern(run.font());
            let coords: Vec<i16> = run.normalized_coords().iter().map(|c| c.to_bits()).collect();
            let variations = fonts.intern_coords(&coords);
            let synth = run.synthesis();
            let fm = run.font_metrics();
            for cl in run.visual_clusters() {
                let range = cl.text_range();
                let newline = cl.is_hard_line_break();
                let is_tab = text.text.get(range.clone()) == Some("\t");
                let glyphs = if newline || is_tab { Vec::new() } else { cl.glyphs().collect() };
                raw.push(RawCluster {
                    range,
                    advance: if newline { 0.0 } else { cl.advance() },
                    rtl: cl.is_rtl(),
                    newline,
                    tab: is_tab,
                    space: cl.is_space_or_nbsp(),
                    glyphs,
                    key: RunKey {
                        font,
                        size: run.font_size().to_bits(),
                        variations,
                        embolden: synth.embolden(),
                        skew: synth.skew().unwrap_or(0.0) as i8,
                        brush: cl.style().brush,
                    },
                    ascent: fm.ascent,
                    descent: fm.descent,
                });
            }
        }

        // Cut this line for max_lines or an overflowing width.
        let content_w: f32 = raw.iter().map(|c| if c.tab { tab } else { c.advance }).sum();
        let cut_by_lines = dropped && li + 1 == visible;
        let overflow = max_width.is_some_and(|w| content_w > w + 0.01);
        let ellipsis = if text.ellipsis && (cut_by_lines || overflow) { aux.and_then(|a| a.ellipsis.clone()) } else { None };
        if text.ellipsis && overflow {
            out.truncated = true;
        }
        let mut realign = false;
        if let Some((_, ell_w)) = &ellipsis {
            let limit = max_width.unwrap_or(f32::INFINITY) - ell_w;
            let mut order: Vec<usize> = (0..raw.len()).collect();
            order.sort_by_key(|&i| raw[i].range.start);
            let mut acc = 0.0;
            let mut keep = vec![false; raw.len()];
            for i in order {
                if raw[i].newline {
                    continue;
                }
                let adv = if raw[i].tab { tab } else { raw[i].advance };
                if acc + adv > limit {
                    break;
                }
                acc += adv;
                keep[i] = true;
            }
            let mut k = keep.into_iter();
            raw.retain(|_| k.next().unwrap_or(false));
            realign = true;
        }

        let ell_w = ellipsis.as_ref().map(|e| e.1).unwrap_or(0.0);
        let mut x = if realign {
            let w: f32 = raw.iter().map(|c| if c.tab { tab } else { c.advance }).sum::<f32>() + ell_w;
            let free = (container - w).max(0.0);
            match (text.align, rtl) {
                (Align::Left, _) | (Align::Start | Align::Justify, false) | (Align::End, true) => 0.0,
                (Align::Right, _) | (Align::Start | Align::Justify, true) | (Align::End, false) => free,
                (Align::Center, _) => free * 0.5,
            }
        } else {
            m.offset
        };
        let line_left = x;
        let baseline = m.baseline;
        let first_cluster = out.clusters.len() as u32;

        let mut ellipsis_run = None;
        if let (Some((run, w)), true) = (&ellipsis, rtl) {
            ellipsis_run = Some(PlacedRun { run: run.clone(), origin: Point::new(x, baseline), style: text.base.paint, range: text.text.len()..text.text.len() });
            x += w;
        }

        let mut cur: Option<(RunKey, PlacedRun, f32)> = None;
        for c in &raw {
            let adv = if c.tab {
                let rel = x - line_left;
                ((rel / tab).floor() + 1.0) * tab - rel
            } else {
                c.advance
            };
            out.clusters.push(ClusterBox {
                range: c.range.start as u32..c.range.end as u32,
                x0: x,
                x1: x + adv,
                rtl: c.rtl,
                newline: c.newline,
            });
            if !c.glyphs.is_empty() {
                let continues = matches!(&cur, Some((k, _, _)) if *k == c.key);
                if !continues {
                    if let Some((_, r, _)) = cur.take() {
                        out.runs.push(r);
                    }
                    let style = text.style_at(c.key.brush);
                    let run = GlyphRun {
                        font: c.key.font,
                        size: f32::from_bits(c.key.size),
                        variations: c.key.variations,
                        glyphs: Vec::new(),
                        ascent: c.ascent,
                        descent: c.descent,
                        advance: 0.0,
                        hint: HintMode::Vertical,
                        aa: AaMode::Grayscale,
                        // FreeType's synthetic bold strength: ppem / 24 (ftsynth.c).
                        embolden: if c.key.embolden { f32::from_bits(c.key.size) / 24.0 } else { 0.0 },
                        skew_deg: c.key.skew as f32,
                    };
                    let placed = PlacedRun {
                        run: Arc::new(run),
                        origin: Point::new(x, baseline),
                        style: style.paint,
                        range: c.range.clone(),
                    };
                    cur = Some((c.key, placed, x));
                }
                let (_, placed, origin_x) = cur.as_mut().expect("set above");
                let run = Arc::get_mut(&mut placed.run).expect("unshared while building");
                let mut pen = x - *origin_x;
                for g in &c.glyphs {
                    run.glyphs.push(Glyph { id: g.id, x: pen + g.x, y: g.y });
                    pen += g.advance;
                }
                run.advance = (x + adv) - *origin_x;
                placed.range.start = placed.range.start.min(c.range.start);
                placed.range.end = placed.range.end.max(c.range.end);
            }
            x += adv;
        }
        if let Some((_, r, _)) = cur.take() {
            out.runs.push(r);
        }
        if let (Some((run, w)), false) = (&ellipsis, rtl) {
            ellipsis_run = Some(PlacedRun { run: run.clone(), origin: Point::new(x, baseline), style: text.base.paint, range: text.text.len()..text.text.len() });
            x += w;
        }
        if let Some(r) = ellipsis_run {
            out.runs.push(r);
        }

        let last_cluster = out.clusters.len() as u32;
        // The line's extent: its ink-bearing clusters (hanging spaces and the newline excluded),
        // or all its clusters for a line of spaces.
        let boxes = &out.clusters[first_cluster as usize..];
        let inked: Vec<usize> = (0..raw.len()).filter(|&i| !raw[i].newline && !raw[i].space).collect();
        let pick: Vec<usize> = if inked.is_empty() { (0..raw.len()).filter(|&i| !raw[i].newline).collect() } else { inked };
        let (mut left, mut right) = match pick.first() {
            Some(&i) => (boxes[i].x0, boxes[i].x1),
            None => (line_left, line_left),
        };
        for &i in &pick {
            left = left.min(boxes[i].x0);
            right = right.max(boxes[i].x1);
        }
        if ellipsis.is_some() {
            left = left.min(line_left);
            right = right.max(x);
        }
        out.lines.push(LineInfo {
            range: line.text_range(),
            top: m.block_min_coord,
            bottom: m.block_max_coord,
            baseline,
            left,
            right,
            clusters: first_cluster..last_cluster,
            rtl,
        });
    }

    let w = out.lines.iter().map(|l| l.right - l.left).fold(0.0, f32::max);
    let h = out.lines.last().map(|l| l.bottom).unwrap_or(0.0);
    out.size = Size::new(w, h);
    out
}

fn rich_from_spec(spec: &TextSpec<'_>) -> RichText {
    RichText::plain(
        spec.text,
        TextStyle {
            family: spec.family.to_string(),
            size: spec.size,
            weight: spec.weight,
            italic: spec.italic,
            line_height: spec.line_height,
            letter_spacing: spec.letter_spacing,
            wrap: spec.wrap,
            ..TextStyle::default()
        },
    )
}

impl TextMeasure for TextSystem {
    fn measure(&mut self, spec: &TextSpec<'_>, max_width: Option<f32>) -> Size {
        self.layout(&rich_from_spec(spec), max_width).size()
    }

    fn content_widths(&mut self, spec: &TextSpec<'_>) -> (f32, f32) {
        let rt = rich_from_spec(spec);
        let (min, max) = TextSystem::content_widths(self, &rt);
        if rt.text.contains('\t') {
            // Tab stops make the unwrapped width depend on positions parley does not see.
            let placed = self.layout(&rt, None).size().w;
            return (min.min(placed), placed);
        }
        (min, max)
    }
}
