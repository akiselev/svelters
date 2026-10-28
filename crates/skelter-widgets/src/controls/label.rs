//! `label` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/label.rs`): text; Hoverable, and
//! Pressable only while `clickable` is set or it has links (so a label never steals clicks from
//! what is under it).
//!
//! `wrap-mode`: `wrap` (height for width), `truncate` (one line, cut at the end with `…`),
//! `extend` (one line, never wrapped). `elide` (`start`, `middle`, `end`) cuts one line to the
//! width wherever `wrap-mode` says, and so does CSS `text-overflow: ellipsis`; the accessible value
//! stays the full text.
//!
//! Rich text (WIDGET-PARITY Q4): with `runs` the label is one paragraph, its own `text` first and
//! then the runs (`controls::rich`), laid out at the label's CSS font size. A link run is hit-tested
//! on its glyphs: the pointer on it underlines it (DESIGN R18) and shows the pointer cursor (R10),
//! and a click on it sends `link-clicked` (and `open-url` when it has a target) instead of
//! `clicked`. The hit area and drawn rect are the text's, as egui's label.
//!
//! Selection (`selectable`, WIDGET-PARITY §2.10): a selectable label is `:read-only`, shows the
//! I-beam, selects by drag (a double click a word, a triple click everything), takes focus on press,
//! and while focused selects all with Ctrl/Cmd+A, extends with Shift+arrows and sends `copy` with the
//! selected text on Ctrl/Cmd+C (writing the clipboard is the host's). `bind:selection-start` /
//! `bind:selection-end` are the selection in chars of the shown text (equal: none; a negative end is
//! the end of the text). Leaving focus collapses it unless the platform keeps unfocused selections.
//!
//! `for` (a buddy label): a click on the label moves focus to the node with that anchor (skelter-core
//! resolves anchors).

use crate::controls::common::{self, Units};
use crate::controls::rich::{self, Rich};
use crate::interact::{Hoverable, Intents, PressParams, Pressable, Sense, StateBits, Unit};
use crate::kind_props;
use crate::paint::{self, Align, Elide, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, HitCx, HitResult, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use crate::shim::text::{RichText, TextLayout, TextShaper};
use skelter_base::accesskit;
use skelter_base::color::Color;
use skelter_base::cursor::CursorIcon;
use skelter_base::geom::{Point, Rect, Size};
use std::sync::Arc;

pub const UA: Ua = Ua(&[UaRule::base(&[("text-align", Keyword("start"))])]);

kind_props! {
    /// The label's props.
    pub struct LabelProps;
    text "text": String = Str("") ; Str, Layout;
    /// `wrap`, `truncate` or `extend`.
    wrap_mode "wrap-mode": String = Enum("wrap") ; Enum(&["wrap", "truncate", "extend"]), Layout;
    /// CSS font weight of the text (a prop the runtime sets beats CSS).
    font_weight "font-weight": i64 = I64(400) ; I64, Layout;
    /// Cut one line with `…` to the width: `none`, `start`, `middle`, `end`.
    elide "elide": String = Enum("none") ; Enum(&["none", "start", "middle", "end"]), Layout;
    /// Rich runs after `text`: strings and `{text, b, i, code, href, color}` records (`controls::rich`).
    runs "runs": Vec<Value> = None ; List, Layout;
    /// It takes clicks and sends `clicked` (set while a runtime listens to `clicked`).
    clickable "clickable": bool = Bool(false) ; Bool, Property;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
    /// The text can be selected and copied; off by default.
    selectable "selectable": bool = Bool(false) ; Bool, Paint;
    /// The selection's start, in chars of the shown text (`bind:`).
    selection_start "selection-start": i64 = I64(0) ; I64, Paint, bindable;
    /// The selection's end, in chars (`bind:`); a negative end is the end of the text.
    selection_end "selection-end": i64 = I64(0) ; I64, Paint, bindable;
    /// The anchor of the node a click on this label focuses (a buddy label).
    for_anchor "for": String = Str("") ; Str, Property;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("clicked", &[], "Clicked (only while `clickable`), not on a link run."),
    SignalSpec::new("link-clicked", &[("index", ValueType::I64), ("href", ValueType::Str)], "A link run was clicked: its index in `runs` and its `href`.").cancelable(),
    SignalSpec::new("copy", &[("text", ValueType::Str)], "Ctrl/Cmd+C on a focused selectable label: the selected text, for the host's clipboard."),
    SignalSpec::new("open-url", &[("url", ValueType::Str), ("new-tab", ValueType::Bool)], "The default of a `link-clicked` whose run has a target: the host opens `url` (skelter never opens anything itself)."),
];

pub static SPEC: KindSpec = KindSpec { tag: "label", props: LabelProps::SPECS, signals: SIGNALS, slots: &[], children: true, role: accesskit::Role::Label, name_from: NameFrom::Prop("text"), text_prop: Some("text"), focusable: false };

/// What a label shows, from its props: the input to [`measure`], [`place`] and [`paint`] (also for
/// foreign painters that draw a label without a node).
#[derive(Clone, Copy, Debug)]
pub struct Look<'a> {
    pub text: &'a str,
    /// `wrap`, `truncate` or `extend`.
    pub wrap_mode: &'a str,
    /// `none`, `start`, `middle`, `end`.
    pub elide: &'a str,
    /// The runs, when the label has any (one paragraph instead of `text`).
    pub rich: Option<&'a Rich>,
    /// The link run drawn underlined (the pointer is on it).
    pub lit: Option<usize>,
}

