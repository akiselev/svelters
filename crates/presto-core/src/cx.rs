//! The contexts a [`Kind`](crate::Kind) is called with (architecture §4.2). Each gives read access to
//! the node ([`NodeView`]: its props, computed style, state and size) and the services of its phase:
//! text measurement in [`MeasureCx`], text shaping and images in [`PrepareCx`], the display list in
//! [`RecordCx`], the document itself in [`EventCx`], the AccessKit node in [`A11yCx`].

use crate::common;
use crate::document::Document;
use crate::ids::{PropId, SignalId};
use crate::registry::KindEntry;
use crate::style::ComputedBox;
use crate::tree::{Dirty, PropStore};
use crate::value::Value;
use presto_base::color::Fill;
use presto_base::commit::ImageData;
use presto_base::cursor::CursorIcon;
use presto_base::display::{DisplayList, Glyphs, Line, LineCap, Prim, Quad, Shadow};
use presto_base::geom::{Point, Rect, RoundedRect, Size};
use presto_base::ids::{ImageId, NodeId, RunId, WindowId};
use presto_base::text::{GlyphRun, TextMeasure, TextSpec};
use std::sync::{Arc, OnceLock};
use svelters_style::StateMask;

/// Shapes text into glyph runs for recording. Implemented over `presto-text`'s `TextSystem`; a
/// document without one records no text (it still measures through [`TextMeasure`]).
pub trait TextShaper {
    fn shape(&mut self, spec: &TextSpec<'_>, max_width: Option<f32>) -> ShapedText;
}

/// Shaped text: its size and its runs with their baseline origins (local to the text's top-left).
#[derive(Clone, Debug, Default)]
pub struct ShapedText {
    pub size: Size,
    pub runs: Vec<(Arc<GlyphRun>, Point)>,
}

/// Services the document lends its kinds by type: a rich-text shaping system, an icon registry, an
/// image cache. Set with [`Document::set_service`]; kinds reach them with `cx.service::<T>()` in
/// measure, prepare, record and event handling.
#[derive(Default)]
pub struct Services {
    map: std::collections::HashMap<std::any::TypeId, Box<dyn std::any::Any>>,
}

impl Services {
    pub fn insert<T: 'static>(&mut self, t: T) -> Option<T> {
        self.map.insert(std::any::TypeId::of::<T>(), Box::new(t)).and_then(|b| b.downcast().ok().map(|b| *b))
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map.get(&std::any::TypeId::of::<T>()).and_then(|b| b.downcast_ref())
    }
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&std::any::TypeId::of::<T>()).and_then(|b| b.downcast_mut())
    }
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map.remove(&std::any::TypeId::of::<T>()).and_then(|b| b.downcast().ok().map(|b| *b))
    }
}

fn common_defaults() -> &'static [Option<Value>] {
    static D: OnceLock<Vec<Option<Value>>> = OnceLock::new();
    D.get_or_init(|| common::COMMON.props.iter().map(|p| p.default.to_value()).collect())
}

/// Read access to one node.
#[derive(Clone, Copy)]
pub struct NodeView<'a> {
    pub(crate) id: NodeId,
    pub(crate) entry: &'a KindEntry,
    pub(crate) props: &'a PropStore,
    pub(crate) style: &'a ComputedBox,
    pub(crate) state: StateMask,
    pub(crate) size: Size,
    pub(crate) children: &'a [NodeId],
    pub(crate) window: Option<WindowId>,
    pub(crate) css: Option<&'a svelters_style::ComputedStyle>,
}

