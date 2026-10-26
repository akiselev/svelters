//! `NSEvent` to skelter input events: pointer (mouse and tablet pen), wheel with gesture and momentum
//! phases, magnify and rotate, keys.

use crate::keyboard::Layout;
use crate::keymap::{self, OptionAsAlt};
use crate::time;
use objc2_app_kit::{NSEvent, NSEventPhase, NSEventSubtype, NSEventType, NSView};
use skelter_base::event::{
    GestureEvent, GesturePhase, InputEvent, KeyEvent, KeyState, MouseButton, PointerEvent,
    PointerPhase, PointerType, ScrollDelta, WheelEvent,
};
use skelter_base::geom::{Point, Vec2};
use skelter_base::smallvec::SmallVec;
use skelter_base::time::Time;

pub(crate) fn raw_flags(e: &NSEvent) -> u64 {
    e.modifierFlags().0 as u64
}

pub(crate) fn os_time(e: &NSEvent) -> Time {
    time::from_secs(e.timestamp())
}

/// The event's location in `view` (flipped: points from the top-left).
pub(crate) fn location(view: &NSView, e: &NSEvent) -> Point {
    let p = view.convertPoint_fromView(e.locationInWindow(), None);
    Point::new(p.x as f32, p.y as f32)
}

pub(crate) fn button(number: isize) -> MouseButton {
    match number {
        0 => MouseButton::Primary,
        1 => MouseButton::Secondary,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        n => MouseButton::Other(n.clamp(0, u16::MAX as isize) as u16),
    }
}

/// `NSEvent.pressedMouseButtons` bits (left, right, middle, back, forward) are skelter's bits.
pub(crate) fn buttons() -> u32 {
    (NSEvent::pressedMouseButtons() & 0xFFFF) as u32
}

pub(crate) fn gesture_phase(p: NSEventPhase) -> Option<GesturePhase> {
    if p.contains(NSEventPhase::Began) || p.contains(NSEventPhase::MayBegin) {
        Some(GesturePhase::Started)
    } else if p.contains(NSEventPhase::Ended) {
        Some(GesturePhase::Ended)
    } else if p.contains(NSEventPhase::Cancelled) {
        Some(GesturePhase::Cancelled)
    } else if p.contains(NSEventPhase::Changed) || p.contains(NSEventPhase::Stationary) {
        Some(GesturePhase::Moved)
    } else {
        None
    }
}

fn is_tablet_point(e: &NSEvent) -> bool {
    e.r#type() == NSEventType::TabletPoint || e.subtype() == NSEventSubtype::TabletPoint
}

/// Pen pointers get ids above the mouse's 0, one per tablet device.
fn pen_id(e: &NSEvent) -> u32 {
    1 + (e.deviceID() as u32 & 0xFFFF)
}

/// A mouse or tablet event (not an enter/exit tracking event: those carry no button or pressure).
pub(crate) fn pointer(
    view: &NSView,
    e: &NSEvent,
    phase: PointerPhase,
    with_button: bool,
) -> InputEvent {
    let pen = is_tablet_point(e);
    let (kind, pointer, tilt) = if pen {
        let t = e.tilt();
        (
            PointerType::Pen,
            pen_id(e),
            Vec2::new(t.x as f32, t.y as f32),
        )
    } else {
        (PointerType::Mouse, 0, Vec2::ZERO)
    };
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer,
        kind,
        button: with_button.then(|| button(e.buttonNumber())),
        buttons: buttons(),
        pos: location(view, e),
        modifiers: keymap::modifiers_from_flags(raw_flags(e)),
        pressure: e.pressure(),
        tilt,
        history: SmallVec::new(),
    })
}

/// `mouseEntered:` / `mouseExited:`.
pub(crate) fn crossing(view: &NSView, e: &NSEvent, phase: PointerPhase) -> InputEvent {
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button: None,
        buttons: buttons(),
        pos: location(view, e),
        modifiers: keymap::modifiers_from_flags(raw_flags(e)),
        pressure: 0.0,
        tilt: Vec2::ZERO,
        history: SmallVec::new(),
    })
}