impl Look<'_> {
    /// The cut, if any: `elide`, `wrap-mode="truncate"` (an end cut) or `text-overflow: ellipsis`.
    fn elision(&self, s: &StyleView) -> Option<Elide> {
        Elide::parse(self.elide).or_else(|| (self.wrap_mode == "truncate").then_some(Elide::End)).or_else(|| Elide::of_style(s))
    }

    /// The text shown (a rich label's: all runs), uncut.
    pub fn shown(&self) -> &str {
        self.rich.map_or(self.text, |r| r.plain.as_str())
    }

    /// The paragraph for `width` and the width it wraps at: the runs at the label's CSS font size
    /// (`wrap` wraps, a cut truncates them at the end), else the plain text (cut by measuring).
    fn paragraph(&self, shaper: &mut dyn TextShaper, s: &StyleView, width: Option<f32>) -> (RichText, Option<f32>) {
        let base = paint::text_style(s);
        let rtl = s.is_rtl();
        if let Some(r) = self.rich {
            let mut t = rich::rich_text(r, &base, &s.theme().fonts().mono).in_dir(rtl);
            if let (Some(w), true) = (width, self.wrap_mode == "truncate" || self.elision(s).is_some()) {
                t.ellipsis = true;
                t.max_lines = Some(1);
                return (t, Some(w));
            }
            return (t, (self.wrap_mode == "wrap").then_some(width).flatten());
        }
        match (self.elision(s), self.wrap_mode, width) {
            (Some(mode), _, Some(w)) => (RichText::plain(&paint::elide(shaper, s, self.text, mode, w), base).in_dir(rtl), None),
            (None, "wrap", Some(w)) => (RichText::plain(self.text, base).in_dir(rtl), Some(w)),
            _ => (RichText::plain(self.text, base).in_dir(rtl), None),
        }
    }
}

/// Its size under `s` (with the `font-weight` prop) when `width` points are available (`None`:
/// unbounded). Only `wrap` uses the width: a cut label measures its whole text and is cut where it
/// is placed.
pub fn measure(shaper: &mut dyn TextShaper, s: &StyleView, look: &Look, width: Option<f32>) -> Size {
    if look.rich.is_some() {
        let w = width.filter(|_| look.wrap_mode == "wrap");
        let (t, wrap) = look.paragraph(shaper, s, w);
        return shaper.layout(&t, wrap.map(|w| w.max(0.0))).size();
    }
    match (look.wrap_mode, width) {
        ("wrap", Some(w)) if look.elision(s).is_none() => paint::measure_text(shaper, s, look.text, Some(w)),
        _ => paint::measure_text(shaper, s, look.text, None),
    }
}

/// A label's paragraph placed in its rect: the text, its layout and the rect it takes (the hit
/// area and drawn rect).
#[derive(Clone, Debug)]
pub struct Placed {
    pub text: RichText,
    pub layout: Arc<TextLayout>,
    pub rect: Rect,
}

/// The paragraph in `r` under `s`: placed by `text-align`, centred vertically, on the pixel grid
/// at `scale`.
pub fn place(shaper: &mut dyn TextShaper, s: &StyleView, look: &Look, r: Rect, scale: f32) -> Placed {
    let w = (look.wrap_mode != "extend" || look.elision(s).is_some()).then(|| r.width());
    let (text, wrap) = look.paragraph(shaper, s, w);
    let layout = shaper.layout(&text, wrap.map(|w| w.max(0.0)));
    let size = layout.size();
    let rtl = s.is_rtl();
    // the runs' extent: a right-to-left paragraph laid out at a width sits at that width's right
    // edge (skelter-text aligns it to its own start), so in `rtl` the ink, not the box, is placed
    let (x0, x1) = layout.runs().iter().filter(|pr| !pr.run.glyphs.is_empty()).fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), pr| (a.min(pr.origin.x), b.max(pr.origin.x + pr.run.advance)));
    let (x0, x1) = if x0.is_finite() { (x0, x1) } else { (0.0, size.w) };
    let x = match (Align::of(s, Align::Start), rtl) {
        (Align::Start, false) => r.min.x,
        (Align::Start, true) => r.max.x - x1,
        (Align::Center, _) => (r.min.x + r.max.x) / 2.0 - size.w / 2.0,
        (Align::End, false) => r.max.x - size.w,
        (Align::End, true) => r.min.x - x0,
    };
    let y = (r.min.y + r.max.y) / 2.0 - size.h / 2.0;
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let min = Point::new((x * scale).round() / scale, (y * scale).round() / scale);
    Placed { text, layout, rect: Rect::from_origin_size(min, size) }
}

