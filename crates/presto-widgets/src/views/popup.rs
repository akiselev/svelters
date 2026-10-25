//! `popup` / `popover`: a floating surface next to an anchor, shown while `open`. The kind keeps its
//! props, places itself with [`place`] (side, alignment, gap, flip, shift, fit) and reports opening
//! and closing; the window's [`OverlayStack`](super::overlay::OverlayStack) owns its layer, routes
//! presses to it and light-dismisses it (the kind turns the stack's [`Closed`] into its `close`
//! signal through [`Popup::dismissed`]).
//!
//! Props: `open`, `placement` (`bottom-start` …), `anchor` (a node id, a `{x, y, width, height}`
//! rect, or `{x, y}` for a point), `gap`, `modal`, `light-dismiss`, `escape-dismiss`, `match-width`,
//! `flip`, `shift`, `fit`. Signals: `open`, `close` (`reason`), `toggle` (`open`).

use super::overlay::{Closed, Dismiss, DismissReason, OverlayClass};
use super::placement::{AnchorParams, Placed, Placement, place};
use super::legacy::{Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal};
use super::draw;
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::event::KeyState;
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::ids::NodeId;
use presto_base::key::Key;

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

pub const PLACEMENTS: &[&str] = &[
    "top", "top-start", "top-center", "top-end", "bottom", "bottom-start", "bottom-center", "bottom-end", "left", "left-start",
    "left-center", "left-end", "right", "right-start", "right-center", "right-end",
];

static SPEC: KindSpec = KindSpec {
    name: "popup",
    props: &[
        prop("open", PropType::Bool, ChangeClass::Structure),
        prop("placement", PropType::Enum(PLACEMENTS), ChangeClass::Property),
        prop("anchor", PropType::Record, ChangeClass::Property),
        prop("gap", PropType::F64, ChangeClass::Property),
        prop("modal", PropType::Bool, ChangeClass::Structure),
        prop("light-dismiss", PropType::Bool, ChangeClass::Property),
        prop("escape-dismiss", PropType::Bool, ChangeClass::Property),
        prop("match-width", PropType::Bool, ChangeClass::Layout),
        prop("flip", PropType::Bool, ChangeClass::Property),
        prop("shift", PropType::Bool, ChangeClass::Property),
        prop("fit", PropType::Bool, ChangeClass::Layout),
    ],
    signals: &[signal("open", true, false), signal("close", true, false), signal("toggle", true, false)],
    slots: &[""],
    role: accesskit::Role::Dialog,
};

/// A `popup` node.
#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    pub open: bool,
    pub placement: Placement,
    pub anchor: Anchor,
    pub gap: f32,
    pub modal: bool,
    pub dismiss: Dismiss,
    pub match_width: bool,
    pub flip: bool,
    pub shift: bool,
    pub fit: bool,
    /// Where it went last ([`Popup::place_in`]).
    placed: Option<Placed>,
}

impl Default for Popup {
    fn default() -> Self {
        Popup {
            open: false,
            placement: Placement::BOTTOM_START,
            anchor: Anchor::None,
            gap: 4.0,
            modal: false,
            dismiss: Dismiss::LIGHT,
            match_width: false,
            flip: true,
            shift: true,
            fit: true,
            placed: None,
        }
    }
}

fn anchor_of(v: &Value) -> Result<Anchor, PropError> {
    match v {
        Value::Null => Ok(Anchor::None),
        Value::I64(n) => Ok(Anchor::Node(NodeId(*n as u64))),
        Value::Record(_) => {
            let f = |k: &str| v.field(k).and_then(Value::as_f32);
            let (Some(x), Some(y)) = (f("x"), f("y")) else { return Err(PropError::Invalid("anchor needs x and y")) };
            match (f("width"), f("height")) {
                (Some(w), Some(h)) => Ok(Anchor::Rect(Rect::from_origin_size(Point::new(x, y), Size::new(w.max(0.0), h.max(0.0))))),
                _ => Ok(Anchor::Point(Point::new(x, y))),
            }
        }
        _ => Err(PropError::Invalid("anchor: a node id, a rect or a point")),
    }
}

impl Popup {
    /// The overlay class it opens as.
    pub fn class(&self) -> OverlayClass {
        if self.modal { OverlayClass::Modal } else { OverlayClass::Popup }
    }

