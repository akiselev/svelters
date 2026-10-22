//! RT2 inline rich text (plans/milestones/RT2-rich-text.md): a `label` whose children are runs
//! (`span`, `b`, `i`, `code`, `a`; `kinds/rich.rs`) is one paragraph, laid out as one egui
//! [`LayoutJob`] with a section per run and painted as one galley.
//!
//! - [`collect`] walks the label's run subtree in order and gives each run its format from its
//!   computed style (`color`, `background-color`, `font-size`, `font-style`, `font-family`,
//!   `text-decoration-line`) and its `font-weight` prop, inheriting what the run does not set from the
//!   enclosing run. Without a style engine (no sheets) the tag defaults apply: `b` 700, `i` italic,
//!   `code` monospace, `a` the link colour.
//! - The label keeps the result in `W::Label::rich` for measuring ([`refresh`], run whenever the
//!   label is measured) and for its text (`text_of`: all runs, the AccessKit value). A run's text,
//!   weight or style change, or a run inserted or removed, remeasures the label ([`touched`]).
//! - [`paint`] collects again (this frame's hover and focus styles), paints the galley, and gives
//!   every `a` run an egui interaction at its glyphs under the run's own id: hover, click, keyboard
//!   focus (Tab; Enter or Space activates) and an AccessKit `Link` node. The run's `:hover` and
//!   `:focus` states follow, so `label a:hover` rules apply.
//!
//! This is the seam OW1a's own label uses: [`layout_job`] gives the job its `paint/text` primitive
//! lays out instead of a single-format text.

use crate::scene::{NodeId, Scene, StateBits, W};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Rect, Stroke};
use svelters_style::{ComputedValue, StyleView};

/// The look of one run, after inheritance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunFmt {
    /// CSS weight (100-900).
    pub weight: i64,
    pub italic: bool,
    pub mono: bool,
    /// Points; `None`: the label's size.
    pub size: Option<f32>,
    /// `None`: the label's text colour (or the link colour on a link).
    pub color: Option<Color32>,
    pub background: Option<Color32>,
    pub underline: bool,
    pub strike: bool,
}

/// One section of the paragraph: `plain[start..end]` drawn with `fmt`.
#[derive(Clone, Debug, PartialEq)]
pub struct RunSeg {
    /// The run node the text belongs to (`None`: a plain run, [`from_value`]).
    pub node: Option<NodeId>,
    pub start: usize,
    pub end: usize,
    pub fmt: RunFmt,
    /// The `a` run this text is inside, if any.
    pub link: Option<NodeId>,
    /// A plain run's link target ([`from_value`]); node runs keep theirs on the `a` node.
    pub href: Option<String>,
}

/// A label's runs: its whole text and the sections over it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rich {
    pub plain: String,
    pub runs: Vec<RunSeg>,
}

impl Rich {
    /// The link nodes in paragraph order, each with its byte ranges.
    pub fn links(&self) -> Vec<(NodeId, Vec<(usize, usize)>)> {
        let mut out: Vec<(NodeId, Vec<(usize, usize)>)> = Vec::new();
        for r in &self.runs {
            let Some(l) = r.link else { continue };
            match out.iter_mut().find(|(n, _)| *n == l) {
                Some((_, v)) => v.push((r.start, r.end)),
                None => out.push((l, vec![(r.start, r.end)])),
            }
        }
        out
    }
}

/// The tag of a run kind (`b`, `i`, …), or `None` for other kinds.
fn run_tag(s: &Scene, i: usize) -> Option<&'static str> {
    matches!(s.w(i), W::Run { .. }).then(|| crate::kinds::kind(s.core[i].kind as usize).tag.unwrap_or("span"))
}

