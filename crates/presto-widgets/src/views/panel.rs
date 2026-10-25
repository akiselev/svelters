//! `panel` (ported from svelters-egui `widgets/panel.rs`): a side or top/bottom panel with the chrome
//! background, sized by `size` along its side's axis (width for `left`/`right`, height for
//! `top`/`bottom`; it does not shrink). `resizable` makes the inner edge a [`Sash`]: drag it within
//! the panel's `min-*`/`max-*`, or focus it and use the arrow keys; `changed(size)` reports each new
//! size. The separator is a 1 pt line inside the panel along its inner edge, kept clear of content by
//! [`Kind::chrome_padding`]; the sash takes the laid-out box in [`Kind::prepare`].

use super::common;
use super::split::{Sash, SashIntent, SashParams, leave_event, record_sash};
use crate::controls::common::host_view;
use crate::kind_props;
use crate::paint::{self, PaintCx, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalSpec, StyleCx, Value, ValueType};
use presto_base::accesskit;
use presto_base::cursor::CursorIcon;
use presto_base::event::{KeyState, MouseButton, PointerPhase};
use presto_base::geom::{Rect, Size};
use presto_base::time::Time;

/// The panel's UA sheet: the chrome surface (`panel-bg`, the theme's side bar and panel colour).
pub const UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("panel-bg"))])]);

kind_props! {
    /// The panel's props.
    pub struct PanelProps;
    /// The edge it sits on; the sash is on the opposite (inner) edge.
    side "side": String = Enum("left") ; Enum(&["left", "right", "top", "bottom"]), Layout;
    /// Its width (`left`/`right`) or height (`top`/`bottom`) in points; unset: its content's.
    size "size": Option<f64> = None ; F64, Layout, bindable;
    /// The inner edge is a sash.
    resizable "resizable": bool = Bool(false) ; Bool, Paint;
    /// Draws the line on the inner edge.
    separator "separator": bool = Bool(true) ; Bool, Paint;
}

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[SignalSpec::new("changed", &[("size", ValueType::F64)], "The user resized the panel.")];

pub static SPEC: KindSpec = KindSpec {
    tag: "panel",
    props: PanelProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Pane,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// Where a panel sits; its sash is on the opposite (inner) edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelSide {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl PanelSide {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "left" => PanelSide::Left,
            "right" => PanelSide::Right,
            "top" => PanelSide::Top,
            "bottom" => PanelSide::Bottom,
            _ => return None,
        })
    }
    /// The axis its size is measured on.
    pub fn axis(self) -> usize {
        if matches!(self, PanelSide::Top | PanelSide::Bottom) { 1 } else { 0 }
    }
    /// +1 when dragging the inner edge towards larger coordinates grows the panel.
    fn sign(self) -> f32 {
        if matches!(self, PanelSide::Right | PanelSide::Bottom) { -1.0 } else { 1.0 }
    }
}

/// A side or top/bottom panel.
#[derive(Clone, Debug)]
pub struct Panel {
    pub props: PanelProps,
    pub side: PanelSide,
    /// Its width or height in points; `None` for its content's.
    pub size: Option<f32>,
    pub resizable: bool,
    /// Draws the line on the inner edge.
    pub separator: bool,
    /// The sash's limits (the panel's `min-*`/`max-*` along its axis).
    pub min: f32,
    pub max: f32,
    /// The sash's hit area thickness.
    pub hit: f32,
    sash: Sash,
    /// The border box as laid out.
    rect: Size,
    now: Time,
}

impl Default for Panel {
    fn default() -> Self {
        Panel { props: PanelProps::default(), side: PanelSide::Left, size: None, resizable: false, separator: true, min: 0.0, max: f32::INFINITY, hit: 4.0, sash: Sash::default(), rect: Size::ZERO, now: Time::ZERO }
    }
}

impl Panel {
    /// The layout's border box size.
    pub fn set_layout(&mut self, size: Size) {
        self.rect = size;
        let ax = self.side.axis();
        self.sash.set_size(self.size.unwrap_or(if ax == 0 { size.w } else { size.h }));
    }

    /// The inner edge coordinate along the axis (local).
    fn edge(&self) -> f32 {
        match self.side {
            PanelSide::Left => self.rect.w,
            PanelSide::Top => self.rect.h,
            PanelSide::Right | PanelSide::Bottom => 0.0,
        }
    }

    /// The separator line inside the panel along its inner edge, and the sash's hit area.
    pub fn edge_rects(&self) -> (Rect, Rect) {
        let at = self.edge();
        let inward = self.side.sign();
        let (l0, l1) = if inward > 0.0 { (at - 1.0, at) } else { (at, at + 1.0) };
        let (h0, h1) = (at - self.hit / 2.0, at + self.hit / 2.0);
        if self.side.axis() == 0 {
            (Rect::new(l0, 0.0, l1, self.rect.h), Rect::new(h0, 0.0, h1, self.rect.h))
        } else {
            (Rect::new(0.0, l0, self.rect.w, l1), Rect::new(0.0, h0, self.rect.w, h1))
        }
    }

