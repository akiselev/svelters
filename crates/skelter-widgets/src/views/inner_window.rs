//! `inner-window`: a floating window inside an OS window (svelters-egui `widgets/dialog.rs`,
//! `overlays.rs`). It owns its frame while the user moves or resizes it (architecture §4.5: the
//! frame is native-owned state during a drag; JS writes to `x`/`y`/`width`/`height` are refused as
//! stale meanwhile).
//!
//! It computes:
//! - **move**: a press on the title bar drags the frame once the pointer passed the platform's drag
//!   threshold, clamped so the frame stays inside the bounds;
//! - **resize**: eight handles straddling the border (edges [`EDGE_HIT`] thick, corners [`CORNER`]
//!   square); a left or top handle keeps the opposite edge still; never below the minimum size nor
//!   past the bounds;
//! - **title bar buttons**: close (`close` signal) and collapse (also a double click on the title
//!   bar), which folds it to its title bar;
//! - **raise**: any press on it emits `raise` (the overlay stack moves it to the top of its band).
//!
//! Look (UA tables, restyled with `::part(titlebar)`, `::part(close)`, `::part(collapse)`): the raised
//! window surface with `window-shadow`, a hairline under the title bar, and toolbar-style buttons
//! with `:hover` and `:active`. The heading prop is `heading` (the common `title` stays the tooltip).
//!
//! Positions in [`InnerWindow::pointer`] are window points; [`Kind::event`] gets frame-local ones
//! and converts with the frame it had when the event arrived.

use super::common::{emit, ms};
use crate::controls::common::host_view;
use crate::interact::StateBits;
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, ChildCx, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, StyleCx, Value, ValueType};
use skelter_base::accesskit;
use skelter_base::color::Color;
use skelter_base::cursor::CursorIcon;
use skelter_base::display::{Line, LineCap, Prim};
use skelter_base::event::{MouseButton, PointerEvent, PointerPhase};
use skelter_base::geom::{Point, Rect, Size, Vec2};
use skelter_base::time::Time;

/// The hit thickness of an edge handle (straddling the border).
pub const EDGE_HIT: f32 = 6.0;
/// The side of a corner handle.
pub const CORNER: f32 = 12.0;
pub const TITLE_H: f32 = 28.0;
pub const MIN_SIZE: Size = Size::new(120.0, 60.0);
const BUTTON: f32 = 20.0;

/// Which edges a resize handle moves: per axis, -1 (left/top), 0 or +1 (right/bottom).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edges {
    pub x: i8,
    pub y: i8,
}

impl Edges {
    pub fn cursor(self) -> CursorIcon {
        match (self.x, self.y) {
            (0, _) => CursorIcon::NsResize,
            (_, 0) => CursorIcon::EwResize,
            (a, b) if a == b => CursorIcon::NwseResize,
            _ => CursorIcon::NeswResize,
        }
    }
}

/// What is under a point of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Resize(Edges),
    Close,
    Collapse,
    TitleBar,
    Body,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    /// Pressed on the title bar; moving once past the threshold.
    Move { start: Point, frame: Rect, armed: bool },
    Resize { start: Point, frame: Rect, edges: Edges },
    /// Pressed on a title bar button: it acts on release over the same button.
    Button(Zone),
}

/// The window's UA sheet: the raised surface, widget border, window radius and `window-shadow`.
pub const UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("surface-raised")),
    ("border-color", Token("border-widget")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("window-radius")),
    ("box-shadow", Token("window-shadow")),
])]);

/// `::part(titlebar)`'s UA sheet: the heading in the strong foreground; `border-color` is the
/// hairline under the bar.
pub const TITLEBAR_UA: Ua = Ua(&[UaRule::base(&[("color", Token("fg-strong")), ("font-weight", Number(600.0)), ("border-color", Token("border-widget"))])]);