impl<'a> NodeView<'a> {
    /// The engine's computed style (every property of the style table, custom properties included),
    /// for kinds that draw from author CSS beyond [`ComputedBox`]. `None` before the first restyle.
    pub fn css(&self) -> Option<&'a svelters_style::ComputedStyle> {
        self.css
    }
    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn kind(&self) -> &'a KindEntry {
        self.entry
    }
    pub fn window(&self) -> Option<WindowId> {
        self.window
    }
    /// A prop's value, or its default.
    pub fn prop(&self, p: PropId) -> Option<&'a Value> {
        if let Some(v) = self.props.get(p) {
            return Some(v);
        }
        if let Some(i) = self.entry.prop_index(p) {
            return self.entry.defaults[i].as_ref();
        }
        let i = (0..common::COMMON.props.len()).find(|&i| common::COMMON.prop_id(i) == p)?;
        common_defaults()[i].as_ref()
    }
    pub fn prop_bool(&self, p: PropId) -> bool {
        self.prop(p).and_then(Value::as_bool).unwrap_or(false)
    }
    pub fn prop_f64(&self, p: PropId) -> Option<f64> {
        self.prop(p).and_then(Value::as_f64)
    }
    pub fn prop_str(&self, p: PropId) -> Option<&'a str> {
        self.prop(p).and_then(Value::as_str)
    }
    /// The text-content prop's value (`""` when the kind has none or it is unset).
    pub fn text(&self) -> &'a str {
        self.entry.text_prop.and_then(|p| self.prop_str(p)).unwrap_or("")
    }
    pub fn style(&self) -> &'a ComputedBox {
        self.style
    }
    pub fn state(&self) -> StateMask {
        self.state
    }
    pub fn disabled(&self) -> bool {
        self.state.contains(StateMask::DISABLED)
    }
    /// The border-box size from the last layout.
    pub fn size(&self) -> Size {
        self.size
    }
    /// The border box in local coordinates.
    pub fn bounds(&self) -> Rect {
        Rect::from_origin_size(Point::ZERO, self.size)
    }
    /// The padding box (inside the borders).
    pub fn padding_box(&self) -> Rect {
        self.bounds().inset(self.style.border_widths)
    }
    pub fn children(&self) -> &'a [NodeId] {
        self.children
    }
}

/// [`Kind::prop_changed`](crate::Kind::prop_changed): ask for more work than the prop's class.
pub struct PropCx<'a> {
    pub node: NodeView<'a>,
    pub(crate) request: Dirty,
}

impl<'a> std::ops::Deref for PropCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl PropCx<'_> {
    pub fn request_paint(&mut self) {
        self.request.insert(Dirty::PAINT);
    }
    /// The leaf's measured content changed.
    pub fn request_measure(&mut self) {
        self.request.insert(Dirty::MEASURE);
    }
    pub fn request_layout(&mut self) {
        self.request.insert(Dirty::LAYOUT);
    }
    pub fn request_a11y(&mut self) {
        self.request.insert(Dirty::A11Y);
    }
}

/// [`Kind::layout_style`](crate::Kind::layout_style).
pub struct StyleCx<'a> {
    pub node: NodeView<'a>,
    pub(crate) cascaded: &'a taffy::Style,
}

impl<'a> std::ops::Deref for StyleCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl StyleCx<'_> {
    /// The taffy style the cascade computed for the node (CSS over the kind's defaults).
    pub fn default_layout(&self) -> taffy::Style {
        self.cascaded.clone()
    }
}

/// [`Kind::measure`](crate::Kind::measure).
pub struct MeasureCx<'a> {
    pub node: NodeView<'a>,
    pub(crate) text: &'a mut dyn TextMeasure,
    pub(crate) services: &'a mut Services,
}

impl<'a> std::ops::Deref for MeasureCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl MeasureCx<'_> {
    /// `text` in the node's computed font, at `max_width`.
    pub fn measure_text(&mut self, text: &str, max_width: Option<f32>) -> Size {
        let spec = self.node.style.text_spec(text);
        self.text.measure(&spec, max_width)
    }
    /// (min-content, max-content) widths of `text` in the node's font.
    pub fn content_widths(&mut self, text: &str) -> (f32, f32) {
        let spec = self.node.style.text_spec(text);
        self.text.content_widths(&spec)
    }
    pub fn text_measure(&mut self) -> &mut dyn TextMeasure {
        self.text
    }
    pub fn service<T: 'static>(&mut self) -> Option<&mut T> {
        self.services.get_mut()
    }
}

/// Images a kind uploads; the commit carries them to the compositor's resource store.
#[derive(Default)]
pub(crate) struct ImageSink {
    pub next: u64,
    pub added: Vec<(ImageId, Arc<ImageData>)>,
    pub removed: Vec<ImageId>,
}

/// [`Kind::prepare`](crate::Kind::prepare).
pub struct PrepareCx<'a> {
    pub node: NodeView<'a>,
    pub(crate) shaper: Option<&'a mut (dyn TextShaper + 'static)>,
    pub(crate) images: &'a mut ImageSink,
    pub(crate) request: Dirty,
    pub(crate) services: &'a mut Services,
}

