//! winit window events → presto events. [`WindowInput`] keeps the per-window state the translation
//! needs (scale, held buttons, modifiers, last pointer position) and turns each winit event into
//! zero or one presto event, positions in window points.

use crate::keys;
use presto_base::cursor::CursorIcon;
use presto_base::event::{
    GestureEvent, GesturePhase, ImeEvent, InputEvent, KeyEvent, KeyState, Modifiers, MouseButton,
    PointerEvent, PointerPhase, PointerSample, PointerType, ScrollDelta, Theme, WheelEvent,
    WindowEvent,
};
use presto_base::geom::{DeviceSize, Point, Scale, Vec2};
use smallvec::SmallVec;
use winit::event::{self as we, ElementState, MouseScrollDelta, TouchPhase};

/// One translated winit event.
#[derive(Clone, Debug, PartialEq)]
pub enum Translated {
    Input(InputEvent),
    Window(WindowEvent),
}

/// Per-window translation state.
#[derive(Clone, Debug)]
pub struct WindowInput {
    pub scale: f32,
    pub buttons: u32,
    pub modifiers: Modifiers,
    pub pos: Point,
    /// Samples the OS coalesced into the next move (the Windows shim's `GetMouseMovePointsEx`).
    pub pending_history: SmallVec<[PointerSample; 4]>,
}

impl WindowInput {
    pub fn new(scale: f32) -> Self {
        WindowInput {
            scale,
            buttons: 0,
            modifiers: Modifiers::default(),
            pos: Point::ZERO,
            pending_history: SmallVec::new(),
        }
    }

    fn point(&self, x: f64, y: f64) -> Point {
        Point::new(
            (x / self.scale as f64) as f32,
            (y / self.scale as f64) as f32,
        )
    }

