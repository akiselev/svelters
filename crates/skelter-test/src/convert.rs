//! Arguments the harness verbs take in either vocabulary: skelter's (`Key`, `Point`, `Size`,
//! `Modifiers`, `MouseButton`, `InputEvent`) or egui's, so tests written against egui_kittest keep their
//! calls (`h.key_press(egui::Key::Enter)`, `h.hover_at(egui::pos2(..))`, `h.event(egui::Event::..)`).
//! The egui side needs feature `egui`.

use skelter_base::event::{InputEvent, Modifiers, MouseButton};
use skelter_base::geom::{Point, Size};
use skelter_base::key::Key;

pub trait IntoKey {
    fn into_key(self) -> Key;
}

impl IntoKey for Key {
    fn into_key(self) -> Key {
        self
    }
}

#[cfg(feature = "egui")]
impl IntoKey for egui::Key {
    /// skelter's key table is egui's, by name.
    #[track_caller]
    fn into_key(self) -> Key {
        crate::egui_event::key(self)
            .unwrap_or_else(|| panic!("egui key {:?} has no skelter key", self.name()))
    }
}

pub trait IntoPoint {
    fn into_point(self) -> Point;
}

impl IntoPoint for Point {
    fn into_point(self) -> Point {
        self
    }
}

#[cfg(feature = "egui")]
impl IntoPoint for egui::Pos2 {
    fn into_point(self) -> Point {
        Point::new(self.x, self.y)
    }
}

pub trait IntoSize {
    fn into_size(self) -> Size;
}

impl IntoSize for Size {
    fn into_size(self) -> Size {
        self
    }
}

#[cfg(feature = "egui")]
impl IntoSize for egui::Vec2 {
    fn into_size(self) -> Size {
        Size::new(self.x, self.y)
    }
}

pub trait IntoModifiers {
    /// `macos`: egui's `command` is Cmd there, Ctrl elsewhere.
    fn into_modifiers(self, macos: bool) -> Modifiers;
}

impl IntoModifiers for Modifiers {
    fn into_modifiers(self, _macos: bool) -> Modifiers {
        self
    }
}

#[cfg(feature = "egui")]
impl IntoModifiers for egui::Modifiers {
    fn into_modifiers(self, macos: bool) -> Modifiers {
        crate::egui_event::modifiers(self, macos)
    }
}

pub trait IntoButton {
    fn into_button(self) -> MouseButton;
}

impl IntoButton for MouseButton {
    fn into_button(self) -> MouseButton {
        self
    }
}

#[cfg(feature = "egui")]
impl IntoButton for egui::PointerButton {
    fn into_button(self) -> MouseButton {
        crate::egui_event::button(self)
    }
}

/// An injected event: skelter input, or an egui event the adapter translates as the platform would
/// deliver it.
#[derive(Clone, Debug)]
pub enum TestEvent {
    Skelter(InputEvent),
    #[cfg(feature = "egui")]
    Egui(egui::Event),
}

impl From<InputEvent> for TestEvent {
    fn from(e: InputEvent) -> Self {
        TestEvent::Skelter(e)
    }
}

#[cfg(feature = "egui")]
impl From<egui::Event> for TestEvent {
    fn from(e: egui::Event) -> Self {
        TestEvent::Egui(e)
    }
}
