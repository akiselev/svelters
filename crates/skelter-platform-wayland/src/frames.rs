//! Input frame assembly: `wl_pointer`, `wl_touch` and `zwp_tablet_tool_v2` send their state piecewise and
//! close each logical event with a `frame`. These assemblers collect the pieces and turn each frame into
//! skelter [`InputEvent`]s. They hold no Wayland objects (surfaces are the caller's keys), so tests drive them
//! with plain values.
//!
//! Scroll rules (`wl_pointer` v9):
//! - a wheel frame with `axis_value120` (or v5–v7 `axis_discrete`) becomes `ScrollDelta::Lines`, one line per
//!   detent (120 units); a wheel without either becomes pixels;
//! - finger and continuous sources become `ScrollDelta::Pixels` with a gesture phase: the first frame of a
//!   sequence is `Started`, later ones `Moved`, and `axis_stop` on every scrolling axis ends it (`Ended`,
//!   possibly with a zero delta) — the start of a kinetic fling in the app;
//! - Wayland's axis values grow downwards and rightwards; skelter's positive y scrolls content up (wheel away
//!   from the user), so both axes are negated.

use skelter_base::event::{
    GestureEvent, GesturePhase, InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerSample, PointerType,
    ScrollDelta, WheelEvent,
};
use skelter_base::geom::{Point, Vec2};
use skelter_base::time::Time;
use smallvec::SmallVec;

/// The mouse's pointer id.
pub const MOUSE_POINTER: u32 = 0;
/// Touch points are `TOUCH_POINTER_BASE | wl_touch id`.
pub const TOUCH_POINTER_BASE: u32 = 0x1000_0000;
/// Tablet tools are `PEN_POINTER_BASE | tool index`.
pub const PEN_POINTER_BASE: u32 = 0x2000_0000;

/// Linux `BTN_*` codes (`linux/input-event-codes.h`).
pub mod btn {
    pub const LEFT: u32 = 0x110;
    pub const RIGHT: u32 = 0x111;
    pub const MIDDLE: u32 = 0x112;
    pub const SIDE: u32 = 0x113;
    pub const EXTRA: u32 = 0x114;
    pub const FORWARD: u32 = 0x115;
    pub const BACK: u32 = 0x116;
    pub const TASK: u32 = 0x117;
    /// Tablet tool buttons.
    pub const STYLUS: u32 = 0x14b;
    pub const STYLUS2: u32 = 0x14c;
    pub const STYLUS3: u32 = 0x149;
}

pub fn mouse_button(code: u32) -> MouseButton {
    match code {
        btn::LEFT => MouseButton::Primary,
        btn::RIGHT => MouseButton::Secondary,
        btn::MIDDLE => MouseButton::Middle,
        btn::SIDE | btn::BACK => MouseButton::Back,
        btn::EXTRA | btn::FORWARD => MouseButton::Forward,
        other => MouseButton::Other(other.saturating_sub(btn::LEFT) as u16),
    }
}