/// `::part(close)` and `::part(collapse)`'s UA sheet: toolbar buttons.
pub const BUTTON_UA: Ua = Ua(&[
    UaRule::base(&[("color", Token("muted")), ("border-radius", Token("radius")), ("cursor", Keyword("pointer"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("toolbar-hover")), ("color", Token("fg"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("toolbar-active"))]),
]);

kind_props! {
    /// The inner window's props.
    pub struct InnerWindowProps;
    /// The title bar's text.
    heading "heading": String = Str("") ; Str, Paint;
    /// The frame (in its parent's points: the node is laid out there), native-owned while it is
    /// moved or resized.
    x "x": f64 = F64(40.0) ; F64, Layout, bindable;
    y "y": f64 = F64(40.0) ; F64, Layout, bindable;
    width "width": f64 = F64(320.0) ; F64, Layout, bindable;
    height "height": f64 = F64(240.0) ; F64, Layout, bindable;
    resizable "resizable": bool = Bool(true) ; Bool, Paint;
    collapsible "collapsible": bool = Bool(true) ; Bool, Paint;
    closable "closable": bool = Bool(true) ; Bool, Paint;
    /// Folded to its title bar.
    collapsed "collapsed": bool = Bool(false) ; Bool, Layout, bindable;
    min_width "min-width": f64 = F64(120.0) ; F64, Layout;
    min_height "min-height": f64 = F64(60.0) ; F64, Layout;
}

const RECT_ARGS: &[(&str, ValueType)] = &[("x", ValueType::F64), ("y", ValueType::F64), ("width", ValueType::F64), ("height", ValueType::F64)];

pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("raise", &[], "A press on it: bring it to the top of its band."),
    SignalSpec::new("close", &[], "Its close button was clicked.").cancelable(),
    SignalSpec::new("toggle", &[("expanded", ValueType::Bool)], "It collapsed or expanded."),
    SignalSpec::new("move", RECT_ARGS, "A move ended at this frame."),
    SignalSpec::new("resize", RECT_ARGS, "A resize ended at this frame."),
    SignalSpec::new("drag-started", &[], ""),
    SignalSpec::new("drag-stopped", &[], ""),
];

pub static SPEC: KindSpec = KindSpec {
    tag: "inner-window",
    props: InnerWindowProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Window,
    name_from: NameFrom::Prop("heading"),
    text_prop: None,
    focusable: false,
};

/// An `inner-window` node.
#[derive(Clone, Debug, PartialEq)]
pub struct InnerWindow {
    pub props: InnerWindowProps,
    /// Window points; the expanded size (collapsed shows only the title bar). The `x`/`y`/`width`/
    /// `height` props read it.
    pub frame: Rect,
    /// Where the frame must stay (the window's content area).
    pub bounds: Rect,
    gesture: Option<Gesture>,
    /// The last title bar click (double click collapses).
    last_title_click: Option<Time>,
    /// The title bar button under the pointer (`:hover`).
    hot: Option<Zone>,
}

impl Default for InnerWindow {
    fn default() -> Self {
        let props = InnerWindowProps::default();
        InnerWindow {
            frame: Rect::from_origin_size(Point::new(props.x as f32, props.y as f32), Size::new(props.width as f32, props.height as f32)),
            props,
            bounds: Rect::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY),
            gesture: None,
            last_title_click: None,
            hot: None,
        }
    }
}

/// `r` moved (not resized) to lie inside `b` (its start inside when it is larger).
fn clamp_into(r: Rect, b: Rect) -> Rect {
    let s = r.size();
    let x = r.min.x.min(b.max.x - s.w).max(b.min.x);
    let y = r.min.y.min(b.max.y - s.h).max(b.min.y);
    Rect::from_origin_size(Point::new(x, y), s)
}

impl InnerWindow {
    /// The smallest frame a resize leaves.
    pub fn min(&self) -> Size {
        Size::new(self.props.min_width.max(0.0) as f32, self.props.min_height.max(0.0) as f32)
    }

    /// The body: the frame below the title bar (where the content goes).
    pub fn body(&self) -> Rect {
        Rect::new(self.frame.min.x, self.frame.min.y + TITLE_H, self.frame.max.x, self.frame.max.y.max(self.frame.min.y + TITLE_H))
    }

    /// The frame as drawn (only the title bar while collapsed).
    pub fn shown_frame(&self) -> Rect {
        if self.props.collapsed { Rect::new(self.frame.min.x, self.frame.min.y, self.frame.max.x, self.frame.min.y + TITLE_H) } else { self.frame }
    }