    fn params(&self) -> SashParams {
        SashParams { axis: self.side.axis(), sign: self.side.sign(), min: self.min.max(0.0), max: self.max, step: 10.0, collapsible: false, double_click: false }
    }

    fn apply(&mut self, cx: &mut EventCx<'_>, intents: Vec<SashIntent>) {
        for i in intents {
            match i {
                SashIntent::Start => cx.capture = Some(true),
                SashIntent::Resize { size } | SashIntent::Commit { size } | SashIntent::Cancel { size } => {
                    if self.size != Some(size) {
                        self.size = Some(size);
                        self.props.size = Some(size as f64);
                        cx.prop_changed(P::size.id());
                        common::emit(cx, &SPEC, "changed", vec![Value::F64(size as f64)]);
                    }
                }
            }
        }
    }

    /// The sash's AccessKit node (a `Splitter` named after the panel, else what it does).
    pub fn sash_a11y(&self, name: &str, node: &mut accesskit::Node) {
        self.sash.a11y(&self.params(), node);
        node.set_label(if name.is_empty() { "Resize panel".to_string() } else { name.to_string() });
    }

    /// Focus moved to the sash (or off it).
    pub fn focus_sash(&mut self, f: bool) {
        self.sash.set_focused(f);
    }
}

impl Kind for Panel {
    fn parts() -> &'static [&'static str] {
        &["sash"]
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        // a drag owns the size until it ends
        if prop == P::size.id() && self.sash.dragging() {
            return Err(PropError::Stale);
        }
        self.props.set(prop, v)?;
        self.side = PanelSide::parse(&self.props.side).unwrap_or_default();
        self.size = self.props.size.map(|s| s as f32);
        if let Some(s) = self.size {
            self.sash.set_size(s);
        }
        self.resizable = self.props.resizable;
        self.separator = self.props.separator;
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    /// `size` fixes the panel's extent along its side's axis (it does not shrink).
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        if let Some(v) = self.size {
            let d = taffy::Dimension::length(v.max(0.0));
            if self.side.axis() == 0 {
                s.size.width = d;
            } else {
                s.size.height = d;
            }
            s.flex_shrink = 0.0;
        }
        s
    }

    /// Room for the separator line along the inner edge, so content does not run under it.
    fn chrome_padding(&self, _cx: &mut MeasureCx) -> Option<[f32; 4]> {
        if !self.separator {
            return None;
        }
        // [left, right, top, bottom]
        Some(match self.side {
            PanelSide::Left => [0.0, 1.0, 0.0, 0.0],
            PanelSide::Right => [1.0, 0.0, 0.0, 0.0],
            PanelSide::Top => [0.0, 0.0, 0.0, 1.0],
            PanelSide::Bottom => [0.0, 0.0, 1.0, 0.0],
        })
    }

    /// Takes the laid-out box for the sash.
    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.set_layout(cx.size);
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = host_view(&cx.style, UA);
        let styles = crate::shim::core::Styles { computed: cx.style.computed, parts: cx.style.parts, children: cx.style.children, state: cx.style.state, dark: cx.style.dark };
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &host, rect);
        let (line, _) = self.edge_rects();
        let shown = self.resizable && self.sash.hover_shown(self.now);
        if self.separator || shown {
            record_sash(&mut pc, &styles, line, self.side.axis(), shown, self.sash.dragging());
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.now = cx.now;
        if !self.resizable {
            return Handled::No;
        }
        let synthetic;
        let ev = match ev {
            Event::Hover(false) => {
                synthetic = Event::Pointer(leave_event(PointerPhase::Leave));
                &synthetic
            }
            Event::CaptureLost => {
                synthetic = Event::Pointer(leave_event(PointerPhase::Cancel));
                &synthetic
            }
            e => e,
        };
        match ev {
            Event::Pointer(p) => {
                let (_, hit) = self.edge_rects();
                let inside = p.phase != PointerPhase::Leave && hit.contains(p.pos);
                let primary = p.button == Some(MouseButton::Primary) || (p.phase != PointerPhase::Down && p.buttons & 1 != 0);
                let was_shown = self.sash.hover_shown(cx.now);
                let params = self.params();
                let dragging = self.sash.dragging();
                let intents = self.sash.pointer(cx.now, cx.policy, &params, p.phase, primary, p.pos, inside);
                if was_shown != self.sash.hover_shown(cx.now) {
                    cx.repaint = true;
                }
                if let Some(t) = self.sash.wake() {
                    cx.wake_at(t);
                }
                if dragging && !self.sash.dragging() {
                    cx.capture = Some(false);
                }
                self.apply(cx, intents);
                if inside || self.sash.dragging() {
                    cx.cursor = Some(if self.side.axis() == 0 { CursorIcon::ColResize } else { CursorIcon::RowResize });
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let Some(key) = k.key else { return Handled::No };
                let params = self.params();
                if !self.sash.claims(&params, key) {
                    return Handled::No;
                }
                let intents = self.sash.key(&params, key, k.modifiers);
                self.apply(cx, intents);
                Handled::Yes
            }
            Event::Timer => {
                cx.repaint = true;
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{Native, Platform};
    use super::super::split::Probe;
    use presto_base::event::{Modifiers, PointerEvent, PointerType};
    use presto_base::geom::{Point, Vec2};

    fn ptr(phase: PointerPhase, x: f32) -> Event {
        Event::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
            buttons: if matches!(phase, PointerPhase::Down | PointerPhase::Move) { 1 } else { 0 },
            pos: Point::new(x, 50.0),
            modifiers: Modifiers::default(),
            pressure: 0.5,
            tilt: Vec2::ZERO,
            history: Default::default(),
        })
    }

    #[test]
    fn a_right_panel_grows_when_its_left_edge_moves_left() {
        let pol = *Native::for_os(Platform::Windows).policy();
        let mut p = Panel { min: 100.0, max: 400.0, ..Default::default() };
        p.set_prop(P::side.id(), &Value::str("right")).unwrap();
        p.set_prop(P::resizable.id(), &Value::Bool(true)).unwrap();
        p.set_layout(Size::new(200.0, 300.0));
        let (line, hit) = p.edge_rects();
        assert_eq!(line, Rect::new(0.0, 0.0, 1.0, 300.0));
        assert!(hit.contains(Point::new(-1.0, 10.0)));
        let mut t = Probe::new(pol, Size::new(200.0, 300.0));
        assert_eq!(t.send(&mut p, ptr(PointerPhase::Down, 0.0)), Handled::Yes);
        t.send(&mut p, ptr(PointerPhase::Move, -50.0));
        assert_eq!(p.size, Some(250.0));
        t.send(&mut p, ptr(PointerPhase::Move, -500.0));
        assert_eq!(p.size, Some(400.0), "clamped to max");
        t.send(&mut p, ptr(PointerPhase::Up, -500.0));
        assert_eq!(t.capture, Some(false));
        assert_eq!(t.count("changed"), 2);
        assert_eq!(p.get_prop(P::size.id()), Some(Value::F64(400.0)));
        assert!(t.changed.iter().all(|n| *n == "size") && !t.changed.is_empty());
    }

    #[test]
    fn a_js_write_during_a_drag_is_refused() {
        let pol = *Native::for_os(Platform::Linux).policy();
        let mut p = Panel::default();
        p.set_prop(P::resizable.id(), &Value::Bool(true)).unwrap();
        p.set_layout(Size::new(200.0, 300.0));
        let mut t = Probe::new(pol, Size::new(200.0, 300.0));
        t.send(&mut p, ptr(PointerPhase::Down, 200.0));
        t.send(&mut p, ptr(PointerPhase::Move, 230.0));
        assert_eq!(p.size, Some(230.0));
        assert_eq!(p.set_prop(P::size.id(), &Value::F64(100.0)), Err(PropError::Stale));
        t.send(&mut p, ptr(PointerPhase::Up, 230.0));
        assert_eq!(p.set_prop(P::size.id(), &Value::F64(100.0)), Ok(()));
        assert_eq!(p.size, Some(100.0));
    }

    #[test]
    fn records_the_surface_and_the_separator() {
        let mut p = Panel::default();
        p.set_layout(Size::new(200.0, 300.0));
        let (list, _) = crate::controls::testkit::record(&p, Size::new(200.0, 300.0), crate::interact::StateBits::NONE);
        assert_eq!(crate::controls::testkit::kinds(&list), ["quad", "quad"], "surface, then the separator line");
    }

    #[test]
    fn layout_hooks_fix_the_size_and_clear_the_separator() {
        let mut p = Panel { side: PanelSide::Right, size: Some(240.0), ..Panel::default() };
        let st = p.layout_style(&StyleCx { computed: None, state: crate::interact::StateBits::NONE, dark: true, base: None });
        assert_eq!((st.size.width, st.flex_shrink), (taffy::Dimension::length(240.0), 0.0));
        let mut text = crate::shim::testing::MonoShaper;
        let style = crate::shim::core::Styles { computed: None, parts: &[], children: &[], state: crate::interact::StateBits::NONE, dark: true };
        let mut mcx = MeasureCx { style, text: &mut text, scale: 1.0 };
        assert_eq!(p.chrome_padding(&mut mcx), Some([1.0, 0.0, 0.0, 0.0]), "a right panel's inner edge is its left");
        let style = crate::shim::core::Styles { computed: None, parts: &[], children: &[], state: crate::interact::StateBits::NONE, dark: true };
        let mut text = crate::shim::testing::MonoShaper;
        p.prepare(&mut PrepareCx { size: Size::new(240.0, 300.0), style, text: &mut text });
        assert_eq!(p.edge_rects().0, Rect::new(0.0, 0.0, 1.0, 300.0));
    }
}
