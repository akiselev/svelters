//! `modal`: a dialog over a backdrop that covers the whole window (svelters-egui `widgets/dialog.rs`).
//! The node is the full-window overlay entry; the dialog box sits centred in it at its content's
//! size (clamped to the window less a margin), with an optional title row. Nothing below is reached
//! while it is open (the overlay stack's modal block).
//!
//! It computes the dialog rect ([`Modal::dialog_rect`]), the cancel gestures (Escape, a press and
//! release on the backdrop, both only when `dismissable`; the `cancel` signal with `reason`), and its
//! dialog focus scope ([`FocusScope`]): Tab and Shift+Tab cycle inside it, and focus goes back to the
//! opener when it closes.

use super::draw;
use super::shim::{Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::event::{KeyState, MouseButton, PointerPhase};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::ids::NodeId;
use presto_base::key::Key;

/// The space kept between the dialog and the window's edges.
pub const MARGIN: f32 = 24.0;
/// The title row's height.
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

static SPEC: KindSpec = KindSpec {
    name: "modal",
    props: &[
        prop("open", PropType::Bool, ChangeClass::Structure),
        prop("title", PropType::Str, ChangeClass::Layout),
        prop("dismissable", PropType::Bool, ChangeClass::Property),
    ],
    signals: &[signal("cancel", false, true), signal("close", true, false)],
    slots: &[""],
    role: accesskit::Role::Dialog,
};

/// A `modal` node.
#[derive(Clone, Debug, PartialEq)]
pub struct Modal {
    pub open: bool,
    pub title: String,
    /// Escape and a backdrop press cancel it.
    pub dismissable: bool,
    /// The content's laid-out size (without title and padding).
    pub content: Size,
    pub scope: FocusScope,
    /// A primary press went down on the backdrop (a cancel needs the release there too).
    backdrop_down: bool,
}

impl Default for Modal {
    fn default() -> Self {
        Modal { open: false, title: String::new(), dismissable: true, content: Size::ZERO, scope: FocusScope::default(), backdrop_down: false }
    }
}

impl Modal {
    /// The dialog box in a window of `window` size: centred, at most the window less [`MARGIN`].
    pub fn dialog_rect(&self, window: Size) -> Rect {
        let title = if self.title.is_empty() { 0.0 } else { TITLE_H };
        let want = Size::new(self.content.w + 2.0 * PAD, self.content.h + 2.0 * PAD + title);
        let s = Size::new(want.w.min((window.w - 2.0 * MARGIN).max(0.0)), want.h.min((window.h - 2.0 * MARGIN).max(0.0)));
        let min = Point::new(((window.w - s.w) / 2.0).round(), ((window.h - s.h) / 2.0).round());
        Rect::from_origin_size(min, s)
    }

    /// Where the content goes inside the window.
    pub fn content_rect(&self, window: Size) -> Rect {
        let d = self.dialog_rect(window);
        let title = if self.title.is_empty() { 0.0 } else { TITLE_H };
        Rect::new(d.min.x + PAD, d.min.y + PAD + title, d.max.x - PAD, d.max.y - PAD)
    }

    /// Asks to cancel: `cancel` is cancelable; the app (or the default when nobody prevents it)
    /// closes it with [`Modal::close`].
    fn cancel(&mut self, cx: &mut EventCx<'_>, reason: &'static str) {
        cx.emit("cancel", Value::Record(vec![("reason".into(), Value::Str(reason.into()))]));
    }

    /// Closes it; focus goes back to the opener (returned).
    pub fn close(&mut self, cx: &mut EventCx<'_>, reason: &'static str) -> Option<NodeId> {
        if !self.open {
            return None;
        }
        self.open = false;
        self.backdrop_down = false;
        cx.emit("close", Value::Record(vec![("reason".into(), Value::Str(reason.into()))]));
        cx.mark(ChangeClass::Structure);
        self.scope.opener
    }
}

impl Kind for Modal {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, name: &str, v: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "open" => self.open = v.as_bool().ok_or(PropError::Invalid("expected a bool"))?,
            "title" => self.title = v.as_str().ok_or(PropError::Invalid("expected a string"))?.to_string(),
            "dismissable" => self.dismissable = v.as_bool().ok_or(PropError::Invalid("expected a bool"))?,
            _ => return Err(PropError::Unknown),
        }
        Ok(SPEC.prop(name).map_or(ChangeClass::Paint, |p| p.change))
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let p = cx.palette;
        draw::fill(cx.list, Rect::from_origin_size(Point::ZERO, cx.size), 0.0, p.backdrop);
        let d = self.dialog_rect(cx.size);
        draw::shadow(cx.list, d, p.radius * 2.0, p.shadow, 16.0, Vec2::new(0.0, 4.0));
        draw::boxed(cx.list, d, p.radius * 2.0, p.surface_raised, 1.0, p.border);
        if !self.title.is_empty() {
            let th = cx.labels.measure(&self.title, p.font_size, None).h;
            let origin = Point::new(d.min.x + PAD, d.min.y + PAD + (TITLE_H - th) / 2.0);
            cx.labels.record(cx.list, &self.title, origin, p.font_size, p.text, Some((d.width() - 2.0 * PAD).max(0.0)));
        }
    }

    /// Escape and a backdrop click ask to cancel; every pointer event is taken (nothing below the
    /// modal is reached).
    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        if !self.open {
            return Handled::No;
        }
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed && k.key == Some(Key::Escape) => {
                if self.dismissable {
                    self.cancel(cx, "escape");
                }
                Handled::Yes
            }
            Event::Pointer(pe) => {
                let on_backdrop = !self.dialog_rect(cx.size).contains(pe.pos);
                match pe.phase {
                    PointerPhase::Down if pe.button == Some(MouseButton::Primary) => self.backdrop_down = on_backdrop,
                    PointerPhase::Up if pe.button == Some(MouseButton::Primary) => {
                        if std::mem::take(&mut self.backdrop_down) && on_backdrop && self.dismissable {
                            self.cancel(cx, "backdrop");
                        }
                    }
                    PointerPhase::Cancel => self.backdrop_down = false,
                    _ => {}
                }
                Handled::Yes
            }
            Event::Wheel(_) => Handled::Yes,
            _ => Handled::No,
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Dialog);
        node.set_modal();
        if !self.title.is_empty() {
            node.set_label(self.title.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::shim::{Os, Policy};
    use presto_base::event::{KeyEvent, KeyLocation, Modifiers, PointerEvent, PointerType};
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
        Modal { open: true, title: "Rename".into(), content: Size::new(200.0, 100.0), scope: FocusScope { opener: Some(NodeId(9)) }, ..Modal::default() }
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
        let pol = Policy::for_os(Os::Linux);
        let size = Size::new(800.0, 600.0);
        let mut m = open_modal();
        let mut cx = EventCx::new(Time::ZERO, size, &pol);
        assert_eq!(m.event(&mut cx, &esc()), Handled::Yes);
        assert_eq!(cx.emitted("cancel").next().and_then(|e| e.detail.field("reason").cloned()), Some(Value::Str("escape".into())));
        // a press inside the dialog dragged out to the backdrop is not a backdrop click
        let mut cx = EventCx::new(Time::ZERO, size, &pol);
        m.event(&mut cx, &ptr(PointerPhase::Down, 400.0, 300.0));
        m.event(&mut cx, &ptr(PointerPhase::Up, 10.0, 10.0));
        assert_eq!(cx.emitted("cancel").count(), 0);
        // a backdrop click
        m.event(&mut cx, &ptr(PointerPhase::Down, 10.0, 10.0));
        assert_eq!(m.event(&mut cx, &ptr(PointerPhase::Up, 12.0, 10.0)), Handled::Yes);
        assert_eq!(cx.emitted("cancel").next().and_then(|e| e.detail.field("reason").cloned()), Some(Value::Str("backdrop".into())));
        // not dismissable: nothing cancels, but presses are still swallowed
        m.dismissable = false;
        let mut cx = EventCx::new(Time::ZERO, size, &pol);
        assert_eq!(m.event(&mut cx, &esc()), Handled::Yes);
        m.event(&mut cx, &ptr(PointerPhase::Down, 10.0, 10.0));
        assert_eq!(m.event(&mut cx, &ptr(PointerPhase::Up, 10.0, 10.0)), Handled::Yes);
        assert!(cx.emitted.is_empty());
        // closing returns the opener
        assert_eq!(m.close(&mut cx, "accept"), Some(NodeId(9)));
        assert_eq!(m.close(&mut cx, "accept"), None, "already closed");
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
