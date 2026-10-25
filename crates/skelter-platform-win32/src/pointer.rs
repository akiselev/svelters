//! Pointer input: `WM_POINTER*` messages to [`PointerEvent`]s and [`WheelEvent`]s.
//!
//! The mouse comes through the pointer stack too (`EnableMouseInPointer`), so mouse, pen and touch
//! share one path with QPC timestamps (`POINTER_INFO::PerformanceCount`) and the coalesced samples of
//! `GetPointerInfoHistory` (and `GetPointerPenInfoHistory` for pressure). The translation is split
//! into a pure part ([`translate`]) over [`PointerSnapshot`]s and the OS reads that fill them.

use crate::util::qpc_to_time;
use skelter_base::event::{
    Modifiers, MouseButton, PointerEvent, PointerPhase, PointerSample, PointerType, ScrollDelta, WheelEvent,
};
use skelter_base::geom::{Point, Scale, Vec2};
use skelter_base::time::Time;
use smallvec::SmallVec;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::Pointer::*;
use windows::Win32::UI::WindowsAndMessaging::{
    PEN_MASK_PRESSURE, PEN_MASK_TILT_X, PEN_MASK_TILT_Y, PT_MOUSE, PT_PEN, PT_TOUCH, PT_TOUCHPAD, POINTER_INPUT_TYPE, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN,
    WM_POINTERENTER, WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
};

/// One pointer frame as the OS reported it, in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSnapshot {
    pub kind: PointerType,
    pub os_id: u32,
    pub flags: u32,
    pub change: i32,
    pub screen: (i32, i32),
    pub qpc: u64,
    /// 0..1 when the device reports pressure.
    pub pressure: Option<f32>,
    /// Degrees, -90..90.
    pub tilt: (f32, f32),
}

/// Which `WM_POINTER*` message a snapshot came with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerMsg {
    Down,
    Up,
    Update,
    Enter,
    Leave,
    CaptureChanged,
}

impl PointerMsg {
    pub fn from_msg(msg: u32) -> Option<Self> {
        Some(match msg {
            WM_POINTERDOWN => PointerMsg::Down,
            WM_POINTERUP => PointerMsg::Up,
            WM_POINTERUPDATE => PointerMsg::Update,
            WM_POINTERENTER => PointerMsg::Enter,
            WM_POINTERLEAVE => PointerMsg::Leave,
            WM_POINTERCAPTURECHANGED => PointerMsg::CaptureChanged,
            _ => return None,
        })
    }
}

/// Maps the window's client origin (screen pixels) and scale to window points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientMap {
    pub origin: (i32, i32),
    pub scale: Scale,
}

impl ClientMap {
    pub fn to_point(&self, screen: (i32, i32)) -> Point {
        let s = if self.scale.0 > 0.0 { self.scale.0 } else { 1.0 };
        Point::new((screen.0 - self.origin.0) as f32 / s, (screen.1 - self.origin.1) as f32 / s)
    }
}

pub fn pointer_type(t: POINTER_INPUT_TYPE) -> Option<PointerType> {
    match t {
        PT_MOUSE | PT_TOUCHPAD => Some(PointerType::Mouse),
        PT_PEN => Some(PointerType::Pen),
        PT_TOUCH => Some(PointerType::Touch),
        _ => None,
    }
}

/// Held buttons from `POINTER_FLAG_*BUTTON` (primary 1, secondary 2, middle 4, back 8, forward 16).
pub fn buttons_from_flags(flags: u32) -> u32 {
    let mut b = 0;
    for (flag, bit) in [
        (POINTER_FLAG_FIRSTBUTTON, 1),
        (POINTER_FLAG_SECONDBUTTON, 2),
        (POINTER_FLAG_THIRDBUTTON, 4),
        (POINTER_FLAG_FOURTHBUTTON, 8),
        (POINTER_FLAG_FIFTHBUTTON, 16),
    ] {
        if flags & flag.0 != 0 {
            b |= bit;
        }
    }
    b
}

