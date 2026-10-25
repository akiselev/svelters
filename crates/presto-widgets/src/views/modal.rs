//! `modal`: a dialog over a backdrop that covers the whole window (svelters-egui `widgets/dialog.rs`).
//! The node is the full-window overlay entry; the dialog box sits centred in it at its content's
//! size (clamped to the window less a margin), with an optional heading row (`heading`: the common
//! `title` prop stays the tooltip every node has). Nothing below is reached
//! while it is open (the overlay stack's modal block).
//!
//! It computes the dialog rect ([`Modal::dialog_rect`]), the cancel gestures (Escape, a press and
//! release on the backdrop, both only when `dismissable`; the `cancel` signal with `reason`), and its
//! dialog focus scope ([`FocusScope`]): Tab and Shift+Tab cycle inside it, and focus goes back to the
//! opener when it closes.

use super::common::emit;
use crate::controls::common::host_view;
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, Ua, UaRule, UaValue::*};
use crate::interact::StateBits;
use crate::shim::core::{A11yCx, ChildCx, Event, EventCx, Handled, Kind, KindSpec, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value, ValueType};
use presto_base::accesskit;
use presto_base::event::{KeyState, MouseButton, PointerPhase};
use presto_base::geom::{Point, Rect, Size};
use presto_base::ids::NodeId;
use presto_base::key::Key;

/// The space kept between the dialog and the window's edges.
pub const MARGIN: f32 = 24.0;
/// The heading row's height.
pub const TITLE_H: f32 = 32.0;
const PAD: f32 = 12.0;

/// A dialog focus scope: the focusable nodes inside a dialog in tree order, and where focus came from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusScope {
    /// Focus goes back here when the scope closes.
    pub opener: Option<NodeId>,
}

impl FocusScope {
    /// Where Tab (or Shift+Tab with `back`) moves from `current` among `members` (tree order),
    /// wrapping inside the scope. `None` when the scope has nothing focusable.
    pub fn next(&self, members: &[NodeId], current: Option<NodeId>, back: bool) -> Option<NodeId> {
        if members.is_empty() {
            return None;
        }
        let n = members.len();
        let i = match current.and_then(|c| members.iter().position(|m| *m == c)) {
            Some(i) if back => (i + n - 1) % n,
            Some(i) => (i + 1) % n,
            None if back => n - 1,
            None => 0,
        };
        Some(members[i])
    }

    /// Where focus goes when the scope opens: the `autofocus` node, else the first member.
    pub fn initial(&self, members: &[NodeId], autofocus: Option<NodeId>) -> Option<NodeId> {
        autofocus.filter(|a| members.contains(a)).or_else(|| members.first().copied())
    }
}

/// The node's UA sheet: the backdrop dimming the window.
pub const UA: Ua = Ua(&[UaRule::base(&[("background-color", Color(presto_base::color::Color::rgba(0.0, 0.0, 0.0, 0.4)))])]);

/// `::part(dialog)`'s UA sheet: the raised surface with the window radius and `window-shadow`.
pub const DIALOG_UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("surface-raised")),
    ("border-color", Token("border-widget")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("window-radius")),
    ("box-shadow", Token("window-shadow")),
])]);

/// `::part(heading)`'s UA sheet: the strong foreground, semibold.
pub const HEADING_UA: Ua = Ua(&[UaRule::base(&[("color", Token("fg-strong")), ("font-weight", Number(600.0))])]);

kind_props! {
    /// The modal's props.
    pub struct ModalProps;
    /// Shown; closing natively writes it back.
    open "open": bool = Bool(false) ; Bool, Structure, bindable;
    /// The dialog's heading row (empty: none).
    heading "heading": String = Str("") ; Str, Layout;
    /// Escape and a backdrop click ask to cancel it.
    dismissable "dismissable": bool = Bool(true) ; Bool, Property;
}

pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("cancel", &[("reason", ValueType::Str)], "Escape or a backdrop click asked to close it (`escape`, `backdrop`).").cancelable(),
    SignalSpec::new("close", &[("reason", ValueType::Str)], "It closed."),
];

pub static SPEC: KindSpec = KindSpec {
    tag: "modal",
    props: ModalProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Dialog,
    name_from: NameFrom::Prop("heading"),
    text_prop: None,
    focusable: false,
};

/// A `modal` node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Modal {
    pub props: ModalProps,
    /// The content's laid-out size (without heading and padding).
    pub content: Size,
    pub scope: FocusScope,
    /// A primary press went down on the backdrop (a cancel needs the release there too).
    backdrop_down: bool,
}

impl Modal {
    fn heading_h(&self) -> f32 {
        if self.props.heading.is_empty() { 0.0 } else { TITLE_H }
    }

    /// The dialog box in a window of `window` size: centred, at most the window less [`MARGIN`].
    pub fn dialog_rect(&self, window: Size) -> Rect {
        let want = Size::new(self.content.w + 2.0 * PAD, self.content.h + 2.0 * PAD + self.heading_h());
        let s = Size::new(want.w.min((window.w - 2.0 * MARGIN).max(0.0)), want.h.min((window.h - 2.0 * MARGIN).max(0.0)));
        let min = Point::new(((window.w - s.w) / 2.0).round(), ((window.h - s.h) / 2.0).round());
        Rect::from_origin_size(min, s)
    }

    /// Where the content goes inside the window.
    pub fn content_rect(&self, window: Size) -> Rect {
        let d = self.dialog_rect(window);
        Rect::new(d.min.x + PAD, d.min.y + PAD + self.heading_h(), d.max.x - PAD, d.max.y - PAD)
    }

    /// Asks to cancel: `cancel` goes to JS at event time and, unless a listener prevents it, the
    /// modal closes ([`Modal::close`]).
    fn cancel(&mut self, cx: &mut EventCx<'_>, reason: &'static str) {
        let prevented = match SPEC.signal("cancel") {
            Some(id) => cx.emit_sync(id, vec![Value::str(reason)]),
            None => false,
        };
        if !prevented {
            self.close(cx, reason);
        }
    }

    /// Closes it (`open` written back as native state); focus goes back to the opener (returned).
    pub fn close(&mut self, cx: &mut EventCx<'_>, reason: &'static str) -> Option<NodeId> {
        if !self.props.open {
            return None;
        }
        self.props.open = false;
        self.backdrop_down = false;
        emit(cx, &SPEC, "close", vec![Value::str(reason)]);
        cx.prop_changed(P::open.id());
        self.scope.opener
    }
}

