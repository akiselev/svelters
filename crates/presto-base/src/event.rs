//! Platform input and window events. Every input event arrives in an [`InputEnvelope`] with an
//! [`InputSeq`] and the OS timestamp, carried through dispatch → JS → commit → present for the latency
//! instrumentation (architecture §16.3).

use crate::geom::{DeviceSize, Point, Scale, Vec2};
use crate::ids::{InputSeq, WindowId};
use crate::key::Key;
use crate::time::Time;
use smallvec::SmallVec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const SHIFT: Modifiers = Modifiers(1);
    pub const CTRL: Modifiers = Modifiers(2);
    pub const ALT: Modifiers = Modifiers(4);
    /// Cmd on macOS, the Windows/Super key elsewhere.
    pub const SUPER: Modifiers = Modifiers(8);
    /// AltGr (kept apart from Ctrl+Alt: winit #2945).
    pub const ALT_GR: Modifiers = Modifiers(16);
    pub const fn contains(self, o: Modifiers) -> bool {
        self.0 & o.0 == o.0
    }
    pub const fn union(self, o: Modifiers) -> Modifiers {
        Modifiers(self.0 | o.0)
    }
    /// The platform's primary shortcut modifier: Cmd on macOS, Ctrl elsewhere.
    pub const fn primary(self, macos: bool) -> bool {
        if macos { self.contains(Self::SUPER) } else { self.contains(Self::CTRL) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerType {
    Mouse,
    Pen,
    Touch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Up,
    Move,
    Enter,
    Leave,
    Cancel,
}

/// One coalesced sample (window points), for pointer history (architecture §11.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSample {
    pub pos: Point,
    pub time: Time,
    pub pressure: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointerEvent {
    pub phase: PointerPhase,
    /// 0 for the mouse; touch and pen pointers get their own ids.
    pub pointer: u32,
    pub kind: PointerType,
    pub button: Option<MouseButton>,
    /// Buttons held after this event, bit per `MouseButton` (primary = 1, secondary = 2, middle = 4).
    pub buttons: u32,
    pub pos: Point,
    pub modifiers: Modifiers,
    pub pressure: f32,
    pub tilt: Vec2,
    /// Earlier samples merged into this event, oldest first (empty when the OS gave none).
    pub history: SmallVec<[PointerSample; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    /// Wheel detents (lines); positive y scrolls content up (wheel away from the user).
    Lines(Vec2),
    /// Precise deltas in points (touchpads, macOS precise scrolling).
    Pixels(Vec2),
    Pages(Vec2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WheelEvent {
    pub pos: Point,
    pub delta: ScrollDelta,
    /// Touchpad gesture phase (macOS `phase`, Wayland `axis_stop`, DirectManipulation); `None` = wheel.
    pub phase: Option<GesturePhase>,
    /// OS-generated momentum (macOS `momentumPhase`).
    pub momentum: Option<GesturePhase>,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyLocation {
    Standard,
    Left,
    Right,
    Numpad,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    /// The logical key (layout-mapped), egui-named; `None` when it has no egui name.
    pub key: Option<Key>,
    /// The physical key's platform-independent code: the USB HID usage as `page << 16 | usage`
    /// (Chromium's form; keyboard keys are page 7, so A is `0x0007_0004`), 0 when unknown.
    pub physical: u32,
    /// Text this key press produces (after dead keys), if any.
    pub text: Option<String>,
    pub state: KeyState,
    pub repeat: bool,
    pub location: KeyLocation,
    pub modifiers: Modifiers,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImeEvent {
    Enabled,
    /// The composition text and the cursor range in it (byte offsets).
    Preedit { text: String, cursor: Option<(usize, usize)> },
    Commit(String),
    /// Delete text around the cursor (bytes before, bytes after).
    DeleteSurrounding { before: usize, after: usize },
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureEvent {
    Pinch { delta: f32, phase: GesturePhase },
    /// `delta_rad` is clockwise-positive (egui's convention); backends convert the OS's sign.
    Rotate { delta_rad: f32, phase: GesturePhase },
    DoubleTap,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Pointer(PointerEvent),
    Wheel(WheelEvent),
    Key(KeyEvent),
    Ime(ImeEvent),
    Gesture(GestureEvent),
    ModifiersChanged(Modifiers),
    FileHovered(std::path::PathBuf),
    FileDropped(std::path::PathBuf),
    FileHoverCancelled,
}

impl InputEvent {
    /// Discrete events dispatch at once; continuous ones coalesce per frame (architecture §11.1).
    pub fn is_discrete(&self) -> bool {
        !matches!(
            self,
            InputEvent::Pointer(PointerEvent { phase: PointerPhase::Move, .. }) | InputEvent::Wheel(_)
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputEnvelope {
    pub seq: InputSeq,
    pub window: WindowId,
    /// The OS's timestamp of the event, on presto's clock.
    pub os_time: Time,
    /// When presto received it.
    pub recv_time: Time,
    pub event: InputEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    Resized { size: DeviceSize, scale: Scale },
    ScaleChanged { scale: Scale },
    /// The window's new position: the content's top-left in global points.
    Moved { x: i32, y: i32 },
    Focused(bool),
    /// Hidden, minimised or fully covered (or the compositor stopped frame callbacks for 100 ms).
    Occluded(bool),
    CloseRequested,
    ThemeChanged(Theme),
}
