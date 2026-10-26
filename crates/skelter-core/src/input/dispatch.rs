//! Event-time dispatch (architecture §11.1, svelters EV1): for each platform event, the hit test on
//! the last frame's hit list, the native defaults (hover, press, click, focus, Tab, wheel scrolling,
//! the kinds' own handlers, innermost first), then the JS-facing events with their capture/target/
//! bubble paths, which svelters-skelter delivers before the microtask drain.

use crate::common;
use crate::cx::{Emitted, EventCx};
use crate::document::Document;
use crate::ids::SignalId;
use crate::input::{Delivery, Dispatched, JsEvent, Phase};
use crate::kind::{Event, FocusReason, Handled, HitResult, PointerInput, WheelInput};
use crate::tree::ListenFlags;
use crate::value::Value;
use skelter_base::event::{ImeEvent, InputEnvelope, InputEvent, KeyEvent, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType, ScrollDelta, WheelEvent};
use skelter_base::geom::{Point, Vec2};
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::key::Key;
use skelter_base::time::{Span, Time};
use smallvec::SmallVec;
use std::sync::Arc;
use svelters_style::StateMask;

/// Two clicks closer than this (and 4 points) count as a double click.
const DOUBLE_CLICK: Span = Span::from_millis(500);
const DOUBLE_CLICK_SLOP: f32 = 4.0;
/// Points per wheel line, and per page as a fraction of the viewport (Chromium's defaults).
const LINE: f32 = 40.0;
const PAGE: f32 = 0.875;

/// Calls `node`'s kind's native handler.
pub(crate) fn call_kind(doc: &mut Document, node: NodeId, ev: &Event, emitted: &mut Vec<Emitted>, pointer: Option<u32>) -> Handled {
    let Some(i) = doc.idx(node) else { return Handled::No };
    let Some(mut k) = doc.kinds[i].take() else { return Handled::No };
    let h = {
        let mut cx = EventCx { doc, node, emitted, pointer };
        k.event(&mut cx, ev)
    };
    doc.restore_kind(node, k);
    if !doc.peer_queue.is_empty() {
        crate::input::peers::run(doc, emitted);
    }
    h
}

/// Calls native handlers from `target` outwards until one handles the event (disabled nodes skip).
fn bubble_native(doc: &mut Document, target: NodeId, ev: &Event, emitted: &mut Vec<Emitted>, pointer: Option<u32>) -> Option<NodeId> {
    let mut a = Some(target);
    while let Some(n) = a {
        if !doc.state(n).contains(StateMask::DISABLED) && call_kind(doc, n, ev, emitted, pointer) == Handled::Yes {
            return Some(n);
        }
        a = doc.parent(n);
    }
    None
}

/// The listener calls of `signal` dispatched at `target`: capture from the root down, the target,
/// then (if it bubbles) up to the root.
pub(crate) fn listener_path(doc: &Document, target: NodeId, signal: SignalId, bubbles: bool) -> SmallVec<[Delivery; 4]> {
    let mut chain: SmallVec<[NodeId; 16]> = SmallVec::new();
    let mut a = Some(target);
    while let Some(n) = a {
        chain.push(n);
        a = doc.parent(n);
    }
    let mut path = SmallVec::new();
    let of = |n: NodeId, capture: bool| {
        doc.listeners(n).iter().filter(move |l| l.signal == signal && l.flags.contains(ListenFlags::CAPTURE) == capture).map(|l| l.flags)
    };
    for &n in chain.iter().skip(1).rev() {
        for flags in of(n, true) {
            path.push(Delivery { node: n, phase: Phase::Capture, flags });
        }
    }
    for flags in of(target, true).chain(of(target, false)) {
        path.push(Delivery { node: target, phase: Phase::Target, flags });
    }
    if bubbles {
        for &n in chain.iter().skip(1) {
            for flags in of(n, false) {
                path.push(Delivery { node: n, phase: Phase::Bubble, flags });
            }
        }
    }
    path
}

