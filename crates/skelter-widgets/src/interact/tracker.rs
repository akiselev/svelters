//! The interaction substrate: what egui's `Ui::interact` and `InputState` computed for a widget, as a
//! per-host state machine over the skelter events the host receives (skelter-core has hit-tested,
//! captured and hovered them).
//!
//! egui 0.37's rules, kept (`egui/src/input_state/mod.rs`, `interaction.rs`, `context.rs`):
//! - a press is a click candidate until the pointer moves more than `max_click_dist` from the press
//!   origin or it is held longer than `max_click_duration` (0.8 s);
//! - a release of a click candidate clicks the host, wherever the pointer is (the candidate was
//!   cleared if it moved too far); its count is 2 (3) when the previous click (two) came within
//!   `max_double_click_delay` and `max_click_dist`, and a double click also reports a click;
//! - a host that senses only drags is dragged from the press; one that senses clicks and drags is
//!   dragged once the press can no longer be a click, or the pointer left it;
//! - Escape ends a drag (`drag_stopped`);
//! - Space or Enter on the focused host clicks it, as does AccessKit `Click`;
//! - while a press is held elsewhere, the host is not hovered.
//!
//! The distances and the double-click delay come from the platform policy (`drag_threshold`,
//! `double_click_ms`); egui's own row is 6 points and 300 ms.

use super::policy::Policy;
use super::{Resp, StateBits};
use crate::shim::core::{Event, EventCx};
use skelter_base::accesskit;
use skelter_base::event::{KeyState, Modifiers, MouseButton, PointerPhase, ScrollDelta};
use skelter_base::geom::{Point, Rect, Size, Vec2};
use skelter_base::key::Key;
use skelter_base::time::{Span, Time};
use smallvec::SmallVec;

/// What a host's units sense (merged over its units): egui's `Sense`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sense {
    pub click: bool,
    pub drag: bool,
    /// It takes keyboard focus (in the Tab order).
    pub focusable: bool,
}

impl Sense {
    pub const HOVER: Sense = Sense { click: false, drag: false, focusable: false };
    pub const CLICK: Sense = Sense { click: true, drag: false, focusable: true };
    pub const DRAG: Sense = Sense { click: false, drag: true, focusable: true };
    pub const CLICK_AND_DRAG: Sense = Sense { click: true, drag: true, focusable: true };
    pub const FOCUSABLE: Sense = Sense { click: false, drag: false, focusable: true };

    pub const fn union(self, o: Sense) -> Sense {
        Sense { click: self.click || o.click, drag: self.drag || o.drag, focusable: self.focusable || o.focusable }
    }
}

/// The click and drag decision constants (egui's `InputOptions`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickOptions {
    /// Points the pointer may move from the press origin and still click.
    pub max_click_dist: f32,
    /// Seconds a press may last and still click.
    pub max_click_duration: f64,
    /// Seconds between the clicks of a double click.
    pub max_double_click_delay: f64,
}

impl ClickOptions {
    /// egui's defaults: 6 points, 0.8 s, 0.3 s.
    pub const EGUI: ClickOptions = ClickOptions { max_click_dist: 6.0, max_click_duration: 0.8, max_double_click_delay: 0.3 };

    /// The policy's thresholds with egui's press duration.
    pub fn of(p: &Policy) -> ClickOptions {
        ClickOptions { max_click_dist: p.drag_threshold, max_click_duration: 0.8, max_double_click_delay: p.double_click_ms as f64 / 1000.0 }
    }
}

impl Default for ClickOptions {
    fn default() -> Self {
        ClickOptions::EGUI
    }
}

/// Modifiers as units test them (egui's `Modifiers`): `command` is the platform's shortcut modifier
/// (Cmd on macOS, Ctrl elsewhere).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub mac_cmd: bool,
    pub command: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { alt: false, ctrl: false, shift: false, mac_cmd: false, command: false };

    pub fn of(m: Modifiers, macos: bool) -> Mods {
        let ctrl = m.contains(Modifiers::CTRL);
        let mac_cmd = macos && m.contains(Modifiers::SUPER);
        Mods { alt: m.contains(Modifiers::ALT), ctrl, shift: m.contains(Modifiers::SHIFT), mac_cmd, command: if macos { mac_cmd } else { ctrl } }
    }
    pub fn any(&self) -> bool {
        self.alt || self.ctrl || self.shift || self.mac_cmd || self.command
    }
    pub fn is_none(&self) -> bool {
        !self.any()
    }
    pub fn shift_only(&self) -> bool {
        self.shift && !(self.alt || self.command || self.ctrl || self.mac_cmd)
    }
}