impl<'a> std::ops::Deref for PrepareCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl PrepareCx<'_> {
    /// `text` shaped in the node's computed font (`None` without a shaper).
    pub fn shape_text(&mut self, text: &str, max_width: Option<f32>) -> Option<ShapedText> {
        let spec = self.node.style.text_spec(text);
        self.shaper.as_deref_mut().map(|s| s.shape(&spec, max_width))
    }
    /// Uploads an image; the id stays valid until [`PrepareCx::release_image`].
    pub fn upload_image(&mut self, data: Arc<ImageData>) -> ImageId {
        self.images.next += 1;
        let id = ImageId(self.images.next);
        self.images.added.push((id, data));
        id
    }
    pub fn release_image(&mut self, id: ImageId) {
        self.images.removed.push(id);
    }
    /// Re-record after this prepare even if the node was not content-dirty.
    pub fn request_paint(&mut self) {
        self.request.insert(Dirty::PAINT);
    }
    pub fn service<T: 'static>(&mut self) -> Option<&mut T> {
        self.services.get_mut()
    }
}

/// Glyph runs recorded this frame; the commit sends the new ones.
#[derive(Default)]
pub(crate) struct RunSink {
    pub next: u64,
    pub added: Vec<(RunId, Arc<GlyphRun>)>,
}

/// [`Kind::record`](crate::Kind::record): the node's display list, in local coordinates.
pub struct RecordCx<'a> {
    pub node: NodeView<'a>,
    pub(crate) list: &'a mut DisplayList,
    pub(crate) runs: &'a mut RunSink,
    /// The runs this node's list references (replaced runs are dropped when the list is).
    pub(crate) node_runs: &'a mut Vec<RunId>,
    pub(crate) shaper: Option<&'a mut (dyn TextShaper + 'static)>,
    pub(crate) services: &'a mut Services,
}

impl<'a> std::ops::Deref for RecordCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl RecordCx<'_> {
    /// Adds a primitive with its ink rect.
    pub fn push(&mut self, p: Prim, ink: Rect) {
        self.list.push(p, ink);
    }
    pub fn service<T: 'static>(&mut self) -> Option<&mut T> {
        self.services.get_mut()
    }
    /// Hands a shaped run to the compositor; the id is valid for this node's list.
    pub fn add_run(&mut self, run: Arc<GlyphRun>) -> RunId {
        self.runs.next += 1;
        let id = RunId(self.runs.next);
        self.runs.added.push((id, run));
        self.node_runs.push(id);
        id
    }
    /// Draws a run with its baseline origin at `origin`.
    pub fn draw_run(&mut self, run: Arc<GlyphRun>, origin: Point, color: presto_base::color::Color) {
        let ink = Rect::new(origin.x, origin.y - run.ascent, origin.x + run.advance, origin.y + run.descent);
        let id = self.add_run(run);
        self.list.push(Prim::Glyphs(Glyphs { run: id, origin, color }), ink);
    }
    /// Shapes and draws `text` at `at` (its top-left) in the node's font and colour; returns the size
    /// (`None` without a shaper).
    pub fn draw_text(&mut self, text: &str, at: Point, max_width: Option<f32>) -> Option<Size> {
        let style = self.node.style;
        let spec = style.text_spec(text);
        let shaped = self.shaper.as_deref_mut()?.shape(&spec, max_width);
        for (run, o) in shaped.runs {
            self.draw_run(run, Point::new(at.x + o.x, at.y + o.y), style.color);
        }
        Some(shaped.size)
    }
    /// The CSS box: shadows, background, border, then outline, from the computed style.
    pub fn draw_box(&mut self) {
        let s = self.node.style;
        let r = self.node.bounds();
        for sh in s.shadows.iter().filter(|s| !s.inset) {
            let ink = r.translate(sh.offset).inflate(sh.blur * 1.5 + sh.spread.max(0.0));
            let p = Shadow { rect: r, radii: s.radii, color: sh.color, blur: sh.blur, spread: sh.spread, offset: sh.offset, inset: false };
            self.list.push(Prim::Shadow(p), ink);
        }
        let bg_visible = match &s.background {
            Fill::Solid(c) => c.a > 0.0,
            Fill::Linear { .. } => true,
        };
        let border = s.border_color.a > 0.0 && s.border_widths != presto_base::geom::Insets::ZERO;
        if bg_visible || border {
            let q = Quad {
                rect: r,
                radii: s.radii,
                fill: s.background.clone(),
                border_widths: if border { s.border_widths } else { presto_base::geom::Insets::ZERO },
                border_color: s.border_color,
                border_style: s.border_style,
            };
            self.list.push(Prim::Quad(q), r);
        }
        for sh in s.shadows.iter().filter(|s| s.inset) {
            let p = Shadow { rect: r, radii: s.radii, color: sh.color, blur: sh.blur, spread: sh.spread, offset: sh.offset, inset: true };
            self.list.push(Prim::Shadow(p), r);
        }
        if s.outline_width > 0.0 && s.outline_color.a > 0.0 {
            let o = r.inflate(s.outline_offset + s.outline_width);
            let q = Quad {
                rect: o,
                radii: s.radii,
                fill: Fill::Solid(presto_base::color::Color::TRANSPARENT),
                border_widths: presto_base::geom::Insets::all(s.outline_width),
                border_color: s.outline_color,
                border_style: s.border_style,
            };
            self.list.push(Prim::Quad(q), o);
        }
    }
    /// A straight line in the node's colour.
    pub fn draw_line(&mut self, from: Point, to: Point, width: f32) {
        let ink = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y)).inflate(width);
        let l = Line { from, to, width, color: self.node.style.color, cap: LineCap::Butt, dash: None, wavy: false };
        self.list.push(Prim::Line(l), ink);
    }
    /// Clips what follows to the padding box, rounded (until [`RecordCx::pop_clip`]).
    pub fn push_clip(&mut self, r: RoundedRect) {
        self.list.push(Prim::PushClip(r), Rect::ZERO);
    }
    pub fn pop_clip(&mut self) {
        self.list.push(Prim::PopClip, Rect::ZERO);
    }
}