/// Builds the JS event for `signal` at `target` (`None` when nobody listens).
fn js_event(doc: &Document, target: NodeId, signal: SignalId, arg: Option<Value>) -> Option<JsEvent> {
    let spec = doc.view(target).and_then(|v| v.kind().signal(signal)).or_else(|| common::signal(signal));
    let flags = spec.map(|s| s.flags).unwrap_or_default();
    let bubbles = flags.contains(crate::kind::SignalFlags::BUBBLES);
    let path = listener_path(doc, target, signal, bubbles);
    if path.is_empty() {
        return None;
    }
    let cancelable = flags.contains(crate::kind::SignalFlags::CANCELABLE) && path.iter().any(|d| !d.flags.contains(ListenFlags::PASSIVE));
    let sync = flags.contains(crate::kind::SignalFlags::SYNC) || path.iter().any(|d| d.flags.contains(ListenFlags::SYNC));
    Some(JsEvent { signal, target, arg, path, cancelable, sync })
}

/// Delivers `signal` at `target` to JS now through the document's sync deliverer and reports what
/// the listeners did (`Ok(default)` when nobody listens). `Err(arg)` hands the argument back when
/// the document has no deliverer: the caller queues the event instead.
pub(crate) fn deliver_now(doc: &mut Document, target: NodeId, signal: SignalId, arg: Option<Value>) -> Result<crate::kind::Outcome, Option<Value>> {
    if doc.delivering > 0 {
        // nested (JS inside the deliverer clicked or focused something): delivered, not cancelable,
        // once the listener call that caused it returns
        if let Some(mut ev) = js_event(doc, target, signal, arg) {
            ev.cancelable = false;
            doc.nested.push(ev);
        }
        return Ok(Default::default());
    }
    if doc.sync_delivery.is_none() {
        return Err(arg);
    }
    let Some(ev) = js_event(doc, target, signal, arg) else { return Ok(Default::default()) };
    let mut f = doc.sync_delivery.take().expect("checked");
    doc.delivering += 1;
    let o = f(doc, &ev);
    // the sync events its listeners caused, in order, until none is left
    loop {
        let batch = std::mem::take(&mut doc.nested);
        if batch.is_empty() {
            break;
        }
        for e in batch {
            f(doc, &e);
        }
    }
    doc.delivering -= 1;
    if doc.sync_delivery.is_none() {
        doc.sync_delivery = Some(f);
    }
    Ok(crate::kind::Outcome { prevented: o.prevented && ev.cancelable, stopped: o.stopped })
}

/// A DOM event delivered before its native default (with a sync deliverer), or listed after it.
enum Pre {
    Delivered(crate::kind::Outcome),
    Later(Option<Value>),
}

impl Pre {
    fn prevented(&self) -> bool {
        matches!(self, Pre::Delivered(o) if o.prevented)
    }
}

/// A kind's emitted signal, as a JS event queued on the document.
pub(crate) fn queue_emitted(doc: &mut Document, e: Emitted) {
    if let Some(ev) = js_event(doc, e.node, e.signal, e.arg) {
        doc.input_events.push(ev);
    }
}

fn modifiers_record(m: Modifiers, fields: &mut Vec<(Arc<str>, Value)>) {
    fields.push(("shiftKey".into(), Value::Bool(m.contains(Modifiers::SHIFT))));
    fields.push(("ctrlKey".into(), Value::Bool(m.contains(Modifiers::CTRL))));
    fields.push(("altKey".into(), Value::Bool(m.contains(Modifiers::ALT))));
    fields.push(("metaKey".into(), Value::Bool(m.contains(Modifiers::SUPER))));
}

fn button_index(b: Option<MouseButton>) -> i64 {
    match b {
        Some(MouseButton::Primary) | None => 0,
        Some(MouseButton::Middle) => 1,
        Some(MouseButton::Secondary) => 2,
        Some(MouseButton::Back) => 3,
        Some(MouseButton::Forward) => 4,
        Some(MouseButton::Other(n)) => n as i64,
    }
}