/// One input item of a pass, as units iterate them (egui's `Event::Key`/`Text`/`MouseWheel`).
#[derive(Clone, Debug, PartialEq)]
pub enum FrameEvent {
    Key { key: Key, pressed: bool, repeat: bool, modifiers: Mods },
    Text(String),
    /// A wheel or touchpad step in points (skelter's sign: positive y scrolls the content up);
    /// `touchpad` when it came with a gesture phase.
    Wheel { delta: Vec2, touchpad: bool, ended: bool },
}

/// The pointer as units read it this pass (egui's `PointerState`), host-local.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerFrame {
    latest: Option<Point>,
    interact: Option<Point>,
    origin: Option<Point>,
    primary_down: bool,
    any_down: bool,
    primary_pressed: bool,
    any_pressed: bool,
    any_released: bool,
    delta: Vec2,
}

impl PointerFrame {
    pub fn latest_pos(&self) -> Option<Point> {
        self.latest
    }
    /// Where the pointer was when it last interacted (pressed, moved while held, released).
    pub fn interact_pos(&self) -> Option<Point> {
        self.interact
    }
    /// Where the held press began.
    pub fn press_origin(&self) -> Option<Point> {
        self.origin
    }
    pub fn primary_down(&self) -> bool {
        self.primary_down
    }
    pub fn any_down(&self) -> bool {
        self.any_down
    }
    pub fn primary_pressed(&self) -> bool {
        self.primary_pressed
    }
    pub fn any_pressed(&self) -> bool {
        self.any_pressed
    }
    pub fn any_released(&self) -> bool {
        self.any_released
    }
    pub fn delta(&self) -> Vec2 {
        self.delta
    }
    pub fn is_moving(&self) -> bool {
        self.delta != Vec2::ZERO
    }
}

/// This pass's input (egui's `InputState`, for one host).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputFrame {
    /// Seconds on skelter's clock.
    pub time: f64,
    pub modifiers: Mods,
    pub events: SmallVec<[FrameEvent; 2]>,
    pub pointer: PointerFrame,
    /// The wheel delta in points this pass (sum of the `Wheel` events).
    pub smooth_scroll_delta: Vec2,
    /// The wheel step in notches when the device reported lines or pages (egui's
    /// `MouseWheelUnit::Line`/`Page`: one per detent); `None` for precise point deltas (touchpads).
    pub wheel_notches: Option<Vec2>,
    /// A press outside the host and its subtree this pass ([`Event::OutsidePress`], host-local).
    pub outside_press: Option<Point>,
}

impl InputFrame {
    /// A press (or repeat) of `key` this pass, with any modifiers (egui's `key_pressed`).
    pub fn key_pressed(&self, key: Key) -> bool {
        self.events.iter().any(|e| matches!(e, FrameEvent::Key { key: k, pressed: true, .. } if *k == key))
    }
    pub fn any_key_pressed(&self) -> bool {
        self.events.iter().any(|e| matches!(e, FrameEvent::Key { pressed: true, .. }))
    }
    /// The text typed this pass.
    pub fn text(&self) -> impl Iterator<Item = &str> {
        self.events.iter().filter_map(|e| match e {
            FrameEvent::Text(t) => Some(t.as_str()),
            _ => None,
        })
    }
}

/// One pass of a host: its [`Resp`], its input, and the AccessKit actions addressed to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pass {
    pub resp: Resp,
    pub input: InputFrame,
    pub a11y: SmallVec<[accesskit::ActionRequest; 1]>,
}

fn button_index(b: MouseButton) -> Option<usize> {
    match b {
        MouseButton::Primary => Some(0),
        MouseButton::Secondary => Some(1),
        MouseButton::Middle => Some(2),
        MouseButton::Back => Some(3),
        MouseButton::Forward => Some(4),
        MouseButton::Other(_) => None,
    }
}