/// [`Kind::hit_test`](crate::Kind::hit_test).
pub struct HitCx<'a> {
    pub node: NodeView<'a>,
}

impl<'a> std::ops::Deref for HitCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl HitCx<'_> {
    /// The rounded border box contains `p` (local coordinates).
    pub fn box_hit(&self, p: Point) -> crate::kind::HitResult {
        let r = self.node.bounds();
        if !r.contains(p) {
            return crate::kind::HitResult::Miss;
        }
        if in_rounded(r, self.node.style.radii, p) { crate::kind::HitResult::Hit } else { crate::kind::HitResult::Miss }
    }
}

/// `p` (inside `r`) is inside `r` with corner radii `c`.
pub(crate) fn in_rounded(r: Rect, c: presto_base::geom::Corners, p: Point) -> bool {
    let corner = |cx: f32, cy: f32, rad: f32| {
        let (dx, dy) = (p.x - cx, p.y - cy);
        dx * dx + dy * dy <= rad * rad
    };
    let (x0, y0, x1, y1) = (r.min.x, r.min.y, r.max.x, r.max.y);
    if c.tl > 0.0 && p.x < x0 + c.tl && p.y < y0 + c.tl {
        return corner(x0 + c.tl, y0 + c.tl, c.tl);
    }
    if c.tr > 0.0 && p.x > x1 - c.tr && p.y < y0 + c.tr {
        return corner(x1 - c.tr, y0 + c.tr, c.tr);
    }
    if c.br > 0.0 && p.x > x1 - c.br && p.y > y1 - c.br {
        return corner(x1 - c.br, y1 - c.br, c.br);
    }
    if c.bl > 0.0 && p.x < x0 + c.bl && p.y > y1 - c.bl {
        return corner(x0 + c.bl, y1 - c.bl, c.bl);
    }
    true
}

/// A signal a kind emits from its native default handler, delivered to JS listeners.
#[derive(Clone, Debug, PartialEq)]
pub struct Emitted {
    pub node: NodeId,
    pub signal: SignalId,
    pub arg: Option<Value>,
}

/// [`Kind::event`](crate::Kind::event): the document, for the kind's own node and the services of
/// input (focus, pointer capture, cursor), plus the signals to emit.
pub struct EventCx<'a> {
    pub(crate) doc: &'a mut Document,
    pub(crate) node: NodeId,
    pub(crate) emitted: &'a mut Vec<Emitted>,
    pub(crate) pointer: Option<u32>,
}