/// The button a `POINTER_BUTTON_CHANGE_TYPE` names, and whether it went down.
pub fn button_change(change: i32) -> Option<(MouseButton, bool)> {
    Some(match POINTER_BUTTON_CHANGE_TYPE(change) {
        POINTER_CHANGE_FIRSTBUTTON_DOWN => (MouseButton::Primary, true),
        POINTER_CHANGE_FIRSTBUTTON_UP => (MouseButton::Primary, false),
        POINTER_CHANGE_SECONDBUTTON_DOWN => (MouseButton::Secondary, true),
        POINTER_CHANGE_SECONDBUTTON_UP => (MouseButton::Secondary, false),
        POINTER_CHANGE_THIRDBUTTON_DOWN => (MouseButton::Middle, true),
        POINTER_CHANGE_THIRDBUTTON_UP => (MouseButton::Middle, false),
        POINTER_CHANGE_FOURTHBUTTON_DOWN => (MouseButton::Back, true),
        POINTER_CHANGE_FOURTHBUTTON_UP => (MouseButton::Back, false),
        POINTER_CHANGE_FIFTHBUTTON_DOWN => (MouseButton::Forward, true),
        POINTER_CHANGE_FIFTHBUTTON_UP => (MouseButton::Forward, false),
        _ => return None,
    })
}

/// skelter's pointer id: 0 for the mouse (whatever id the OS gave it), the OS id for pen and touch.
pub fn skelter_pointer_id(kind: PointerType, os_id: u32) -> u32 {
    if kind == PointerType::Mouse { 0 } else { os_id }
}

/// The event for one pointer message. `history` holds the frames the OS coalesced into this one,
/// newest first, as `GetPointerInfoHistory` returns them (the first entry is the current frame).
pub fn translate(
    msg: PointerMsg,
    snap: &PointerSnapshot,
    history: &[PointerSnapshot],
    map: ClientMap,
    modifiers: Modifiers,
) -> PointerEvent {
    let change = button_change(snap.change);
    let canceled = snap.flags & POINTER_FLAG_CANCELED.0 != 0;
    let (phase, button) = match msg {
        PointerMsg::Enter => (PointerPhase::Enter, None),
        PointerMsg::Leave => (PointerPhase::Leave, None),
        PointerMsg::CaptureChanged => (PointerPhase::Cancel, None),
        PointerMsg::Up if canceled => (PointerPhase::Cancel, change.map(|c| c.0)),
        // Another button pressed or released while one is held arrives as an update.
        PointerMsg::Down | PointerMsg::Up | PointerMsg::Update => match change {
            Some((b, true)) => (PointerPhase::Down, Some(b)),
            Some((b, false)) => (PointerPhase::Up, Some(b)),
            None if msg == PointerMsg::Down => (PointerPhase::Down, Some(MouseButton::Primary)),
            None if msg == PointerMsg::Up => (PointerPhase::Up, Some(MouseButton::Primary)),
            None => (PointerPhase::Move, None),
        },
    };
    let pressure = snap.pressure.unwrap_or(if snap.flags & POINTER_FLAG_INCONTACT.0 != 0 { 0.5 } else { 0.0 });
    let mut samples: SmallVec<[PointerSample; 4]> = SmallVec::new();
    if phase == PointerPhase::Move {
        // Oldest first, without the current frame.
        for h in history.iter().skip(1).rev() {
            samples.push(PointerSample {
                pos: map.to_point(h.screen),
                time: qpc_to_time(h.qpc as i64),
                pressure: h.pressure.unwrap_or(pressure),
            });
        }
    }
    PointerEvent {
        phase,
        pointer: skelter_pointer_id(snap.kind, snap.os_id),
        kind: snap.kind,
        button,
        buttons: buttons_from_flags(snap.flags),
        pos: map.to_point(snap.screen),
        modifiers,
        pressure,
        tilt: Vec2::new(snap.tilt.0, snap.tilt.1),
        history: samples,
    }
}

/// A wheel step from `WM_POINTERWHEEL`/`WM_POINTERHWHEEL`: one detent is 120 (`WHEEL_DELTA`);
/// high-resolution wheels send fractions. Positive vertical deltas (wheel away from the user) stay
/// positive; horizontal deltas are negated so positive x scrolls toward the start like winit.
pub fn wheel(pos: Point, delta: i16, horizontal: bool, modifiers: Modifiers) -> WheelEvent {
    let lines = delta as f32 / 120.0;
    let v = if horizontal { Vec2::new(-lines, 0.0) } else { Vec2::new(0.0, lines) };
    WheelEvent { pos, delta: ScrollDelta::Lines(v), phase: None, momentum: None, modifiers }
}