/// The `buttons` bit for a button (primary 1, secondary 2, middle 4, back 8, forward 16, as the DOM).
pub fn button_bit(b: MouseButton) -> u32 {
    match b {
        MouseButton::Primary => 1,
        MouseButton::Secondary => 2,
        MouseButton::Middle => 4,
        MouseButton::Back => 8,
        MouseButton::Forward => 16,
        MouseButton::Other(n) => 1u32.checked_shl(5 + n as u32).unwrap_or(0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisSource {
    Wheel,
    Finger,
    Continuous,
    WheelTilt,
}

/// One `wl_pointer` event, with surface coordinates in points (`wl_fixed` already converted) and the
/// surface as the caller's key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RawPointer<S> {
    Enter { surface: S, pos: Point },
    Leave { surface: S },
    Motion { time: Time, pos: Point },
    Button { time: Time, button: u32, pressed: bool },
    Axis { time: Time, axis: Axis, value: f64 },
    AxisSource(AxisSource),
    AxisStop { time: Time, axis: Axis },
    /// v5–v7 detent count.
    AxisDiscrete { axis: Axis, discrete: i32 },
    /// v8+: 120 per detent, fractions for high-resolution wheels.
    AxisValue120 { axis: Axis, value120: i32 },
    Frame,
}

/// An assembled event for the surface it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Assembled<S> {
    pub surface: S,
    pub time: Time,
    pub event: InputEvent,
}

#[derive(Default, Clone, Copy)]
struct AxisAcc {
    value: f64,
    v120: i32,
    has_v120: bool,
    active: bool,
    stopped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Discrete<S> {
    Leave(S),
    Enter(S, Point),
    Button(Time, u32, bool),
}

/// Assembles `wl_pointer` frames. `framed` is false for `wl_pointer` before v5, where every event stands
/// alone.
pub struct PointerAssembler<S> {
    framed: bool,
    focus: Option<S>,
    pos: Point,
    buttons: u32,
    // The open frame.
    discrete: SmallVec<[Discrete<S>; 4]>,
    motions: SmallVec<[(Time, Point); 4]>,
    axes: [AxisAcc; 2],
    axis_time: Option<Time>,
    source: Option<AxisSource>,
    // A finger/continuous scroll sequence is running (between its first frame and `axis_stop`).
    scrolling: bool,
}

impl<S: Copy + PartialEq> PointerAssembler<S> {
    pub fn new(framed: bool) -> Self {
        PointerAssembler {
            framed,
            focus: None,
            pos: Point::default(),
            buttons: 0,
            discrete: SmallVec::new(),
            motions: SmallVec::new(),
            axes: [AxisAcc::default(); 2],
            axis_time: None,
            source: None,
            scrolling: false,
        }
    }

    pub fn focus(&self) -> Option<S> {
        self.focus
    }

    pub fn position(&self) -> Point {
        self.pos
    }

    pub fn buttons(&self) -> u32 {
        self.buttons
    }

    /// Feeds one event; returns the events of a frame when it closes.
    pub fn push(&mut self, ev: RawPointer<S>, mods: Modifiers, now: Time) -> SmallVec<[Assembled<S>; 4]> {
        match ev {
            RawPointer::Enter { surface, pos } => self.discrete.push(Discrete::Enter(surface, pos)),
            RawPointer::Leave { surface } => self.discrete.push(Discrete::Leave(surface)),
            RawPointer::Motion { time, pos } => self.motions.push((time, pos)),
            RawPointer::Button { time, button, pressed } => self.discrete.push(Discrete::Button(time, button, pressed)),
            RawPointer::Axis { time, axis, value } => {
                let a = &mut self.axes[axis as usize];
                a.value += value;
                a.active = true;
                self.axis_time = Some(time);
            }
            RawPointer::AxisSource(s) => self.source = Some(s),
            RawPointer::AxisStop { time, axis } => {
                let a = &mut self.axes[axis as usize];
                a.stopped = true;
                self.axis_time = Some(time);
            }
            RawPointer::AxisDiscrete { axis, discrete } => {
                let a = &mut self.axes[axis as usize];
                a.v120 += discrete * 120;
                a.has_v120 = true;
            }
            RawPointer::AxisValue120 { axis, value120 } => {
                let a = &mut self.axes[axis as usize];
                a.v120 += value120;
                a.has_v120 = true;
            }
            RawPointer::Frame => return self.flush(mods, now),
        }
        if self.framed { SmallVec::new() } else { self.flush(mods, now) }
    }

    fn flush(&mut self, mods: Modifiers, now: Time) -> SmallVec<[Assembled<S>; 4]> {
        let mut out = SmallVec::new();
        for d in std::mem::take(&mut self.discrete) {
            match d {
                Discrete::Leave(s) => {
                    let ev = self.pointer_event(PointerPhase::Leave, None, mods);
                    out.push(Assembled { surface: s, time: now, event: InputEvent::Pointer(ev) });
                    if self.focus == Some(s) {
                        self.focus = None;
                    }
                    // The compositor releases implicit grabs it ends with a leave; buttons no longer reach us.
                    self.buttons = 0;
                    self.scrolling = false;
                }
                Discrete::Enter(s, pos) => {
                    self.focus = Some(s);
                    self.pos = pos;
                    let ev = self.pointer_event(PointerPhase::Enter, None, mods);
                    out.push(Assembled { surface: s, time: now, event: InputEvent::Pointer(ev) });
                }
                Discrete::Button(time, code, pressed) => {
                    // Motion in the same frame happened at the same instant: apply it first.
                    self.flush_motion(&mut out, mods);
                    let Some(s) = self.focus else { continue };
                    let b = mouse_button(code);
                    let phase = if pressed {
                        self.buttons |= button_bit(b);
                        PointerPhase::Down
                    } else {
                        self.buttons &= !button_bit(b);
                        PointerPhase::Up
                    };
                    let ev = self.pointer_event(phase, Some(b), mods);
                    out.push(Assembled { surface: s, time, event: InputEvent::Pointer(ev) });
                }
            }
        }
        self.flush_motion(&mut out, mods);
        self.flush_axes(&mut out, mods, now);
        out
    }

    fn flush_motion(&mut self, out: &mut SmallVec<[Assembled<S>; 4]>, mods: Modifiers) {
        let Some(&(time, pos)) = self.motions.last() else { return };
        let history: SmallVec<[PointerSample; 4]> = self.motions[..self.motions.len() - 1]
            .iter()
            .map(|&(time, pos)| PointerSample { pos, time, pressure: 0.0 })
            .collect();
        self.motions.clear();
        self.pos = pos;
        let Some(s) = self.focus else { return };
        let mut ev = self.pointer_event(PointerPhase::Move, None, mods);
        ev.history = history;
        out.push(Assembled { surface: s, time, event: InputEvent::Pointer(ev) });
    }

    fn flush_axes(&mut self, out: &mut SmallVec<[Assembled<S>; 4]>, mods: Modifiers, now: Time) {
        let [v, h] = std::mem::take(&mut self.axes);
        let source = self.source.take();
        let time = self.axis_time.take().unwrap_or(now);
        let any_motion = v.active || h.active;
        let any_stop = v.stopped || h.stopped;
        if !any_motion && !any_stop {
            return;
        }
        let Some(s) = self.focus else {
            self.scrolling = false;
            return;
        };
        let wheel = matches!(source, Some(AxisSource::Wheel | AxisSource::WheelTilt));
        let (delta, phase) = if wheel || (source.is_none() && (v.has_v120 || h.has_v120)) {
            let delta = if v.has_v120 || h.has_v120 {
                ScrollDelta::Lines(Vec2::new(-(h.v120 as f32) / 120.0, -(v.v120 as f32) / 120.0))
            } else {
                ScrollDelta::Pixels(Vec2::new(-h.value as f32, -v.value as f32))
            };
            (delta, None)
        } else {
            let delta = ScrollDelta::Pixels(Vec2::new(-h.value as f32, -v.value as f32));
            // A stop on its own (or with the last delta) ends the sequence. A sequence's axes stop together in
            // practice; a stop on one axis while the other still moves continues it.
            let ends = any_stop && !(v.active && !v.stopped) && !(h.active && !h.stopped);
            let phase = if ends {
                if !self.scrolling && !any_motion {
                    // A stop for a sequence we never saw start (it began before the pointer entered).
                    return;
                }
                self.scrolling = false;
                GesturePhase::Ended
            } else if self.scrolling {
                GesturePhase::Moved
            } else {
                self.scrolling = true;
                GesturePhase::Started
            };
            (delta, Some(phase))
        };
        let ev = WheelEvent { pos: self.pos, delta, phase, momentum: None, modifiers: mods };
        out.push(Assembled { surface: s, time, event: InputEvent::Wheel(ev) });
    }

    fn pointer_event(&self, phase: PointerPhase, button: Option<MouseButton>, mods: Modifiers) -> PointerEvent {
        PointerEvent {
            phase,
            pointer: MOUSE_POINTER,
            kind: PointerType::Mouse,
            button,
            buttons: self.buttons,
            pos: self.pos,
            modifiers: mods,
            pressure: if self.buttons & 1 != 0 { 0.5 } else { 0.0 },
            tilt: Vec2::default(),
            history: SmallVec::new(),
        }
    }
}

/// One `wl_touch` event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RawTouch<S> {
    Down { time: Time, surface: S, id: i32, pos: Point },
    Up { time: Time, id: i32 },
    Motion { time: Time, id: i32, pos: Point },
    Frame,
    Cancel,
}

#[derive(Clone)]
struct TouchPoint<S> {
    id: i32,
    surface: S,
    pos: Point,
    // Within the open frame.
    down: Option<Time>,
    moves: SmallVec<[(Time, Point); 2]>,
    up: Option<Time>,
}

/// Assembles `wl_touch` frames into touch pointer events (one pointer id per touch point).
pub struct TouchAssembler<S> {
    points: Vec<TouchPoint<S>>,
}

impl<S: Copy + PartialEq> Default for TouchAssembler<S> {
    fn default() -> Self {
        TouchAssembler { points: Vec::new() }
    }
}

impl<S: Copy + PartialEq> TouchAssembler<S> {
    pub fn push(&mut self, ev: RawTouch<S>, mods: Modifiers) -> SmallVec<[Assembled<S>; 4]> {
        match ev {
            RawTouch::Down { time, surface, id, pos } => {
                self.points.retain(|p| p.id != id);
                self.points.push(TouchPoint { id, surface, pos, down: Some(time), moves: SmallVec::new(), up: None });
            }
            RawTouch::Up { time, id } => {
                if let Some(p) = self.points.iter_mut().find(|p| p.id == id) {
                    p.up = Some(time);
                }
            }
            RawTouch::Motion { time, id, pos } => {
                if let Some(p) = self.points.iter_mut().find(|p| p.id == id) {
                    p.moves.push((time, pos));
                }
            }
            RawTouch::Frame => return self.flush(mods),
            RawTouch::Cancel => {
                let out = self
                    .points
                    .drain(..)
                    .map(|p| Assembled {
                        surface: p.surface,
                        time: p.up.or(p.down).unwrap_or_default(),
                        event: InputEvent::Pointer(touch_event(PointerPhase::Cancel, p.id, p.pos, mods, false)),
                    })
                    .collect();
                return out;
            }
        }
        SmallVec::new()
    }

    fn flush(&mut self, mods: Modifiers) -> SmallVec<[Assembled<S>; 4]> {
        let mut out = SmallVec::new();
        for p in &mut self.points {
            if let Some(t) = p.down.take() {
                let ev = touch_event(PointerPhase::Down, p.id, p.pos, mods, true);
                out.push(Assembled { surface: p.surface, time: t, event: InputEvent::Pointer(ev) });
            }
            if let Some(&(t, pos)) = p.moves.last() {
                let history = p.moves[..p.moves.len() - 1]
                    .iter()
                    .map(|&(time, pos)| PointerSample { pos, time, pressure: 1.0 })
                    .collect();
                p.pos = pos;
                p.moves.clear();
                let mut ev = touch_event(PointerPhase::Move, p.id, pos, mods, true);
                ev.history = history;
                out.push(Assembled { surface: p.surface, time: t, event: InputEvent::Pointer(ev) });
            }
            if let Some(t) = p.up {
                let ev = touch_event(PointerPhase::Up, p.id, p.pos, mods, false);
                out.push(Assembled { surface: p.surface, time: t, event: InputEvent::Pointer(ev) });
            }
        }
        self.points.retain(|p| p.up.is_none());
        out
    }
}

fn touch_event(phase: PointerPhase, id: i32, pos: Point, mods: Modifiers, pressed: bool) -> PointerEvent {
    PointerEvent {
        phase,
        pointer: TOUCH_POINTER_BASE | (id as u32 & 0x0FFF_FFFF),
        kind: PointerType::Touch,
        button: matches!(phase, PointerPhase::Down | PointerPhase::Up).then_some(MouseButton::Primary),
        buttons: if pressed { 1 } else { 0 },
        pos,
        modifiers: mods,
        pressure: if pressed { 1.0 } else { 0.0 },
        tilt: Vec2::default(),
        history: SmallVec::new(),
    }
}

/// One `zwp_tablet_tool_v2` event (axes already normalised: pressure 0–1, tilt in degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RawTool<S> {
    ProximityIn { surface: S },
    ProximityOut,
    Down,
    Up,
    Motion { pos: Point },
    Pressure(f32),
    Tilt(Vec2),
    Button { button: u32, pressed: bool },
    Frame { time: Time },
}