/// Paints `p` (from [`place`]) under `s`: the runs' fills (`code`), each run in its colour (`color`, link runs in the theme's
/// `link`), then the decorations (the lit link's underline).
pub fn paint(pc: &mut PaintCx, s: &StyleView, look: &Look, p: &Placed) {
    let colors = rich::Colors { text: s.color("color").unwrap_or(Color::BLACK), link: paint::ua_token(s.is_dark(), "link").unwrap_or(Color::BLACK) };
    let color = |style: u32| rich::color(look.rich, colors, style);
    if let Some(r) = look.rich {
        rich::paint_backgrounds(pc, r, &p.layout, p.rect.min);
    }
    paint::paint_runs(pc, &p.layout, p.rect.min, &color);
    let lit = |style: u32| look.lit.is_some_and(|i| style == i as u32 + 1);
    rich::paint_decorations(pc, &p.text, &p.layout, p.rect.min, &color, &lit);
}

/// Static text.
#[derive(Debug, Default)]
pub struct Label {
    pub props: LabelProps,
    units: Units,
    hover: Hoverable,
    press: Pressable,
    /// The link run the pointer is on.
    lit: Option<usize>,
    /// The paragraph as placed after the last layout (for hit-testing links and the text rect).
    placed: Option<Placed>,
    /// The run children folded at the last prepare (`controls::run`), for events.
    child_runs: Vec<Value>,
    /// A pointer selection is in progress (the press began on the text).
    selecting: bool,
    /// The selection's fixed end while selecting, in bytes of the placed text.
    sel_anchor: usize,
}

/// The byte offset of char `c` in `t` (its end past the last char).
fn byte_of(t: &str, c: usize) -> usize {
    t.char_indices().nth(c).map_or(t.len(), |(b, _)| b)
}

/// The char index of byte `b` in `t`.
fn char_of(t: &str, b: usize) -> usize {
    t[..b.min(t.len())].chars().count()
}

/// The word around byte `b` of `t` (alphanumerics and `_`; a lone other char otherwise).
fn word_at(t: &str, b: usize) -> std::ops::Range<usize> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let b = b.min(t.len());
    let at = t[b..].chars().next().or_else(|| t[..b].chars().next_back());
    let Some(c0) = at else { return b..b };
    if !word(c0) {
        let start = if t[b..].starts_with(c0) { b } else { b - c0.len_utf8() };
        return start..start + c0.len_utf8();
    }
    let start = t[..b].char_indices().rev().take_while(|(_, c)| word(*c)).last().map_or(b, |(i, _)| i);
    let end = t[b..].char_indices().find(|(_, c)| !word(*c)).map_or(t.len(), |(i, _)| b + i);
    start..end
}

impl Label {
    /// The selection as a byte range of `text` (the placed text), when it selects anything.
    fn selection_bytes(&self, text: &str) -> Option<std::ops::Range<usize>> {
        if !self.props.selectable {
            return None;
        }
        let n = text.chars().count() as i64;
        let end = if self.props.selection_end < 0 { n } else { self.props.selection_end.min(n) };
        let start = self.props.selection_start.clamp(0, n);
        let (a, b) = (start.min(end), start.max(end));
        (a < b).then(|| byte_of(text, a as usize)..byte_of(text, b as usize))
    }

    /// Writes the selection (bytes of `text`, `anchor` → `caret`) to the props, if it changed.
    fn set_selection(&mut self, cx: &mut EventCx, text: &str, anchor: usize, caret: usize) {
        let (s, e) = (char_of(text, anchor) as i64, char_of(text, caret) as i64);
        if (s, e) != (self.props.selection_start, self.props.selection_end) {
            self.props.selection_start = s;
            self.props.selection_end = e;
            cx.prop_changed(P::selection_start.id());
            cx.prop_changed(P::selection_end.id());
        }
    }