fn color(v: Option<&ComputedValue>) -> Option<Color32> {
    match v {
        Some(ComputedValue::Color(k)) => {
            let [r, g, b, a] = k.rgba8();
            Some(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => None,
    }
}

fn keyword(v: Option<&ComputedValue>) -> Option<&str> {
    match v {
        Some(ComputedValue::Keyword(k)) => Some(k),
        Some(ComputedValue::String(k) | ComputedValue::Raw(k)) => Some(k),
        _ => None,
    }
}

/// The runs of label `i`, or `None` when it has no run children.
pub fn collect(s: &Scene, i: usize) -> Option<Rich> {
    // a label without runs (no children, or only an attached context menu) is plain text
    if !matches!(s.w(i), W::Label { .. }) || !s.children[i].iter().any(|&c| s.idx(c).is_some_and(|j| matches!(s.w(j), W::Run { .. }))) {
        return None;
    }
    let W::Label { text, weight, .. } = s.w(i) else { return None };
    let label_size = s.slots.id_at(i).and_then(|id| s.style.computed(id)).and_then(|c| match c.get("font-size") {
        Some(ComputedValue::Length(l)) => Some(l.px),
        _ => None,
    });
    let base = RunFmt { weight: *weight, ..RunFmt::default() };
    let mut rich = Rich { plain: text.clone(), runs: Vec::new() };
    if !text.is_empty() {
        let id = s.slots.id_at(i)?;
        rich.runs.push(RunSeg { node: Some(id), start: 0, end: text.len(), fmt: base, link: None, href: None });
    }
    for &c in &s.children[i] {
        visit(s, c, base, None, label_size, &mut rich);
    }
    Some(rich)
}

fn visit(s: &Scene, id: NodeId, parent: RunFmt, link: Option<NodeId>, label_size: Option<f32>, out: &mut Rich) {
    let Some(i) = s.idx(id) else { return };
    let Some(tag) = run_tag(s, i) else { return };
    let W::Run { text, weight, .. } = s.w(i) else { return };
    let mut f = parent;
    // the tag defaults (the theme's base sheet states the same; they hold without one)
    match tag {
        "b" => f.weight = f.weight.max(700),
        "i" => f.italic = true,
        "code" => f.mono = true,
        _ => {}
    }
    let link = if tag == "a" { Some(id) } else { link };
    if let Some(c) = s.style.computed(id) {
        // read through the engine's typed view (OW-CSS); only what differs from the parent's
        // computed value counts: an inherited value keeps the enclosing run's format (and this
        // tag's default)
        let v = StyleView::new(c);
        let pv = s.core[i].parent.and_then(|p| s.style.computed(p)).map(|pc| StyleView::new(pc));
        let own = |name: &str| v.get(name).filter(|x| pv.as_ref().is_none_or(|pv| pv.get(name) != Some(*x)));
        if let Some(k) = keyword(own("font-style")) {
            f.italic = k == "italic" || k == "oblique";
        }
        if let Some(k) = keyword(own("font-family")) {
            f.mono = k.to_ascii_lowercase().contains("mono");
        }
        if let Some(ComputedValue::FontWeight(w)) = own("font-weight") {
            f.weight = i64::from(*w);
        }
        if let Some(k) = keyword(own("text-decoration-line")) {
            // decorations of an enclosing run stay (CSS 2 §16.3.1)
            f.underline |= k.contains("underline");
            f.strike |= k.contains("line-through");
        }
        if let Some(col) = color(own("color")) {
            f.color = Some(col);
        }
        if let Some(bg) = v.background().map(|k| {
            let [r, g, b, a] = k.rgba8();
            Color32::from_rgba_unmultiplied(r, g, b, a)
        }).filter(|c| c.a() > 0)
        {
            f.background = Some(bg);
        }
        if let Some(ComputedValue::Length(l)) = own("font-size")
            && Some(l.px) != label_size
        {
            f.size = Some(l.px);
        }
    }
    if *weight > 0 {
        f.weight = *weight;
    }
    if !text.is_empty() {
        let start = out.plain.len();
        out.plain.push_str(text);
        out.runs.push(RunSeg { node: Some(id), start, end: out.plain.len(), fmt: f, link, href: None });
    }
    for &c in &s.children[i] {
        visit(s, c, f, link, label_size, out);
    }
}

/// Recollects label `i`'s runs into `W::Label::rich` (before it is measured).
pub(crate) fn refresh(s: &mut Scene, i: usize) {
    if !matches!(s.w(i), W::Label { .. }) {
        return;
    }
    let new = collect(s, i).map(Box::new);
    if let W::Label { rich, .. } = s.w_mut(i) {
        *rich = new;
    }
}

/// The label a run node belongs to (its nearest non-run ancestor, when that is a label).
pub(crate) fn host_of(s: &Scene, i: usize) -> Option<NodeId> {
    let mut p = s.core[i].parent;
    while let Some(id) = p {
        let j = s.idx(id)?;
        match s.w(j) {
            W::Run { .. } => p = s.core[j].parent,
            W::Label { .. } => return Some(id),
            _ => return None,
        }
    }
    None
}

/// Node `i` (a run, or a label or run whose children changed) changed what its label shows:
/// remeasure the label.
pub(crate) fn touched(s: &mut Scene, i: usize) {
    let host = match s.w(i) {
        W::Label { .. } => s.slots.id_at(i),
        W::Run { .. } => host_of(s, i),
        _ => None,
    };
    if let Some(h) = host {
        s.mark_measure(h);
    }
}

/// The colours a job falls back to.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub text: Color32,
    pub link: Color32,
}

/// The paragraph as one layout job: a section per run, wrapped at `wrap` points if given.
/// `underline` adds the hover/focus underline to that link's runs (DESIGN R18).
pub fn layout_job(ctx: &egui::Context, style: &egui::Style, rich: &Rich, colors: Colors, wrap: Option<f32>, underline: Option<NodeId>) -> LayoutJob {
    let mut job = LayoutJob::default();
    let body = egui::TextStyle::Body.resolve(style);
    for r in &rich.runs {
        let text = &rich.plain[r.start..r.end];
        let f = r.fmt;
        let size = f.size.unwrap_or(body.size);
        let font_id = if f.mono {
            FontId::new(size, FontFamily::Monospace)
        } else {
            let mut id = crate::fonts::text_font(ctx, style, egui::TextStyle::Body, f.weight, text);
            id.size = size;
            id
        };
        let is_link = r.link.is_some() || r.href.is_some();
        let color = f.color.unwrap_or(if is_link { colors.link } else { colors.text });
        let line = Stroke::new(1.0, color);
        let under = f.underline || (r.link.is_some() && r.link == underline);
        job.append(
            text,
            0.0,
            TextFormat {
                font_id,
                color,
                italics: f.italic,
                background: f.background.unwrap_or(Color32::TRANSPARENT),
                underline: if under { line } else { Stroke::NONE },
                strikethrough: if f.strike { line } else { Stroke::NONE },
                ..TextFormat::default()
            },
        );
    }
    job.wrap.max_width = wrap.unwrap_or(f32::INFINITY);
    job
}

/// A plain run list, for text that is not nodes (a toast's `show`): a string (one run), or a list
/// of strings and maps `{text, b?, i?, code?, href?}` (booleans; `href` a string). Each map's flags
/// give the tag defaults (`b` 700, `i` italic, `code` monospace; `href` the link colour); there is
/// no per-run CSS (no node to match) and no markup is parsed.
pub fn from_value(v: &svelters_abi::Value) -> Result<Rich, String> {
    use svelters_abi::Value;
    let mut rich = Rich::default();
    let mut push = |text: &str, fmt: RunFmt, href: Option<String>| {
        let start = rich.plain.len();
        rich.plain.push_str(text);
        rich.runs.push(RunSeg { node: None, start, end: rich.plain.len(), fmt, link: None, href });
    };
    let base = RunFmt { weight: 400, ..RunFmt::default() };
    match v {
        Value::Str(s) => push(s, base, None),
        Value::List(items) => {
            for (k, it) in items.iter().enumerate() {
                match it {
                    Value::Str(s) => push(s, base, None),
                    Value::Map(fields) => {
                        let (mut text, mut f, mut href) = (String::new(), base, None);
                        for (key, val) in fields {
                            let on = matches!(val, Value::Bool(true));
                            match (key.as_str(), val) {
                                ("text", Value::Str(s)) => text = s.clone(),
                                ("b", _) => f.weight = if on { 700 } else { 400 },
                                ("i", _) => f.italic = on,
                                ("code", _) => f.mono = on,
                                ("href", Value::Str(s)) => href = Some(s.clone()).filter(|h| !h.is_empty()),
                                (other, _) => return Err(format!("run {k}: unknown or mistyped field `{other}` (text: string; b, i, code: bool; href: string)")),
                            }
                        }
                        push(&text, f, href);
                    }
                    other => return Err(format!("run {k}: a string or {{text, b, i, code, href}}, not {other:?}")),
                }
            }
        }
        other => return Err(format!("a string or a list of runs, not {other:?}")),
    }
    Ok(rich)
}

impl Rich {
    /// The plain run's `href` at `pos` (relative to the galley's origin) in `galley`, laid out from
    /// this paragraph.
    pub fn href_at(&self, galley: &egui::Galley, pos: egui::Vec2) -> Option<&str> {
        let c = galley.cursor_from_pos(pos).index.0;
        let b = self.plain.char_indices().nth(c).map_or(self.plain.len(), |(b, _)| b);
        self.runs.iter().find(|r| r.start <= b && b < r.end).and_then(|r| r.href.as_deref())
    }
}

/// The size of the paragraph (measure).
pub(crate) fn measure(m: &crate::layout::Metrics, rich: &Rich, wrap: Option<f32>) -> egui::Vec2 {
    let colors = Colors { text: Color32::WHITE, link: Color32::WHITE };
    let job = layout_job(&m.ctx, &m.style, rich, colors, wrap, None);
    m.ctx.fonts_mut(|f| f.layout_job(job)).size()
}

/// The bounds of the glyphs in `galley` for the byte ranges `ranges` of its text, one rect per row.
fn glyph_rects(galley: &egui::Galley, plain: &str, ranges: &[(usize, usize)], origin: egui::Pos2) -> Vec<Rect> {
    let mut out = Vec::new();
    for &(start, end) in ranges {
        let (a, b) = (plain[..start].chars().count(), plain[..end].chars().count());
        let mut k = 0; // char index at the start of the row
        for row in &galley.rows {
            let n = row.char_count_excluding_newline().0;
            let (lo, hi) = (a.max(k), b.min(k + n));
            if lo < hi {
                let r = row.rect();
                let x0 = row.pos.x + row.x_offset(egui::text::CharIndex(lo - k));
                let x1 = row.pos.x + row.x_offset(egui::text::CharIndex(hi - k));
                out.push(Rect::from_min_max(origin + egui::vec2(x0, r.min.y), origin + egui::vec2(x1, r.max.y)));
            }
            k += n + usize::from(row.ends_with_newline);
        }
    }
    out
}

/// What a rich label's pass produced: its response and the links activated.
pub(crate) struct Painted {
    pub resp: egui::Response,
    /// `a` runs activated this frame, with their `href`.
    pub activated: Vec<(NodeId, String)>,
}

/// Paints rich label `i` in `ui` (its rect is the leaf's), with one interaction per link.
pub(crate) fn paint(s: &mut Scene, i: usize, ui: &mut egui::Ui, wrap_mode: &str, sense: egui::Sense) -> Option<Painted> {
    let mut rich = collect(s, i)?;
    let colors = Colors { text: ui.visuals().text_color(), link: ui.visuals().hyperlink_color };
    let wrap = (wrap_mode == "wrap").then(|| ui.available_width());
    let links = rich.links();
    // the link the pointer or keyboard focus is on (underlined, `:hover`/`:focus`)
    let lit = links.iter().map(|(n, _)| *n).find(|&n| {
        s.idx(n).is_some_and(|j| {
            let id = s.core[j].egui_id;
            ui.ctx().memory(|m| m.has_focus(id)) || ui.ctx().read_response(id).is_some_and(|r| r.hovered())
        })
    });
    let mut job = layout_job(ui.ctx(), ui.style(), &rich, colors, wrap, lit);
    if wrap_mode == "truncate" {
        job.wrap = egui::text::TextWrapping::truncate_at_width(ui.available_width());
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let (rect, resp) = ui.allocate_exact_size(galley.size(), sense);
    let label_resp = resp.clone();
    label_resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), rich.plain.as_str()));
    let mut activated = Vec::new();
    let mut lit_now = None;
    for (node, ranges) in &links {
        let Some(j) = s.idx(*node) else { continue };
        let id = s.core[j].egui_id;
        let rects = glyph_rects(&galley, &rich.plain, ranges, rect.min);
        let Some(bounds) = rects.iter().copied().reduce(|a, b| a.union(b)) else { continue };
        let text: String = ranges.iter().map(|&(a, b)| &rich.plain[a..b]).collect();
        let r = ui.interact(bounds, id, egui::Sense::click());
        // a pointer in the union but between a wrapped link's rows is not on it
        let on = r.hovered() && r.hover_pos().is_none_or(|p| rects.iter().any(|q| q.contains(p)));
        let r = if on { r.on_hover_cursor(egui::CursorIcon::PointingHand) } else { r };
        let W::Run { href, .. } = s.w(j) else { continue };
        let href = href.clone();
        let name = s.common(j).accessible_name.clone();
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, if name.is_empty() { text.as_str() } else { name.as_str() }));
        // a pointer click lands on the glyphs; keyboard and AccessKit clicks have no position
        if r.clicked() && r.interact_pointer_pos().is_none_or(|p| rects.iter().any(|q| q.contains(p))) {
            activated.push((*node, href));
        }
        if r.has_focus() {
            ui.painter().rect_stroke(bounds.expand(1.0), 2.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
        }
        let mut b = StateBits(s.core[j].state.0 & !(StateBits::HOVERED.0 | StateBits::FOCUSED.0));
        b.set(StateBits::HOVERED, on);
        b.set(StateBits::FOCUSED, r.has_focus());
        s.set_walk_state(j, b);
        if on || r.has_focus() {
            lit_now = Some(*node);
        }
    }
    // a hover or focus that began this frame restyles the link before it is drawn
    let galley = if lit_now != lit {
        rich = collect(s, i)?;
        let mut job = layout_job(ui.ctx(), ui.style(), &rich, colors, wrap, lit_now);
        if wrap_mode == "truncate" {
            job.wrap = egui::text::TextWrapping::truncate_at_width(ui.available_width());
        }
        ui.fonts_mut(|f| f.layout_job(job))
    } else {
        galley
    };
    ui.painter().galley(rect.min, galley, colors.text);
    Some(Painted { resp, activated })
}