/// Assembles one tablet tool's frames into pen pointer events.
pub struct ToolAssembler<S> {
    pointer: u32,
    surface: Option<S>,
    pos: Point,
    pressure: f32,
    tilt: Vec2,
    buttons: u32,
    // The open frame.
    entered: Option<S>,
    left: bool,
    down: Option<bool>,
    moved: bool,
    button_changes: SmallVec<[(u32, bool); 2]>,
}

impl<S: Copy + PartialEq> ToolAssembler<S> {
    pub fn new(index: u32) -> Self {
        ToolAssembler {
            pointer: PEN_POINTER_BASE | (index & 0x0FFF_FFFF),
            surface: None,
            pos: Point::default(),
            pressure: 0.0,
            tilt: Vec2::default(),
            buttons: 0,
            entered: None,
            left: false,
            down: None,
            moved: false,
            button_changes: SmallVec::new(),
        }
    }

    pub fn push(&mut self, ev: RawTool<S>, mods: Modifiers) -> SmallVec<[Assembled<S>; 4]> {
        match ev {
            RawTool::ProximityIn { surface } => self.entered = Some(surface),
            RawTool::ProximityOut => self.left = true,
            RawTool::Down => self.down = Some(true),
            RawTool::Up => self.down = Some(false),
            RawTool::Motion { pos } => {
                self.pos = pos;
                self.moved = true;
            }
            RawTool::Pressure(p) => {
                self.pressure = p;
                self.moved = true;
            }
            RawTool::Tilt(t) => {
                self.tilt = t;
                self.moved = true;
            }
            RawTool::Button { button, pressed } => self.button_changes.push((button, pressed)),
            RawTool::Frame { time } => return self.flush(time, mods),
        }
        SmallVec::new()
    }

