//! `popup` / `popover`: a floating surface next to an anchor, shown while `open`. The kind keeps its
//! props, places itself with [`place`] (side, alignment, gap, flip, shift, fit) and reports opening
//! and closing; the window's [`OverlayStack`](super::overlay::OverlayStack) owns its layer, routes
//! presses to it and light-dismisses it (the kind turns the stack's [`Closed`] into its `close`
//! signal through [`Popup::dismissed`]).
//!
//! Props: `open`, `placement` (`bottom-start` …), `anchor` (a record: `{node}`, `{x, y, width, height}`
//! rect, or `{x, y}` for a point), `gap`, `modal`, `light-dismiss`, `escape-dismiss`, `match-width`,
//! `flip`, `shift`, `fit`. Signals: `open`, `close` (`reason`), `toggle` (`open`).

use super::common::emit;
use super::overlay::{Closed, Dismiss, DismissReason, OverlayClass};
use super::placement::{AnchorParams, Placed, Placement, place};
use crate::controls::common::host_view;
use crate::controls::props::PropValue;
use crate::kind_props;
use crate::paint::{self, PaintCx, Ua, UaRule, UaValue::*};
use crate::interact::StateBits;
use crate::shim::core::{A11yCx, ChildCx, Default as Dv, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use skelter_base::accesskit;
use skelter_base::event::KeyState;
use skelter_base::geom::{Point, Rect, Size};
use skelter_base::ids::NodeId;
use skelter_base::key::Key;
use std::sync::Arc;

/// What the surface is anchored to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// Not set: the surface centres in the bounds.
    None,
    /// Another node's border box (resolved by the caller each frame).
    Node(NodeId),
    /// A fixed rect, window points.
    Rect(Rect),
    /// A point (a context menu at the pointer).
    Point(Point),
}

/// The `anchor` prop: a record `{node}`, `{x, y, width, height}` or `{x, y}`; unset is [`Anchor::None`].
impl PropValue for Anchor {
    fn from_value(v: &Value) -> Option<Anchor> {
        let Value::Record(fields) = v else {
            // an empty list is "unset"
            return matches!(v, Value::List(l) if l.is_empty()).then_some(Anchor::None);
        };
        let f = |k: &str| fields.iter().find(|(n, _)| &**n == k).and_then(|(_, v)| v.as_f64()).map(|v| v as f32);
        if let Some((_, n)) = fields.iter().find(|(n, _)| &**n == "node") {
            return n.as_i64().map(|n| Anchor::Node(NodeId(n as u64)));
        }
        let (x, y) = (f("x")?, f("y")?);
        Some(match (f("width"), f("height")) {
            (Some(w), Some(h)) => Anchor::Rect(Rect::from_origin_size(Point::new(x, y), Size::new(w.max(0.0), h.max(0.0)))),
            _ => Anchor::Point(Point::new(x, y)),
        })
    }
    fn to_value(&self) -> Value {
        let n = |k: &str, v: f32| (Arc::from(k), Value::F64(v as f64));
        match *self {
            Anchor::None => Value::List(Vec::new()),
            Anchor::Node(id) => Value::Record(vec![(Arc::from("node"), Value::I64(id.0 as i64))]),
            Anchor::Rect(r) => Value::Record(vec![n("x", r.min.x), n("y", r.min.y), n("width", r.width()), n("height", r.height())]),
            Anchor::Point(p) => Value::Record(vec![n("x", p.x), n("y", p.y)]),
        }
    }
    fn from_default(_d: Dv) -> Anchor {
        Anchor::None
    }
}

pub const PLACEMENTS: &[&str] = &[
    "top", "top-start", "top-center", "top-end", "bottom", "bottom-start", "bottom-center", "bottom-end", "left", "left-start",
    "left-center", "left-end", "right", "right-start", "right-center", "right-end",
];

/// The surface's UA sheet: the menu surface tokens (fill, widget border, radius, `popup-shadow`).
pub const UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("popup-shadow")),
])]);

