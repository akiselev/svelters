//! `popup` / `popover`: a floating surface next to an anchor, shown while `open`. Open, it is in the
//! window's top layer (`position: fixed`, `skelter_core::Kind::top_layer`) with its children: laid
//! out against the window, out of its owner's flow, placed with [`place`] (side, alignment, gap,
//! flip, shift) next to its anchor (unset: its owner, the element it is declared in), drawn above
//! the content whatever clips its owner. The window's [`OverlayStack`](super::overlay::OverlayStack)
//! owns its layer, routes presses to it and light-dismisses it (the kind turns the stack's closing
//! into its `close` signal, [`Popup::dismissed`] and `overlay_closed`).
//!
//! Props: `open`, `placement` (`bottom-start` …), `anchor-to` (the common `anchor` name of the element
//! it positions against; it closes, `anchor-hidden`, when no element of the window has it), `anchor`
//! (a record: `{node}`, `{x, y, width, height}` rect, or `{x, y}` for a point), `gap`, `modal`, `light-dismiss`, `escape-dismiss`, `match-width`,
//! `flip`, `shift`, `fit`. Signals: `open`, `close` (`reason`), `toggle` (`open`).

use super::common::emit;
use super::overlay::{Closed, Dismiss, DismissReason, OverlayClass};
use super::placement::{Align, AnchorParams, Placed, Placement, Side, place};
use crate::controls::common::host_view;
use crate::controls::props::PropValue;
use crate::kind_props;
use crate::paint::{self, PaintCx, Ua, UaRule, UaValue::*};
use crate::interact::StateBits;
use crate::shim::core::{A11yCx, ChildCx, Default as Dv, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, StyleCx, TopLayerCx, Value, ValueType};
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
    /// The common `anchor` name of the element it positions against (wins over `anchor`).
    anchor_to "anchor-to": String = Str("") ; Str, Property;
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
    /// Right-to-left: `-start`/`-end` on a top or bottom side are logical (start is the right).
    rtl: bool,
}