    fn flush(&mut self, time: Time, mods: Modifiers) -> SmallVec<[Assembled<S>; 4]> {
        let mut out = SmallVec::new();
        if let Some(s) = self.entered.take() {
            self.surface = Some(s);
            out.push(self.assembled(s, time, PointerPhase::Enter, None, mods));
        }
        let Some(s) = self.surface else {
            self.reset_frame();
            return out;
        };
        if std::mem::take(&mut self.moved) {
            out.push(self.assembled(s, time, PointerPhase::Move, None, mods));
        }
        if let Some(down) = self.down.take() {
            let phase = if down {
                self.buttons |= 1;
                PointerPhase::Down
            } else {
                self.buttons &= !1;
                PointerPhase::Up
            };
            out.push(self.assembled(s, time, phase, Some(MouseButton::Primary), mods));
        }
        for (code, pressed) in std::mem::take(&mut self.button_changes) {
            let b = match code {
                btn::STYLUS => MouseButton::Secondary,
                btn::STYLUS2 => MouseButton::Middle,
                other => MouseButton::Other(other as u16),
            };
            let phase = if pressed {
                self.buttons |= button_bit(b);
                PointerPhase::Down
            } else {
                self.buttons &= !button_bit(b);
                PointerPhase::Up
            };
            out.push(self.assembled(s, time, phase, Some(b), mods));
        }
        if std::mem::take(&mut self.left) {
            self.buttons = 0;
            out.push(self.assembled(s, time, PointerPhase::Leave, None, mods));
            self.surface = None;
        }
        out
    }