    /// Pointer and keyboard selection on one pass (`selectable`).
    fn select(&mut self, cx: &mut EventCx, pass: &crate::interact::Pass, ev: &Event) {
        let Some(pl) = self.placed.clone() else { return };
        let Some(geom) = pl.layout.geometry.clone() else { return };
        let text = pl.text.text.as_str();
        let hit = |p: Point| geom.0.hit_byte(Point::new(p.x - pl.rect.min.x, p.y - pl.rect.min.y));
        if pass.input.pointer.any_pressed() && pass.resp.down_on {
            if let Some(p) = pass.input.pointer.latest_pos() {
                let b = hit(p);
                let r = match self.units.tracker.press_click_count() {
                    2 => word_at(text, b),
                    3 => 0..text.len(),
                    _ => b..b,
                };
                self.sel_anchor = r.start;
                self.selecting = true;
                self.set_selection(cx, text, r.start, r.end);
                if !cx.focused {
                    cx.focus_request = Some(true);
                }
            }
        } else if self.selecting && pass.resp.down_on && pass.input.pointer.is_moving()
            && let Some(p) = pass.input.pointer.latest_pos()
        {
            let anchor = self.sel_anchor;
            self.set_selection(cx, text, anchor, hit(p));
        }
        if !pass.resp.down_on {
            self.selecting = false;
        }
        if let Event::Focus { gained: false, .. } = ev
            && !cx.policy.keep_selection_unfocused
        {
            let caret = byte_of(text, self.props.selection_end.max(0) as usize);
            self.set_selection(cx, text, caret, caret);
        }
        if !cx.focused {
            return;
        }
        let n = text.chars().count() as i64;
        for e in pass.input.events.iter() {
            let crate::interact::FrameEvent::Key { key, pressed: true, modifiers: m, .. } = e else { continue };
            match key {
                skelter_base::key::Key::A if m.command => self.set_selection(cx, text, 0, text.len()),
                skelter_base::key::Key::C if m.command => {
                    if let Some(r) = self.selection_bytes(text) {
                        common::emit(cx, &SPEC, "copy", vec![Value::str(&text[r])]);
                    }
                }
                skelter_base::key::Key::ArrowLeft | skelter_base::key::Key::ArrowRight if m.shift => {
                    let d = if *key == skelter_base::key::Key::ArrowLeft { -1 } else { 1 };
                    let end = if self.props.selection_end < 0 { n } else { self.props.selection_end.min(n) };
                    let caret = (end + d).clamp(0, n) as usize;
                    let anchor = byte_of(text, self.props.selection_start.clamp(0, n) as usize);
                    self.set_selection(cx, text, anchor, byte_of(text, caret));
                }
                _ => {}
            }
        }
    }

    /// The runs, when the label has any: its `runs` prop, then its run children (`children`, as
    /// `runs` records). `set_prop` rejects runs that don't parse; children always do.
    pub fn rich(&self) -> Option<Rich> {
        self.rich_with(&self.child_runs)
    }

    fn rich_with(&self, children: &[Value]) -> Option<Rich> {
        if self.props.runs.is_empty() && children.is_empty() {
            return None;
        }
        if children.is_empty() {
            return rich::from_value(&self.props.text, &self.props.runs).ok();
        }
        let all: Vec<Value> = self.props.runs.iter().chain(children).cloned().collect();
        rich::from_value(&self.props.text, &all).ok()
    }

    fn look<'a>(&'a self, rich: Option<&'a Rich>) -> Look<'a> {
        Look { text: &self.props.text, wrap_mode: &self.props.wrap_mode, elide: &self.props.elide, rich, lit: self.lit }
    }

    fn view<'a>(&self, s: &crate::shim::core::Styles<'a>) -> StyleView<'a> {
        let mut state = s.state;
        if self.props.disabled {
            state |= StateBits::DISABLED;
        }
        StyleView::new(s.computed, UA, state, s.dark).with_theme(s.theme).with_inherited(s.inherited).with_font_weight(Some(self.props.font_weight))
    }
}

impl Kind for Label {
    fn folds_children() -> bool {
        true
    }
    fn prop_state(&self) -> crate::interact::StateBits {
        { let mut s = crate::interact::StateBits::NONE; s.set(crate::interact::StateBits::READ_ONLY, self.props.selectable); s }
    }
    fn prop_states() -> crate::interact::StateBits {
        crate::interact::StateBits::READ_ONLY
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        if prop == P::runs.id()
            && let Value::List(items) = v
            && rich::from_value("", items).is_err()
        {
            return Err(PropError::Type { prop: "runs", expected: ValueType::List });
        }
        self.props.set(prop, v)?;
        if prop == P::runs.id() || prop == P::text.id() {
            self.lit = None;
        }
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        // a wrapping label's min-content width is its longest word (CSS), so a row can shrink it
        let wraps = self.props.wrap_mode == "wrap" && self.props.elide == "none";
        let width = known.width.or(match avail.width {
            taffy::AvailableSpace::Definite(w) => Some(w),
            taffy::AvailableSpace::MinContent if wraps => Some(0.0),
            _ => None,
        });
        let rich = self.rich_with(cx.style.children);
        let s = measure(cx.text, &self.view(&cx.style), &self.look(rich.as_ref()), width);
        taffy::Size { width: s.w, height: s.h }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.child_runs = cx.style.children.to_vec();
        let rich = self.rich();
        let r = Rect::from_origin_size(Point::ZERO, cx.size);
        let placed = place(cx.text, &self.view(&cx.style), &self.look(rich.as_ref()), r, 1.0);
        self.placed = Some(placed);
    }

    fn record(&self, cx: &mut RecordCx) {
        let rich = self.rich_with(cx.style.children);
        let look = self.look(rich.as_ref());
        let s = self.view(&cx.style);
        let r = cx.rect();
        let placed = place(cx.text, &s, &look, r, cx.scale);
        // the selection lies under the text
        if let (Some(range), Some(geom)) = (self.selection_bytes(&placed.text.text), placed.layout.geometry.as_ref()) {
            let fill = paint::ua_token(s.is_dark(), "selection").unwrap_or(Color::rgba(0.2, 0.4, 0.8, 0.4));
            for rect in geom.0.selection_rects(range) {
                let rect = rect.translate(placed.rect.min.to_vec());
                let q = skelter_base::display::Quad { rect, radii: Default::default(), fill: skelter_base::color::Fill::Solid(fill), border_widths: Default::default(), border_color: Color::TRANSPARENT, border_style: Default::default() };
                cx.list.push(skelter_base::display::Prim::Quad(q), rect);
            }
        }
        paint(&mut PaintCx::of(cx), &s, &look, &placed);
    }