    fn pointer(&mut self, phase: PointerPhase, button: Option<MouseButton>) -> PointerEvent {
        PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button,
            buttons: self.buttons,
            pos: self.pos,
            modifiers: self.modifiers,
            pressure: if self.buttons & 1 != 0 { 0.5 } else { 0.0 },
            tilt: Vec2::ZERO,
            history: if phase == PointerPhase::Move {
                std::mem::take(&mut self.pending_history)
            } else {
                SmallVec::new()
            },
        }
    }

    /// Translates one event. `None` for events presto does not deliver (redraws, axis motion).
    pub fn translate(&mut self, ev: &we::WindowEvent) -> Option<Translated> {
        use we::WindowEvent as W;
        Some(match ev {
            W::Resized(s) => Translated::Window(WindowEvent::Resized {
                size: DeviceSize {
                    w: s.width,
                    h: s.height,
                },
                scale: Scale(self.scale),
            }),
            W::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = *scale_factor as f32;
                Translated::Window(WindowEvent::ScaleChanged {
                    scale: Scale(self.scale),
                })
            }
            W::Moved(p) => Translated::Window(WindowEvent::Moved { x: p.x, y: p.y }),
            W::CloseRequested => Translated::Window(WindowEvent::CloseRequested),
            W::Focused(f) => {
                if !f {
                    self.buttons = 0;
                }
                Translated::Window(WindowEvent::Focused(*f))
            }
            W::Occluded(o) => Translated::Window(WindowEvent::Occluded(*o)),
            W::ThemeChanged(t) => Translated::Window(WindowEvent::ThemeChanged(match t {
                winit::window::Theme::Light => Theme::Light,
                winit::window::Theme::Dark => Theme::Dark,
            })),
            W::DroppedFile(p) => Translated::Input(InputEvent::FileDropped(p.clone())),
            W::HoveredFile(p) => Translated::Input(InputEvent::FileHovered(p.clone())),
            W::HoveredFileCancelled => Translated::Input(InputEvent::FileHoverCancelled),
            W::ModifiersChanged(m) => {
                self.modifiers = modifiers(m.state());
                Translated::Input(InputEvent::ModifiersChanged(self.modifiers))
            }
            W::KeyboardInput { event, .. } => Translated::Input(InputEvent::Key(KeyEvent {
                key: keys::key_from_winit(&event.logical_key, event.physical_key),
                physical: keys::hid_usage(event.physical_key),
                text: event.text.as_ref().map(|t| t.to_string()),
                state: key_state(event.state),
                repeat: event.repeat,
                location: keys::location(event.location),
                modifiers: self.modifiers,
            })),
            W::Ime(i) => Translated::Input(InputEvent::Ime(ime(i))),
            W::CursorMoved { position, .. } => {
                self.pos = self.point(position.x, position.y);
                Translated::Input(InputEvent::Pointer(self.pointer(PointerPhase::Move, None)))
            }
            W::CursorEntered { .. } => {
                Translated::Input(InputEvent::Pointer(self.pointer(PointerPhase::Enter, None)))
            }
            W::CursorLeft { .. } => {
                Translated::Input(InputEvent::Pointer(self.pointer(PointerPhase::Leave, None)))
            }
            W::MouseInput { state, button, .. } => {
                let b = mouse_button(*button);
                let bit = button_bit(b);
                let phase = match state {
                    ElementState::Pressed => {
                        self.buttons |= bit;
                        PointerPhase::Down
                    }
                    ElementState::Released => {
                        self.buttons &= !bit;
                        PointerPhase::Up
                    }
                };
                Translated::Input(InputEvent::Pointer(self.pointer(phase, Some(b))))
            }
            W::MouseWheel { delta, phase, .. } => {
                let (delta, phase) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (ScrollDelta::Lines(Vec2::new(*x, *y)), None)
                    }
                    MouseScrollDelta::PixelDelta(p) => (
                        ScrollDelta::Pixels(Vec2::new(
                            (p.x / self.scale as f64) as f32,
                            (p.y / self.scale as f64) as f32,
                        )),
                        Some(gesture_phase(*phase)),
                    ),
                };
                Translated::Input(InputEvent::Wheel(WheelEvent {
                    pos: self.pos,
                    delta,
                    phase,
                    momentum: None,
                    modifiers: self.modifiers,
                }))
            }
            W::PanGesture { delta, phase, .. } => {
                Translated::Input(InputEvent::Wheel(WheelEvent {
                    pos: self.pos,
                    delta: ScrollDelta::Pixels(Vec2::new(
                        delta.x / self.scale,
                        delta.y / self.scale,
                    )),
                    phase: Some(gesture_phase(*phase)),
                    momentum: None,
                    modifiers: self.modifiers,
                }))
            }
            W::PinchGesture { delta, phase, .. } => {
                Translated::Input(InputEvent::Gesture(GestureEvent::Pinch {
                    delta: *delta as f32,
                    phase: gesture_phase(*phase),
                }))
            }
            W::RotationGesture { delta, phase, .. } => {
                Translated::Input(InputEvent::Gesture(GestureEvent::Rotate {
                    delta_rad: delta.to_radians(),
                    phase: gesture_phase(*phase),
                }))
            }
            W::DoubleTapGesture { .. } => {
                Translated::Input(InputEvent::Gesture(GestureEvent::DoubleTap))
            }
            W::Touch(t) => {
                let pos = self.point(t.location.x, t.location.y);
                let phase = match t.phase {
                    TouchPhase::Started => PointerPhase::Down,
                    TouchPhase::Moved => PointerPhase::Move,
                    TouchPhase::Ended => PointerPhase::Up,
                    TouchPhase::Cancelled => PointerPhase::Cancel,
                };
                let down = matches!(phase, PointerPhase::Down | PointerPhase::Move);
                Translated::Input(InputEvent::Pointer(PointerEvent {
                    phase,
                    pointer: touch_pointer_id(t.id),
                    kind: PointerType::Touch,
                    button: matches!(phase, PointerPhase::Down | PointerPhase::Up)
                        .then_some(MouseButton::Primary),
                    buttons: down as u32,
                    pos,
                    modifiers: self.modifiers,
                    pressure: t
                        .force
                        .map_or(if down { 0.5 } else { 0.0 }, |f| f.normalized() as f32),
                    tilt: Vec2::ZERO,
                    history: SmallVec::new(),
                }))
            }
            W::ActivationTokenDone { .. }
            | W::Destroyed
            | W::TouchpadPressure { .. }
            | W::AxisMotion { .. }
            | W::RedrawRequested => return None,
        })
    }
}

/// Touch pointers get ids from 1 up; 0 is the mouse.
pub fn touch_pointer_id(id: u64) -> u32 {
    (id as u32).wrapping_add(1).max(1)
}

pub fn modifiers(m: winit::keyboard::ModifiersState) -> Modifiers {
    let mut out = Modifiers::default();
    if m.shift_key() {
        out = out.union(Modifiers::SHIFT);
    }
    if m.control_key() {
        out = out.union(Modifiers::CTRL);
    }
    if m.alt_key() {
        out = out.union(Modifiers::ALT);
    }
    if m.super_key() {
        out = out.union(Modifiers::SUPER);
    }
    out
}