    fn reset_frame(&mut self) {
        self.left = false;
        self.down = None;
        self.moved = false;
        self.button_changes.clear();
    }

    fn assembled(&self, s: S, time: Time, phase: PointerPhase, button: Option<MouseButton>, mods: Modifiers) -> Assembled<S> {
        let ev = PointerEvent {
            phase,
            pointer: self.pointer,
            kind: PointerType::Pen,
            button,
            buttons: self.buttons,
            pos: self.pos,
            modifiers: mods,
            pressure: if self.buttons & 1 != 0 { self.pressure } else { 0.0 },
            tilt: self.tilt,
            history: SmallVec::new(),
        };
        Assembled { surface: s, time, event: InputEvent::Pointer(ev) }
    }
}

/// Pinch and rotate from `zwp_pointer_gesture_pinch_v1`: the protocol reports the absolute scale and
/// rotation since `begin`; skelter wants per-event deltas.
#[derive(Default)]
pub struct PinchAssembler {
    scale: f32,
}

impl PinchAssembler {
    pub fn begin(&mut self) -> SmallVec<[GestureEvent; 2]> {
        self.scale = 1.0;
        let mut out = SmallVec::new();
        out.push(GestureEvent::Pinch { delta: 0.0, phase: GesturePhase::Started });
        out.push(GestureEvent::Rotate { delta_rad: 0.0, phase: GesturePhase::Started });
        out
    }

    /// `scale` is absolute since begin; `rotation_deg` is the delta since the last update (degrees, clockwise).
    pub fn update(&mut self, scale: f32, rotation_deg: f32) -> SmallVec<[GestureEvent; 2]> {
        let delta = scale - self.scale;
        self.scale = scale;
        let mut out = SmallVec::new();
        if delta != 0.0 {
            out.push(GestureEvent::Pinch { delta, phase: GesturePhase::Moved });
        }
        if rotation_deg != 0.0 {
            out.push(GestureEvent::Rotate { delta_rad: rotation_deg.to_radians(), phase: GesturePhase::Moved });
        }
        out
    }