kind_props! {
    /// The popup's props.
    pub struct PopupProps;
    /// Shown; closing natively (Escape, light dismiss) writes it back.
    open "open": bool = Bool(false) ; Bool, Structure, bindable;
    placement "placement": String = Enum("bottom-start") ; Enum(PLACEMENTS), Property;
    /// A record: `{node}`, `{x, y, width, height}` or `{x, y}` (window points).
    anchor "anchor": Anchor = None ; Record, Property;
    gap "gap": f64 = F64(4.0) ; F64, Property;
    modal "modal": bool = Bool(false) ; Bool, Structure;
    light_dismiss "light-dismiss": bool = Bool(true) ; Bool, Property;
    escape_dismiss "escape-dismiss": bool = Bool(true) ; Bool, Property;
    match_width "match-width": bool = Bool(false) ; Bool, Layout;
    flip "flip": bool = Bool(true) ; Bool, Property;
    shift "shift": bool = Bool(true) ; Bool, Property;
    fit "fit": bool = Bool(true) ; Bool, Layout;
}

pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("open", &[], "It opened."),
    SignalSpec::new("close", &[("reason", ValueType::Str)], "It closed (`escape`, `outside-press`, `programmatic`, …)."),
    SignalSpec::new("toggle", &[("open", ValueType::Bool)], "It opened or closed."),
];

pub static SPEC: KindSpec = KindSpec {
    tag: "popup",
    props: PopupProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Dialog,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: false,
};

/// A `popup` node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Popup {
    pub props: PopupProps,
    /// Where it went last ([`Popup::place_in`]).
    placed: Option<Placed>,
}

impl Popup {
    pub fn placement(&self) -> Placement {
        Placement::parse(&self.props.placement).unwrap_or(Placement::BOTTOM_START)
    }

    /// The overlay class it opens as.
    pub fn class(&self) -> OverlayClass {
        if self.props.modal { OverlayClass::Modal } else { OverlayClass::Popup }
    }

    /// The overlay stack's dismiss flags for it (a modal popup never light-dismisses).
    pub fn dismiss_flags(&self) -> Dismiss {
        Dismiss { outside: self.props.light_dismiss && !self.props.modal, escape: self.props.escape_dismiss }
    }

    /// The last placement.
    pub fn placed(&self) -> Option<Placed> {
        self.placed
    }

    /// Places a surface of `content` size next to `anchor` (the resolved anchor rect; `None` for
    /// [`Anchor::None`], which centres it) inside `bounds` (window points).
    pub fn place_in(&mut self, anchor: Option<Rect>, bounds: Rect, content: Size) -> Placed {
        let p = &self.props;
        let placed = match anchor {
            Some(a) => place(&AnchorParams {
                anchor: a,
                size: content,
                bounds,
                placement: self.placement(),
                gap: p.gap as f32,
                cross: 0.0,
                flip: p.flip,
                shift: p.shift,
                fit: p.fit,
                match_anchor_width: p.match_width,
            }),
            None => {
                let s = Size::new(content.w.min(bounds.width()), content.h.min(bounds.height()));
                let min = Point::new(bounds.min.x + (bounds.width() - s.w) / 2.0, bounds.min.y + (bounds.height() - s.h) / 2.0);
                Placed { side: self.placement().side, rect: Rect::from_origin_size(min, s) }
            }
        };
        self.placed = Some(placed);
        placed
    }

    /// The fixed anchor rect, when it is not another node.
    pub fn fixed_anchor(&self) -> Option<Rect> {
        match self.props.anchor {
            Anchor::Rect(r) => Some(r),
            Anchor::Point(p) => Some(Rect::from_origin_size(p, Size::ZERO)),
            Anchor::None | Anchor::Node(_) => None,
        }
    }

    /// Opens or closes it from Rust or a native default, emitting `open`/`close` and `toggle`; `open`
    /// is written back as native state.
    pub fn set_open(&mut self, cx: &mut EventCx<'_>, open: bool, reason: DismissReason) {
        if self.props.open == open {
            return;
        }
        self.props.open = open;
        if open {
            emit(cx, &SPEC, "open", Vec::new());
        } else {
            self.placed = None;
            emit(cx, &SPEC, "close", vec![Value::str(reason.name())]);
        }
        emit(cx, &SPEC, "toggle", vec![Value::Bool(open)]);
        cx.prop_changed(P::open.id());
    }