impl Kind for Modal {
    fn parts() -> &'static [&'static str] {
        &["dialog", "heading"]
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
        self.props.set(prop, v)
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    /// The content is not laid out, painted or hit while the modal is closed.
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
        let dialog = host.part(cx.style.part_css("dialog"), DIALOG_UA);
        let heading = dialog.part(cx.style.part_css("heading"), HEADING_UA);
        let (rect, d) = (cx.rect(), self.dialog_rect(cx.size));
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &host, rect);
        paint::paint_box(&mut pc, &dialog, d);
        if !self.props.heading.is_empty() {
            let row = Rect::new(d.min.x + PAD, d.min.y + PAD, d.max.x - PAD, d.min.y + PAD + TITLE_H);
            let layout = paint::text_layout(pc.text, &heading, &self.props.heading, None);
            paint::paint_text(&mut pc, &heading, &layout, row, Align::Start);
        }
    }

    /// Escape and a backdrop click ask to cancel; every pointer event is taken (nothing below the
    /// modal is reached).
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        if !self.props.open {
            return Handled::No;
        }
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed && k.key == Some(Key::Escape) => {
                if self.props.dismissable {
                    self.cancel(cx, "escape");
                }
                Handled::Yes
            }
            Event::Pointer(pe) => {
                let on_backdrop = !self.dialog_rect(cx.size).contains(pe.pos);
                match pe.phase {
                    PointerPhase::Down if pe.button == Some(MouseButton::Primary) => self.backdrop_down = on_backdrop,
                    PointerPhase::Up if pe.button == Some(MouseButton::Primary) => {
                        if std::mem::take(&mut self.backdrop_down) && on_backdrop && self.props.dismissable {
                            self.cancel(cx, "backdrop");
                        }
                    }
                    PointerPhase::Cancel => self.backdrop_down = false,
                    _ => {}
                }
                Handled::Yes
            }
            Event::CaptureLost => {
                self.backdrop_down = false;
                Handled::Yes
            }
            Event::Wheel(_) | Event::Hover(_) => Handled::Yes,
            _ => Handled::No,
        }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Dialog);
        cx.node.set_modal();
        if !self.props.heading.is_empty() {
            cx.node.set_label(self.props.heading.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::{Policy, StateBits};
    use presto_base::event::{KeyEvent, KeyLocation, Modifiers, PointerEvent, PointerType};
    use presto_base::geom::Vec2;
    use presto_base::time::Time;

    fn ptr(phase: PointerPhase, x: f32, y: f32) -> Event {
        Event::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: Some(MouseButton::Primary),
            buttons: 0,
            pos: Point::new(x, y),
            modifiers: Modifiers::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: Default::default(),
        })
    }

    fn esc() -> Event {
        Event::Key(KeyEvent { key: Some(Key::Escape), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() })
    }

    fn open_modal() -> Modal {
        let mut m = Modal { content: Size::new(200.0, 100.0), scope: FocusScope { opener: Some(NodeId(9)) }, ..Modal::default() };
        m.props.open = true;
        m.props.heading = "Rename".into();
        m
    }

    /// The signals `cx` holds with their first argument.
    fn signals(cx: &EventCx) -> Vec<(&'static str, Option<Value>)> {
        cx.emitted.iter().map(|e| (SPEC.signals[e.signal.0 as usize].name, e.args.first().cloned())).collect()
    }

    #[test]
    fn centres_and_clamps_the_dialog() {
        let m = open_modal();
        let d = m.dialog_rect(Size::new(800.0, 600.0));
        assert_eq!(d.size(), Size::new(224.0, 156.0));
        assert_eq!(d.min, Point::new(288.0, 222.0));
        let small = m.dialog_rect(Size::new(200.0, 150.0));
        assert_eq!(small, Rect::new(24.0, 24.0, 176.0, 126.0));
        assert_eq!(m.content_rect(Size::new(800.0, 600.0)).min, Point::new(300.0, 266.0));
    }

    #[test]
    fn escape_and_backdrop_click_cancel_inside_presses_do_not() {
        let pol = Policy::default();
        let size = Size::new(800.0, 600.0);
        let mut st = StateBits::NONE;
        let mut m = open_modal();
        let mut cx = EventCx::new(Time::ZERO, size, &mut st, &pol);
        assert_eq!(m.event(&mut cx, &esc()), Handled::Yes);
        assert_eq!(signals(&cx), vec![("cancel", Some(Value::str("escape"))), ("close", Some(Value::str("escape")))], "nobody prevented it: it closes");
        assert!(!m.props.open);
        assert_eq!(cx.changed, vec![P::open.id()]);
        // a press inside the dialog dragged out to the backdrop is not a backdrop click
        let mut m = open_modal();
        let mut cx = EventCx::new(Time::ZERO, size, &mut st, &pol);
        m.event(&mut cx, &ptr(PointerPhase::Down, 400.0, 300.0));
        m.event(&mut cx, &ptr(PointerPhase::Up, 10.0, 10.0));
        assert!(signals(&cx).is_empty());
        // a backdrop click
        m.event(&mut cx, &ptr(PointerPhase::Down, 10.0, 10.0));
        assert_eq!(m.event(&mut cx, &ptr(PointerPhase::Up, 12.0, 10.0)), Handled::Yes);
        assert_eq!(signals(&cx)[0], ("cancel", Some(Value::str("backdrop"))));
        assert!(SPEC.signals[0].cancelable, "`cancel` can be prevented");
        // not dismissable: nothing cancels, but presses are still swallowed
        let mut m = open_modal();
        m.props.dismissable = false;
        let mut cx = EventCx::new(Time::ZERO, size, &mut st, &pol);
        assert_eq!(m.event(&mut cx, &esc()), Handled::Yes);
        m.event(&mut cx, &ptr(PointerPhase::Down, 10.0, 10.0));
        assert_eq!(m.event(&mut cx, &ptr(PointerPhase::Up, 10.0, 10.0)), Handled::Yes);
        assert!(cx.emitted.is_empty());
        // closing returns the opener and writes `open` back
        assert_eq!(m.close(&mut cx, "accept"), Some(NodeId(9)));
        assert_eq!(cx.changed, vec![P::open.id()]);
        assert_eq!(m.close(&mut cx, "accept"), None, "already closed");
    }

    #[test]
    fn a_prevented_cancel_keeps_it_open() {
        let pol = Policy::default();
        let mut st = StateBits::NONE;
        let mut m = open_modal();
        let mut seen = Vec::new();
        let mut prevent = |sig: crate::shim::core::SignalId, args: &[Value]| {
            seen.push((SPEC.signals[sig.0 as usize].name, args.to_vec()));
            true
        };
        let mut cx = EventCx::new(Time::ZERO, Size::new(800.0, 600.0), &mut st, &pol);
        cx.sync = Some(&mut prevent);
        m.event(&mut cx, &esc());
        assert!(cx.emitted.is_empty(), "delivered at event time, not queued");
        assert!(cx.changed.is_empty());
        drop(cx);
        assert_eq!(seen, vec![("cancel", vec![Value::str("escape")])]);
        assert!(m.props.open, "the listener kept it open");
    }

    #[test]
    fn closed_it_hides_its_content_and_open_is_a_state() {
        let mut m = open_modal();
        assert_eq!(m.prop_state(), StateBits::OPEN);
        let mut style = taffy::Style::default();
        assert!(m.child_layout(&ChildCx { index: 0, anchors: &[""] }, &mut style));
        m.props.open = false;
        assert!(!m.child_layout(&ChildCx { index: 0, anchors: &[""] }, &mut style));
        assert_eq!(style.display, taffy::Display::None);
        assert_eq!(m.prop_state(), StateBits::NONE);
        assert_eq!(Modal::prop_states(), StateBits::OPEN);
    }

    #[test]
    fn records_backdrop_dialog_and_heading() {
        let m = open_modal();
        let (list, _) = testkit::record(&m, Size::new(800.0, 600.0), StateBits::NONE);
        // backdrop, window-shadow, dialog surface, heading
        assert_eq!(testkit::kinds(&list), ["quad", "shadow", "quad", "glyphs"]);
        assert_eq!(SPEC.prop("heading").map(|p| SPEC.prop_spec(p).unwrap().name), Some("heading"));
        assert_eq!(SPEC.prop("title"), None, "the common `title` stays the tooltip");
    }

    #[test]
    fn tab_cycles_inside_the_scope() {
        let s = FocusScope::default();
        let m = [NodeId(1), NodeId(2), NodeId(3)];
        assert_eq!(s.initial(&m, Some(NodeId(2))), Some(NodeId(2)));
        assert_eq!(s.initial(&m, Some(NodeId(7))), Some(NodeId(1)));
        assert_eq!(s.next(&m, Some(NodeId(3)), false), Some(NodeId(1)));
        assert_eq!(s.next(&m, Some(NodeId(1)), true), Some(NodeId(3)));
        assert_eq!(s.next(&m, None, true), Some(NodeId(3)));
        assert_eq!(s.next(&[], None, false), None);
    }
}
