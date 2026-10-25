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
//! Positions in [`InnerWindow::pointer`] are window points; [`Kind::event`] gets frame-local ones
//! and converts with the frame it had when the event arrived.

use super::draw;
use super::shim::{Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::cursor::CursorIcon;
use presto_base::event::{MouseButton, PointerEvent, PointerPhase};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::time::Time;

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

static SPEC: KindSpec = KindSpec {
    name: "inner-window",
    props: &[
        prop("title", PropType::Str, ChangeClass::Paint),
        prop("x", PropType::F64, ChangeClass::Property),
        prop("y", PropType::F64, ChangeClass::Property),
        prop("width", PropType::F64, ChangeClass::Layout),
        prop("height", PropType::F64, ChangeClass::Layout),
        prop("resizable", PropType::Bool, ChangeClass::Paint),
        prop("collapsible", PropType::Bool, ChangeClass::Paint),
        prop("closable", PropType::Bool, ChangeClass::Paint),
        prop("collapsed", PropType::Bool, ChangeClass::Layout),
        prop("min-width", PropType::F64, ChangeClass::Layout),
        prop("min-height", PropType::F64, ChangeClass::Layout),
    ],
    signals: &[
        signal("raise", true, false),
        signal("close", false, true),
        signal("toggle", true, false),
        signal("move", true, false),
        signal("resize", true, false),
        signal("drag-started", true, false),
        signal("drag-stopped", true, false),
    ],
    slots: &[""],
    role: accesskit::Role::Window,
};

/// An `inner-window` node.
#[derive(Clone, Debug, PartialEq)]
pub struct InnerWindow {
    pub title: String,
    /// Window points; the expanded size (collapsed shows only the title bar).
    pub frame: Rect,
    /// Where the frame must stay (the window's content area).
    pub bounds: Rect,
    pub resizable: bool,
    pub collapsible: bool,
    pub closable: bool,
    pub collapsed: bool,
    pub min: Size,
    gesture: Option<Gesture>,
    /// The last title bar click (double click collapses).
    last_title_click: Option<Time>,
}

impl Default for InnerWindow {
    fn default() -> Self {
        InnerWindow {
            title: String::new(),
            frame: Rect::from_origin_size(Point::new(40.0, 40.0), Size::new(320.0, 240.0)),
            bounds: Rect::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY),
            resizable: true,
            collapsible: true,
            closable: true,
            collapsed: false,
            min: MIN_SIZE,
            gesture: None,
            last_title_click: None,
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
    /// The frame as drawn (only the title bar while collapsed).
    pub fn shown_frame(&self) -> Rect {
        if self.collapsed { Rect::new(self.frame.min.x, self.frame.min.y, self.frame.max.x, self.frame.min.y + TITLE_H) } else { self.frame }
    }

    /// A move or resize is in progress (its frame is native-owned).
    pub fn dragging(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Move { armed: true, .. }) | Some(Gesture::Resize { .. }))
    }

    fn close_rect(&self) -> Option<Rect> {
        let f = self.frame;
        self.closable.then(|| Rect::from_origin_size(Point::new(f.max.x - 4.0 - BUTTON, f.min.y + (TITLE_H - BUTTON) / 2.0), Size::new(BUTTON, BUTTON)))
    }

    fn collapse_rect(&self) -> Option<Rect> {
        let f = self.frame;
        let right = if self.closable { f.max.x - 4.0 - BUTTON - 2.0 } else { f.max.x - 4.0 };
        self.collapsible.then(|| Rect::from_origin_size(Point::new(right - BUTTON, f.min.y + (TITLE_H - BUTTON) / 2.0), Size::new(BUTTON, BUTTON)))
    }

    /// What is under `p` (window points), if anything.
    pub fn zone(&self, p: Point) -> Option<Zone> {
        let f = self.shown_frame();
        if self.resizable && !self.collapsed {
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
        match edges.x {
            1 => r.max.x = (frame.max.x + d.x).min(b.max.x).max(frame.min.x + self.min.w),
            -1 => r.min.x = (frame.min.x + d.x).max(b.min.x).min(frame.max.x - self.min.w),
            _ => {}
        }
        match edges.y {
            1 => r.max.y = (frame.max.y + d.y).min(b.max.y).max(frame.min.y + self.min.h),
            -1 => r.min.y = (frame.min.y + d.y).max(b.min.y).min(frame.max.y - self.min.h),
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
                cx.emit("raise", Value::Null);
                cx.focus = true;
                self.gesture = match zone {
                    Zone::Resize(edges) => {
                        cx.capture = Some(true);
                        cx.emit("drag-started", Value::Null);
                        Some(Gesture::Resize { start: p, frame: self.frame, edges })
                    }
                    Zone::TitleBar => {
                        cx.capture = Some(true);
                        let double = self.last_title_click.is_some_and(|t| cx.now - t <= cx.policy.double_click);
                        if double && self.collapsible {
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
                        cx.emit("drag-started", Value::Null);
                    }
                    if now_armed {
                        let to = clamp_into(frame.translate(d), self.bounds);
                        if to != self.frame {
                            self.frame = to;
                            cx.cursor = Some(CursorIcon::Grabbing);
                            cx.mark(ChangeClass::Property);
                        }
                    }
                    Handled::Yes
                }
                Some(Gesture::Resize { start, frame, edges }) => {
                    let to = self.resized(frame, edges, p - start);
                    if to != self.frame {
                        self.frame = to;
                        cx.mark(ChangeClass::Layout);
                    }
                    cx.cursor = Some(edges.cursor());
                    Handled::Yes
                }
                Some(Gesture::Button(_)) => Handled::Yes,
                None => {
                    match self.zone(p) {
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
                            cx.emit("drag-stopped", Value::Null);
                        }
                    }
                    Gesture::Button(zone) if !cancelled && self.zone(p) == Some(zone) => match zone {
                        Zone::Close => cx.emit("close", Value::Null),
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
        let detail = Value::Record(vec![
            ("x".into(), Value::F64(f.min.x as f64)),
            ("y".into(), Value::F64(f.min.y as f64)),
            ("width".into(), Value::F64(f.width() as f64)),
            ("height".into(), Value::F64(f.height() as f64)),
        ]);
        cx.emit(what, detail);
        cx.emit("drag-stopped", Value::Null);
    }

    fn toggle_collapsed(&mut self, cx: &mut EventCx<'_>) {
        self.collapsed = !self.collapsed;
        cx.emit("toggle", Value::Bool(!self.collapsed));
        cx.mark(ChangeClass::Layout);
    }

    /// The bounds changed (the OS window resized): keeps the frame inside.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.frame = clamp_into(self.frame, bounds);
    }
}

impl Kind for InnerWindow {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, name: &str, v: &Value) -> Result<ChangeClass, PropError> {
        let num = |v: &Value| v.as_f32().ok_or(PropError::Invalid("expected a number"));
        let flag = |v: &Value| v.as_bool().ok_or(PropError::Invalid("expected a bool"));
        let geometry = matches!(name, "x" | "y" | "width" | "height");
        if geometry && self.dragging() {
            return Err(PropError::Stale);
        }
        let f = self.frame;
        match name {
            "title" => self.title = v.as_str().ok_or(PropError::Invalid("expected a string"))?.to_string(),
            "x" => self.frame = clamp_into(f.translate(Vec2::new(num(v)? - f.min.x, 0.0)), self.bounds),
            "y" => self.frame = clamp_into(f.translate(Vec2::new(0.0, num(v)? - f.min.y)), self.bounds),
            "width" => self.frame.max.x = f.min.x + num(v)?.max(self.min.w),
            "height" => self.frame.max.y = f.min.y + num(v)?.max(self.min.h),
            "resizable" => self.resizable = flag(v)?,
            "collapsible" => self.collapsible = flag(v)?,
            "closable" => self.closable = flag(v)?,
            "collapsed" => self.collapsed = flag(v)?,
            "min-width" => self.min.w = num(v)?.max(0.0),
            "min-height" => self.min.h = num(v)?.max(0.0),
            _ => return Err(PropError::Unknown),
        }
        Ok(SPEC.prop(name).map_or(ChangeClass::Paint, |p| p.change))
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let p = cx.palette;
        let f = self.shown_frame();
        // local coordinates: the node is the frame
        let local = Rect::from_origin_size(Point::ZERO, f.size());
        let o = Vec2::new(-self.frame.min.x, -self.frame.min.y);
        draw::shadow(cx.list, local, p.radius * 2.0, p.shadow, 12.0, Vec2::new(0.0, 3.0));
        draw::boxed(cx.list, local, p.radius * 2.0, p.surface_raised, 1.0, p.border);
        draw::line(cx.list, Point::new(0.0, TITLE_H), Point::new(local.max.x, TITLE_H), 1.0, p.border);
        let th = cx.labels.measure(&self.title, p.font_size, None).h;
        let title_w = self.collapse_rect().or(self.close_rect()).map_or(local.max.x, |r| r.min.x + o.x) - 16.0;
        cx.labels.record(cx.list, &self.title, Point::new(8.0, (TITLE_H - th) / 2.0), p.font_size, p.text, Some(title_w.max(0.0)));
        if let Some(r) = self.close_rect() {
            let c = r.translate(o).inflate(-6.0);
            draw::line(cx.list, c.min, c.max, 1.5, p.text_muted);
            draw::line(cx.list, Point::new(c.max.x, c.min.y), Point::new(c.min.x, c.max.y), 1.5, p.text_muted);
        }
        if let Some(r) = self.collapse_rect() {
            let c = r.translate(o).inflate(-6.0);
            let y = (c.min.y + c.max.y) / 2.0;
            draw::line(cx.list, Point::new(c.min.x, y), Point::new(c.max.x, y), 1.5, p.text_muted);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        match ev {
            Event::Pointer(pe) => {
                let p = pe.pos + self.frame.min.to_vec();
                self.pointer(cx, pe, p)
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Window);
        if !self.title.is_empty() {
            node.set_label(self.title.clone());
        }
        if self.collapsible {
            node.set_expanded(!self.collapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::shim::{Os, Policy};
    use presto_base::event::{Modifiers, PointerType};

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
        InnerWindow {
            title: "Layers".into(),
            frame: Rect::new(100.0, 100.0, 300.0, 250.0),
            bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
            ..InnerWindow::default()
        }
    }

    fn drive(w: &mut InnerWindow, pol: &Policy, now: i64, steps: &[(PointerPhase, f32, f32)]) -> Vec<&'static str> {
        let mut names = Vec::new();
        for &(ph, x, y) in steps {
            let mut cx = EventCx::new(Time(now as u64 * 1_000_000), Size::new(800.0, 600.0), pol);
            w.pointer(&mut cx, &pe(ph), Point::new(x, y));
            names.extend(cx.emitted.iter().map(|e| e.signal));
        }
        names
    }

    #[test]
    fn title_bar_drag_waits_for_the_threshold_and_clamps() {
        let pol = Policy::for_os(Os::Linux); // threshold 8
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
        let pol = Policy::for_os(Os::Linux);
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
        assert_eq!(w.set_prop("x", &Value::F64(0.0)), Err(PropError::Stale));
    }

    #[test]
    fn close_collapse_and_double_click() {
        let pol = Policy::for_os(Os::Linux);
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
        assert!(w.collapsed);
        assert_eq!(w.shown_frame().height(), TITLE_H);
        assert_eq!(w.zone(Point::new(150.0, 200.0)), None, "the body is folded away");
        // the bounds shrink: the frame moves back inside
        w.set_bounds(Rect::new(0.0, 0.0, 250.0, 200.0));
        assert_eq!(w.frame.max.x, 250.0);
    }
}