    pub fn end(&mut self, cancelled: bool) -> SmallVec<[GestureEvent; 2]> {
        let phase = if cancelled { GesturePhase::Cancelled } else { GesturePhase::Ended };
        let mut out = SmallVec::new();
        out.push(GestureEvent::Pinch { delta: 0.0, phase });
        out.push(GestureEvent::Rotate { delta_rad: 0.0, phase });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 7;
    const NOMOD: Modifiers = Modifiers(0);

    fn t(ms: u64) -> Time {
        Time(ms * 1_000_000)
    }

    fn feed(a: &mut PointerAssembler<u32>, evs: &[RawPointer<u32>]) -> Vec<Assembled<u32>> {
        let mut out = Vec::new();
        for &e in evs {
            out.extend(a.push(e, NOMOD, t(1000)));
        }
        out
    }

    fn pointer(e: &Assembled<u32>) -> &PointerEvent {
        match &e.event {
            InputEvent::Pointer(p) => p,
            other => panic!("not a pointer event: {other:?}"),
        }
    }

    fn wheel(e: &Assembled<u32>) -> &WheelEvent {
        match &e.event {
            InputEvent::Wheel(w) => w,
            other => panic!("not a wheel event: {other:?}"),
        }
    }

    #[test]
    fn nothing_is_emitted_before_frame() {
        let mut a = PointerAssembler::new(true);
        assert!(a.push(RawPointer::Enter { surface: W, pos: Point::new(1.0, 2.0) }, NOMOD, t(1)).is_empty());
        assert!(a.push(RawPointer::Motion { time: t(2), pos: Point::new(3.0, 4.0) }, NOMOD, t(2)).is_empty());
        let out = a.push(RawPointer::Frame, NOMOD, t(3));
        assert_eq!(out.len(), 2);
        assert_eq!(pointer(&out[0]).phase, PointerPhase::Enter);
        assert_eq!(pointer(&out[0]).pos, Point::new(1.0, 2.0));
        assert_eq!(pointer(&out[1]).phase, PointerPhase::Move);
        assert_eq!(pointer(&out[1]).pos, Point::new(3.0, 4.0));
        assert_eq!(out[1].time, t(2));
    }

    #[test]
    fn unframed_pointers_emit_each_event() {
        let mut a = PointerAssembler::new(false);
        assert_eq!(a.push(RawPointer::Enter { surface: W, pos: Point::new(1.0, 2.0) }, NOMOD, t(1)).len(), 1);
        assert_eq!(a.push(RawPointer::Motion { time: t(2), pos: Point::new(3.0, 4.0) }, NOMOD, t(2)).len(), 1);
    }

    #[test]
    fn leave_then_enter_in_one_frame_moves_focus() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: 1, pos: Point::new(0.0, 0.0) }, RawPointer::Frame]);
        let out = feed(
            &mut a,
            &[RawPointer::Leave { surface: 1 }, RawPointer::Enter { surface: 2, pos: Point::new(5.0, 5.0) }, RawPointer::Frame],
        );
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].surface, pointer(&out[0]).phase), (1, PointerPhase::Leave));
        assert_eq!((out[1].surface, pointer(&out[1]).phase), (2, PointerPhase::Enter));
        assert_eq!(a.focus(), Some(2));
    }

    #[test]
    fn several_motions_in_a_frame_become_history() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: W, pos: Point::new(0.0, 0.0) }, RawPointer::Frame]);
        let out = feed(
            &mut a,
            &[
                RawPointer::Motion { time: t(10), pos: Point::new(1.0, 0.0) },
                RawPointer::Motion { time: t(11), pos: Point::new(2.0, 0.0) },
                RawPointer::Motion { time: t(12), pos: Point::new(3.0, 0.0) },
                RawPointer::Frame,
            ],
        );
        assert_eq!(out.len(), 1);
        let p = pointer(&out[0]);
        assert_eq!(p.pos, Point::new(3.0, 0.0));
        assert_eq!(p.history.len(), 2);
        assert_eq!(p.history[0].time, t(10));
        assert_eq!(p.history[1].pos, Point::new(2.0, 0.0));
    }

    #[test]
    fn buttons_track_state_and_apply_motion_first() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: W, pos: Point::new(0.0, 0.0) }, RawPointer::Frame]);
        let out = feed(
            &mut a,
            &[
                RawPointer::Motion { time: t(20), pos: Point::new(9.0, 9.0) },
                RawPointer::Button { time: t(20), button: btn::LEFT, pressed: true },
                RawPointer::Frame,
            ],
        );
        assert_eq!(out.len(), 2);
        assert_eq!(pointer(&out[0]).phase, PointerPhase::Move);
        let down = pointer(&out[1]);
        assert_eq!((down.phase, down.button, down.buttons), (PointerPhase::Down, Some(MouseButton::Primary), 1));
        assert_eq!(down.pos, Point::new(9.0, 9.0));
        let out = feed(&mut a, &[RawPointer::Button { time: t(30), button: btn::RIGHT, pressed: true }, RawPointer::Frame]);
        assert_eq!(pointer(&out[0]).buttons, 3);
        let out = feed(&mut a, &[RawPointer::Button { time: t(40), button: btn::LEFT, pressed: false }, RawPointer::Frame]);
        let up = pointer(&out[0]);
        assert_eq!((up.phase, up.buttons), (PointerPhase::Up, 2));
    }

    #[test]
    fn side_buttons_map_to_back_and_forward() {
        assert_eq!(mouse_button(btn::SIDE), MouseButton::Back);
        assert_eq!(mouse_button(btn::EXTRA), MouseButton::Forward);
        assert_eq!(mouse_button(btn::TASK), MouseButton::Other(7));
    }

    #[test]
    fn wheel_detents_become_lines_with_skelter_sign() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: W, pos: Point::new(4.0, 4.0) }, RawPointer::Frame]);
        // One detent towards the user (Wayland positive = down).
        let out = feed(
            &mut a,
            &[
                RawPointer::AxisSource(AxisSource::Wheel),
                RawPointer::AxisValue120 { axis: Axis::Vertical, value120: 120 },
                RawPointer::Axis { time: t(50), axis: Axis::Vertical, value: 15.0 },
                RawPointer::Frame,
            ],
        );
        assert_eq!(out.len(), 1);
        let w = wheel(&out[0]);
        assert_eq!(w.delta, ScrollDelta::Lines(Vec2::new(0.0, -1.0)));
        assert_eq!(w.phase, None);
        assert_eq!(w.pos, Point::new(4.0, 4.0));
        assert_eq!(out[0].time, t(50));
    }

    #[test]
    fn high_resolution_wheels_give_fractional_lines() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: W, pos: Point::new(0.0, 0.0) }, RawPointer::Frame]);
        let out = feed(
            &mut a,
            &[
                RawPointer::AxisSource(AxisSource::Wheel),
                RawPointer::AxisValue120 { axis: Axis::Vertical, value120: -30 },
                RawPointer::Axis { time: t(1), axis: Axis::Vertical, value: -3.75 },
                RawPointer::Frame,
            ],
        );
        assert_eq!(wheel(&out[0]).delta, ScrollDelta::Lines(Vec2::new(0.0, 0.25)));
    }

    #[test]
    fn legacy_discrete_counts_as_detents() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: W, pos: Point::new(0.0, 0.0) }, RawPointer::Frame]);
        let out = feed(
            &mut a,
            &[
                RawPointer::AxisSource(AxisSource::Wheel),
                RawPointer::AxisDiscrete { axis: Axis::Horizontal, discrete: 2 },
                RawPointer::Axis { time: t(1), axis: Axis::Horizontal, value: 30.0 },
                RawPointer::Frame,
            ],
        );
        assert_eq!(wheel(&out[0]).delta, ScrollDelta::Lines(Vec2::new(-2.0, 0.0)));
    }

    #[test]
    fn touchpad_scroll_runs_started_moved_ended() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: W, pos: Point::new(0.0, 0.0) }, RawPointer::Frame]);
        let finger = |dy: f64, ms: u64| {
            [
                RawPointer::AxisSource(AxisSource::Finger),
                RawPointer::Axis { time: t(ms), axis: Axis::Vertical, value: dy },
                RawPointer::Frame,
            ]
        };
        let out = feed(&mut a, &finger(2.5, 1));
        assert_eq!(wheel(&out[0]).phase, Some(GesturePhase::Started));
        assert_eq!(wheel(&out[0]).delta, ScrollDelta::Pixels(Vec2::new(0.0, -2.5)));
        let out = feed(&mut a, &finger(3.0, 2));
        assert_eq!(wheel(&out[0]).phase, Some(GesturePhase::Moved));
        let out = feed(
            &mut a,
            &[
                RawPointer::AxisSource(AxisSource::Finger),
                RawPointer::AxisStop { time: t(3), axis: Axis::Vertical },
                RawPointer::Frame,
            ],
        );
        let w = wheel(&out[0]);
        assert_eq!(w.phase, Some(GesturePhase::Ended));
        assert_eq!(w.delta, ScrollDelta::Pixels(Vec2::new(0.0, 0.0)));
        assert_eq!(out[0].time, t(3));
        // A new sequence starts again.
        let out = feed(&mut a, &finger(1.0, 4));
        assert_eq!(wheel(&out[0]).phase, Some(GesturePhase::Started));
    }

    #[test]
    fn stop_without_a_seen_start_is_dropped() {
        let mut a = PointerAssembler::new(true);
        feed(&mut a, &[RawPointer::Enter { surface: W, pos: Point::new(0.0, 0.0) }, RawPointer::Frame]);
        let out = feed(
            &mut a,
            &[
                RawPointer::AxisSource(AxisSource::Finger),
                RawPointer::AxisStop { time: t(3), axis: Axis::Vertical },
                RawPointer::Frame,
            ],
        );
        assert!(out.is_empty());
    }

    #[test]
    fn events_without_focus_are_dropped() {
        let mut a = PointerAssembler::new(true);
        let out = feed(
            &mut a,
            &[
                RawPointer::Motion { time: t(1), pos: Point::new(1.0, 1.0) },
                RawPointer::Button { time: t(1), button: btn::LEFT, pressed: true },
                RawPointer::Frame,
            ],
        );
        assert!(out.is_empty());
    }

    #[test]
    fn leave_clears_buttons() {
        let mut a = PointerAssembler::new(true);
        feed(
            &mut a,
            &[
                RawPointer::Enter { surface: W, pos: Point::new(0.0, 0.0) },
                RawPointer::Button { time: t(1), button: btn::LEFT, pressed: true },
                RawPointer::Frame,
            ],
        );
        assert_eq!(a.buttons(), 1);
        feed(&mut a, &[RawPointer::Leave { surface: W }, RawPointer::Frame]);
        assert_eq!(a.buttons(), 0);
        assert_eq!(a.focus(), None);
    }

    #[test]
    fn touch_frames_carry_down_move_up_per_point() {
        let mut a = TouchAssembler::default();
        assert!(a.push(RawTouch::Down { time: t(1), surface: W, id: 3, pos: Point::new(1.0, 1.0) }, NOMOD).is_empty());
        a.push(RawTouch::Down { time: t(1), surface: W, id: 4, pos: Point::new(9.0, 9.0) }, NOMOD);
        let out = a.push(RawTouch::Frame, NOMOD);
        assert_eq!(out.len(), 2);
        assert_eq!(pointer(&out[0]).pointer, TOUCH_POINTER_BASE | 3);
        assert_eq!(pointer(&out[1]).pointer, TOUCH_POINTER_BASE | 4);
        assert!(out.iter().all(|e| pointer(e).phase == PointerPhase::Down && pointer(e).kind == PointerType::Touch));

        a.push(RawTouch::Motion { time: t(2), id: 3, pos: Point::new(2.0, 2.0) }, NOMOD);
        a.push(RawTouch::Motion { time: t(3), id: 3, pos: Point::new(3.0, 3.0) }, NOMOD);
        a.push(RawTouch::Up { time: t(3), id: 4 }, NOMOD);
        let out = a.push(RawTouch::Frame, NOMOD);
        assert_eq!(out.len(), 2);
        let mv = pointer(&out[0]);
        assert_eq!((mv.phase, mv.pos, mv.history.len()), (PointerPhase::Move, Point::new(3.0, 3.0), 1));
        let up = pointer(&out[1]);
        assert_eq!((up.phase, up.pointer, up.pos), (PointerPhase::Up, TOUCH_POINTER_BASE | 4, Point::new(9.0, 9.0)));
    }

    #[test]
    fn touch_cancel_cancels_every_point() {
        let mut a = TouchAssembler::default();
        a.push(RawTouch::Down { time: t(1), surface: W, id: 1, pos: Point::new(1.0, 1.0) }, NOMOD);
        a.push(RawTouch::Down { time: t(1), surface: W, id: 2, pos: Point::new(2.0, 2.0) }, NOMOD);
        a.push(RawTouch::Frame, NOMOD);
        let out = a.push(RawTouch::Cancel, NOMOD);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|e| pointer(e).phase == PointerPhase::Cancel));
        assert!(a.push(RawTouch::Frame, NOMOD).is_empty());
    }

    #[test]
    fn pen_frames() {
        let mut a = ToolAssembler::new(0);
        a.push(RawTool::ProximityIn { surface: W }, NOMOD);
        a.push(RawTool::Motion { pos: Point::new(5.0, 5.0) }, NOMOD);
        let out = a.push(RawTool::Frame { time: t(1) }, NOMOD);
        assert_eq!(out.len(), 2);
        assert_eq!(pointer(&out[0]).phase, PointerPhase::Enter);
        assert_eq!(pointer(&out[1]).kind, PointerType::Pen);

        a.push(RawTool::Down, NOMOD);
        a.push(RawTool::Pressure(0.75), NOMOD);
        a.push(RawTool::Tilt(Vec2::new(10.0, -5.0)), NOMOD);
        let out = a.push(RawTool::Frame { time: t(2) }, NOMOD);
        assert_eq!(out.len(), 2);
        let down = pointer(&out[1]);
        assert_eq!((down.phase, down.buttons, down.pressure), (PointerPhase::Down, 1, 0.75));
        assert_eq!(down.tilt, Vec2::new(10.0, -5.0));

        a.push(RawTool::Button { button: btn::STYLUS, pressed: true }, NOMOD);
        let out = a.push(RawTool::Frame { time: t(3) }, NOMOD);
        assert_eq!(pointer(&out[0]).button, Some(MouseButton::Secondary));
        assert_eq!(pointer(&out[0]).buttons, 3);

        a.push(RawTool::Up, NOMOD);
        a.push(RawTool::ProximityOut, NOMOD);
        let out = a.push(RawTool::Frame { time: t(4) }, NOMOD);
        let phases: Vec<_> = out.iter().map(|e| pointer(e).phase).collect();
        assert_eq!(phases, [PointerPhase::Up, PointerPhase::Leave]);
        assert!(a.push(RawTool::Motion { pos: Point::new(1.0, 1.0) }, NOMOD).is_empty());
        assert!(a.push(RawTool::Frame { time: t(5) }, NOMOD).is_empty());
    }

    #[test]
    fn pinch_reports_deltas() {
        let mut p = PinchAssembler::default();
        assert_eq!(p.begin().len(), 2);
        let ev = p.update(1.25, 0.0);
        assert_eq!(ev.as_slice(), [GestureEvent::Pinch { delta: 0.25, phase: GesturePhase::Moved }]);
        let ev = p.update(1.5, 90.0);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0], GestureEvent::Pinch { delta: 0.25, phase: GesturePhase::Moved });
        assert!(matches!(ev[1], GestureEvent::Rotate { delta_rad, .. } if (delta_rad - std::f32::consts::FRAC_PI_2).abs() < 1e-6));
        assert!(p.end(true).iter().all(|g| matches!(
            g,
            GestureEvent::Pinch { phase: GesturePhase::Cancelled, .. } | GestureEvent::Rotate { phase: GesturePhase::Cancelled, .. }
        )));
    }
}