    fn hit_test(&self, cx: &HitCx, p: Point) -> HitResult {
        // a label that takes clicks (`clickable`, `for`) takes them on its whole box, as a button
        // does, also when layout made it wider than its text; else only its text is hit
        if self.props.clickable || !self.props.for_anchor.is_empty() {
            return cx.box_hit(p);
        }
        match &self.placed {
            Some(pl) if pl.rect.contains(p) => HitResult::Hit,
            Some(_) => HitResult::Miss,
            None => cx.box_hit(p),
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        cx.state.set(StateBits::DISABLED, self.props.disabled);
        cx.enabled = !self.props.disabled;
        let rich = self.rich();
        let clickable = self.props.clickable || rich.as_ref().is_some_and(Rich::has_links) || !self.props.for_anchor.is_empty();
        let params = PressParams::default();
        let mut sense = if clickable { Sense::HOVER.union(self.press.sense(&params)) } else { Sense::HOVER };
        if self.props.selectable {
            sense = sense.union(Sense::CLICK_AND_DRAG);
        }
        cx.state.set(StateBits::READ_ONLY, self.props.selectable);
        let mut fired = Intents::default();
        let (hover, press) = (&mut self.hover, &mut self.press);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            if clickable {
                press.input(&params, icx, &mut fired);
            }
        });
        if self.props.selectable && cx.enabled {
            self.select(cx, &pass, ev);
        }
        let placed = self.placed.as_ref();
        let link_at = |p: Point| rich.as_ref().zip(placed).and_then(|(r, pl)| rich::link_at(r, &pl.layout, pl.rect.min, p));
        let lit = if pass.resp.hovered { pass.input.pointer.latest_pos().and_then(link_at) } else { None };
        if lit != self.lit {
            self.lit = lit;
            cx.repaint = true;
        }
        if lit.is_some() && cx.enabled {
            cx.cursor = Some(CursorIcon::Pointer);
        } else if self.props.selectable && pass.resp.hovered {
            cx.cursor = Some(CursorIcon::Text);
        }
        let mut handled = pass.resp.down_on;
        for _ in fired.iter().filter(|p| p.is_click()) {
            handled = true;
            // a pointer click on a link's glyphs is the link's; keyboard and AccessKit clicks have
            // no position and are the label's
            let link = pass.resp.interact_pointer_pos.and_then(link_at);
            match (link, rich.as_ref()) {
                (Some(i), Some(r)) => {
                    let run = &r.runs[i];
                    let href = run.href.clone().unwrap_or_default();
                    let index = run.item.map_or(-1, |k| k as i64);
                    let prevented = common::emit_sync(cx, &SPEC, "link-clicked", vec![Value::I64(index), Value::str(&href)]);
                    if !prevented && !href.is_empty() {
                        common::emit(cx, &SPEC, "open-url", vec![Value::str(&href), Value::Bool(false)]);
                    }
                }
                _ => {
                    if self.props.clickable {
                        common::emit(cx, &SPEC, "clicked", Vec::new());
                    }
                    if !self.props.for_anchor.is_empty() {
                        cx.focus_anchor = Some(self.props.for_anchor.clone());
                    }
                }
            }
        }
        if handled || (ev.is_pointer() && pass.resp.hovered) { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Label);
        let rich = self.rich();
        let text = rich.as_ref().map_or(self.props.text.as_str(), |r| r.plain.as_str());
        cx.node.set_value(text);
        if self.props.disabled {
            cx.node.set_disabled();
        }
        if self.props.clickable {
            self.press.a11y(&PressParams::default(), cx.node);
        }
        if self.props.selectable {
            cx.node.add_action(accesskit::Action::Focus);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::rich::testing as rt;
    use crate::controls::testkit::{self, Driver};
    use crate::shim::core::Styles;
    use crate::shim::testing::MonoShaper;
    use skelter_base::change::ChangeClass;
    use skelter_base::display::Prim;

    fn rec(fields: &[(&str, Value)]) -> Value {
        Value::Record(fields.iter().map(|(k, v)| (Arc::from(*k), v.clone())).collect())
    }

    fn font_size() -> f32 {
        StyleView::ua_only(UA, StateBits::NONE, true).font_size()
    }

    fn measure_at(l: &Label, w: f32) -> Size {
        let mut text = MonoShaper;
        let mut cx = MeasureCx { style: Styles { computed: None, parts: &[], children: &[], state: StateBits::NONE, dark: true, theme: None, inherited: None }, text: &mut text, scale: 1.0 };
        let avail = taffy::Size { width: taffy::AvailableSpace::Definite(w), height: taffy::AvailableSpace::MaxContent };
        let s = l.measure(&mut cx, taffy::Size { width: None, height: None }, avail);
        Size::new(s.width, s.height)
    }

    fn label(text: &str) -> Label {
        let mut l = Label::default();
        l.props.text = text.into();
        l
    }

    /// "see docs": the label's text, then a link run.
    fn linked() -> Label {
        let mut l = label("see ");
        let runs = Value::List(vec![rec(&[("text", Value::str("docs")), ("href", Value::str("https://x"))])]);
        l.set_prop(P::runs.id(), &runs).unwrap();
        l
    }

    #[test]
    fn spec() {
        let s = Label::spec();
        assert_eq!(s.tag, "label");
        assert_eq!(s.role, accesskit::Role::Label);
        assert!(!s.focusable);
        for p in ["text", "wrap-mode", "elide", "runs", "font-weight"] {
            assert_eq!(s.prop_spec(s.prop(p).unwrap()).unwrap().change, ChangeClass::Layout, "{p}");
        }
        assert_eq!(s.prop_spec(s.prop("wrap-mode").unwrap()).unwrap().default, crate::shim::core::Default::Enum("wrap"));
        assert!(s.signal("clicked").is_some() && s.signal("open-url").is_some());
        assert!(s.signals[s.signal("link-clicked").unwrap().0 as usize].cancelable);
        let mut l = Label::default();
        assert!(l.set_prop(s.prop("wrap-mode").unwrap(), &Value::str("bogus")).is_err());
        assert!(l.set_prop(s.prop("runs").unwrap(), &Value::List(vec![Value::Bool(true)])).is_err(), "runs that don't parse");
        l.set_prop(s.prop("elide").unwrap(), &Value::str("middle")).unwrap();
        assert_eq!(l.get_prop(s.prop("elide").unwrap()), Some(Value::str("middle")));
    }

    #[test]
    fn measures_and_wraps_at_the_available_width() {
        let l = label("abcdefghij");
        let adv = MonoShaper::advance(font_size());
        let one = testkit::measure(&l, StateBits::NONE);
        assert_eq!(one.w, 10.0 * adv);
        let wrapped = measure_at(&l, 5.5 * adv);
        assert_eq!((wrapped.w, wrapped.h), (5.0 * adv, 2.0 * one.h), "two lines of five");
        // a cut label measures its whole text (it is cut where it is placed)
        let mut t = label("abcdefghij");
        t.props.elide = "end".into();
        assert_eq!(measure_at(&t, 5.5 * adv), one);
        let mut e = label("abcdefghij");
        e.props.wrap_mode = "extend".into();
        assert_eq!(measure_at(&e, 5.5 * adv), one);
    }

    #[test]
    fn records_its_text_on_the_left() {
        let l = label("Hi");
        let (list, res) = testkit::record(&l, Size::new(100.0, 30.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["glyphs"]);
        let Prim::Glyphs(g) = &list.items[0] else { unreachable!() };
        assert_eq!(g.origin.x, 0.0, "text-align: start");
        assert_eq!(g.color, paint::ua_token(true, "fg").unwrap());
        assert_eq!(res.runs[0].glyphs.len(), 2);
        let (list, _) = testkit::record(&l, Size::new(100.0, 30.0), StateBits::DISABLED);
        let Prim::Glyphs(g) = &list.items[0] else { unreachable!() };
        assert_eq!(g.color, paint::ua_token(true, "disabled-fg").unwrap());
    }

    #[test]
    fn elides_to_the_width() {
        let adv = MonoShaper::advance(font_size());
        for (mode, wrap) in [("end", "wrap"), ("none", "truncate")] {
            let mut l = label("abcdefghij");
            l.props.elide = mode.into();
            l.props.wrap_mode = wrap.into();
            let (_, res) = testkit::record(&l, Size::new(40.0, 20.0), StateBits::NONE);
            let ids: Vec<u32> = res.runs[0].glyphs.iter().map(|g| g.id).collect();
            assert_eq!(ids.len(), (40.0 / adv).floor() as usize, "{mode}/{wrap}: the widest cut that fits");
            assert_eq!(ids.last(), Some(&('…' as u32)));
            assert_eq!(ids[0], 'a' as u32);
        }
        let mut l = label("abcdefghij");
        l.props.elide = "start".into();
        let (_, res) = testkit::record(&l, Size::new(40.0, 20.0), StateBits::NONE);
        assert_eq!(res.runs[0].glyphs[0].id, '…' as u32);
        assert_eq!(res.runs[0].glyphs.last().unwrap().id, 'j' as u32);
    }

    #[test]
    fn rich_runs_get_their_own_colours() {
        let mut l = linked();
        let white = rec(&[("text", Value::str("!")), ("color", Value::Color(Color::WHITE))]);
        let runs = Value::List(vec![rec(&[("text", Value::str("docs")), ("href", Value::str("https://x"))]), white]);
        l.set_prop(P::runs.id(), &runs).unwrap();
        let (list, res) = rt::record(&l, Size::new(200.0, 30.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["glyphs", "glyphs", "glyphs"], "a glyph run per span");
        let colors: Vec<Color> = list.items.iter().map(|p| if let Prim::Glyphs(g) = p { g.color } else { unreachable!() }).collect();
        assert_eq!(colors, [paint::ua_token(true, "fg").unwrap(), paint::ua_token(true, "link").unwrap(), Color::WHITE]);
        let adv = MonoShaper::advance(font_size());
        let Prim::Glyphs(g) = &list.items[1] else { unreachable!() };
        assert_eq!(g.origin.x, 4.0 * adv, "the link run follows `see `");
        assert_eq!(res.runs.len(), 3);
        let n = testkit::a11y(&l, StateBits::NONE, Size::new(200.0, 30.0));
        assert_eq!(n.value(), Some("see docs!"), "the accessible value is all runs");
    }

    #[test]
    fn a_cut_rich_label_ends_in_an_ellipsis() {
        let mut l = linked();
        l.props.wrap_mode = "truncate".into();
        let adv = MonoShaper::advance(font_size());
        let (_, res) = rt::record(&l, Size::new(6.5 * adv, 20.0), StateBits::NONE);
        let ids: Vec<u32> = res.runs.iter().flat_map(|r| r.glyphs.iter().map(|g| g.id)).collect();
        assert_eq!(ids.len(), 6, "one line at the width");
        assert_eq!(ids.last(), Some(&('…' as u32)));
    }

    /// The centre of char `k` of the label's single line.
    fn at_char(l: &Label, k: f32) -> (f32, f32) {
        let pl = l.placed.as_ref().unwrap();
        let adv = MonoShaper::advance(font_size());
        (pl.rect.min.x + (k + 0.5) * adv, (pl.rect.min.y + pl.rect.max.y) / 2.0)
    }

    fn click_at(d: &mut Driver<Label>, x: f32, y: f32) {
        d.hover(true);
        d.move_to(x, y);
        d.down(x, y);
        d.advance(50);
        d.up(x, y);
        d.advance(50);
    }

    #[test]
    fn a_link_run_is_hit_tested_and_signals() {
        let size = Size::new(200.0, 30.0);
        let mut d = Driver::new(linked(), size);
        rt::prepare(&mut d.kind, size, StateBits::NONE);
        let (x, y) = at_char(&d.kind, 5.0);
        d.hover(true);
        d.move_to(x, y);
        assert_eq!(d.kind.lit, Some(1), "the pointer is on the link");
        // DESIGN R18: the lit link is underlined
        let (list, _) = rt::record(&d.kind, size, StateBits::HOVERED);
        assert_eq!(testkit::kinds(&list), ["glyphs", "glyphs", "line"]);
        let Prim::Line(line) = &list.items[2] else { unreachable!() };
        assert_eq!(line.color, paint::ua_token(true, "link").unwrap());
        click_at(&mut d, x, y);
        assert_eq!(d.names(), ["link-clicked", "open-url"]);
        assert_eq!(d.signals[0].1, vec![Value::I64(0), Value::str("https://x")]);
        assert_eq!(d.signals[1].1, vec![Value::str("https://x"), Value::Bool(false)]);
        d.take();
        // off the link: not clickable, so nothing; the underline goes
        let (x0, y0) = at_char(&d.kind, 1.0);
        click_at(&mut d, x0, y0);
        assert!(d.take().is_empty());
        assert_eq!(d.kind.lit, None);
    }

    #[test]
    fn clickable_sends_clicked() {
        let size = Size::new(100.0, 30.0);
        let mut d = Driver::new(label("Click me"), size);
        rt::prepare(&mut d.kind, size, StateBits::NONE);
        let (x, y) = at_char(&d.kind, 1.0);
        click_at(&mut d, x, y);
        assert!(d.take().is_empty(), "a label never steals clicks unless it is clickable");
        d.set("clickable", Value::Bool(true));
        click_at(&mut d, x, y);
        assert_eq!(d.take(), ["clicked"]);
        d.set("disabled", Value::Bool(true));
        click_at(&mut d, x, y);
        assert!(d.take().is_empty());
        assert!(d.state.contains(StateBits::DISABLED));
    }

    #[test]
    fn the_hit_area_is_the_text() {
        let size = Size::new(100.0, 30.0);
        let mut l = label("Hi");
        rt::prepare(&mut l, size, StateBits::NONE);
        let hc = HitCx { size };
        assert_eq!(l.hit_test(&hc, Point::new(1.0, 15.0)), HitResult::Hit);
        assert_eq!(l.hit_test(&hc, Point::new(90.0, 15.0)), HitResult::Miss);
    }

    /// A label that takes clicks is hit on its whole box (layout may make it wider than its text).
    #[test]
    fn a_clickable_label_is_hit_on_its_whole_box() {
        let size = Size::new(100.0, 30.0);
        let mut l = label("Hi");
        l.props.clickable = true;
        rt::prepare(&mut l, size, StateBits::NONE);
        let hc = HitCx { size };
        assert_eq!(l.hit_test(&hc, Point::new(90.0, 15.0)), HitResult::Hit);
        assert_eq!(l.hit_test(&hc, Point::new(110.0, 15.0)), HitResult::Miss, "not outside it");
    }

    #[test]
    fn a11y_node() {
        let mut l = label("Name");
        l.props.clickable = true;
        let n = testkit::a11y(&l, StateBits::NONE, Size::new(60.0, 20.0));
        assert_eq!(n.role(), accesskit::Role::Label);
        assert_eq!(n.value(), Some("Name"));
        assert!(n.supports_action(accesskit::Action::Click));
    }

    fn selectable(text: &str) -> Driver<Label> {
        let mut l = Label::default();
        l.props.text = text.into();
        l.props.selectable = true;
        let size = Size::new(200.0, 20.0);
        testkit::prepare(&mut l, size, StateBits::NONE);
        Driver::new(l, size)
    }

    fn sel(d: &Driver<Label>) -> (i64, i64) {
        (d.kind.props.selection_start, d.kind.props.selection_end)
    }

    fn col(k: f32) -> f32 {
        k * MonoShaper::advance(font_size())
    }

    #[test]
    fn selection_spec() {
        let s = Label::spec();
        for p in ["selectable", "selection-start", "selection-end", "for"] {
            assert!(s.prop(p).is_some(), "{p}");
        }
        assert!(s.prop_spec(s.prop("selection-start").unwrap()).unwrap().bindable);
        assert!(s.signal("copy").is_some());
    }

    #[test]
    fn word_bounds() {
        assert_eq!(word_at("hello world", 2), 0..5);
        assert_eq!(word_at("hello world", 5), 5..6, "a lone space");
        assert_eq!(word_at("hello world", 11), 6..11, "at the end: the last word");
        assert_eq!(word_at("", 0), 0..0);
    }

    #[test]
    fn drag_selects_and_focuses() {
        let mut d = selectable("hello world");
        d.hover(true);
        d.down(col(0.0), 10.0);
        assert!(d.focused, "a press focuses a selectable label");
        assert!(d.state.contains(StateBits::READ_ONLY));
        d.advance(20);
        d.move_to(col(3.0), 10.0);
        d.move_to(col(5.0), 10.0);
        d.up(col(5.0), 10.0);
        assert_eq!(sel(&d), (0, 5));
        assert!(d.changed.contains(&"selection-start") && d.changed.contains(&"selection-end"));
    }

    #[test]
    fn double_and_triple_click() {
        let mut d = selectable("hello world");
        d.hover(true);
        for _ in 0..2 {
            d.down(col(8.0), 10.0);
            d.advance(20);
            d.up(col(8.0), 10.0);
            d.advance(20);
        }
        assert_eq!(sel(&d), (6, 11), "a double click selects the word");
        d.down(col(8.0), 10.0);
        d.advance(20);
        d.up(col(8.0), 10.0);
        assert_eq!(sel(&d), (0, 11), "a triple click selects everything");
    }

    #[test]
    fn keyboard_select_all_copy_and_extend() {
        let mut d = selectable("hello world");
        d.focus(true);
        d.key_with(skelter_base::key::Key::A, skelter_base::event::Modifiers::CTRL);
        assert_eq!(sel(&d), (0, 11));
        d.key_with(skelter_base::key::Key::C, skelter_base::event::Modifiers::CTRL);
        assert_eq!(d.signals.last(), Some(&("copy", vec![Value::str("hello world")])));
        d.key_with(skelter_base::key::Key::ArrowLeft, skelter_base::event::Modifiers::SHIFT);
        assert_eq!(sel(&d), (0, 10));
    }

    #[test]
    fn focus_loss_collapses_on_the_egui_row() {
        let mut d = selectable("hello world");
        d.focus(true);
        d.key_with(skelter_base::key::Key::A, skelter_base::event::Modifiers::CTRL);
        d.focus(false);
        let (s, e) = sel(&d);
        assert_eq!(s, e, "egui's row does not keep an unfocused selection");
    }

    #[test]
    fn records_the_selection_under_the_text() {
        let mut l = Label::default();
        l.props.text = "hello".into();
        l.props.selectable = true;
        l.props.selection_start = 1;
        l.props.selection_end = 3;
        let (list, _) = testkit::record(&l, Size::new(200.0, 20.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "glyphs"]);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert!((q.rect.min.x - col(1.0)).abs() < 1e-3 && (q.rect.max.x - col(3.0)).abs() < 1e-3);
        // a negative end is the end of the text; not selectable draws none
        l.props.selection_end = -1;
        let (list, _) = testkit::record(&l, Size::new(200.0, 20.0), StateBits::NONE);
        let Prim::Quad(q) = &list.items[0] else { unreachable!() };
        assert!((q.rect.max.x - col(5.0)).abs() < 1e-3);
        l.props.selectable = false;
        assert_eq!(testkit::kinds(&testkit::record(&l, Size::new(200.0, 20.0), StateBits::NONE).0), ["glyphs"]);
    }
}