pub fn key_state(s: ElementState) -> KeyState {
    match s {
        ElementState::Pressed => KeyState::Pressed,
        ElementState::Released => KeyState::Released,
    }
}

pub fn mouse_button(b: we::MouseButton) -> MouseButton {
    match b {
        we::MouseButton::Left => MouseButton::Primary,
        we::MouseButton::Right => MouseButton::Secondary,
        we::MouseButton::Middle => MouseButton::Middle,
        we::MouseButton::Back => MouseButton::Back,
        we::MouseButton::Forward => MouseButton::Forward,
        we::MouseButton::Other(n) => MouseButton::Other(n),
    }
}

/// The `PointerEvent::buttons` bit of a button (primary 1, secondary 2, middle 4, back 8, forward
/// 16, DOM order); other buttons have none.
pub fn button_bit(b: MouseButton) -> u32 {
    match b {
        MouseButton::Primary => 1,
        MouseButton::Secondary => 2,
        MouseButton::Middle => 4,
        MouseButton::Back => 8,
        MouseButton::Forward => 16,
        MouseButton::Other(_) => 0,
    }
}

pub fn gesture_phase(p: TouchPhase) -> GesturePhase {
    match p {
        TouchPhase::Started => GesturePhase::Started,
        TouchPhase::Moved => GesturePhase::Moved,
        TouchPhase::Ended => GesturePhase::Ended,
        TouchPhase::Cancelled => GesturePhase::Cancelled,
    }
}

pub fn ime(i: &we::Ime) -> ImeEvent {
    match i {
        we::Ime::Enabled => ImeEvent::Enabled,
        we::Ime::Preedit(text, cursor) => ImeEvent::Preedit {
            text: text.clone(),
            cursor: *cursor,
        },
        we::Ime::Commit(s) => ImeEvent::Commit(s.clone()),
        we::Ime::Disabled => ImeEvent::Disabled,
    }
}