    /// The overlay stack closed it.
    pub fn dismissed(&mut self, cx: &mut EventCx<'_>, closed: &Closed) {
        self.set_open(cx, false, closed.reason);
    }
}

impl Kind for Popup {
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
        self.props.set(prop, v)?;
        if prop == P::open.id() && !self.props.open {
            self.placed = None;
        }
        Ok(())
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    /// The content is not laid out, painted or hit while it is closed.
    fn child_layout(&self, _cx: &ChildCx, style: &mut taffy::Style) -> bool {
        if !self.props.open {
            style.display = taffy::Display::None;
        }
        self.props.open
    }

    fn prop_state(&self) -> StateBits {
        if self.props.open { StateBits::OPEN } else { StateBits::NONE }
    }
    fn prop_states() -> StateBits {
        StateBits::OPEN
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = host_view(&cx.style, UA);
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &host, rect);
        paint::paint_outline(&mut pc, &host, rect);
    }

    /// Escape while focus is inside it closes it when the stack did not (a popup opened outside a
    /// stack, in tests and embedders).
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed && k.key == Some(Key::Escape) && self.props.open && self.props.escape_dismiss => {
                self.set_open(cx, false, DismissReason::Escape);
                Handled::Yes
            }
            Event::OutsidePress(_) if self.light_dismiss() => {
                self.set_open(cx, false, DismissReason::OutsidePress);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// Open and light-dismissable (`light-dismiss`, not `modal`): a press outside closes it.
    fn light_dismiss(&self) -> bool {
        self.props.open && self.dismiss_flags().outside
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(if self.props.modal { accesskit::Role::Dialog } else { accesskit::Role::Group });
        if self.props.modal {
            cx.node.set_modal();
        }
        if !self.props.open {
            cx.node.set_hidden();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::{Policy, StateBits};
    use crate::views::overlay::OverlayId;
    use crate::views::placement::Side;
    use skelter_base::event::{KeyEvent, KeyLocation, Modifiers};
    use skelter_base::time::Time;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Value {
        Anchor::Rect(Rect::from_origin_size(Point::new(x, y), Size::new(w, h))).to_value()
    }

    fn set(p: &mut Popup, name: &str, v: Value) -> Result<(), PropError> {
        p.set_prop(SPEC.prop(name).unwrap(), &v)
    }

    /// The names of the signals `cx` holds and the first argument of each.
    fn signals(cx: &EventCx) -> Vec<(&'static str, Option<Value>)> {
        cx.emitted.iter().map(|e| (SPEC.signals[e.signal.0 as usize].name, e.args.first().cloned())).collect()
    }

    #[test]
    fn places_through_the_kind_with_flip_and_match_width() {
        let mut p = Popup::default();
        set(&mut p, "anchor", rect(100.0, 270.0, 50.0, 20.0)).unwrap();
        set(&mut p, "placement", Value::str("bottom-start")).unwrap();
        set(&mut p, "gap", Value::F64(2.0)).unwrap();
        set(&mut p, "match-width", Value::Bool(true)).unwrap();
        let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        let placed = p.place_in(p.fixed_anchor(), bounds, Size::new(30.0, 40.0));
        assert_eq!(placed.side, Side::Top, "no room below");
        assert_eq!(placed.rect, Rect::new(100.0, 228.0, 150.0, 268.0));
        assert_eq!(p.placed(), Some(placed));
        assert!(set(&mut p, "placement", Value::str("middle")).is_err(), "not one of the placements");
        assert_eq!(SPEC.prop("nope"), None);
    }

    #[test]
    fn a_point_anchor_and_no_anchor() {
        let mut p = Popup::default();
        set(&mut p, "anchor", Anchor::Point(Point::new(10.0, 20.0)).to_value()).unwrap();
        assert_eq!(p.props.anchor, Anchor::Point(Point::new(10.0, 20.0)));
        set(&mut p, "anchor", Value::List(Vec::new())).unwrap();
        assert_eq!(p.props.anchor, Anchor::None, "unset");
        let placed = p.place_in(None, Rect::new(0.0, 0.0, 400.0, 300.0), Size::new(100.0, 50.0));
        assert_eq!(placed.rect, Rect::new(150.0, 125.0, 250.0, 175.0), "centred");
        set(&mut p, "anchor", Value::Record(vec![(Arc::from("node"), Value::I64(7))])).unwrap();
        assert_eq!((p.props.anchor, p.fixed_anchor()), (Anchor::Node(NodeId(7)), None));
        assert!(set(&mut p, "anchor", Value::Record(vec![(Arc::from("x"), Value::F64(1.0))])).is_err(), "needs x and y");
    }

    #[test]
    fn escape_and_dismissal_emit_close_and_toggle() {
        let pol = Policy::default();
        let mut st = StateBits::NONE;
        let mut p = Popup::default();
        let mut cx = EventCx::new(Time::ZERO, Size::new(100.0, 100.0), &mut st, &pol);
        p.set_open(&mut cx, true, DismissReason::Programmatic);
        assert_eq!(signals(&cx), vec![("open", None), ("toggle", Some(Value::Bool(true)))]);
        assert_eq!(cx.changed, vec![P::open.id()], "written back as native state");
        let esc = Event::Key(KeyEvent { key: Some(Key::Escape), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() });
        let mut cx = EventCx::new(Time::ZERO, Size::new(100.0, 100.0), &mut st, &pol);
        assert_eq!(p.event(&mut cx, &esc), Handled::Yes);
        assert!(!p.props.open);
        assert_eq!(signals(&cx)[0], ("close", Some(Value::str("escape"))));
        // closed by the stack
        let mut cx = EventCx::new(Time::ZERO, Size::new(100.0, 100.0), &mut st, &pol);
        p.set_open(&mut cx, true, DismissReason::Programmatic);
        p.dismissed(&mut cx, &Closed { id: OverlayId(1), node: NodeId(1), reason: DismissReason::OutsidePress, restore_focus: None });
        assert!(!p.props.open);
        assert!(signals(&cx).contains(&("close", Some(Value::str("outside-press")))));
    }

    #[test]
    fn modal_popups_never_light_dismiss() {
        let mut p = Popup::default();
        set(&mut p, "modal", Value::Bool(true)).unwrap();
        assert_eq!(p.class(), OverlayClass::Modal);
        assert_eq!(p.dismiss_flags(), Dismiss { outside: false, escape: true });
    }

    #[test]
    fn records_the_menu_surface() {
        let (list, _) = testkit::record(&Popup::default(), Size::new(120.0, 80.0), StateBits::NONE);
        // popup-shadow (elev-3) under the filled, bordered surface
        assert_eq!(testkit::kinds(&list), ["shadow", "quad"]);
    }

    #[test]
    fn closed_it_hides_its_content_and_open_is_a_state() {
        let mut p = Popup::default();
        let mut style = taffy::Style::default();
        assert!(!p.child_layout(&ChildCx { index: 0, anchors: &[""], slots: &[] }, &mut style));
        assert_eq!(style.display, taffy::Display::None);
        assert_eq!(p.prop_state(), StateBits::NONE);
        p.props.open = true;
        let mut style = taffy::Style::default();
        assert!(p.child_layout(&ChildCx { index: 0, anchors: &[""], slots: &[] }, &mut style));
        assert_ne!(style.display, taffy::Display::None);
        assert_eq!(p.prop_state(), StateBits::OPEN);
    }

    #[test]
    fn an_outside_press_closes_it_when_light_dismissable() {
        let pol = crate::interact::Policy::default();
        let mut st = StateBits::NONE;
        let mut p = Popup::default();
        assert!(!p.light_dismiss(), "closed: no outside presses wanted");
        p.props.open = true;
        p.props.light_dismiss = true;
        assert!(p.light_dismiss());
        let mut cx = EventCx::new(skelter_base::time::Time::ZERO, Size::new(100.0, 50.0), &mut st, &pol);
        assert_eq!(p.event(&mut cx, &Event::OutsidePress(Point::new(-5.0, -5.0))), Handled::Yes);
        assert!(!p.props.open);
        assert_eq!(signals(&cx)[0], ("close", Some(Value::str("outside-press"))));
        // a modal popup never light-dismisses
        let mut p = Popup::default();
        p.props.open = true;
        p.props.light_dismiss = true;
        p.props.modal = true;
        assert!(!p.light_dismiss());
    }
}