/// The per-host interaction state machine (see the module docs). Keep one per node (or node part)
/// for as long as the node lives.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    pub sense: Sense,
    pub options: ClickOptions,
    /// skelter-core says the pointer is over the host and nothing covers it.
    hover: bool,
    latest: Option<Point>,
    /// Buttons held, by [`button_index`] (as the host last saw them).
    down: [bool; 5],
    /// Buttons held anywhere, from the last pointer event's `buttons` mask.
    buttons: u32,
    origin: Option<Point>,
    press_start: f64,
    moved_too_much: bool,
    press_count: u32,
    potential_click: bool,
    potential_drag: bool,
    dragged: bool,
    last_click: f64,
    last_last_click: f64,
    last_click_pos: Option<Point>,
    focused: bool,
}

impl Tracker {
    pub fn new(sense: Sense) -> Tracker {
        Tracker { sense, last_click: f64::NEG_INFINITY, last_last_click: f64::NEG_INFINITY, ..Tracker::default() }
    }

    /// With the policy's click thresholds.
    pub fn with_policy(mut self, p: &Policy) -> Tracker {
        self.options = ClickOptions::of(p);
        self
    }

    pub fn is_dragged(&self) -> bool {
        self.dragged
    }
    pub fn is_down_on(&self) -> bool {
        self.potential_click || self.potential_drag
    }
    /// The click count a click ending the held press would have (2 for the second press of a double
    /// click), known from the press on (double-click-and-drag selection).
    pub fn press_click_count(&self) -> u32 {
        self.press_count
    }

    fn any_down(&self) -> bool {
        self.down.iter().any(|d| *d)
    }

    fn could_be_click(&self, now: f64) -> bool {
        !self.moved_too_much && now - self.press_start <= self.options.max_click_duration
    }

    fn click_count(&self, now: f64, pos: Point) -> u32 {
        let d = self.last_click_pos.map_or(0.0, |p| (p - pos).length());
        let double = now - self.last_click < self.options.max_double_click_delay && d < self.options.max_click_dist;
        let triple = double && self.last_click - self.last_last_click < self.options.max_double_click_delay;
        if triple {
            3
        } else if double {
            2
        } else {
            1
        }
    }

    /// When the host needs an [`Event::Timer`] for the press to turn into a drag (it senses clicks and
    /// drags, a press is held, and it can still click).
    pub fn next_wake(&self) -> Option<Time> {
        (self.sense.click && self.sense.drag && self.potential_drag && !self.dragged && !self.moved_too_much)
            .then(|| Time(0) + Span::from_secs_f64(self.press_start + self.options.max_click_duration + 0.001))
    }

    /// Feeds one event; returns the pass the host's units run on. `cx` gives the time, the host size,
    /// enabled, focus and the policy's platform.
    pub fn pass(&mut self, cx: &EventCx<'_>, ev: &Event) -> Pass {
        self.pass_at(cx.now, cx.size, cx.enabled, cx.focused, cx.policy, ev)
    }