    /// A move or resize is in progress (its frame is native-owned).
    pub fn dragging(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Move { armed: true, .. }) | Some(Gesture::Resize { .. }))
    }

    fn close_rect(&self) -> Option<Rect> {
        let f = self.frame;
        self.props.closable.then(|| Rect::from_origin_size(Point::new(f.max.x - 4.0 - BUTTON, f.min.y + (TITLE_H - BUTTON) / 2.0), Size::new(BUTTON, BUTTON)))
    }

    fn collapse_rect(&self) -> Option<Rect> {
        let f = self.frame;
        let right = if self.props.closable { f.max.x - 4.0 - BUTTON - 2.0 } else { f.max.x - 4.0 };
        self.props.collapsible.then(|| Rect::from_origin_size(Point::new(right - BUTTON, f.min.y + (TITLE_H - BUTTON) / 2.0), Size::new(BUTTON, BUTTON)))
    }

    /// What is under `p` (window points), if anything.
    pub fn zone(&self, p: Point) -> Option<Zone> {
        let f = self.shown_frame();
        if self.props.resizable && !self.props.collapsed {
            let h = EDGE_HIT / 2.0;
            let outer = f.inflate(h);
            if outer.contains(p) {
                let near = |v: f32, lo: f32, hi: f32| -> i8 {
                    if v < lo + h {
                        -1
                    } else if v >= hi - h {
                        1
                    } else {
                        0
                    }
                };
                let (mut ex, mut ey) = (near(p.x, f.min.x, f.max.x), near(p.y, f.min.y, f.max.y));
                // a corner takes CORNER points along both edges
                if ex != 0 && ey == 0 {
                    ey = if p.y < f.min.y + CORNER { -1 } else if p.y >= f.max.y - CORNER { 1 } else { 0 };
                } else if ey != 0 && ex == 0 {
                    ex = if p.x < f.min.x + CORNER { -1 } else if p.x >= f.max.x - CORNER { 1 } else { 0 };
                }
                if ex != 0 || ey != 0 {
                    return Some(Zone::Resize(Edges { x: ex, y: ey }));
                }
            }
        }
        if !f.contains(p) {
            return None;
        }
        if self.close_rect().is_some_and(|r| r.contains(p)) {
            return Some(Zone::Close);
        }
        if self.collapse_rect().is_some_and(|r| r.contains(p)) {
            return Some(Zone::Collapse);
        }
        if p.y < f.min.y + TITLE_H {
            return Some(Zone::TitleBar);
        }
        Some(Zone::Body)
    }

    /// The frame for a resize by `d` from `frame` with `edges`, within the minimum size and bounds.
    fn resized(&self, frame: Rect, edges: Edges, d: Vec2) -> Rect {
        let mut r = frame;
        let b = self.bounds;
        let min = self.min();
        match edges.x {
            1 => r.max.x = (frame.max.x + d.x).min(b.max.x).max(frame.min.x + min.w),
            -1 => r.min.x = (frame.min.x + d.x).max(b.min.x).min(frame.max.x - min.w),
            _ => {}
        }
        match edges.y {
            1 => r.max.y = (frame.max.y + d.y).min(b.max.y).max(frame.min.y + min.h),
            -1 => r.min.y = (frame.min.y + d.y).max(b.min.y).min(frame.max.y - min.h),
            _ => {}
        }
        r
    }

    /// One pointer event at window-point `p`. Returns whether it is taken.
    pub fn pointer(&mut self, cx: &mut EventCx<'_>, pe: &PointerEvent, p: Point) -> Handled {
        let primary = pe.button == Some(MouseButton::Primary);
        match pe.phase {
            PointerPhase::Down if primary => {
                let Some(zone) = self.zone(p) else { return Handled::No };
                emit(cx, &SPEC, "raise", Vec::new());
                cx.focus_request = Some(true);
                self.gesture = match zone {
                    Zone::Resize(edges) => {
                        cx.capture = Some(true);
                        emit(cx, &SPEC, "drag-started", Vec::new());
                        Some(Gesture::Resize { start: p, frame: self.frame, edges })
                    }
                    Zone::TitleBar => {
                        cx.capture = Some(true);
                        let double = self.last_title_click.is_some_and(|t| cx.now - t <= ms(cx.policy.double_click_ms));
                        if double && self.props.collapsible {
                            self.last_title_click = None;
                            self.toggle_collapsed(cx);
                            None
                        } else {
                            self.last_title_click = Some(cx.now);
                            Some(Gesture::Move { start: p, frame: self.frame, armed: false })
                        }
                    }
                    Zone::Close | Zone::Collapse => {
                        cx.capture = Some(true);
                        Some(Gesture::Button(zone))
                    }
                    Zone::Body => None,
                };
                Handled::Yes
            }
            PointerPhase::Move => match self.gesture {
                Some(Gesture::Move { start, frame, armed }) => {
                    let d = p - start;
                    let now_armed = armed || d.length() >= cx.policy.drag_threshold;
                    self.gesture = Some(Gesture::Move { start, frame, armed: now_armed });
                    if now_armed && !armed {
                        emit(cx, &SPEC, "drag-started", Vec::new());
                    }
                    if now_armed {
                        let to = clamp_into(frame.translate(d), self.bounds);
                        if to != self.frame {
                            self.frame = to;
                            cx.cursor = Some(CursorIcon::Grabbing);
                            self.frame_changed(cx);
                        }
                    }
                    Handled::Yes
                }
                Some(Gesture::Resize { start, frame, edges }) => {
                    let to = self.resized(frame, edges, p - start);
                    if to != self.frame {
                        self.frame = to;
                        self.frame_changed(cx);
                    }
                    cx.cursor = Some(edges.cursor());
                    Handled::Yes
                }
                Some(Gesture::Button(_)) => Handled::Yes,
                None => {
                    let zone = self.zone(p);
                    self.set_hot(cx, zone);
                    match zone {
                        Some(Zone::Resize(e)) => cx.cursor = Some(e.cursor()),
                        Some(_) => cx.cursor = Some(CursorIcon::Default),
                        None => return Handled::No,
                    }
                    Handled::Yes
                }
            },
            PointerPhase::Up | PointerPhase::Cancel => {
                let Some(g) = self.gesture.take() else { return if self.zone(p).is_some() { Handled::Yes } else { Handled::No } };
                cx.capture = Some(false);
                let cancelled = pe.phase == PointerPhase::Cancel;
                match g {
                    Gesture::Move { frame, armed, .. } => {
                        if armed && self.frame != frame {
                            self.end_drag(cx, "move");
                        }
                    }
                    Gesture::Resize { frame, .. } => {
                        if self.frame != frame {
                            self.end_drag(cx, "resize");
                        } else {
                            emit(cx, &SPEC, "drag-stopped", Vec::new());
                        }
                    }
                    Gesture::Button(zone) if !cancelled && self.zone(p) == Some(zone) => match zone {
                        Zone::Close => emit(cx, &SPEC, "close", Vec::new()),
                        Zone::Collapse => self.toggle_collapsed(cx),
                        _ => {}
                    },
                    Gesture::Button(_) => {}
                }
                Handled::Yes
            }
            _ => {
                if self.zone(p).is_some() {
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
        }
    }

    fn end_drag(&mut self, cx: &mut EventCx<'_>, what: &'static str) {
        let f = self.frame;
        let args = [f.min.x, f.min.y, f.width(), f.height()].map(|v| Value::F64(v as f64)).to_vec();
        emit(cx, &SPEC, what, args);
        emit(cx, &SPEC, "drag-stopped", Vec::new());
    }

    fn toggle_collapsed(&mut self, cx: &mut EventCx<'_>) {
        self.props.collapsed = !self.props.collapsed;
        emit(cx, &SPEC, "toggle", vec![Value::Bool(!self.props.collapsed)]);
        cx.prop_changed(P::collapsed.id());
    }

    /// The frame moved natively: the geometry props follow, as native state.
    fn frame_changed(&mut self, cx: &mut EventCx<'_>) {
        let f = self.frame;
        for (id, v, old) in [
            (P::x.id(), f.min.x, self.props.x),
            (P::y.id(), f.min.y, self.props.y),
            (P::width.id(), f.width(), self.props.width),
            (P::height.id(), f.height(), self.props.height),
        ] {
            if v as f64 != old {
                cx.prop_changed(id);
            }
        }
        self.sync_props();
    }

    fn sync_props(&mut self) {
        let f = self.frame;
        (self.props.x, self.props.y, self.props.width, self.props.height) = (f.min.x as f64, f.min.y as f64, f.width() as f64, f.height() as f64);
    }

    fn set_hot(&mut self, cx: &mut EventCx<'_>, zone: Option<Zone>) {
        let hot = zone.filter(|z| matches!(z, Zone::Close | Zone::Collapse));
        if hot != self.hot {
            self.hot = hot;
            cx.repaint = true;
        }
    }

    /// The bounds changed (the OS window resized): keeps the frame inside.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.frame = clamp_into(self.frame, bounds);
        self.sync_props();
    }

    /// A title bar button's view in its state (`:hover` under the pointer, `:active` while pressed).
    fn button_view<'a>(&self, v: &StyleView<'a>, zone: Zone) -> StyleView<'a> {
        let mut s = v.state();
        s.set(StateBits::HOVERED, self.hot == Some(zone));
        s.set(StateBits::PRESSED, self.gesture == Some(Gesture::Button(zone)));
        v.in_state(s)
    }
}