/// presto's cursor → winit's; `None` = hide the cursor.
pub fn cursor_icon(c: CursorIcon) -> Option<winit::window::CursorIcon> {
    use winit::window::CursorIcon as W;
    Some(match c {
        CursorIcon::Default => W::Default,
        CursorIcon::ContextMenu => W::ContextMenu,
        CursorIcon::Help => W::Help,
        CursorIcon::Pointer => W::Pointer,
        CursorIcon::Progress => W::Progress,
        CursorIcon::Wait => W::Wait,
        CursorIcon::Cell => W::Cell,
        CursorIcon::Crosshair => W::Crosshair,
        CursorIcon::Text => W::Text,
        CursorIcon::VerticalText => W::VerticalText,
        CursorIcon::Alias => W::Alias,
        CursorIcon::Copy => W::Copy,
        CursorIcon::Move => W::Move,
        CursorIcon::NoDrop => W::NoDrop,
        CursorIcon::NotAllowed => W::NotAllowed,
        CursorIcon::Grab => W::Grab,
        CursorIcon::Grabbing => W::Grabbing,
        CursorIcon::EResize => W::EResize,
        CursorIcon::NResize => W::NResize,
        CursorIcon::NeResize => W::NeResize,
        CursorIcon::NwResize => W::NwResize,
        CursorIcon::SResize => W::SResize,
        CursorIcon::SeResize => W::SeResize,
        CursorIcon::SwResize => W::SwResize,
        CursorIcon::WResize => W::WResize,
        CursorIcon::EwResize => W::EwResize,
        CursorIcon::NsResize => W::NsResize,
        CursorIcon::NeswResize => W::NeswResize,
        CursorIcon::NwseResize => W::NwseResize,
        CursorIcon::ColResize => W::ColResize,
        CursorIcon::RowResize => W::RowResize,
        CursorIcon::AllScroll => W::AllScroll,
        CursorIcon::ZoomIn => W::ZoomIn,
        CursorIcon::ZoomOut => W::ZoomOut,
        CursorIcon::None => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::event::{DeviceId, WindowEvent as W};

    fn dev() -> DeviceId {
        DeviceId::dummy()
    }

    fn pointer(t: Option<Translated>) -> PointerEvent {
        match t {
            Some(Translated::Input(InputEvent::Pointer(p))) => p,
            other => panic!("not a pointer event: {other:?}"),
        }
    }

    #[test]
    fn positions_are_in_points_and_buttons_accumulate() {
        let mut w = WindowInput::new(2.0);
        let p = pointer(w.translate(&W::CursorMoved {
            device_id: dev(),
            position: PhysicalPosition::new(200.0, 100.0),
        }));
        assert_eq!(p.pos, Point::new(100.0, 50.0));
        assert_eq!(p.phase, PointerPhase::Move);

        let p = pointer(w.translate(&W::MouseInput {
            device_id: dev(),
            state: ElementState::Pressed,
            button: we::MouseButton::Left,
        }));
        assert_eq!(
            (p.phase, p.button, p.buttons),
            (PointerPhase::Down, Some(MouseButton::Primary), 1)
        );
        let p = pointer(w.translate(&W::MouseInput {
            device_id: dev(),
            state: ElementState::Pressed,
            button: we::MouseButton::Right,
        }));
        assert_eq!(p.buttons, 3);
        let p = pointer(w.translate(&W::MouseInput {
            device_id: dev(),
            state: ElementState::Released,
            button: we::MouseButton::Left,
        }));
        assert_eq!(
            (p.phase, p.buttons, p.pos),
            (PointerPhase::Up, 2, Point::new(100.0, 50.0))
        );
        // Losing focus forgets held buttons (their release goes to another window).
        w.translate(&W::Focused(false));
        assert_eq!(w.buttons, 0);
    }

    #[test]
    fn coalesced_history_rides_on_the_next_move_only() {
        let mut w = WindowInput::new(1.0);
        w.pending_history.push(PointerSample {
            pos: Point::new(1.0, 1.0),
            time: presto_base::time::Time(5),
            pressure: 0.0,
        });
        let p = pointer(w.translate(&W::CursorMoved {
            device_id: dev(),
            position: PhysicalPosition::new(2.0, 2.0),
        }));
        assert_eq!(p.history.len(), 1);
        let p = pointer(w.translate(&W::CursorMoved {
            device_id: dev(),
            position: PhysicalPosition::new(3.0, 3.0),
        }));
        assert!(p.history.is_empty());
    }

    #[test]
    fn wheel_lines_stay_lines_and_pixels_become_points_with_a_phase() {
        let mut w = WindowInput::new(2.0);
        match w.translate(&W::MouseWheel {
            device_id: dev(),
            delta: MouseScrollDelta::LineDelta(0.0, 1.0),
            phase: TouchPhase::Moved,
        }) {
            Some(Translated::Input(InputEvent::Wheel(e))) => {
                assert_eq!(e.delta, ScrollDelta::Lines(Vec2::new(0.0, 1.0)));
                assert_eq!(e.phase, None);
            }
            other => panic!("{other:?}"),
        }
        match w.translate(&W::MouseWheel {
            device_id: dev(),
            delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(4.0, -8.0)),
            phase: TouchPhase::Started,
        }) {
            Some(Translated::Input(InputEvent::Wheel(e))) => {
                assert_eq!(e.delta, ScrollDelta::Pixels(Vec2::new(2.0, -4.0)));
                assert_eq!(e.phase, Some(GesturePhase::Started));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn window_events_and_modifiers() {
        let mut w = WindowInput::new(1.5);
        assert_eq!(
            w.translate(&W::Resized(PhysicalSize::new(300, 150))),
            Some(Translated::Window(WindowEvent::Resized {
                size: DeviceSize { w: 300, h: 150 },
                scale: Scale(1.5)
            }))
        );
        assert_eq!(
            w.translate(&W::CloseRequested),
            Some(Translated::Window(WindowEvent::CloseRequested))
        );
        assert_eq!(w.translate(&W::RedrawRequested), None);
        let m = winit::keyboard::ModifiersState::SHIFT | winit::keyboard::ModifiersState::SUPER;
        assert_eq!(modifiers(m), Modifiers::SHIFT.union(Modifiers::SUPER));
        assert_eq!(
            w.translate(&W::Ime(we::Ime::Preedit("かな".into(), Some((0, 3))))),
            Some(Translated::Input(InputEvent::Ime(ImeEvent::Preedit {
                text: "かな".into(),
                cursor: Some((0, 3))
            })))
        );
    }

    #[test]
    fn touch_ids_never_collide_with_the_mouse() {
        assert_eq!(touch_pointer_id(0), 1);
        assert_ne!(touch_pointer_id(u32::MAX as u64), 0);
    }

    #[test]
    fn every_cursor_but_none_maps() {
        assert_eq!(
            cursor_icon(CursorIcon::Text),
            Some(winit::window::CursorIcon::Text)
        );
        assert_eq!(cursor_icon(CursorIcon::None), None);
    }
}