    /// [`Tracker::pass`] without an [`EventCx`].
    pub fn pass_at(&mut self, now: Time, size: Size, enabled: bool, focused: bool, policy: &Policy, ev: &Event) -> Pass {
        let t = now.as_secs_f64();
        let macos = policy.platform == Some(super::Platform::Macos);
        let rect = Rect::from_origin_size(Point::ZERO, size);
        let mut pass = Pass::default();
        pass.input.time = t;
        self.focused = focused;
        let mut r = Resp { rect, enabled, has_focus: focused, ..Resp::default() };
        let mut p = PointerFrame { latest: self.latest, interact: self.latest, origin: self.origin, ..PointerFrame::default() };
        let was_dragged = self.dragged;
        let mut clicked: [bool; 5] = [false; 5];
        let mut count = 0;
        let mut released = false;
        match ev {
            Event::Pointer(pe) => {
                pass.input.modifiers = Mods::of(pe.modifiers, macos);
                self.buttons = pe.buttons;
                let old = self.latest;
                match pe.phase {
                    PointerPhase::Down => {
                        self.latest = Some(pe.pos);
                        if let Some(i) = pe.button.and_then(button_index) {
                            self.down[i] = true;
                            p.any_pressed = true;
                            p.primary_pressed = i == 0;
                        }
                        self.origin = Some(pe.pos);
                        self.press_start = t;
                        self.moved_too_much = false;
                        self.press_count = self.click_count(t, pe.pos);
                        if enabled {
                            self.potential_click |= self.sense.click;
                            self.potential_drag |= self.sense.drag;
                            if self.sense.drag && !self.sense.click {
                                self.dragged = true;
                            }
                        }
                    }
                    PointerPhase::Move => {
                        self.latest = Some(pe.pos);
                        if let Some(o) = self.origin {
                            self.moved_too_much |= (pe.pos - o).length() > self.options.max_click_dist;
                        }
                    }
                    PointerPhase::Up => {
                        self.latest = Some(pe.pos);
                        p.any_released = true;
                        released = true;
                        let i = pe.button.and_then(button_index);
                        let could_click = self.could_be_click(t);
                        if could_click && self.potential_click && !self.dragged {
                            count = self.click_count(t, pe.pos);
                            self.last_last_click = self.last_click;
                            self.last_click = t;
                            self.last_click_pos = Some(pe.pos);
                            if let Some(i) = i {
                                clicked[i] = enabled;
                            }
                        }
                        if let Some(i) = i {
                            self.down[i] = false;
                        }
                        self.potential_click = false;
                        self.potential_drag = false;
                        self.dragged = false;
                        self.origin = None;
                    }
                    PointerPhase::Enter => {
                        self.latest = Some(pe.pos);
                    }
                    PointerPhase::Leave => {
                        self.latest = None;
                    }
                    PointerPhase::Cancel => self.cancel_press(),
                }
                if let (Some(a), Some(b)) = (old, self.latest) {
                    p.delta = b - a;
                }
                if self.latest.is_some() {
                    p.interact = self.latest;
                }
            }
            Event::Hover(h) => self.hover = *h,
            Event::Wheel(w) => {
                pass.input.modifiers = Mods::of(w.modifiers, macos);
                let delta = match w.delta {
                    ScrollDelta::Pixels(v) => v,
                    ScrollDelta::Lines(v) => v * policy.scroll.line,
                    ScrollDelta::Pages(v) => Vec2::new(v.x * size.w, v.y * size.h),
                };
                pass.input.wheel_notches = match w.delta {
                    ScrollDelta::Lines(v) | ScrollDelta::Pages(v) => Some(v),
                    ScrollDelta::Pixels(_) => None,
                };
                let ended = matches!(w.phase, Some(skelter_base::event::GesturePhase::Ended | skelter_base::event::GesturePhase::Cancelled));
                pass.input.smooth_scroll_delta = delta;
                pass.input.events.push(FrameEvent::Wheel { delta, touchpad: w.phase.is_some(), ended });
            }
            Event::Key(k) => {
                let mods = Mods::of(k.modifiers, macos);
                pass.input.modifiers = mods;
                let pressed = k.state == KeyState::Pressed;
                if let Some(key) = k.key {
                    pass.input.events.push(FrameEvent::Key { key, pressed, repeat: k.repeat, modifiers: mods });
                    if pressed && key == Key::Escape && self.dragged {
                        // egui: Escape aborts a drag
                        self.dragged = false;
                        self.potential_drag = false;
                    }
                    // egui: Space and Enter work like a primary click on the focused host
                    if pressed && enabled && focused && self.sense.click && matches!(key, Key::Space | Key::Enter) {
                        clicked[0] = true;
                        count = 1;
                    }
                }
                if pressed
                    && let Some(text) = &k.text
                    && !text.is_empty()
                    && !text.chars().all(char::is_control)
                    && !mods.command
                    && !mods.ctrl
                {
                    pass.input.events.push(FrameEvent::Text(text.clone()));
                }
            }
            Event::Focus { gained, .. } => {
                self.focused = *gained;
                r.has_focus = *gained;
            }
            Event::A11y(req) => {
                if enabled && self.sense.click && req.action == accesskit::Action::Click {
                    clicked[0] = true;
                    count = 1;
                }
                pass.a11y.push(req.clone());
            }
            // a virtual child's action is the kind's, not a click on the node
            Event::Timer | Event::A11yChild { .. } => {}
            Event::CaptureLost => self.cancel_press(),
            Event::OutsidePress(p) => pass.input.outside_press = Some(*p),
        }
        // the press can no longer click: no click candidate; a click-and-drag host starts dragging
        if self.origin.is_some() && !self.could_be_click(t) {
            self.potential_click = false;
        }
        if !self.dragged && self.potential_drag && enabled && self.origin.is_some() {
            let inside = self.latest.is_some_and(|q| rect.contains(q));
            let decided = !self.could_be_click(t);
            if decided || !inside || !self.sense.click {
                self.dragged = true;
            }
        }
        p.primary_down = self.down[0];
        p.any_down = self.any_down();
        p.origin = self.origin;
        p.latest = self.latest;
        let interacting = self.is_down_on() || self.dragged;
        r.down_on = interacting && enabled;
        r.dragged = self.dragged && enabled;
        r.drag_started = self.dragged && !was_dragged && enabled;
        r.drag_stopped = was_dragged && !self.dragged;
        if r.dragged || r.drag_stopped {
            r.drag_delta = p.delta;
        }
        r.clicked = clicked[0];
        r.secondary_clicked = clicked[1];
        r.middle_clicked = clicked[2];
        r.double_clicked = clicked[0] && count == 2;
        r.triple_clicked = clicked[0] && count == 3;
        let any_clicked = clicked.iter().any(|c| *c);
        r.contains_pointer = self.hover || self.latest.is_some_and(|q| rect.contains(q));
        // egui: while a press is held on another node, nothing else is hovered
        let held_elsewhere = self.buttons != 0 && !interacting && !released;
        r.hovered = enabled && (self.hover || interacting) && !held_elsewhere && (self.hover || r.contains_pointer);
        if interacting || any_clicked || r.drag_stopped {
            r.interact_pointer_pos = p.interact.or(self.latest);
        }
        pass.input.pointer = p;
        pass.resp = r;
        pass
    }