    /// The overlay stack's dismiss flags for it (a modal popup never light-dismisses).
    pub fn dismiss_flags(&self) -> Dismiss {
        if self.modal { Dismiss { outside: false, escape: self.dismiss.escape } } else { self.dismiss }
    }

    /// The last placement.
    pub fn placed(&self) -> Option<Placed> {
        self.placed
    }

    /// Places a surface of `content` size next to `anchor` (the resolved anchor rect; `None` for
    /// [`Anchor::None`], which centres it) inside `bounds` (window points).
    pub fn place_in(&mut self, anchor: Option<Rect>, bounds: Rect, content: Size) -> Placed {
        let placed = match anchor {
            Some(a) => place(&AnchorParams {
                anchor: a,
                size: content,
                bounds,
                placement: self.placement,
                gap: self.gap,
                cross: 0.0,
                flip: self.flip,
                shift: self.shift,
                fit: self.fit,
                match_anchor_width: self.match_width,
            }),
            None => {
                let s = Size::new(content.w.min(bounds.width()), content.h.min(bounds.height()));
                let min = Point::new(bounds.min.x + (bounds.width() - s.w) / 2.0, bounds.min.y + (bounds.height() - s.h) / 2.0);
                Placed { side: self.placement.side, rect: Rect::from_origin_size(min, s) }
            }
        };
        self.placed = Some(placed);
        placed
    }

    /// The fixed anchor rect, when it is not another node.
    pub fn fixed_anchor(&self) -> Option<Rect> {
        match self.anchor {
            Anchor::Rect(r) => Some(r),
            Anchor::Point(p) => Some(Rect::from_origin_size(p, Size::ZERO)),
            Anchor::None | Anchor::Node(_) => None,
        }
    }

    /// Opens or closes it from Rust or a native default, emitting `open`/`close` and `toggle`.
    pub fn set_open(&mut self, cx: &mut EventCx<'_>, open: bool, reason: DismissReason) {
        if self.open == open {
            return;
        }
        self.open = open;
        if open {
            cx.emit("open", Value::Null);
        } else {
            self.placed = None;
            cx.emit("close", Value::Record(vec![("reason".into(), Value::Str(reason.name().into()))]));
        }
        cx.emit("toggle", Value::Bool(open));
        cx.mark(ChangeClass::Structure);
    }

    /// The overlay stack closed it.
    pub fn dismissed(&mut self, cx: &mut EventCx<'_>, closed: &Closed) {
        self.set_open(cx, false, closed.reason);
    }
}