/// The DOM `PointerEvent` fields.
fn pointer_record(p: &PointerInput) -> Value {
    let mut f: Vec<(Arc<str>, Value)> = vec![
        ("clientX".into(), Value::F64(p.pos.x as f64)),
        ("clientY".into(), Value::F64(p.pos.y as f64)),
        ("offsetX".into(), Value::F64(p.local.x as f64)),
        ("offsetY".into(), Value::F64(p.local.y as f64)),
        ("button".into(), Value::I64(button_index(p.button))),
        ("buttons".into(), Value::I64(p.buttons as i64)),
        ("pointerId".into(), Value::I64(p.pointer as i64)),
        (
            "pointerType".into(),
            Value::from(match p.kind {
                PointerType::Mouse => "mouse",
                PointerType::Pen => "pen",
                PointerType::Touch => "touch",
            }),
        ),
        ("pressure".into(), Value::F64(p.pressure as f64)),
        ("detail".into(), Value::I64(p.count as i64)),
    ];
    modifiers_record(p.modifiers, &mut f);
    Value::Record(f.into())
}

fn key_record(k: &KeyEvent) -> Value {
    let key = k.text.clone().filter(|t| !t.chars().any(char::is_control)).or_else(|| k.key.map(|k| k.name().to_string())).unwrap_or_default();
    let mut f: Vec<(Arc<str>, Value)> = vec![
        ("key".into(), Value::from(key)),
        ("code".into(), Value::I64(k.physical as i64)),
        ("repeat".into(), Value::Bool(k.repeat)),
    ];
    modifiers_record(k.modifiers, &mut f);
    Value::Record(f.into())
}

fn wheel_record(w: &WheelInput) -> Value {
    let (d, mode) = match w.delta {
        ScrollDelta::Pixels(v) => (v, 0),
        ScrollDelta::Lines(v) => (v, 1),
        ScrollDelta::Pages(v) => (v, 2),
    };
    let mut f: Vec<(Arc<str>, Value)> = vec![
        ("clientX".into(), Value::F64(w.pos.x as f64)),
        ("clientY".into(), Value::F64(w.pos.y as f64)),
        ("deltaX".into(), Value::F64(-d.x as f64)),
        ("deltaY".into(), Value::F64(-d.y as f64)),
        ("deltaMode".into(), Value::I64(mode)),
    ];
    modifiers_record(w.modifiers, &mut f);
    Value::Record(f.into())
}