    fn cancel_press(&mut self) {
        self.down = [false; 5];
        self.buttons = 0;
        self.potential_click = false;
        self.potential_drag = false;
        self.dragged = false;
        self.origin = None;
    }

    /// The bits a [`Pass`] implies before units run: none (units own their bits). Kept for kinds that
    /// run no units but mirror hover (`box`).
    pub fn hover_bits(pass: &Pass) -> StateBits {
        if pass.resp.hovered || (!pass.resp.enabled && pass.resp.contains_pointer) { StateBits::HOVERED } else { StateBits::NONE }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::event::{PointerEvent, PointerType};

    fn ptr(phase: PointerPhase, x: f32, y: f32, buttons: u32) -> Event {
        let button = matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary);
        Event::Pointer(PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button, buttons, pos: Point::new(x, y), modifiers: Modifiers::default(), pressure: 0.0, tilt: Vec2::ZERO, history: SmallVec::new() })
    }

    fn at(ms: u64) -> Time {
        Time(ms * 1_000_000)
    }

    struct H {
        t: Tracker,
        p: Policy,
    }

    impl H {
        fn new(s: Sense) -> H {
            H { t: Tracker::new(s), p: Policy::default() }
        }
        fn ev(&mut self, ms: u64, e: Event) -> Resp {
            self.t.pass_at(at(ms), Size::new(100.0, 20.0), true, false, &self.p, &e).resp
        }
    }

    #[test]
    fn press_release_inside_clicks() {
        let mut h = H::new(Sense::CLICK);
        h.ev(0, Event::Hover(true));
        let d = h.ev(10, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(d.down_on && d.hovered && !d.clicked);
        let u = h.ev(50, ptr(PointerPhase::Up, 6.0, 5.0, 0));
        assert!(u.clicked && !u.double_clicked && !u.down_on);
    }

    #[test]
    fn moving_too_far_or_holding_too_long_is_no_click() {
        let mut h = H::new(Sense::CLICK);
        h.ev(0, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        h.ev(10, ptr(PointerPhase::Move, 20.0, 5.0, 1));
        assert!(!h.ev(20, ptr(PointerPhase::Up, 20.0, 5.0, 0)).clicked);
        h.ev(1000, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(!h.ev(1900, ptr(PointerPhase::Up, 5.0, 5.0, 0)).clicked, "held longer than 0.8 s");
    }

    #[test]
    fn release_outside_within_the_click_distance_still_clicks() {
        // egui clicks the press's widget wherever the release is, if it can still be a click
        let mut h = H::new(Sense::CLICK);
        h.ev(0, ptr(PointerPhase::Down, 98.0, 5.0, 1));
        assert!(h.ev(10, ptr(PointerPhase::Up, 102.0, 5.0, 0)).clicked);
    }

    #[test]
    fn double_and_triple_click() {
        let mut h = H::new(Sense::CLICK);
        h.ev(0, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(h.ev(20, ptr(PointerPhase::Up, 5.0, 5.0, 0)).clicked);
        h.ev(100, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert_eq!(h.t.press_click_count(), 2);
        let second = h.ev(120, ptr(PointerPhase::Up, 5.0, 5.0, 0));
        assert!(second.clicked && second.double_clicked, "a double click reports a click too");
        h.ev(200, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        let third = h.ev(220, ptr(PointerPhase::Up, 5.0, 5.0, 0));
        assert!(third.triple_clicked && !third.double_clicked);
        h.ev(2000, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(!h.ev(2020, ptr(PointerPhase::Up, 5.0, 5.0, 0)).double_clicked, "too late for a double click");
    }

    #[test]
    fn drag_only_host_drags_from_the_press() {
        let mut h = H::new(Sense::DRAG);
        let d = h.ev(0, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(d.drag_started && d.dragged);
        let m = h.ev(10, ptr(PointerPhase::Move, 9.0, 5.0, 1));
        assert!(m.dragged && !m.drag_started && m.drag_delta == Vec2::new(4.0, 0.0));
        let u = h.ev(20, ptr(PointerPhase::Up, 9.0, 5.0, 0));
        assert!(u.drag_stopped && !u.dragged && !u.clicked);
    }

    #[test]
    fn click_and_drag_host_waits_for_the_decision() {
        let mut h = H::new(Sense::CLICK_AND_DRAG);
        let d = h.ev(0, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(!d.dragged && d.down_on);
        assert!(!h.ev(10, ptr(PointerPhase::Move, 8.0, 5.0, 1)).dragged, "within the click distance");
        assert!(h.ev(20, ptr(PointerPhase::Move, 20.0, 5.0, 1)).drag_started);
        assert!(!h.ev(30, ptr(PointerPhase::Up, 20.0, 5.0, 0)).clicked);
        // held past the click duration: the timer starts the drag
        h.ev(1000, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(h.t.next_wake().is_some());
        assert!(h.ev(1801, Event::Timer).drag_started);
    }

    #[test]
    fn escape_ends_a_drag() {
        let mut h = H::new(Sense::DRAG);
        h.ev(0, ptr(PointerPhase::Down, 5.0, 5.0, 1));
        let k = Event::Key(skelter_base::event::KeyEvent { key: Some(Key::Escape), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: skelter_base::event::KeyLocation::Standard, modifiers: Modifiers::default() });
        let r = h.ev(10, k);
        assert!(r.drag_stopped && !r.dragged);
    }

    #[test]
    fn space_on_the_focused_host_clicks() {
        let mut t = Tracker::new(Sense::CLICK);
        let p = Policy::default();
        let k = Event::Key(skelter_base::event::KeyEvent { key: Some(Key::Space), physical: 0, text: Some(" ".into()), state: KeyState::Pressed, repeat: false, location: skelter_base::event::KeyLocation::Standard, modifiers: Modifiers::default() });
        assert!(t.pass_at(at(0), Size::new(10.0, 10.0), true, true, &p, &k).resp.clicked);
        assert!(!t.pass_at(at(0), Size::new(10.0, 10.0), true, false, &p, &k).resp.clicked, "only when focused");
    }

    #[test]
    fn accesskit_click_clicks() {
        let mut t = Tracker::new(Sense::CLICK);
        let req = accesskit::ActionRequest { action: accesskit::Action::Click, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(1), data: None };
        let pass = t.pass_at(at(0), Size::new(10.0, 10.0), true, false, &Policy::default(), &Event::A11y(req));
        assert!(pass.resp.clicked && pass.a11y.len() == 1);
    }

    #[test]
    fn not_hovered_while_a_press_is_held_elsewhere() {
        let mut h = H::new(Sense::CLICK);
        h.ev(0, Event::Hover(true));
        assert!(!h.ev(5, ptr(PointerPhase::Move, 5.0, 5.0, 1)).hovered);
        assert!(h.ev(10, ptr(PointerPhase::Move, 5.0, 5.0, 0)).hovered);
    }

    #[test]
    fn disabled_host_never_clicks() {
        let mut t = Tracker::new(Sense::CLICK);
        let p = Policy::default();
        let s = Size::new(10.0, 10.0);
        t.pass_at(at(0), s, false, false, &p, &ptr(PointerPhase::Down, 5.0, 5.0, 1));
        assert!(!t.pass_at(at(10), s, false, false, &p, &ptr(PointerPhase::Up, 5.0, 5.0, 0)).resp.clicked);
    }
}

/// A harness for unit tests: one host's [`Tracker`] and state bits on a virtual clock, feeding one
/// event per call (10 ms apart) and running the caller's units on the pass through [`super::run`],
/// so focus requests and the policy's Escape rule apply as on a node.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::interact::{FocusReason, InputCx};
    use skelter_base::event::{GesturePhase, KeyEvent, KeyLocation, PointerEvent, PointerType, WheelEvent};

    pub struct Rig {
        pub tracker: Tracker,
        pub state: StateBits,
        pub policy: Policy,
        pub size: Size,
        /// The virtual clock in milliseconds.
        pub now_ms: u64,
        pub focused: bool,
        pub enabled: bool,
        held: u32,
    }

    impl Rig {
        pub fn new(sense: Sense) -> Rig {
            Rig { tracker: Tracker::new(sense), state: StateBits::NONE, policy: Policy::default(), size: Size::new(100.0, 20.0), now_ms: 1000, focused: false, enabled: true, held: 0 }
        }

        pub fn advance(&mut self, ms: u64) {
            self.now_ms += ms;
        }

        /// Feeds `ev` 10 ms after the previous one and runs `f` on the pass; applies the focus the
        /// units asked for.
        pub fn send(&mut self, ev: Event, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.now_ms += 10;
            let now = Time(self.now_ms * 1_000_000);
            if let Event::Focus { gained, .. } = &ev {
                self.focused = *gained;
            }
            let pass = self.tracker.pass_at(now, self.size, self.enabled, self.focused, &self.policy, &ev);
            let policy = self.policy;
            let mut state = self.state;
            let mut cx = EventCx::new(now, self.size, &mut state, &policy);
            cx.enabled = self.enabled;
            cx.focused = self.focused;
            super::super::run(&mut cx, &pass, f);
            if let Some(on) = cx.focus_request {
                self.focused = on;
            }
            self.state = state;
            pass
        }

        fn pointer(&mut self, phase: PointerPhase, x: f32, y: f32, button: Option<MouseButton>, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            let buttons = self.held;
            self.send(Event::Pointer(PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button, buttons, pos: Point::new(x, y), modifiers: Modifiers::default(), pressure: 0.0, tilt: Vec2::ZERO, history: SmallVec::new() }), f)
        }

        pub fn hover(&mut self, on: bool, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.send(Event::Hover(on), f)
        }
        pub fn down(&mut self, x: f32, y: f32, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.held = 1;
            self.pointer(PointerPhase::Down, x, y, Some(MouseButton::Primary), f)
        }
        pub fn move_to(&mut self, x: f32, y: f32, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.pointer(PointerPhase::Move, x, y, None, f)
        }
        pub fn up(&mut self, x: f32, y: f32, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.held = 0;
            self.pointer(PointerPhase::Up, x, y, Some(MouseButton::Primary), f)
        }
        pub fn key(&mut self, key: Key, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.key_with(key, Modifiers::default(), None, f)
        }
        /// A key press with modifiers and the text it types.
        pub fn key_with(&mut self, key: Key, modifiers: Modifiers, text: Option<&str>, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.send(Event::Key(KeyEvent { key: Some(key), physical: 0, text: text.map(str::to_owned), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers }), f)
        }
        /// A key press that types `text` (no named key).
        pub fn text(&mut self, text: &str, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.send(Event::Key(KeyEvent { key: None, physical: 0, text: Some(text.to_owned()), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() }), f)
        }
        /// A wheel event; `touchpad` gives it a gesture phase.
        pub fn wheel(&mut self, delta: ScrollDelta, touchpad: bool, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            let phase = touchpad.then_some(GesturePhase::Moved);
            self.send(Event::Wheel(WheelEvent { pos: Point::new(5.0, 5.0), delta, phase, momentum: None, modifiers: Modifiers::default() }), f)
        }
        pub fn focus(&mut self, on: bool, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.send(Event::Focus { gained: on, reason: FocusReason::Keyboard }, f)
        }
        pub fn a11y(&mut self, action: accesskit::Action, data: Option<accesskit::ActionData>, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.send(Event::A11y(accesskit::ActionRequest { action, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(1), data }), f)
        }
        pub fn timer(&mut self, f: impl FnOnce(&mut InputCx<'_>)) -> Pass {
            self.send(Event::Timer, f)
        }
    }
}