fn snapshot_from_info(info: &POINTER_INFO) -> Option<PointerSnapshot> {
    Some(PointerSnapshot {
        kind: pointer_type(info.pointerType)?,
        os_id: info.pointerId,
        flags: info.pointerFlags.0,
        change: info.ButtonChangeType.0,
        screen: (info.ptPixelLocation.x, info.ptPixelLocation.y),
        qpc: info.PerformanceCount,
        pressure: None,
        tilt: (0.0, 0.0),
    })
}

fn snapshot_from_pen(pen: &POINTER_PEN_INFO) -> Option<PointerSnapshot> {
    let mut s = snapshot_from_info(&pen.pointerInfo)?;
    if pen.penMask & PEN_MASK_PRESSURE != 0 {
        s.pressure = Some(pen.pressure as f32 / 1024.0);
    }
    let tx = if pen.penMask & PEN_MASK_TILT_X != 0 { pen.tiltX as f32 } else { 0.0 };
    let ty = if pen.penMask & PEN_MASK_TILT_Y != 0 { pen.tiltY as f32 } else { 0.0 };
    s.tilt = (tx, ty);
    Some(s)
}

/// Reads the pointer's current frame and its coalesced history (newest first, current included).
pub fn read_pointer(pointer_id: u32) -> Option<(PointerSnapshot, Vec<PointerSnapshot>)> {
    let mut ty = POINTER_INPUT_TYPE::default();
    unsafe { GetPointerType(pointer_id, &mut ty) }.ok()?;
    if ty == PT_PEN {
        let mut pen = POINTER_PEN_INFO::default();
        unsafe { GetPointerPenInfo(pointer_id, &mut pen) }.ok()?;
        let snap = snapshot_from_pen(&pen)?;
        let mut count = pen.pointerInfo.historyCount.max(1);
        let mut buf = vec![POINTER_PEN_INFO::default(); count as usize];
        let history = match unsafe { GetPointerPenInfoHistory(pointer_id, &mut count, Some(buf.as_mut_ptr())) } {
            Ok(()) => buf[..(count as usize).min(buf.len())].iter().filter_map(snapshot_from_pen).collect(),
            Err(_) => vec![snap],
        };
        return Some((snap, history));
    }
    let mut info = POINTER_INFO::default();
    unsafe { GetPointerInfo(pointer_id, &mut info) }.ok()?;
    let snap = snapshot_from_info(&info)?;
    let mut count = info.historyCount.max(1);
    let history = if count > 1 {
        let mut buf = vec![POINTER_INFO::default(); count as usize];
        match unsafe { GetPointerInfoHistory(pointer_id, &mut count, Some(buf.as_mut_ptr())) } {
            Ok(()) => buf[..(count as usize).min(buf.len())].iter().filter_map(snapshot_from_info).collect(),
            Err(_) => vec![snap],
        }
    } else {
        vec![snap]
    };
    Some((snap, history))
}

/// The screen position of a pointer (for wheel messages, whose `LPARAM` is in screen pixels too).
pub fn pointer_screen_pos(pointer_id: u32) -> Option<POINT> {
    let mut info = POINTER_INFO::default();
    unsafe { GetPointerInfo(pointer_id, &mut info) }.ok()?;
    Some(info.ptPixelLocation)
}