impl Popup {
    /// `placement`, with `-start`/`-end` on a top or bottom side swapped in rtl (logical alignment;
    /// `left`/`right` stay physical, as CSS's physical sides do).
    pub fn placement(&self) -> Placement {
        let mut p = Placement::parse(&self.props.placement).unwrap_or(Placement::BOTTOM_START);
        if self.rtl && matches!(p.side, Side::Top | Side::Bottom) {
            p.align = match p.align {
                Align::Start => Align::End,
                Align::End => Align::Start,
                a => a,
            };
        }
        p
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

    /// Where it goes in the top layer: next to `anchor` (its anchor's box, [`Popup::anchor_box`];
    /// `None` centres it), inside the window, at its laid-out `size` (window points); `rtl`: its
    /// direction (logical `-start`/`-end`).
    pub fn top_rect(&self, anchor: Option<Rect>, window: Size, size: Size, rtl: bool) -> Rect {
        let bounds = Rect::from_origin_size(Point::ZERO, window);
        // `fit` would shrink it, but its children are laid out at `size`: the stack draws it whole
        let mut p = self.clone();
        p.props.fit = false;
        p.rtl = rtl;
        p.place_in(anchor, bounds, size).rect
    }

    /// Its anchor's box (window points): the element `anchor-to` names, else the `anchor` record,
    /// else its owner. `Err`: `anchor-to` names no element of the window (it was removed).
    pub fn anchor_box(&self, owner: Option<Rect>, rect_of: &dyn Fn(NodeId) -> Option<Rect>, find_anchor: &dyn Fn(&str) -> Option<NodeId>) -> Result<Option<Rect>, ()> {
        if !self.props.anchor_to.is_empty() {
            let n = find_anchor(&self.props.anchor_to).ok_or(())?;
            return Ok(rect_of(n).or(owner));
        }
        Ok(match self.props.anchor {
            Anchor::Node(n) => rect_of(n).or(owner),
            Anchor::None => owner,
            _ => self.fixed_anchor(),
        })
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

    /// Open, it is in the window's top layer.
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        let mut s = cx.default_layout();
        if self.props.open {
            s.position = taffy::Position::Fixed;
        }
        s
    }

    fn top_layer(&self, cx: &TopLayerCx) -> Option<skelter_core::overlay::TopLayer> {
        let anchor = self.anchor_box(cx.owner, cx.rect_of, cx.find_anchor);
        // a press on its anchor is the anchor's own (it toggles the popup), not outside it
        let a = anchor.unwrap_or(cx.owner);
        let r = self.top_rect(a, cx.window, cx.size, cx.rtl);
        let tl = skelter_core::overlay::TopLayer::new(self.class()).at(r.min).with_dismiss(self.dismiss_flags()).with_modal(self.props.modal).with_inside(a.into_iter().collect());
        // its `anchor-to` element went away: core closes it (`close` with `anchor-hidden`)
        Some(if anchor.is_err() { tl.closing(DismissReason::AnchorHidden) } else { tl })
    }

    fn overlay_closed(&mut self, cx: &mut EventCx, reason: DismissReason) -> Handled {
        self.set_open(cx, false, reason);
        Handled::Yes
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
            Event::OutsidePress(_) if self.props.open && self.dismiss_flags().outside => {
                self.set_open(cx, false, DismissReason::OutsidePress);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// The window's overlay stack light-dismisses it (a press on its anchor is not outside it); the
    /// subtree rule of `light_dismiss` would close it on its anchor's press.
    fn light_dismiss(&self) -> bool {
        false
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
        assert!(!p.child_layout(&ChildCx { index: 0, anchors: &[""], slots: &[], children: &[] }, &mut style));
        assert_eq!(style.display, taffy::Display::None);
        assert_eq!(p.prop_state(), StateBits::NONE);
        p.props.open = true;
        let mut style = taffy::Style::default();
        assert!(p.child_layout(&ChildCx { index: 0, anchors: &[""], slots: &[], children: &[] }, &mut style));
        assert_ne!(style.display, taffy::Display::None);
        assert_eq!(p.prop_state(), StateBits::OPEN);
    }

    #[test]
    fn an_outside_press_closes_it_when_light_dismissable() {
        let pol = crate::interact::Policy::default();
        let mut st = StateBits::NONE;
        let mut p = Popup::default();
        // the window's overlay stack light-dismisses it, not the subtree rule (its anchor's press)
        assert!(!p.light_dismiss());
        p.props.open = true;
        p.props.light_dismiss = true;
        assert!(!p.light_dismiss());
        assert_eq!(p.dismiss_flags(), Dismiss::LIGHT);
        let mut cx = EventCx::new(skelter_base::time::Time::ZERO, Size::new(100.0, 50.0), &mut st, &pol);
        assert_eq!(p.event(&mut cx, &Event::OutsidePress(Point::new(-5.0, -5.0))), Handled::Yes);
        assert!(!p.props.open);
        assert_eq!(signals(&cx)[0], ("close", Some(Value::str("outside-press"))));
        // a modal popup never light-dismisses
        let mut p = Popup::default();
        p.props.open = true;
        p.props.light_dismiss = true;
        p.props.modal = true;
        assert!(!p.dismiss_flags().outside);
    }

    #[test]
    fn open_it_is_in_the_top_layer_next_to_its_owner() {
        let mut p = Popup::default();
        let cx = StyleCx { computed: None, state: StateBits::NONE, dark: true, base: None, font_size: None, theme: None };
        assert_ne!(p.layout_style(&cx).position, taffy::Position::Fixed, "closed: in its owner's flow");
        p.props.open = true;
        assert_eq!(p.layout_style(&cx).position, taffy::Position::Fixed);
        let owner = Rect::new(10.0, 10.0, 60.0, 30.0);
        let none = |_: NodeId| None;
        let no_anchor = |_: &str| None;
        let tcx = TopLayerCx { size: Size::new(100.0, 40.0), window: Size::new(300.0, 200.0), owner: Some(owner), rect_of: &none, find_anchor: &no_anchor, rtl: false };
        let tl = p.top_layer(&tcx).expect("a top-layer entry");
        assert_eq!(tl.at, Some(Point::new(10.0, 34.0)), "bottom-start, 4 pt below its owner");
        assert_eq!(tl.inside, vec![owner], "a press on the owner is not outside it");
        assert_eq!((tl.class, tl.dismiss), (OverlayClass::Popup, Dismiss::LIGHT));
        // no room below: it flips above; a node anchor wins over the owner
        p.props.placement = "bottom-end".into();
        let tcx = TopLayerCx { owner: Some(Rect::new(200.0, 180.0, 260.0, 196.0)), ..tcx };
        assert_eq!(p.top_layer(&tcx).unwrap().at, Some(Point::new(160.0, 136.0)));
        let other = |n: NodeId| (n == NodeId(7)).then_some(Rect::new(0.0, 0.0, 20.0, 20.0));
        p.props.anchor = Anchor::Node(NodeId(7));
        let tcx = TopLayerCx { rect_of: &other, ..tcx };
        assert_eq!(p.top_layer(&tcx).unwrap().at, Some(Point::new(0.0, 24.0)), "bottom-end shifted into the window");
    }

    /// dir-widgets' R1: in rtl, `-start`/`-end` on a top or bottom side are logical (start is the
    /// anchor's right edge); `left`/`right` sides stay physical.
    #[test]
    fn start_and_end_are_logical_in_rtl() {
        let mut p = Popup::default();
        p.props.open = true;
        let owner = Rect::new(100.0, 10.0, 160.0, 30.0);
        let none = |_: NodeId| None;
        let no_anchor = |_: &str| None;
        let ltr = TopLayerCx { size: Size::new(80.0, 40.0), window: Size::new(300.0, 200.0), owner: Some(owner), rect_of: &none, find_anchor: &no_anchor, rtl: false };
        let rtl = TopLayerCx { rtl: true, ..ltr };
        assert_eq!(p.top_layer(&ltr).unwrap().at, Some(Point::new(100.0, 34.0)), "bottom-start: left edges meet");
        assert_eq!(p.top_layer(&rtl).unwrap().at, Some(Point::new(80.0, 34.0)), "rtl: right edges meet");
        p.props.placement = "top-end".into();
        assert_eq!(p.top_layer(&rtl).unwrap().at.map(|a| a.x), Some(100.0), "rtl end: the left edge");
        p.props.placement = "right-start".into();
        assert_eq!(p.top_layer(&ltr).unwrap().at, p.top_layer(&rtl).unwrap().at, "a physical side stays");
    }

    #[test]
    fn anchor_to_names_the_element_it_positions_against() {
        let mut p = Popup::default();
        p.props.open = true;
        set(&mut p, "anchor-to", Value::str("t")).unwrap();
        let owner = Rect::new(10.0, 10.0, 60.0, 30.0);
        let button = Rect::new(100.0, 50.0, 160.0, 70.0);
        let rect_of = |n: NodeId| (n == NodeId(9)).then_some(button);
        let found = |a: &str| (a == "t").then_some(NodeId(9));
        let tcx = TopLayerCx { size: Size::new(80.0, 40.0), window: Size::new(300.0, 200.0), owner: Some(owner), rect_of: &rect_of, find_anchor: &found, rtl: false };
        let tl = p.top_layer(&tcx).unwrap();
        assert_eq!(tl.at, Some(Point::new(100.0, 74.0)), "below the named element, not its owner");
        assert_eq!((tl.inside, tl.close), (vec![button], None), "a press on the named element is not outside it");
        // it wins over the `anchor` record
        p.props.anchor = Anchor::Rect(Rect::new(0.0, 0.0, 5.0, 5.0));
        assert_eq!(p.top_layer(&tcx).unwrap().at, Some(Point::new(100.0, 74.0)));
        // no element has the name: it asks to close
        let gone = |_: &str| None;
        let tcx = TopLayerCx { find_anchor: &gone, ..tcx };
        assert_eq!(p.top_layer(&tcx).unwrap().close, Some(DismissReason::AnchorHidden));
    }

    /// POL4-SMALL 1: `anchor-to` in a document: the popup follows the named button as its scroll
    /// container scrolls, flips above it at the window's bottom edge, and closes when it is removed.
    #[test]
    fn anchor_to_follows_a_scrolled_button_flips_at_the_edge_and_closes_with_it() {
        use crate::testdoc::{BOX, child, doc, frame};
        use skelter_core::Value;
        let (mut d, root) = doc(Size::new(300.0, 200.0), |r| {
            crate::host::register::<Popup>(r).unwrap();
        });
        let kind = skelter_core::KindId::of("popup");
        let spec = d.registry().get(kind).unwrap().spec;
        let scroller = child(&mut d, root, BOX, "width: 300px; height: 200px; overflow: scroll; flex-shrink: 0");
        child(&mut d, scroller, BOX, "width: 200px; height: 150px; flex-shrink: 0");
        let button = child(&mut d, scroller, BOX, "width: 60px; height: 20px; flex-shrink: 0");
        child(&mut d, scroller, BOX, "width: 200px; height: 300px; flex-shrink: 0");
        d.set_prop(button, skelter_core::common::ANCHOR, Value::from("t")).unwrap();
        // declared elsewhere: its owner is the root, not the button
        let p = child(&mut d, root, kind, "width: 80px; height: 30px");
        d.set_prop(p, spec.prop_named("anchor-to").unwrap(), Value::from("t")).unwrap();
        d.set_prop(p, spec.prop_named("open").unwrap(), Value::Bool(true)).unwrap();
        frame(&mut d);
        frame(&mut d);
        let b = d.world_rect(button).unwrap();
        assert_eq!(b.min.y, 150.0);
        // no room below (174 + 30 > 200): above it
        assert_eq!(d.world_rect(p).unwrap().min, Point::new(b.min.x, b.min.y - 4.0 - 30.0), "flipped above the button");
        d.set_scroll_offset(scroller, skelter_base::geom::Vec2::new(0.0, 100.0));
        frame(&mut d);
        let b = d.world_rect(button).unwrap();
        assert_eq!(b.min.y, 50.0, "the button scrolled up");
        assert_eq!(d.world_rect(p).unwrap().min, Point::new(b.min.x, b.max.y + 4.0), "below it again, following the scroll");
        // the button goes away: it closes, `open` written back
        d.remove(scroller, button).unwrap();
        frame(&mut d);
        assert_eq!(d.get_prop(p, spec.prop_named("open").unwrap()), Some(Value::Bool(false)));
        frame(&mut d);
        assert!(!d.needs_frame(crate::testdoc::W), "settled");
    }
}