impl Kind for InnerWindow {
    fn parts() -> &'static [&'static str] {
        &["titlebar", "close", "collapse"]
    }
    fn places_children() -> bool {
        true
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        let geometry = [P::x.id(), P::y.id(), P::width.id(), P::height.id()].contains(&prop);
        if geometry && self.dragging() {
            return Err(PropError::Stale);
        }
        self.props.set(prop, v)?;
        if geometry {
            let (f, p, min) = (self.frame, &self.props, self.min());
            self.frame = if prop == P::x.id() || prop == P::y.id() {
                clamp_into(f.translate(Vec2::new(p.x as f32 - f.min.x, p.y as f32 - f.min.y)), self.bounds)
            } else {
                Rect::from_origin_size(f.min, Size::new((p.width as f32).max(min.w), (p.height as f32).max(min.h)))
            };
            self.sync_props();
        }
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    /// The node is the frame: laid out out of flow at `x`/`y` in its parent, at the shown frame's size.
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        let f = self.shown_frame();
        s.position = taffy::Position::Absolute;
        s.inset = taffy::Rect {
            left: taffy::LengthPercentageAuto::length(f.min.x),
            top: taffy::LengthPercentageAuto::length(f.min.y),
            right: taffy::LengthPercentageAuto::auto(),
            bottom: taffy::LengthPercentageAuto::auto(),
        };
        s.size = taffy::Size { width: taffy::Dimension::length(f.width()), height: taffy::Dimension::length(f.height()) };
        s
    }

    /// Moves and resizes stay in the window (the room, in the parent's points as the frame is).
    fn room(&mut self, room: Rect) {
        self.set_bounds(room.translate(self.frame.min.to_vec()));
    }

    /// The content fills the body (the frame below the title bar, in the node's local points);
    /// while collapsed it is not laid out, painted or hit.
    fn child_layout(&self, _cx: &ChildCx, style: &mut taffy::Style) -> bool {
        if self.props.collapsed {
            style.display = taffy::Display::None;
            return false;
        }
        let body = self.body().translate(Vec2::new(-self.frame.min.x, -self.frame.min.y));
        style.position = taffy::Position::Absolute;
        style.inset = taffy::Rect {
            left: taffy::LengthPercentageAuto::length(body.min.x),
            top: taffy::LengthPercentageAuto::length(body.min.y),
            right: taffy::LengthPercentageAuto::auto(),
            bottom: taffy::LengthPercentageAuto::auto(),
        };
        style.size = taffy::Size { width: taffy::Dimension::length(body.width()), height: taffy::Dimension::length(body.height()) };
        true
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = host_view(&cx.style, UA);
        let bar = host.part(cx.style.part_css("titlebar"), TITLEBAR_UA);
        let close_v = self.button_view(&host.part(cx.style.part_css("close"), BUTTON_UA), Zone::Close);
        let collapse_v = self.button_view(&host.part(cx.style.part_css("collapse"), BUTTON_UA), Zone::Collapse);
        // local coordinates: the node is the frame
        let local = Rect::from_origin_size(Point::ZERO, self.shown_frame().size());
        let o = Vec2::new(-self.frame.min.x, -self.frame.min.y);
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &host, local);
        let line = |pc: &mut PaintCx, from: Point, to: Point, width: f32, color: Color| {
            let ink = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y)).inflate(width);
            pc.push(Prim::Line(Line { from, to, width, color, cap: LineCap::Round, dash: None, wavy: false }), ink);
        };
        if !self.props.collapsed {
            let hair = pc.snap_width(1.0);
            line(&mut pc, Point::new(0.0, TITLE_H), Point::new(local.max.x, TITLE_H), hair, bar.color("border-color").unwrap_or(Color::TRANSPARENT));
        }
        let title_right = self.collapse_rect().or(self.close_rect()).map_or(local.max.x, |r| r.min.x + o.x) - 8.0;
        let row = Rect::new(8.0, 0.0, title_right.max(8.0), TITLE_H);
        let heading = paint::elide(pc.text, &bar, &self.props.heading, paint::Elide::End, row.width());
        let layout = paint::text_layout(pc.text, &bar, &heading, None);
        paint::paint_text(&mut pc, &bar, &layout, row, Align::Start);
        if let Some(r) = self.close_rect() {
            let r = r.translate(o);
            paint::paint_box(&mut pc, &close_v, r);
            let c = r.inflate(-6.0);
            let color = close_v.color("color").unwrap_or(Color::BLACK);
            line(&mut pc, c.min, c.max, 1.5, color);
            line(&mut pc, Point::new(c.max.x, c.min.y), Point::new(c.min.x, c.max.y), 1.5, color);
        }
        if let Some(r) = self.collapse_rect() {
            let r = r.translate(o);
            paint::paint_box(&mut pc, &collapse_v, r);
            let c = r.inflate(-6.0);
            let y = (c.min.y + c.max.y) / 2.0;
            line(&mut pc, Point::new(c.min.x, y), Point::new(c.max.x, y), 1.5, collapse_v.color("color").unwrap_or(Color::BLACK));
        }
        paint::paint_outline(&mut pc, &host, local);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::Pointer(pe) => {
                let p = pe.pos + self.frame.min.to_vec();
                self.pointer(cx, pe, p)
            }
            Event::Hover(false) => {
                self.set_hot(cx, None);
                Handled::No
            }
            // the press ended without a release: as a cancel where the pointer last was
            Event::CaptureLost if self.gesture.is_some() => {
                let pe = PointerEvent {
                    phase: PointerPhase::Cancel,
                    pointer: 0,
                    kind: skelter_base::event::PointerType::Mouse,
                    button: None,
                    buttons: 0,
                    pos: Point::ZERO,
                    modifiers: Default::default(),
                    pressure: 0.0,
                    tilt: Vec2::ZERO,
                    history: Default::default(),
                };
                let p = self.frame.min;
                self.pointer(cx, &pe, p)
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Window);
        if !self.props.heading.is_empty() {
            cx.node.set_label(self.props.heading.clone());
        }
        if self.props.collapsible {
            cx.node.set_expanded(!self.props.collapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::{Native, Policy};
    use skelter_base::event::{Modifiers, PointerType};

    fn pe(phase: PointerPhase) -> PointerEvent {
        PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
            buttons: 0,
            pos: Point::ZERO,
            modifiers: Modifiers::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: Default::default(),
        }
    }

    fn win() -> InnerWindow {
        let mut w = InnerWindow { frame: Rect::new(100.0, 100.0, 300.0, 250.0), bounds: Rect::new(0.0, 0.0, 800.0, 600.0), ..InnerWindow::default() };
        w.props.heading = "Layers".into();
        w.sync_props();
        w
    }

    fn drive(w: &mut InnerWindow, pol: &Policy, now: i64, steps: &[(PointerPhase, f32, f32)]) -> Vec<&'static str> {
        let mut names = Vec::new();
        let mut st = StateBits::NONE;
        for &(ph, x, y) in steps {
            let mut cx = EventCx::new(Time(now as u64 * 1_000_000), Size::new(800.0, 600.0), &mut st, pol);
            w.pointer(&mut cx, &pe(ph), Point::new(x, y));
            names.extend(cx.emitted.iter().map(|e| SPEC.signals[e.signal.0 as usize].name));
        }
        names
    }

    #[test]
    fn title_bar_drag_waits_for_the_threshold_and_clamps() {
        let pol = *Native::Gtk.policy(); // threshold 8, double click 400 ms
        let mut w = win();
        let s = drive(&mut w, &pol, 0, &[(PointerPhase::Down, 150.0, 110.0), (PointerPhase::Move, 155.0, 110.0)]);
        assert_eq!(s, vec!["raise"]);
        assert_eq!(w.frame.min, Point::new(100.0, 100.0), "below the threshold");
        assert!(!w.dragging());
        drive(&mut w, &pol, 0, &[(PointerPhase::Move, 170.0, 130.0)]);
        assert!(w.dragging());
        assert_eq!(w.frame.min, Point::new(120.0, 120.0));
        // dragged far past the bottom right: clamped inside the bounds
        drive(&mut w, &pol, 0, &[(PointerPhase::Move, 2000.0, 2000.0)]);
        assert_eq!(w.frame.max, Point::new(800.0, 600.0));
        assert_eq!(w.frame.size(), Size::new(200.0, 150.0));
        let s = drive(&mut w, &pol, 0, &[(PointerPhase::Up, 2000.0, 2000.0)]);
        assert_eq!(s, vec!["move", "drag-stopped"]);
        assert!(!w.dragging());
    }

    #[test]
    fn edges_resize_within_min_size_and_bounds() {
        let pol = *Native::Gtk.policy();
        let mut w = win();
        // the bottom-right corner
        assert_eq!(w.zone(Point::new(299.0, 249.0)), Some(Zone::Resize(Edges { x: 1, y: 1 })));
        assert_eq!(w.zone(Point::new(301.0, 180.0)), Some(Zone::Resize(Edges { x: 1, y: 0 })));
        assert_eq!(w.zone(Point::new(100.0, 105.0)), Some(Zone::Resize(Edges { x: -1, y: -1 })));
        assert_eq!(w.zone(Point::new(200.0, 200.0)), Some(Zone::Body));
        drive(&mut w, &pol, 0, &[(PointerPhase::Down, 299.0, 249.0), (PointerPhase::Move, 359.0, 289.0)]);
        assert_eq!(w.frame, Rect::new(100.0, 100.0, 360.0, 290.0));
        // past the bounds
        drive(&mut w, &pol, 0, &[(PointerPhase::Move, 5000.0, 5000.0)]);
        assert_eq!(w.frame.max, Point::new(800.0, 600.0));
        // below the minimum
        drive(&mut w, &pol, 0, &[(PointerPhase::Move, -5000.0, -5000.0)]);
        assert_eq!(w.frame, Rect::new(100.0, 100.0, 220.0, 160.0));
        let s = drive(&mut w, &pol, 0, &[(PointerPhase::Up, 0.0, 0.0)]);
        assert_eq!(s, vec!["resize", "drag-stopped"]);
        // the left edge keeps the right edge still
        let mut w = win();
        drive(&mut w, &pol, 0, &[(PointerPhase::Down, 99.0, 180.0), (PointerPhase::Move, 500.0, 180.0)]);
        assert_eq!(w.frame, Rect::new(180.0, 100.0, 300.0, 250.0));
        // a JS geometry write during the drag is stale
        assert_eq!(w.set_prop(P::x.id(), &Value::F64(0.0)), Err(PropError::Stale));
    }

    #[test]
    fn close_collapse_and_double_click() {
        let pol = *Native::Gtk.policy();
        let mut w = win();
        let close = w.close_rect().unwrap();
        let c = Point::new((close.min.x + close.max.x) / 2.0, (close.min.y + close.max.y) / 2.0);
        assert_eq!(w.zone(c), Some(Zone::Close));
        // release away from the button: nothing
        let s = drive(&mut w, &pol, 0, &[(PointerPhase::Down, c.x, c.y), (PointerPhase::Up, 150.0, 200.0)]);
        assert_eq!(s, vec!["raise"]);
        let s = drive(&mut w, &pol, 0, &[(PointerPhase::Down, c.x, c.y), (PointerPhase::Up, c.x, c.y)]);
        assert_eq!(s, vec!["raise", "close"]);
        // a double click on the title bar collapses it to its title bar
        drive(&mut w, &pol, 0, &[(PointerPhase::Down, 150.0, 110.0), (PointerPhase::Up, 150.0, 110.0)]);
        let s = drive(&mut w, &pol, 100, &[(PointerPhase::Down, 150.0, 110.0), (PointerPhase::Up, 150.0, 110.0)]);
        assert_eq!(s, vec!["raise", "toggle"]);
        assert!(w.props.collapsed);
        assert_eq!(w.shown_frame().height(), TITLE_H);
        assert_eq!(w.zone(Point::new(150.0, 200.0)), None, "the body is folded away");
        // the bounds shrink: the frame moves back inside
        w.set_bounds(Rect::new(0.0, 0.0, 250.0, 200.0));
        assert_eq!(w.frame.max.x, 250.0);
        assert_eq!(w.get_prop(P::x.id()), Some(Value::F64(50.0)), "the props follow the frame");
    }

    #[test]
    fn geometry_props_follow_the_frame_both_ways() {
        let pol = *Native::Gtk.policy();
        let mut w = win();
        // a move writes x and y back as native state
        let mut st = StateBits::NONE;
        let mut cx = EventCx::new(Time(0), Size::new(800.0, 600.0), &mut st, &pol);
        w.pointer(&mut cx, &pe(PointerPhase::Down), Point::new(150.0, 110.0));
        w.pointer(&mut cx, &pe(PointerPhase::Move), Point::new(170.0, 130.0));
        assert_eq!(cx.changed, vec![P::x.id(), P::y.id()]);
        assert_eq!((w.props.x, w.props.y), (120.0, 120.0));
        w.pointer(&mut cx, &pe(PointerPhase::Up), Point::new(170.0, 130.0));
        // writes from JS move and size the frame, within the minimum and the bounds
        w.set_prop(P::x.id(), &Value::F64(10.0)).unwrap();
        assert_eq!(w.frame.min.x, 10.0);
        w.set_prop(P::width.id(), &Value::F64(50.0)).unwrap();
        assert_eq!(w.frame.width(), 120.0, "min-width");
        assert_eq!(w.get_prop(P::width.id()), Some(Value::F64(120.0)));
        // the common `title` stays the tooltip; the bar shows `heading`
        assert_eq!(SPEC.prop("title"), None);
    }

    #[test]
    fn records_surface_bar_and_buttons() {
        let w = win();
        let (list, res) = testkit::record(&w, w.shown_frame().size(), StateBits::NONE);
        // window-shadow and surface, the hairline, the heading, the × (button box is clear at rest),
        // the collapse bar
        assert_eq!(testkit::kinds(&list), ["shadow", "quad", "line", "glyphs", "line", "line", "line"]);
        assert_eq!(res.runs.len(), 1);
    }

    #[test]
    fn content_fills_the_body_and_hides_when_collapsed() {
        let mut w = InnerWindow::default();
        w.frame = Rect::new(10.0, 20.0, 310.0, 220.0);
        let mut style = taffy::Style::default();
        assert!(w.child_layout(&ChildCx { index: 0, anchors: &[""], slots: &[], children: &[] }, &mut style));
        // in the node's points: the node is the frame (laid out at `x`/`y`, `layout_style`)
        assert_eq!(style.position, taffy::Position::Absolute);
        assert_eq!(style.inset.left, taffy::LengthPercentageAuto::length(0.0));
        assert_eq!(style.inset.top, taffy::LengthPercentageAuto::length(TITLE_H));
        assert_eq!(style.size.width, taffy::Dimension::length(300.0));
        assert_eq!(style.size.height, taffy::Dimension::length(200.0 - TITLE_H));
        w.props.collapsed = true;
        let mut style = taffy::Style::default();
        assert!(!w.child_layout(&ChildCx { index: 0, anchors: &[""], slots: &[], children: &[] }, &mut style));
        assert_eq!(style.display, taffy::Display::None);
    }
}