/// `tabletProximity:`: the pen entered or left the tablet's range.
pub(crate) fn proximity(view: &NSView, e: &NSEvent) -> InputEvent {
    let phase = if e.isEnteringProximity() {
        PointerPhase::Enter
    } else {
        PointerPhase::Leave
    };
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer: 1 + (e.deviceID() as u32 & 0xFFFF),
        kind: PointerType::Pen,
        button: None,
        buttons: buttons(),
        pos: location(view, e),
        modifiers: keymap::modifiers_from_flags(raw_flags(e)),
        pressure: 0.0,
        tilt: Vec2::ZERO,
        history: SmallVec::new(),
    })
}

pub(crate) fn wheel(view: &NSView, e: &NSEvent) -> InputEvent {
    let d = Vec2::new(e.scrollingDeltaX() as f32, e.scrollingDeltaY() as f32);
    let delta = if e.hasPreciseScrollingDeltas() {
        ScrollDelta::Pixels(d)
    } else {
        ScrollDelta::Lines(d)
    };
    InputEvent::Wheel(WheelEvent {
        pos: location(view, e),
        delta,
        phase: gesture_phase(e.phase()),
        momentum: gesture_phase(e.momentumPhase()),
        modifiers: keymap::modifiers_from_flags(raw_flags(e)),
    })
}

pub(crate) fn magnify(e: &NSEvent) -> InputEvent {
    InputEvent::Gesture(GestureEvent::Pinch {
        delta: e.magnification() as f32,
        phase: gesture_phase(e.phase()).unwrap_or(GesturePhase::Moved),
    })
}

/// Rotation in radians, counter-clockwise positive (AppKit reports degrees).
pub(crate) fn rotate(e: &NSEvent) -> InputEvent {
    InputEvent::Gesture(GestureEvent::Rotate {
        delta_rad: (e.rotation() as f32).to_radians(),
        phase: gesture_phase(e.phase()).unwrap_or(GesturePhase::Moved),
    })
}

/// A key press or release, and whether Option acts as Alt for it (then it types nothing).
pub(crate) fn key(e: &NSEvent, state: KeyState, policy: OptionAsAlt) -> (KeyEvent, bool) {
    let code = e.keyCode();
    let raw = raw_flags(e);
    let option_alt = keymap::option_is_alt(policy, raw);
    let chosen = match Layout::current() {
        Some(layout) => {
            let (chars, always) = layout.chars(code);
            keymap::key_chars(
                &chars,
                raw & keymap::flags::COMMAND != 0,
                raw & keymap::flags::SHIFT != 0,
                always,
            )
        }
        None => String::new(),
    };
    let text = match state {
        KeyState::Pressed if !option_alt => {
            let characters = e.characters().map(|s| s.to_string()).unwrap_or_default();
            keymap::typed_text(&characters, raw)
        }
        _ => None,
    };
    let ev = KeyEvent {
        key: keymap::key_for(code, &chosen),
        physical: keymap::physical_code(code),
        text,
        state,
        repeat: state == KeyState::Pressed && e.isARepeat(),
        location: keymap::key_location(code),
        modifiers: keymap::modifiers_from_flags(raw),
    };
    (ev, option_alt)
}

/// A mouse-moved/dragged event's `deltaX`/`deltaY` (unaccelerated device deltas, y down) as raw
/// motion; `None` when the event did not move.
pub(crate) fn raw_motion(dx: f64, dy: f64) -> Option<InputEvent> {
    (dx != 0.0 || dy != 0.0).then_some(InputEvent::RawMotion { dx: dx as f32, dy: dy as f32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_deltas_become_raw_motion() {
        assert_eq!(raw_motion(2.0, -3.5), Some(InputEvent::RawMotion { dx: 2.0, dy: -3.5 }));
        assert_eq!(raw_motion(0.0, 0.0), None, "a move without motion sends nothing");
    }
}
