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
use skelter_base::color::Fill;
use skelter_base::commit::ImageData;
use skelter_base::cursor::CursorIcon;
use skelter_base::display::{DisplayList, Glyphs, Line, LineCap, Prim, Quad, Shadow};
use skelter_base::geom::{Point, Rect, RoundedRect, Size};
use skelter_base::ids::{ImageId, NodeId, RunId, WindowId};
use skelter_base::layer::LayerKind;
use skelter_base::text::{GlyphRun, TextMeasure, TextSpec};
use std::sync::{Arc, OnceLock};
use svelters_style::StateMask;

/// Shapes text into glyph runs for recording. Implemented over `skelter-text`'s `TextSystem`; a
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
    /// Two services at once: `A` to read and `B` to write (a kind that paints from the theme while
    /// it shapes with the text system). `A` and `B` are different types.
    pub fn pair<A: 'static, B: 'static>(&mut self) -> (Option<&A>, Option<&mut B>) {
        let (ta, tb) = (std::any::TypeId::of::<A>(), std::any::TypeId::of::<B>());
        assert_ne!(ta, tb, "Services::pair: two different services");
        match self.map.get_disjoint_mut([&ta, &tb]) {
            [a, b] => (a.and_then(|a| a.downcast_ref()), b.and_then(|b| b.downcast_mut())),
        }
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
    pub(crate) doc: &'a Document,
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
    /// A read view of another live node (a child whose runs a folding kind lays out, a sibling).
    pub fn view(&self, node: NodeId) -> Option<NodeView<'a>> {
        self.doc.view(node)
    }
    /// The `k`-th child's view.
    pub fn child(&self, k: usize) -> Option<NodeView<'a>> {
        self.children.get(k).and_then(|&c| self.doc.view(c))
    }
    /// The computed style of the kind's paint part `name` (`::part(name)`, inheriting from the
    /// node's): `None` for a name not in [`KindSpec::parts`](crate::KindSpec::parts) or before the
    /// first restyle.
    pub fn part(&self, name: &str) -> Option<&'a svelters_style::ComputedStyle> {
        let i = self.doc.idx(self.id)?;
        self.doc.style.nodes[i].parts.iter().find(|(n, _)| *n == name).map(|(_, c)| &**c)
    }
    /// JS listens to `signal` on this node (capture or not).
    pub fn listened(&self, signal: SignalId) -> bool {
        self.doc.listeners(self.id).iter().any(|l| l.signal == signal)
    }
    /// Device pixels per point of the node's window (1 when detached).
    pub fn scale(&self) -> f32 {
        self.window.and_then(|w| self.doc.window(w)).map_or(1.0, |w| w.scale.0)
    }
    /// The padding widths from the last layout (points).
    pub fn padding(&self) -> skelter_base::geom::Insets {
        let Some(i) = self.doc.idx(self.id) else { return skelter_base::geom::Insets::ZERO };
        let p = self.doc.layout[i].layout.padding;
        skelter_base::geom::Insets { top: p.top, right: p.right, bottom: p.bottom, left: p.left }
    }
    /// The content box in local coordinates (inside the borders and the padding).
    pub fn content_box(&self) -> Rect {
        self.padding_box().inset(self.padding())
    }
    /// The border box's window transform from the last rebuild (local → window points).
    pub fn window_transform(&self) -> skelter_base::geom::Affine {
        self.doc.idx(self.id).map_or(skelter_base::geom::Affine::IDENTITY, |i| self.doc.record[i].world_tf)
    }
    /// A window point in the node's local space (the inverse of [`NodeView::window_transform`]).
    pub fn from_window(&self, p: Point) -> Point {
        self.window_transform().inverse().map_or(p, |t| t.apply(p))
    }
    /// The common `anchor` prop (`""` when unset).
    pub fn anchor(&self) -> &'a str {
        self.prop_str(common::ANCHOR).unwrap_or("")
    }
    /// The name of the parent's slot the node is in (`""`: the default slot, or no parent): a
    /// collapsing header lays out its `end` children in its header row.
    pub fn slot_name(&self) -> &'static str {
        let Some(slot) = self.doc.slot_of(self.id) else { return "" };
        let parent = self.doc.parent(self.id).and_then(|p| self.doc.view(p));
        parent.and_then(|p| p.entry.slot(slot)).map_or("", |s| s.name)
    }
}