impl Kind for Popup {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, name: &str, v: &Value) -> Result<ChangeClass, PropError> {
        let bool_of = |v: &Value| v.as_bool().ok_or(PropError::Invalid("expected a bool"));
        match name {
            "open" => {
                let o = bool_of(v)?;
                if !o {
                    self.placed = None;
                }
                self.open = o;
            }
            "placement" => self.placement = v.as_str().and_then(Placement::parse).ok_or(PropError::Invalid("unknown placement"))?,
            "anchor" => self.anchor = anchor_of(v)?,
            "gap" => self.gap = v.as_f32().ok_or(PropError::Invalid("expected a number"))?,
            "modal" => self.modal = bool_of(v)?,
            "light-dismiss" => self.dismiss.outside = bool_of(v)?,
            "escape-dismiss" => self.dismiss.escape = bool_of(v)?,
            "match-width" => self.match_width = bool_of(v)?,
            "flip" => self.flip = bool_of(v)?,
            "shift" => self.shift = bool_of(v)?,
            "fit" => self.fit = bool_of(v)?,
            _ => return Err(PropError::Unknown),
        }
        Ok(SPEC.prop(name).map_or(ChangeClass::Paint, |p| p.change))
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let rect = Rect::from_origin_size(Point::ZERO, cx.size);
        let p = cx.palette;
        draw::shadow(cx.list, rect, p.radius, p.shadow, 8.0, Vec2::new(0.0, 2.0));
        draw::boxed(cx.list, rect, p.radius, p.surface_raised, 1.0, p.border);
    }

    /// Escape while focus is inside it closes it when the stack did not (a popup opened outside a
    /// stack, in tests and embedders).
    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed && k.key == Some(Key::Escape) && self.open && self.dismiss.escape => {
                self.set_open(cx, false, DismissReason::Escape);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(if self.modal { accesskit::Role::Dialog } else { accesskit::Role::Group });
        if self.modal {
            node.set_modal();
        }
        if !self.open {
            node.set_hidden();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::overlay::{Closed, OverlayId};
    use crate::views::placement::Side;
    use crate::views::legacy::{Os, Policy};
    use presto_base::event::{KeyEvent, KeyLocation, Modifiers};
    use presto_base::time::Time;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Value {
        Value::Record(vec![("x".into(), Value::F64(x as f64)), ("y".into(), Value::F64(y as f64)), ("width".into(), Value::F64(w as f64)), ("height".into(), Value::F64(h as f64))])
    }

    #[test]
    fn places_through_the_kind_with_flip_and_match_width() {
        let mut p = Popup::default();
        p.set_prop("anchor", &rect(100.0, 270.0, 50.0, 20.0)).unwrap();
        p.set_prop("placement", &Value::Str("bottom-start".into())).unwrap();
        p.set_prop("gap", &Value::F64(2.0)).unwrap();
        p.set_prop("match-width", &Value::Bool(true)).unwrap();
        let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        let placed = p.place_in(p.fixed_anchor(), bounds, Size::new(30.0, 40.0));
        assert_eq!(placed.side, Side::Top, "no room below");
        assert_eq!(placed.rect, Rect::new(100.0, 228.0, 150.0, 268.0));
        assert_eq!(p.placed(), Some(placed));
        assert_eq!(p.set_prop("placement", &Value::Str("middle".into())), Err(PropError::Invalid("unknown placement")));
        assert_eq!(p.set_prop("nope", &Value::Null), Err(PropError::Unknown));
    }

    #[test]
    fn a_point_anchor_and_no_anchor() {
        let mut p = Popup::default();
        p.set_prop("anchor", &Value::Record(vec![("x".into(), Value::F64(10.0)), ("y".into(), Value::F64(20.0))])).unwrap();
        assert_eq!(p.anchor, Anchor::Point(Point::new(10.0, 20.0)));
        p.set_prop("anchor", &Value::Null).unwrap();
        let placed = p.place_in(None, Rect::new(0.0, 0.0, 400.0, 300.0), Size::new(100.0, 50.0));
        assert_eq!(placed.rect, Rect::new(150.0, 125.0, 250.0, 175.0), "centred");
        p.set_prop("anchor", &Value::I64(7)).unwrap();
        assert_eq!((p.anchor, p.fixed_anchor()), (Anchor::Node(NodeId(7)), None));
    }

    #[test]
    fn escape_and_dismissal_emit_close_and_toggle() {
        let pol = Policy::for_os(Os::Linux);
        let mut p = Popup::default();
        let mut cx = EventCx::new(Time::ZERO, Size::new(100.0, 100.0), &pol);
        p.set_open(&mut cx, true, DismissReason::Programmatic);
        assert_eq!(cx.emitted.iter().map(|e| e.signal).collect::<Vec<_>>(), vec!["open", "toggle"]);
        let esc = Event::Key(KeyEvent { key: Some(Key::Escape), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() });
        let mut cx = EventCx::new(Time::ZERO, Size::new(100.0, 100.0), &pol);
        assert_eq!(p.event(&mut cx, &esc), Handled::Yes);
        assert!(!p.open);
        assert_eq!(cx.emitted("close").next().and_then(|e| e.detail.field("reason").cloned()), Some(Value::Str("escape".into())));
        assert_eq!(cx.changed, Some(ChangeClass::Structure));
        // closed by the stack
        let mut cx = EventCx::new(Time::ZERO, Size::new(100.0, 100.0), &pol);
        p.set_open(&mut cx, true, DismissReason::Programmatic);
        p.dismissed(&mut cx, &Closed { id: OverlayId(1), node: NodeId(1), reason: DismissReason::OutsidePress, restore_focus: None });
        assert!(!p.open);
        assert!(cx.emitted("close").any(|e| e.detail.field("reason") == Some(&Value::Str("outside-press".into()))));
    }

    #[test]
    fn modal_popups_never_light_dismiss() {
        let mut p = Popup::default();
        p.set_prop("modal", &Value::Bool(true)).unwrap();
        assert_eq!(p.class(), OverlayClass::Modal);
        assert_eq!(p.dismiss_flags(), Dismiss { outside: false, escape: true });
    }
}