impl EventCx<'_> {
    pub fn node(&self) -> NodeId {
        self.node
    }
    /// A read view of the node (or of any live node).
    pub fn view(&self, node: NodeId) -> Option<NodeView<'_>> {
        self.doc.view(node)
    }
    pub fn document(&self) -> &Document {
        self.doc
    }
    /// Writes one of the node's props as native-owned state: recorded with this revision, so an
    /// older JS write of it is rejected (architecture §4.5).
    pub fn set_native(&mut self, prop: PropId, v: Value) -> Result<(), crate::PropError> {
        self.doc.set_prop_native(self.node, prop, v).map(|_| ())
    }
    pub fn set_state(&mut self, bits: StateMask, on: bool) {
        self.doc.set_state(self.node, bits, on);
    }
    /// Emits `signal` with `arg` from this node.
    pub fn emit(&mut self, signal: SignalId, arg: Option<Value>) {
        self.emitted.push(Emitted { node: self.node, signal, arg });
    }
    /// Keeps the current pointer on this node until release.
    pub fn capture_pointer(&mut self) {
        if let Some(p) = self.pointer {
            self.doc.input.capture(p, self.node);
        }
    }
    pub fn release_pointer(&mut self) {
        if let Some(p) = self.pointer {
            self.doc.input.release(p);
        }
    }
    /// The event's time (its OS timestamp; a timer's deadline for [`Event::Timer`](crate::Event::Timer)).
    pub fn time(&self) -> presto_base::time::Time {
        self.doc.input.now
    }
    /// Delivers [`Event::Timer`](crate::Event::Timer) to this node at `at` (replacing its earlier
    /// deadline); the app's loop asks [`Document::next_timer`] and calls [`Document::run_timers`].
    pub fn wake_at(&mut self, at: presto_base::time::Time) {
        self.doc.set_timer(self.node, Some(at));
    }
    pub fn cancel_wake(&mut self) {
        self.doc.set_timer(self.node, None);
    }
    /// Gives up keyboard focus (when this node or a descendant has it).
    pub fn blur(&mut self) {
        let Some(w) = self.doc.window_of(self.node) else { return };
        if self.doc.focused(w).is_some_and(|f| self.doc.is_ancestor_or_self(self.node, f)) {
            crate::input::focus::clear_focus(self.doc, w);
        }
    }
    pub fn service<T: 'static>(&mut self) -> Option<&mut T> {
        self.doc.services.get_mut()
    }
    /// Moves keyboard focus to this node.
    pub fn focus(&mut self, reason: crate::kind::FocusReason) {
        let n = self.node;
        self.doc.focus(n, reason);
    }
    pub fn set_cursor(&mut self, c: CursorIcon) {
        self.doc.input.cursor = Some(c);
    }
    pub fn request_paint(&mut self) {
        self.doc.mark(self.node, Dirty::PAINT);
    }
    pub fn request_layout(&mut self) {
        self.doc.mark_layout(self.node);
    }
}

/// [`VirtualChildren::realize`](crate::VirtualChildren::realize): the document, to create, recycle and
/// bind the rows of the virtualising node.
pub struct RealizeCx<'a> {
    pub(crate) doc: &'a mut Document,
    pub(crate) node: NodeId,
}

impl RealizeCx<'_> {
    /// The virtualising node (the rows' parent).
    pub fn node(&self) -> NodeId {
        self.node
    }
    /// The document (the node's own kind value is out while it realises).
    pub fn doc(&mut self) -> &mut Document {
        self.doc
    }
}

/// [`Kind::a11y`](crate::Kind::a11y): the node's AccessKit node, prefilled with its role, bounds and
/// children.
pub struct A11yCx<'a> {
    pub node: NodeView<'a>,
    pub out: &'a mut accesskit::Node,
}

impl<'a> std::ops::Deref for A11yCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl A11yCx<'_> {
    /// The defaults: the label from `aria-label` or the text content, disabled, focusable.
    pub fn default_a11y(&mut self) {
        let v = self.node;
        let label = v.entry.spec.name_from.iter().find_map(|src| {
            let s = match *src {
                crate::kind::NameSource::AriaLabel => v.prop_str(common::ARIA_LABEL)?,
                crate::kind::NameSource::Prop(p) => v.prop_str(v.entry.spec.prop_named(p)?)?,
                crate::kind::NameSource::Text => v.text(),
                // the children's own labels are their nodes'; AccessKit computes it from them
                crate::kind::NameSource::Children => return None,
            };
            (!s.is_empty()).then_some(s)
        });
        if let Some(l) = label {
            self.out.set_label(l);
        }
        if let Some(t) = self.node.prop_str(common::TITLE) {
            self.out.set_description(t);
        }
        if self.node.disabled() {
            self.out.set_disabled();
        }
        if self.node.entry.spec.focusable && !self.node.disabled() {
            self.out.add_action(accesskit::Action::Focus);
        }
    }
}