impl Document {
    /// Handles one platform event at event time: native defaults first, then the JS-facing events
    /// (in [`Dispatched::events`]) for svelters-skelter to deliver. Focus events queued by programmatic
    /// focus moves since the last dispatch come first.
    pub fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        let before = self.revision;
        self.consumed_input(ev.seq);
        self.input.now = ev.os_time;
        let mut out = Dispatched { seq: ev.seq, window: ev.window, revision: self.revision, ..Default::default() };
        let mut emitted = Vec::new();
        self.transaction(|doc| match &ev.event {
            InputEvent::Pointer(p) => doc.dispatch_pointer(ev.window, ev.os_time, p, &mut out, &mut emitted),
            InputEvent::Wheel(w) => doc.dispatch_wheel(ev.window, w, &mut out, &mut emitted),
            InputEvent::Key(k) => doc.dispatch_key(ev.window, k, &mut out, &mut emitted),
            InputEvent::Ime(i) => doc.dispatch_ime(ev.window, i, &mut out, &mut emitted),
            InputEvent::ModifiersChanged(_)
            | InputEvent::Gesture(_)
            | InputEvent::FileHovered(_)
            | InputEvent::FileDropped(_)
            | InputEvent::FileHoverCancelled => {}
        });
        for e in emitted {
            queue_emitted(self, e);
        }
        let mut events = std::mem::take(&mut self.input_events);
        events.append(&mut out.events);
        out.events = events;
        // the transaction always bumps; a dispatch that changed nothing gives the revision back
        let dirty = self.journal_since(before);
        if !dirty {
            self.revision = before;
        }
        out.changed = dirty;
        out.revision = self.revision;
        out
    }

    /// The hit target at `p` in `window` (window points) on the last frame's hit list.
    pub fn hit_test(&self, window: WindowId, p: Point) -> Option<NodeId> {
        self.hit_test_ex(window, p, false)
    }

    /// `scroll`: skip targets that let scrolling through (`HitResult::HitExceptScroll`).
    pub(crate) fn hit_test_ex(&self, window: WindowId, p: Point, scroll: bool) -> Option<NodeId> {
        let w = self.window(window)?;
        for e in w.hits.entries.iter().rev() {
            if !e.world.contains(p) || e.clip.is_some_and(|c| !c.contains(p)) {
                continue;
            }
            let Some(i) = self.idx(e.node) else { continue };
            let local = e.inverse.apply(p);
            let Some(k) = self.kinds[i].as_deref() else { continue };
            let cx = crate::cx::HitCx { node: self.view_at(i) };
            match k.hit_test(&cx, local) {
                HitResult::Miss => continue,
                HitResult::HitExceptScroll if scroll => continue,
                HitResult::Hit | HitResult::HitExceptScroll => return Some(e.node),
            }
        }
        None
    }

    /// `p` in `node`'s local space, from the last frame's hit list.
    pub(crate) fn to_local(&self, window: WindowId, node: NodeId, p: Point) -> Point {
        self.window(window).and_then(|w| w.hits.entries.iter().rev().find(|e| e.node == node)).map_or(p, |e| e.inverse.apply(p))
    }

    fn pointer_input(&self, window: WindowId, node: NodeId, p: &PointerEvent, count: u32) -> PointerInput {
        PointerInput {
            pointer: p.pointer,
            kind: p.kind,
            button: p.button,
            buttons: p.buttons,
            pos: p.pos,
            local: self.to_local(window, node, p.pos),
            modifiers: p.modifiers,
            pressure: p.pressure,
            count,
        }
    }

    pub(crate) fn emit_js(&mut self, out: &mut Dispatched, target: NodeId, signal: SignalId, arg: Option<Value>) {
        if let Some(e) = js_event(self, target, signal, arg) {
            out.events.push(e);
        }
    }

    /// Delivers a cancelable DOM event before the native default when the document has a sync
    /// deliverer; otherwise keeps it for [`Document::post_js`] after the default.
    fn pre_js(&mut self, target: NodeId, signal: SignalId, arg: Option<Value>) -> Pre {
        match deliver_now(self, target, signal, arg) {
            Ok(o) => Pre::Delivered(o),
            Err(arg) => Pre::Later(arg),
        }
    }

    fn post_js(&mut self, out: &mut Dispatched, target: NodeId, signal: SignalId, pre: Pre) {
        if let Pre::Later(arg) = pre {
            self.emit_js(out, target, signal, arg);
        }
    }

    /// Hover follows the newest sample: leave the old path's nodes not on the new path, enter the new.
    pub(crate) fn update_hover(&mut self, window: WindowId, p: &PointerEvent, target: Option<NodeId>, out: &mut Dispatched, emitted: &mut Vec<Emitted>) {
        let mut path = Vec::new();
        let mut a = target;
        while let Some(n) = a {
            path.push(n);
            a = self.parent(n);
        }
        let old = self.input.hover.insert(p.pointer, path.clone()).unwrap_or_default();
        for &n in old.iter().filter(|n| !path.contains(n)) {
            if !self.contains(n) {
                continue;
            }
            self.set_state(n, StateMask::HOVERED, false);
            call_kind(self, n, &Event::HoverLeave, emitted, Some(p.pointer));
            let input = self.pointer_input(window, n, p, 0);
            self.emit_js(out, n, common::POINTERLEAVE, Some(pointer_record(&input)));
        }
        for &n in path.iter().rev().filter(|n| !old.contains(n)) {
            self.set_state(n, StateMask::HOVERED, true);
            call_kind(self, n, &Event::HoverEnter, emitted, Some(p.pointer));
            let input = self.pointer_input(window, n, p, 0);
            self.emit_js(out, n, common::POINTERENTER, Some(pointer_record(&input)));
        }
        self.input.last_pos.insert(window, p.pos);
    }

    fn dispatch_pointer(&mut self, window: WindowId, time: Time, p: &PointerEvent, out: &mut Dispatched, emitted: &mut Vec<Emitted>) {
        // an in-app drag takes the pointer until it ends
        if crate::input::dnd::pointer(self, window, p, out) {
            return;
        }
        let captured = self.input.capture.get(&p.pointer).copied().filter(|&n| self.contains(n));
        let hit = self.hit_test(window, p.pos);
        let target = captured.or(hit);
        out.target = target;
        match p.phase {
            PointerPhase::Move | PointerPhase::Enter => {
                // while captured, hover stays on the captured node's path
                self.update_hover(window, p, if captured.is_some() { captured } else { hit }, out, emitted);
                if let Some(t) = target {
                    let input = self.pointer_input(window, t, p, 0);
                    out.handled |= bubble_native(self, t, &Event::PointerMove(&input), emitted, Some(p.pointer)).is_some();
                    self.emit_js(out, t, common::POINTERMOVE, Some(pointer_record(&input)));
                }
            }
            PointerPhase::Leave => {
                self.update_hover(window, p, None, out, emitted);
            }
            PointerPhase::Down => {
                self.update_hover(window, p, hit, out, emitted);
                // open surfaces a press outside them dismisses hear of it first, topmost first
                self.outside_press(window, p, target, emitted);
                let Some(t) = target else {
                    crate::input::focus::clear_focus(self, window);
                    return;
                };
                let count = self.click_count(t, time, p.pos);
                let input = self.pointer_input(window, t, p, count);
                // EV1: a prevented pointerdown keeps the press from the tree (no press, focus or click)
                let pre = self.pre_js(t, common::POINTERDOWN, Some(pointer_record(&input)));
                // listeners may have removed the target
                if pre.prevented() || !self.contains(t) {
                    return;
                }
                crate::input::dnd::pressed(self, window, t, p);
                self.input.press.insert(p.pointer, t);
                self.set_state(t, StateMask::PRESSED, true);
                // focus the nearest focusable ancestor-or-self; a press elsewhere blurs
                let mut a = Some(t);
                let mut focus = None;
                while let Some(n) = a {
                    if crate::input::focus::can_focus(self, n) {
                        focus = Some(n);
                        break;
                    }
                    a = self.parent(n);
                }
                match focus {
                    Some(f) => crate::input::focus::set_focus(self, Some(f), FocusReason::Pointer),
                    None => crate::input::focus::clear_focus(self, window),
                }
                out.handled |= bubble_native(self, t, &Event::PointerDown(&input), emitted, Some(p.pointer)).is_some();
                self.post_js(out, t, common::POINTERDOWN, pre);
            }
            PointerPhase::Up => {
                let pressed = self.input.press.remove(&p.pointer).filter(|&n| self.contains(n));
                if let Some(n) = pressed {
                    self.set_state(n, StateMask::PRESSED, false);
                }
                crate::input::dnd::released(self, p.pointer);
                let mut prevented = false;
                if let Some(t) = target {
                    let input = self.pointer_input(window, t, p, 0);
                    // EV1: a prevented pointerup cancels the click
                    let pre = self.pre_js(t, common::POINTERUP, Some(pointer_record(&input)));
                    prevented = pre.prevented();
                    if !prevented {
                        out.handled |= bubble_native(self, t, &Event::PointerUp(&input), emitted, Some(p.pointer)).is_some();
                    }
                    self.post_js(out, t, common::POINTERUP, pre);
                }
                self.input.release(p.pointer);
                // a click is a release over the pressed node (or inside it): egui's semantics
                if let (Some(n), Some(h)) = (pressed, hit)
                    && self.is_ancestor_or_self(n, h)
                    && !prevented
                {
                    let count = self.input.last_click.filter(|c| c.0 == n).map_or(1, |c| c.3);
                    let input = self.pointer_input(window, n, p, count);
                    if p.button == Some(MouseButton::Secondary) {
                        self.emit_js(out, n, common::CONTEXTMENU, Some(pointer_record(&input)));
                    } else if matches!(p.button, Some(MouseButton::Primary) | None) {
                        let pre = self.pre_js(n, common::CLICK, Some(pointer_record(&input)));
                        if !pre.prevented() {
                            out.handled |= bubble_native(self, n, &Event::Click(&input), emitted, Some(p.pointer)).is_some();
                        }
                        self.post_js(out, n, common::CLICK, pre);
                        if count == 2 {
                            let pre = self.pre_js(n, common::DBLCLICK, Some(pointer_record(&input)));
                            self.post_js(out, n, common::DBLCLICK, pre);
                        }
                    }
                }
                // hover may have changed while the pointer was captured
                self.update_hover(window, p, hit, out, emitted);
            }
            PointerPhase::Cancel => {
                crate::input::dnd::released(self, p.pointer);
                let pressed = self.input.press.remove(&p.pointer);
                self.input.release(p.pointer);
                if let Some(n) = pressed.filter(|&n| self.contains(n)) {
                    self.set_state(n, StateMask::PRESSED, false);
                    call_kind(self, n, &Event::PointerCancel, emitted, Some(p.pointer));
                    let input = self.pointer_input(window, n, p, 0);
                    self.emit_js(out, n, common::POINTERCANCEL, Some(pointer_record(&input)));
                }
            }
        }
    }

    /// Calls [`Kind::outside_press`](crate::Kind::outside_press) on the light-dismiss nodes of
    /// `window` whose subtree does not hold `target`, topmost (latest in paint order) first.
    fn outside_press(&mut self, window: WindowId, p: &PointerEvent, target: Option<NodeId>, emitted: &mut Vec<Emitted>) {
        if self.light_dismiss.is_empty() {
            return;
        }
        let hits = self.window(window).map(|w| w.hits.clone());
        let order = |n: NodeId| hits.as_ref().and_then(|h| h.entries.iter().rposition(|e| e.node == n));
        let mut open: Vec<NodeId> = self.light_dismiss.iter().copied().filter(|&n| self.window_of(n) == Some(window) && target.is_none_or(|t| !self.is_ancestor_or_self(n, t))).collect();
        open.sort_by_key(|&n| std::cmp::Reverse(order(n)));
        for n in open {
            let Some(i) = self.idx(n) else { continue };
            let Some(mut k) = self.kinds[i].take() else { continue };
            let input = self.pointer_input(window, n, p, 1);
            {
                let mut cx = EventCx { doc: self, node: n, emitted, pointer: Some(p.pointer) };
                k.outside_press(&mut cx, &input);
            }
            self.restore_kind(n, k);
        }
    }

    fn click_count(&mut self, node: NodeId, time: Time, pos: Point) -> u32 {
        let count = match self.input.last_click {
            Some((n, t, at, c)) if n == node && time - t <= DOUBLE_CLICK && (pos - at).length() <= DOUBLE_CLICK_SLOP => c + 1,
            _ => 1,
        };
        self.input.last_click = Some((node, time, pos, count));
        count
    }

    pub(crate) fn is_ancestor_or_self(&self, a: NodeId, mut n: NodeId) -> bool {
        loop {
            if n == a {
                return true;
            }
            match self.parent(n) {
                Some(p) => n = p,
                None => return false,
            }
        }
    }

    fn dispatch_wheel(&mut self, window: WindowId, w: &WheelEvent, out: &mut Dispatched, emitted: &mut Vec<Emitted>) {
        let Some(t) = self.hit_test_ex(window, w.pos, true) else { return };
        out.target = Some(t);
        let input = WheelInput { pos: w.pos, local: self.to_local(window, t, w.pos), delta: w.delta, modifiers: w.modifiers };
        // EV1: a prevented wheel never scrolls
        let pre = self.pre_js(t, common::WHEEL, Some(wheel_record(&input)));
        if pre.prevented() {
            return;
        }
        let native = bubble_native(self, t, &Event::Wheel(&input), emitted, None);
        out.handled |= native.is_some();
        self.post_js(out, t, common::WHEEL, pre);
        if native.is_none() {
            // the default: scroll the nearest scroll container that can move in this direction
            out.handled |= self.scroll_by_wheel(t, &input, out);
        }
    }

    /// Scrolls the innermost scroll container around `t` that can move by the wheel delta.
    fn scroll_by_wheel(&mut self, t: NodeId, w: &WheelInput, out: &mut Dispatched) -> bool {
        let mut a = Some(t);
        while let Some(n) = a {
            if let Some(i) = self.idx(n)
                && let Some(viewport) = self.scroll_viewport(i)
            {
                let mut d = match w.delta {
                    ScrollDelta::Pixels(v) => v,
                    ScrollDelta::Lines(v) => v * LINE,
                    ScrollDelta::Pages(v) => Vec2::new(v.x * viewport.w * PAGE, v.y * viewport.h * PAGE),
                };
                // Shift turns a vertical wheel horizontal (Windows and Linux convention)
                if w.modifiers.contains(Modifiers::SHIFT) && d.x == 0.0 {
                    d = Vec2::new(d.y, 0.0);
                }
                // a positive delta moves content up: the offset grows
                let before = self.scroll_offset(n);
                let after = self.clamp_scroll(i, before - d);
                if after != before {
                    self.set_scroll_offset(n, after);
                    let rec: Vec<(Arc<str>, Value)> = vec![("scrollLeft".into(), Value::F64(after.x as f64)), ("scrollTop".into(), Value::F64(after.y as f64))];
                    self.emit_js(out, n, common::SCROLL, Some(Value::Record(rec.into())));
                    return true;
                }
            }
            a = self.parent(n);
        }
        false
    }

    fn dispatch_key(&mut self, window: WindowId, k: &KeyEvent, out: &mut Dispatched, emitted: &mut Vec<Emitted>) {
        // IME first (§11.2), before any keymap: during a composition every key belongs to the input
        // method; with an IME input source active, so do printable keys without a chord (so that
        // `jj`-style chords cannot break Japanese input)
        let m = k.modifiers;
        let chord = m.contains(Modifiers::CTRL) || m.contains(Modifiers::SUPER) || m.contains(Modifiers::ALT);
        let printable = k.text.as_deref().is_some_and(|t| !t.is_empty() && !t.chars().any(char::is_control));
        if self.input.composing || (self.input.ime_source.contains(&window) && !chord && printable) {
            out.to_ime = true;
            return;
        }
        let Some(target) = self.focused(window).or_else(|| self.window_root(window)) else { return };
        out.target = Some(target);
        // outside a composition the command layer's keymap comes first, then the focused kind, then JS
        if let Some(mut hook) = self.key_hook.take() {
            let taken = hook(window, k);
            self.key_hook = Some(hook);
            if taken {
                out.handled = true;
                return;
            }
        }
        // Escape ends an in-app drag
        if k.state == KeyState::Pressed && k.key == Some(Key::Escape) && crate::input::dnd::cancel(self, window, out) {
            out.handled = true;
            return;
        }
        let signal = if k.state == KeyState::Pressed { common::KEYDOWN } else { common::KEYUP };
        // EV1: a prevented keydown never reaches the focused kind (nor Tab or scope activation)
        let pre = self.pre_js(target, signal, Some(key_record(k)));
        if pre.prevented() {
            out.handled = true;
            return;
        }
        let native = bubble_native(self, target, &Event::Key(k), emitted, None);
        out.handled |= native.is_some();
        self.post_js(out, target, signal, pre);
        let plain = !k.modifiers.contains(Modifiers::CTRL) && !k.modifiers.contains(Modifiers::ALT) && !k.modifiers.contains(Modifiers::SUPER);
        if native.is_none() && k.state == KeyState::Pressed && k.key == Some(Key::Tab) && plain {
            let back = k.modifiers.contains(Modifiers::SHIFT);
            if let Some(n) = crate::input::focus::next_focus(self, window, back) {
                crate::input::focus::set_focus(self, Some(n), FocusReason::Keyboard);
                out.handled = true;
            }
        }
        // Enter and Escape that the focused kind left go to its scope's default and cancel buttons
        if native.is_none() && k.state == KeyState::Pressed && plain && !k.modifiers.contains(Modifiers::SHIFT) && matches!(k.key, Some(Key::Enter | Key::Escape)) {
            out.handled |= crate::input::scope::activate(self, window, target, k.key == Some(Key::Enter), emitted);
        }
    }

    fn dispatch_ime(&mut self, window: WindowId, ev: &ImeEvent, out: &mut Dispatched, emitted: &mut Vec<Emitted>) {
        self.input.composing = match ev {
            ImeEvent::Preedit { text, .. } => !text.is_empty(),
            ImeEvent::Commit(_) | ImeEvent::Disabled => false,
            ImeEvent::Enabled | ImeEvent::DeleteSurrounding { .. } => self.input.composing,
        };
        let Some(target) = self.focused(window) else { return };
        out.target = Some(target);
        out.handled |= call_kind(self, target, &Event::Ime(ev), emitted, None) == Handled::Yes;
    }

    /// Calls `f` with the text-input client of `window`'s focused node, when its kind edits text: the
    /// platform's IME bridge (IMM32/TSF, NSTextInputClient, text-input-v3) talks to it (§11.3).
    pub fn with_text_input<R>(&mut self, window: WindowId, f: impl FnOnce(&mut dyn skelter_base::input::TextInputClient) -> R) -> Option<R> {
        let node = self.focused(window)?;
        let i = self.idx(node)?;
        let mut k = self.kinds[i].take()?;
        let r = k.text_input().map(f);
        self.kinds[i] = Some(k);
        if r.is_some() {
            self.mark(node, crate::tree::Dirty::PAINT | crate::tree::Dirty::A11Y);
        }
        r
    }

    /// The status bar text of `window` (`status-tip`): the innermost hovered node with one, else,
    /// with nothing hovered that has one, the focused node or its nearest ancestor with one.
    pub fn status_tip(&self, window: WindowId) -> Option<&str> {
        let tip = |n: NodeId| self.view(n).and_then(|v| v.prop_str(common::STATUS_TIP)).filter(|t| !t.is_empty());
        let mut hovered = self.input.hover.values().filter(|path| path.first().is_some_and(|&n| self.window_of(n) == Some(window)));
        if let Some(t) = hovered.find_map(|path| path.iter().find_map(|&n| tip(n))) {
            return Some(t);
        }
        let mut a = self.focused(window);
        while let Some(n) = a {
            if let Some(t) = tip(n) {
                return Some(t);
            }
            a = self.parent(n);
        }
        None
    }

    /// The platform reports whether an input method source (a CJK IME, not a plain keyboard layout)
    /// is active for `window`: while it is, printable keys without a chord go to the IME first.
    pub fn set_ime_source(&mut self, window: WindowId, active: bool) {
        if active {
            self.input.ime_source.insert(window);
        } else {
            self.input.ime_source.remove(&window);
        }
    }

    /// The command layer's keymap (cadatomic's one command path): called for every key outside a
    /// composition before the focused kind; returning true consumes the key.
    pub fn set_key_hook(&mut self, hook: Box<dyn FnMut(WindowId, &KeyEvent) -> bool>) {
        self.key_hook = Some(hook);
    }

    /// Whether anything changed since `rev` (a dispatch that hit no handler keeps its revision).
    fn journal_since(&self, rev: skelter_base::ids::Revision) -> bool {
        self.revision > rev && self.last_change >= self.revision
    }
}
