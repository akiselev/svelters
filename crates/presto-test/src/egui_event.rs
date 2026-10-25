//! The `egui::Event`-shaped injection adapter (architecture §16.1): tests written against egui_kittest
//! keep their `harness.event(egui::Event::…)` calls during the transition. Each egui event becomes the
//! presto input the platform would deliver for it, so it takes the production dispatch path.

use presto_base::accesskit::ActionRequest;
use presto_base::event::{
    GestureEvent, GesturePhase, ImeEvent, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers,
    MouseButton, PointerEvent, PointerPhase, PointerType, ScrollDelta, WheelEvent,
};
use presto_base::geom::{Point, Vec2};
use presto_base::key::Key;
use presto_base::smallvec::SmallVec;

/// What one egui event turns into.
#[derive(Clone, Debug, PartialEq)]
pub enum Injected {
    Input(InputEvent),
    /// An AccessKit action, routed to the tree's action handler.
    A11y(ActionRequest),
    /// Window focus gained or lost.
    Focused(bool),
    /// Put `text` on the clipboard, then the paste chord follows as a key event.
    SetClipboard(String),
}

/// Tracks what egui events leave implicit: the pointer position (wheel events carry none), the held
/// buttons and the platform's primary modifier.
#[derive(Clone, Debug)]
pub struct EguiAdapter {
    pub pointer: Point,
    buttons: u32,
    /// Cmd is the primary shortcut modifier (egui's `command` maps to it).
    pub macos: bool,
}

impl Default for EguiAdapter {
    fn default() -> Self {
        Self {
            pointer: Point::ZERO,
            buttons: 0,
            macos: cfg!(target_os = "macos"),
        }
    }
}

pub fn modifiers(m: egui::Modifiers, macos: bool) -> Modifiers {
    let mut out = Modifiers::default();
    if m.shift {
        out = out.union(Modifiers::SHIFT);
    }
    if m.alt {
        out = out.union(Modifiers::ALT);
    }
    if m.ctrl || (m.command && !macos) {
        out = out.union(Modifiers::CTRL);
    }
    if m.mac_cmd || (m.command && macos) {
        out = out.union(Modifiers::SUPER);
    }
    out
}

/// egui's key → presto's (they share egui's names).
pub fn key(k: egui::Key) -> Option<Key> {
    Key::from_name(k.name())
}

fn button(b: egui::PointerButton) -> MouseButton {
    match b {
        egui::PointerButton::Primary => MouseButton::Primary,
        egui::PointerButton::Secondary => MouseButton::Secondary,
        egui::PointerButton::Middle => MouseButton::Middle,
        egui::PointerButton::Extra1 => MouseButton::Back,
        egui::PointerButton::Extra2 => MouseButton::Forward,
    }
}

fn button_bit(b: MouseButton) -> u32 {
    match b {
        MouseButton::Primary => 1,
        MouseButton::Secondary => 2,
        MouseButton::Middle => 4,
        MouseButton::Back => 8,
        MouseButton::Forward => 16,
        MouseButton::Other(_) => 0,
    }
}

fn gesture(p: egui::TouchPhase) -> GesturePhase {
    match p {
        egui::TouchPhase::Start => GesturePhase::Started,
        egui::TouchPhase::Move => GesturePhase::Moved,
        egui::TouchPhase::End => GesturePhase::Ended,
        egui::TouchPhase::Cancel => GesturePhase::Cancelled,
    }
}