/// The OS time of a snapshot on skelter's clock.
pub fn snapshot_time(snap: &PointerSnapshot) -> Time {
    qpc_to_time(snap.qpc as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(flags: u32, change: i32, x: i32, qpc: u64) -> PointerSnapshot {
        PointerSnapshot {
            kind: PointerType::Mouse,
            os_id: 1,
            flags,
            change,
            screen: (x, 20),
            qpc,
            pressure: None,
            tilt: (0.0, 0.0),
        }
    }

    const MAP: ClientMap = ClientMap { origin: (100, 0), scale: Scale(2.0) };

    #[test]
    fn down_up_and_button_changes() {
        let down = snap(POINTER_FLAG_FIRSTBUTTON.0 | POINTER_FLAG_INCONTACT.0, POINTER_CHANGE_FIRSTBUTTON_DOWN.0, 110, 0);
        let ev = translate(PointerMsg::Down, &down, &[down], MAP, Modifiers::default());
        assert_eq!(ev.phase, PointerPhase::Down);
        assert_eq!(ev.button, Some(MouseButton::Primary));
        assert_eq!(ev.buttons, 1);
        assert_eq!(ev.pointer, 0, "the mouse is pointer 0");
        assert_eq!(ev.pos, Point::new(5.0, 10.0));

        let right = snap(
            POINTER_FLAG_FIRSTBUTTON.0 | POINTER_FLAG_SECONDBUTTON.0,
            POINTER_CHANGE_SECONDBUTTON_DOWN.0,
            110,
            0,
        );
        let ev = translate(PointerMsg::Update, &right, &[right], MAP, Modifiers::default());
        assert_eq!((ev.phase, ev.button, ev.buttons), (PointerPhase::Down, Some(MouseButton::Secondary), 3));

        let x2 = snap(0, POINTER_CHANGE_FIFTHBUTTON_UP.0, 110, 0);
        let ev = translate(PointerMsg::Up, &x2, &[x2], MAP, Modifiers::default());
        assert_eq!((ev.phase, ev.button), (PointerPhase::Up, Some(MouseButton::Forward)));
    }

    #[test]
    fn canceled_and_capture_changes_cancel() {
        let up = snap(POINTER_FLAG_CANCELED.0, POINTER_CHANGE_FIRSTBUTTON_UP.0, 110, 0);
        assert_eq!(translate(PointerMsg::Up, &up, &[up], MAP, Modifiers::default()).phase, PointerPhase::Cancel);
        let any = snap(0, 0, 110, 0);
        let ev = translate(PointerMsg::CaptureChanged, &any, &[any], MAP, Modifiers::default());
        assert_eq!(ev.phase, PointerPhase::Cancel);
    }

    #[test]
    fn moves_carry_history_oldest_first() {
        let now = snap(0, 0, 130, 30);
        let history = [now, snap(0, 0, 120, 20), snap(0, 0, 110, 10)];
        let ev = translate(PointerMsg::Update, &now, &history, MAP, Modifiers::default());
        assert_eq!(ev.phase, PointerPhase::Move);
        let xs: Vec<f32> = ev.history.iter().map(|s| s.pos.x).collect();
        assert_eq!(xs, vec![5.0, 10.0]);
        assert_eq!(ev.pos.x, 15.0);
    }

    #[test]
    fn pen_pressure_and_ids() {
        let mut pen = snap(POINTER_FLAG_INCONTACT.0 | POINTER_FLAG_FIRSTBUTTON.0, POINTER_CHANGE_FIRSTBUTTON_DOWN.0, 100, 0);
        pen.kind = PointerType::Pen;
        pen.os_id = 7;
        pen.pressure = Some(0.25);
        pen.tilt = (10.0, -5.0);
        let ev = translate(PointerMsg::Down, &pen, &[pen], MAP, Modifiers::default());
        assert_eq!((ev.pointer, ev.kind, ev.pressure), (7, PointerType::Pen, 0.25));
        assert_eq!(ev.tilt, Vec2::new(10.0, -5.0));
    }

    #[test]
    fn wheel_directions() {
        let up = wheel(Point::ZERO, 120, false, Modifiers::default());
        assert_eq!(up.delta, ScrollDelta::Lines(Vec2::new(0.0, 1.0)));
        let fine = wheel(Point::ZERO, -30, false, Modifiers::default());
        assert_eq!(fine.delta, ScrollDelta::Lines(Vec2::new(0.0, -0.25)));
        let right = wheel(Point::ZERO, 120, true, Modifiers::default());
        assert_eq!(right.delta, ScrollDelta::Lines(Vec2::new(-1.0, 0.0)));
    }

    #[test]
    fn message_kinds() {
        assert_eq!(PointerMsg::from_msg(WM_POINTERUPDATE), Some(PointerMsg::Update));
        assert_eq!(PointerMsg::from_msg(0), None);
    }
}