/// [`Kind::child_layout`](crate::Kind::child_layout): the container and the child it places.
pub struct ChildLayoutCx<'a> {
    pub parent: NodeView<'a>,
    pub child: NodeView<'a>,
    /// The child's index among the container's children.
    pub index: usize,
    pub count: usize,
}

impl<'a> ChildLayoutCx<'a> {
    /// The `anchor` of child `k` (`""` when it has none): a stack shows the child its `current`
    /// names.
    pub fn anchor_of(&self, k: usize) -> &'a str {
        self.parent.child(k).map_or("", |v| v.anchor())
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

impl<'a> StyleCx<'a> {
    /// The taffy style the cascade computed for the node (CSS over the kind's defaults).
    pub fn default_layout(&self) -> taffy::Style {
        self.cascaded.clone()
    }
    /// A document service, read only (the theme a kind's layout follows).
    pub fn service<T: 'static>(&self) -> Option<&'a T> {
        self.node.doc.services.get()
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
    /// All the services ([`Services::pair`] borrows two at once).
    pub fn services(&mut self) -> &mut Services {
        self.services
    }
}

/// Images and paths a kind uploads; the commit carries them to the compositor's resource store.
#[derive(Default)]
pub(crate) struct ImageSink {
    pub next: u64,
    pub added: Vec<(ImageId, Arc<ImageData>)>,
    pub removed: Vec<ImageId>,
    pub next_path: u64,
    pub paths: Vec<(skelter_base::ids::PathId, Arc<skelter_base::commit::PathData>)>,
    pub removed_paths: Vec<skelter_base::ids::PathId>,
    /// Every image and path uploaded and not released (re-sent after a device reset).
    pub live_images: std::collections::HashMap<ImageId, Arc<ImageData>>,
    pub live_paths: std::collections::HashMap<skelter_base::ids::PathId, Arc<skelter_base::commit::PathData>>,
    /// Paths shared by key ([`PrepareCx::share_path`]), with their user counts.
    pub shared_paths: std::collections::HashMap<String, (skelter_base::ids::PathId, usize)>,
}

/// [`Kind::prepare`](crate::Kind::prepare).
pub struct PrepareCx<'a> {
    pub node: NodeView<'a>,
    pub(crate) shaper: Option<&'a mut (dyn TextShaper + 'static)>,
    pub(crate) images: &'a mut ImageSink,
    pub(crate) request: Dirty,
    pub(crate) services: &'a mut Services,
    /// Signals emitted while preparing ([`PrepareCx::emit`]), queued after the prepare.
    pub(crate) emitted: &'a mut Vec<Emitted>,
    /// The scroll offset asked for ([`PrepareCx::scroll_to`]), applied after the prepare.
    pub(crate) scroll: Option<skelter_base::geom::Vec2>,
    /// The caret asked for ([`PrepareCx::set_caret`]), sent after the prepare.
    pub(crate) caret: Option<Option<Rect>>,
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
        self.images.live_images.insert(id, data.clone());
        self.images.added.push((id, data));
        id
    }
    pub fn release_image(&mut self, id: ImageId) {
        self.images.live_images.remove(&id);
        self.images.removed.push(id);
    }
    /// Uploads a path (an icon outline, a chart's curve) for `Prim::Path`; the id stays valid until
    /// [`PrepareCx::release_path`].
    pub fn upload_path(&mut self, data: Arc<skelter_base::commit::PathData>) -> skelter_base::ids::PathId {
        self.images.next_path += 1;
        let id = skelter_base::ids::PathId(self.images.next_path);
        self.images.live_paths.insert(id, data.clone());
        self.images.paths.push((id, data));
        id
    }
    pub fn release_path(&mut self, id: skelter_base::ids::PathId) {
        self.images.live_paths.remove(&id);
        self.images.removed_paths.push(id);
    }
    /// A path shared by `key` across the document (an icon's layer that many nodes draw): uploaded
    /// by `make` for its first user, so every user draws the same `PathId` (one tessellation per
    /// scale step in the renderer). Each call adds a user; [`PrepareCx::unshare_path`] removes one
    /// and releases the path with the last. `None` when `make` gives nothing.
    pub fn share_path(&mut self, key: &str, make: &mut dyn FnMut() -> Option<Arc<skelter_base::commit::PathData>>) -> Option<skelter_base::ids::PathId> {
        if let Some((id, n)) = self.images.shared_paths.get_mut(key) {
            *n += 1;
            return Some(*id);
        }
        let id = self.upload_path(make()?);
        self.images.shared_paths.insert(key.to_owned(), (id, 1));
        Some(id)
    }
    /// Removes one user of the shared path `key`; the last releases it.
    pub fn unshare_path(&mut self, key: &str) {
        let Some((id, n)) = self.images.shared_paths.get_mut(key) else { return };
        *n -= 1;
        if *n == 0 {
            let id = *id;
            self.images.shared_paths.remove(key);
            self.release_path(id);
        }
    }
    /// Re-record after this prepare even if the node was not content-dirty.
    pub fn request_paint(&mut self) {
        self.request.insert(Dirty::PAINT);
    }
    pub fn service<T: 'static>(&mut self) -> Option<&mut T> {
        self.services.get_mut()
    }
    /// All the services ([`Services::pair`] borrows two at once).
    pub fn services(&mut self) -> &mut Services {
        self.services
    }
    /// Emits `signal` with `arg` from this node: queued as a JS event once the prepare returns
    /// ([`Document::take_events`]), not cancelable (a data window a virtual table needs, a restored
    /// state's report).
    pub fn emit(&mut self, signal: SignalId, arg: Option<Value>) {
        self.emitted.push(Emitted { node: self.node.id, signal, arg });
    }
    /// The node's scroll offset (zero unless it is a scroll container or scrolls itself).
    pub fn scroll_offset(&self) -> skelter_base::geom::Vec2 {
        self.node.doc.scroll_offset(self.node.id)
    }
    /// Scrolls the node (a scroll container, or a kind with [`Kind::self_scroll`](crate::Kind::self_scroll))
    /// to `offset` as [`EventCx::scroll_fast`] does, once the prepare returns (after the kind's
    /// [`SelfScroll`](crate::SelfScroll) is taken, so the offset clamps to its new content): the
    /// offset goes to the compositor at once and the content moves without a re-record. What a kind
    /// whose layout scrolls its caret into view uses (text-edit).
    pub fn scroll_to(&mut self, offset: skelter_base::geom::Vec2) {
        self.scroll = Some(offset);
    }
    /// Shows the caret at `rect` (local coordinates; `None` hides it) as [`EventCx::set_caret`]
    /// does, once the prepare returns: a caret a layout or a scroll in the prepare moved.
    pub fn set_caret(&mut self, rect: Option<Rect>) {
        self.caret = Some(rect);
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
    /// The part outside the node's scroll layer ([`RecordCx::fixed`]); swapped with `list` while
    /// it records.
    pub(crate) fixed: &'a mut DisplayList,
    pub(crate) in_fixed: bool,
    /// The node's overlay part ([`RecordCx::overlay`]); swapped with `list` while it records.
    pub(crate) overlay: &'a mut DisplayList,
    pub(crate) overlay_part: &'a mut Option<crate::overlay::OverlayPart>,
    pub(crate) in_overlay: bool,
    pub(crate) runs: &'a mut RunSink,
    /// The runs this node's list references (replaced runs are dropped when the list is).
    pub(crate) node_runs: &'a mut Vec<RunId>,
    pub(crate) shaper: Option<&'a mut (dyn TextShaper + 'static)>,
    pub(crate) services: &'a mut Services,
    /// The external layer this node places ([`RecordCx::external_layer`]).
    pub(crate) external: &'a mut Option<(LayerKind, Rect)>,
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
    /// Records what `f` draws as the node's fixed part, in its border-box space: for a kind with
    /// [`Kind::self_scroll`](crate::Kind::self_scroll) it stays outside the scroll layer (its box:
    /// background, border), so a scroll moves only the rest and re-records nothing. The fixed part
    /// paints below the rest of the list; for any other kind it simply goes first in the one list.
    pub fn fixed<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.in_fixed {
            return f(self);
        }
        std::mem::swap(&mut self.list, &mut self.fixed);
        self.in_fixed = true;
        let r = f(self);
        self.in_fixed = false;
        std::mem::swap(&mut self.list, &mut self.fixed);
        r
    }
    /// Records what `f` draws (node-local points, as the rest of the list) as the node's overlay
    /// surface `part`: a popup, a menu, a select's list. It paints in a layer of the window's overlay
    /// stack, above every content layer and unclipped by the node's ancestors, and pointer input
    /// inside `part.rect` reaches this node before anything under it (positions stay in the node's
    /// space, even outside its box). Recording no part closes the surface. One part per node (a
    /// second call adds to the first, keeping its `part`).
    pub fn overlay<R>(&mut self, part: crate::overlay::OverlayPart, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.in_overlay {
            return f(self);
        }
        if self.overlay_part.is_none() {
            *self.overlay_part = Some(part);
        }
        std::mem::swap(&mut self.list, &mut self.overlay);
        self.in_overlay = true;
        let r = f(self);
        self.in_overlay = false;
        std::mem::swap(&mut self.list, &mut self.overlay);
        r
    }

    /// Places an external layer at this node in paint order: a [`LayerKind::Texture`] (an offscreen
    /// viewport, an egui island), [`LayerKind::OsLayer`] or [`LayerKind::Canvas`], with `bounds` in the
    /// node's border-box space. The node's own list paints below the layer; its children and whatever
    /// paints after the node (later siblings, later subtrees of the same layer) paint above it, in a
    /// continuation layer the document adds after the external one. Other kinds are ignored. Not for
    /// scroll containers (their content layer stays below). Recording without it removes the layer.
    pub fn external_layer(&mut self, kind: LayerKind, bounds: Rect) {
        if matches!(kind, LayerKind::Texture(_) | LayerKind::OsLayer(_) | LayerKind::Canvas(_)) {
            *self.external = Some((kind, bounds));
        }
    }
    pub fn service<T: 'static>(&mut self) -> Option<&mut T> {
        self.services.get_mut()
    }
    /// All the services ([`Services::pair`] borrows two at once).
    pub fn services(&mut self) -> &mut Services {
        self.services
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
    pub fn draw_run(&mut self, run: Arc<GlyphRun>, origin: Point, color: skelter_base::color::Color) {
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
        let border = s.border_color.a > 0.0 && s.border_widths != skelter_base::geom::Insets::ZERO;
        if bg_visible || border {
            let q = Quad {
                rect: r,
                radii: s.radii,
                fill: s.background.clone(),
                border_widths: if border { s.border_widths } else { skelter_base::geom::Insets::ZERO },
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
                fill: Fill::Solid(skelter_base::color::Color::TRANSPARENT),
                border_widths: skelter_base::geom::Insets::all(s.outline_width),
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
pub(crate) fn in_rounded(r: Rect, c: skelter_base::geom::Corners, p: Point) -> bool {
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
    /// Emits `signal` with `arg` from this node (delivered after the event).
    pub fn emit(&mut self, signal: SignalId, arg: Option<Value>) {
        self.emitted.push(Emitted { node: self.node, signal, arg });
    }
    /// The node's virtual rows are stale (its model or row content changed): it realises them
    /// again before the next layout ([`Document::refresh_virtual`]).
    pub fn refresh_virtual(&mut self) {
        let n = self.node;
        self.doc.refresh_virtual(n);
    }
    /// Emits `signal` with `arg` from `node`, the node itself or one of its descendants (a
    /// `tab-view`'s `close-requested` on the `tab` whose close button was clicked), delivered after
    /// the event as [`EventCx::emit`]'s are. False (nothing emitted) for any other node.
    pub fn emit_on(&mut self, node: NodeId, signal: SignalId, arg: Option<Value>) -> bool {
        if !self.doc.contains(node) || !self.doc.is_ancestor_or_self(self.node, node) {
            return false;
        }
        self.emitted.push(Emitted { node, signal, arg });
        true
    }
    /// Writes a prop of `node`, the node itself or one of its descendants, as native-owned state
    /// ([`EventCx::set_native`]): a table writes back the width of the `table-column` the user
    /// resized. An error for any other node.
    pub fn set_native_on(&mut self, node: NodeId, prop: PropId, v: Value) -> Result<(), crate::PropError> {
        if !self.doc.contains(node) || !self.doc.is_ancestor_or_self(self.node, node) {
            return Err(crate::PropError::Stale(node));
        }
        self.doc.set_prop_native(node, prop, v).map(|_| ())
    }
    /// Emits `signal` and, when JS listens and the document has a sync deliverer
    /// ([`Document::set_sync_delivery`]), delivers it now and reports what the listeners did, so a
    /// default action can honour `preventDefault` (a checkbox keeps its state when its cancelable
    /// `changed` is prevented). Without a deliverer it is queued as [`EventCx::emit`] does and the
    /// outcome is empty.
    pub fn emit_sync(&mut self, signal: SignalId, arg: Option<Value>) -> crate::kind::Outcome {
        match crate::input::dispatch::deliver_now(self.doc, self.node, signal, arg) {
            Ok(o) => o,
            Err(arg) => {
                self.emit(signal, arg);
                crate::kind::Outcome::default()
            }
        }
    }
    /// Scrolls the node (a scroll container, or a kind with [`Kind::self_scroll`](crate::Kind::self_scroll))
    /// to `offset` and sends the offset to the compositor at once, without waiting for a frame.
    pub fn scroll_fast(&mut self, offset: skelter_base::geom::Vec2) {
        let n = self.node;
        self.doc.scroll_fast(n, offset);
    }
    /// Shows the caret at `rect` (local coordinates; `None` hides it) through a fast commit: the
    /// compositor draws and blinks it, so a caret move costs no frame. It goes out in the space of
    /// the node's transform node, in its `caret-color` (`color` when `auto`); while the node isn't rendered the caret is
    /// hidden and comes back once it is (`crate::caret`).
    pub fn set_caret(&mut self, rect: Option<Rect>) {
        let n = self.node;
        self.doc.set_caret(n, rect);
    }
    /// The node now has a non-empty text selection: with single selection on (GTK), the previous
    /// owner's [`Kind::selection_lost`](crate::Kind::selection_lost) collapses its own.
    pub fn claim_selection(&mut self) {
        if !self.doc.single_selection {
            return;
        }
        let previous = self.doc.selection_owner.replace(self.node);
        if let Some(p) = previous.filter(|&p| p != self.node && self.doc.contains(p))
            && let Some(i) = self.doc.idx(p)
            && let Some(mut k) = self.doc.kinds[i].take()
        {
            {
                let mut cx = EventCx { doc: self.doc, node: p, emitted: self.emitted, pointer: None };
                k.selection_lost(&mut cx);
            }
            self.doc.restore_kind(p, k);
        }
    }
    /// The node's selection collapsed: it no longer owns the one selection.
    pub fn release_selection(&mut self) {
        if self.doc.selection_owner == Some(self.node) {
            self.doc.selection_owner = None;
        }
    }
    /// Asks the document to deliver `msg` to the members of the node's peer group that `to` picks
    /// ([`Kind::peer`](crate::Kind::peer)), after this event, in order.
    pub fn peer_request(&mut self, to: crate::kind::PeerTarget, msg: crate::kind::PeerMsg) {
        self.doc.peer_queue.push_back((self.node, to, msg));
    }
    /// Moves keyboard focus to the node of this window whose `anchor` is `anchor` (a label's
    /// `for`); false when none can take it (now, or after the next restyle: [`Document::focus`]).
    pub fn focus_anchor(&mut self, anchor: &str) -> bool {
        let Some(w) = self.doc.window_of(self.node) else { return false };
        let Some(n) = self.doc.find_anchor(w, anchor) else { return false };
        self.doc.focus(n, crate::kind::FocusReason::Programmatic);
        self.doc.focused(w) == Some(n) || self.doc.pending_focus(w) == Some(n)
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
    pub fn time(&self) -> skelter_base::time::Time {
        self.doc.input.now
    }
    /// Delivers [`Event::Timer`](crate::Event::Timer) to this node at `at` (replacing its earlier
    /// deadline); the app's loop asks [`Document::next_timer`] and calls [`Document::run_timers`].
    pub fn wake_at(&mut self, at: skelter_base::time::Time) {
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
    /// During a `FocusIn` that a Tab caused: whether it went backwards (Shift+Tab); `None` for any
    /// other focus move. A node that manages focus inside itself (an egui island) starts at its last
    /// part for Shift+Tab, its first for Tab.
    pub fn tab_direction(&self) -> Option<bool> {
        self.doc.window_of(self.node).and_then(|w| self.doc.window(w)).and_then(|w| w.tab_direction)
    }
    /// While this node has focus: the part of it that has focus (a virtual child's index; `None`:
    /// the node itself), which AccessKit then reports as focused ([`crate::FocusTarget`]).
    pub fn set_focus_sub(&mut self, sub: Option<u32>) {
        let Some(w) = self.doc.window_of(self.node) else { return };
        if self.doc.focused(w) == Some(self.node) {
            self.doc.set_focus_sub(w, sub);
        }
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
    /// The node's accessibility data changed (a text field's text, selection or runs): it is sent
    /// again with the next frame, as [`PropCx::request_a11y`] does for a prop.
    pub fn request_a11y(&mut self) {
        self.doc.mark(self.node, Dirty::A11Y);
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
    /// Emits `signal` on the node for JS, queued with the frame's other events (a virtual list's
    /// `setup`/`bind`/`unbind`/`teardown` row requests: JS answers after the frame and the node
    /// asks for its rows again, [`Document::refresh_virtual`]).
    pub fn emit(&mut self, signal: SignalId, arg: Option<Value>) {
        let node = self.node;
        crate::input::dispatch::queue_emitted(self.doc, Emitted { node, signal, arg });
    }
}

/// The text of `v`'s descendants in tree order (their `aria-label`, else their text content),
/// separated by spaces: the name of a button with a text child.
fn descendant_text(v: &NodeView) -> String {
    let mut out = String::new();
    let mut stack: Vec<NodeView> = v.children().iter().rev().filter_map(|&c| v.view(c)).collect();
    while let Some(c) = stack.pop() {
        let own = c.prop_str(common::ARIA_LABEL).filter(|s| !s.is_empty()).unwrap_or(c.text());
        if !own.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(own.trim());
            // a labelled child names its subtree
            if c.prop_str(common::ARIA_LABEL).is_some_and(|s| !s.is_empty()) {
                continue;
            }
        }
        stack.extend(c.children().iter().rev().filter_map(|&k| v.view(k)));
    }
    out
}

/// [`Kind::a11y`](crate::Kind::a11y): the node's AccessKit node, prefilled with its role, bounds and
/// children.
pub struct A11yCx<'a> {
    pub node: NodeView<'a>,
    pub out: &'a mut accesskit::Node,
    /// The virtual children added so far ([`A11yCx::add_virtual_child`]), each with the virtual
    /// child it is under ([`A11yCx::add_virtual_child_of`]; `None`: under the node).
    pub(crate) virtual_children: &'a mut Vec<(accesskit::Node, Option<u32>)>,
}

impl<'a> std::ops::Deref for A11yCx<'a> {
    type Target = NodeView<'a>;
    fn deref(&self) -> &NodeView<'a> {
        &self.node
    }
}

impl A11yCx<'_> {
    /// Adds a virtual child: an AccessKit node for a part the kind draws without a node of its own
    /// (a select's option), placed after the real children, its bounds in the node's own space. It
    /// is sent with the node every time the node is, under [`crate::a11y::virtual_id`]; an action
    /// on it reaches the kind as [`Event::A11yChild`](crate::Event::A11yChild) with the returned
    /// index (the order of the calls). A node has at most [`crate::a11y::MAX_VIRTUAL_CHILDREN`];
    /// further ones are dropped (`None`).
    pub fn add_virtual_child(&mut self, node: accesskit::Node) -> Option<u32> {
        if self.virtual_children.len() >= crate::a11y::MAX_VIRTUAL_CHILDREN {
            return None;
        }
        self.virtual_children.push((node, None));
        Some(self.virtual_children.len() as u32 - 1)
    }

    /// Adds a virtual child under the earlier virtual child `parent` (a table row's cell), after
    /// that one's other children. Its bounds are still in the node's own space (virtual children
    /// have no transform), its index and its actions as [`A11yCx::add_virtual_child`]'s. `None`
    /// when `parent` is not an earlier virtual child or the node has no room left.
    pub fn add_virtual_child_of(&mut self, parent: u32, node: accesskit::Node) -> Option<u32> {
        if parent as usize >= self.virtual_children.len() || self.virtual_children.len() >= crate::a11y::MAX_VIRTUAL_CHILDREN {
            return None;
        }
        self.virtual_children.push((node, Some(parent)));
        Some(self.virtual_children.len() as u32 - 1)
    }

    /// The defaults: the label from `aria-label` or the text content, disabled, focusable.
    pub fn default_a11y(&mut self) {
        let v = self.node;
        let label = v.entry.spec.name_from.iter().find_map(|src| {
            let s: std::borrow::Cow<str> = match *src {
                crate::kind::NameSource::AriaLabel => v.prop_str(common::ARIA_LABEL)?.into(),
                crate::kind::NameSource::Prop(p) => v.prop_str(v.entry.spec.prop_named(p)?)?.into(),
                crate::kind::NameSource::Text => v.text().into(),
                // accesskit computes names from descendants for a few roles only: concatenate here
                crate::kind::NameSource::Children => descendant_text(&v).into(),
            };
            (!s.is_empty()).then_some(s)
        });
        if let Some(l) = label {
            self.out.set_label(&*l);
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