impl EguiAdapter {
    pub fn pointer_event(
        &self,
        phase: PointerPhase,
        pos: Point,
        b: Option<MouseButton>,
        m: Modifiers,
    ) -> PointerEvent {
        PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: b,
            buttons: self.buttons,
            pos,
            modifiers: m,
            pressure: if self.buttons != 0 { 0.5 } else { 0.0 },
            tilt: Vec2::ZERO,
            history: SmallVec::new(),
        }
    }

    fn chord(&self, k: Key) -> Vec<Injected> {
        let m = if self.macos {
            Modifiers::SUPER
        } else {
            Modifiers::CTRL
        };
        [KeyState::Pressed, KeyState::Released]
            .into_iter()
            .map(|state| {
                Injected::Input(InputEvent::Key(KeyEvent {
                    key: Some(k),
                    physical: 0,
                    text: None,
                    state,
                    repeat: false,
                    location: KeyLocation::Standard,
                    modifiers: m,
                }))
            })
            .collect()
    }

    /// Translate one egui event. Events with no presto counterpart translate to nothing:
    /// `MouseMoved` (raw motion is off outside 3D capture), `PasteImage`, `MiddleClickPaste`, and IME
    /// `DeleteSurrounding` (egui counts chars, presto bytes of text the adapter cannot see).
    #[allow(deprecated)]
    pub fn translate(&mut self, ev: &egui::Event) -> Vec<Injected> {
        use egui::Event as E;
        match ev {
            E::Copy => self.chord(Key::C),
            E::Cut => self.chord(Key::X),
            E::Paste(text) => {
                let mut out = vec![Injected::SetClipboard(text.clone())];
                out.extend(self.chord(Key::V));
                out
            }
            E::PasteImage(_) | E::MiddleClickPaste { .. } | E::MouseMoved(_) => Vec::new(),
            E::Text(t) => vec![Injected::Input(InputEvent::Ime(ImeEvent::Commit(
                t.clone(),
            )))],
            E::Key {
                key: k,
                pressed,
                repeat,
                modifiers: m,
                ..
            } => {
                vec![Injected::Input(InputEvent::Key(KeyEvent {
                    key: key(*k),
                    physical: 0,
                    text: None,
                    state: if *pressed {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    },
                    repeat: *repeat,
                    location: KeyLocation::Standard,
                    modifiers: modifiers(*m, self.macos),
                }))]
            }
            E::ModifiersChanged(m) => vec![Injected::Input(InputEvent::ModifiersChanged(
                modifiers(*m, self.macos),
            ))],
            E::PointerMoved(p) => {
                self.pointer = Point::new(p.x, p.y);
                let e = self.pointer_event(
                    PointerPhase::Move,
                    self.pointer,
                    None,
                    Modifiers::default(),
                );
                vec![Injected::Input(InputEvent::Pointer(e))]
            }
            E::PointerButton {
                pos,
                button: b,
                pressed,
                modifiers: m,
            } => {
                self.pointer = Point::new(pos.x, pos.y);
                let b = button(*b);
                if *pressed {
                    self.buttons |= button_bit(b);
                } else {
                    self.buttons &= !button_bit(b);
                }
                let phase = if *pressed {
                    PointerPhase::Down
                } else {
                    PointerPhase::Up
                };
                let e = self.pointer_event(phase, self.pointer, Some(b), modifiers(*m, self.macos));
                vec![Injected::Input(InputEvent::Pointer(e))]
            }
            E::PointerGone => {
                let e = self.pointer_event(
                    PointerPhase::Leave,
                    self.pointer,
                    None,
                    Modifiers::default(),
                );
                vec![Injected::Input(InputEvent::Pointer(e))]
            }
            E::Zoom(f) => vec![Injected::Input(InputEvent::Gesture(GestureEvent::Pinch {
                delta: f - 1.0,
                phase: GesturePhase::Moved,
            }))],
            E::Rotate(r) => vec![Injected::Input(InputEvent::Gesture(GestureEvent::Rotate {
                delta_rad: *r,
                phase: GesturePhase::Moved,
            }))],
            E::Ime(i) => match i {
                egui::ImeEvent::Enabled => {
                    vec![Injected::Input(InputEvent::Ime(ImeEvent::Enabled))]
                }
                egui::ImeEvent::Disabled => {
                    vec![Injected::Input(InputEvent::Ime(ImeEvent::Disabled))]
                }
                egui::ImeEvent::Commit(t) => vec![Injected::Input(InputEvent::Ime(
                    ImeEvent::Commit(t.clone()),
                ))],
                egui::ImeEvent::Preedit {
                    text,
                    active_range_chars,
                } => {
                    let byte = |c: usize| text.char_indices().nth(c).map_or(text.len(), |(b, _)| b);
                    let cursor = active_range_chars
                        .as_ref()
                        .map(|r| (byte(r.start), byte(r.end)));
                    vec![Injected::Input(InputEvent::Ime(ImeEvent::Preedit {
                        text: text.clone(),
                        cursor,
                    }))]
                }
                egui::ImeEvent::DeleteSurrounding { .. } => Vec::new(),
            },
            E::Touch {
                id,
                phase,
                pos,
                force,
                ..
            } => {
                let phase = match phase {
                    egui::TouchPhase::Start => PointerPhase::Down,
                    egui::TouchPhase::Move => PointerPhase::Move,
                    egui::TouchPhase::End => PointerPhase::Up,
                    egui::TouchPhase::Cancel => PointerPhase::Cancel,
                };
                let down = matches!(phase, PointerPhase::Down | PointerPhase::Move);
                let e = PointerEvent {
                    phase,
                    // 0 is the mouse.
                    pointer: (id.0 as u32).wrapping_add(1),
                    kind: PointerType::Touch,
                    button: matches!(phase, PointerPhase::Down | PointerPhase::Up)
                        .then_some(MouseButton::Primary),
                    buttons: if down { 1 } else { 0 },
                    pos: Point::new(pos.x, pos.y),
                    modifiers: Modifiers::default(),
                    pressure: force.unwrap_or(if down { 0.5 } else { 0.0 }),
                    tilt: Vec2::ZERO,
                    history: SmallVec::new(),
                };
                vec![Injected::Input(InputEvent::Pointer(e))]
            }
            E::MouseWheel {
                unit,
                delta,
                phase,
                source,
                modifiers: m,
            } => {
                let d = Vec2::new(delta.x, delta.y);
                let delta = match unit {
                    egui::MouseWheelUnit::Point => ScrollDelta::Pixels(d),
                    egui::MouseWheelUnit::Line => ScrollDelta::Lines(d),
                    egui::MouseWheelUnit::Page => ScrollDelta::Pages(d),
                };
                let phase =
                    matches!(source, egui::MouseWheelSource::Trackpad).then(|| gesture(*phase));
                vec![Injected::Input(InputEvent::Wheel(WheelEvent {
                    pos: self.pointer,
                    delta,
                    phase,
                    momentum: None,
                    modifiers: modifiers(*m, self.macos),
                }))]
            }
            E::WindowFocused(f) => vec![Injected::Focused(*f)],
            E::AccessKitActionRequest(r) => vec![Injected::A11y(r.clone())],
        }
    }
}
