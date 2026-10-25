//! presto input → egui `Event`s for one island (no egui-winit).
//!
//! The translator owns the pointer routing an island needs: events reach egui while the pointer is inside
//! the island's rect, or while a button pressed inside it is held (presto's pointer capture, architecture
//! §11.4, keeps sending them outside the rect); leaving the rect without a capture is egui's
//! `PointerGone`. Keys, text and IME reach egui only while the island has keyboard focus.
//!
//! The key, text and clipboard-shortcut rules follow egui-winit 0.37.0 (`src/lib.rs`,
//! `on_keyboard_input`, `is_cut_command`/`is_copy_command`/`is_paste_command`, `is_printable_char`;
//! MIT OR Apache-2.0, Copyright (c) 2018-2021 Emil Ernerfeldt), so panels behave as they did under
//! eframe. Modifications by the svelters authors.

use presto_base::event::{
    GestureEvent, GesturePhase, ImeEvent, InputEvent, KeyEvent, KeyState, Modifiers, MouseButton,
    PointerEvent, PointerPhase, PointerType, ScrollDelta, WheelEvent,
};
use presto_base::geom::{Point, Rect};
use std::path::PathBuf;

/// What the translator did with one presto event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Routed {
    /// The island took the event: the host doesn't dispatch it further.
    pub consumed: bool,
    /// egui events were queued: the island needs a pass.
    pub queued: bool,
    /// A paste shortcut: the island reads the clipboard and queues `Event::Paste`.
    pub paste: bool,
}

/// Files dragged over or dropped on the island from the OS (egui's `RawInput::hovered_files` and
/// `dropped_files`).
#[derive(Debug, Default)]
pub struct FileDrops {
    pub hovered: Vec<PathBuf>,
    pub dropped: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct InputTranslator {
    /// The island's rect in window points.
    rect: Rect,
    /// The pointer is over the island (as egui last heard).
    inside: bool,
    /// Buttons pressed inside the island and still held (bit per [`button_bit`]).
    captured: u32,
    modifiers: Modifiers,
    focused: bool,
    macos: bool,
    pub files: FileDrops,
}

impl InputTranslator {
    pub fn new(rect: Rect) -> Self {
        InputTranslator {
            rect,
            inside: false,
            captured: 0,
            modifiers: Modifiers::default(),
            focused: false,
            macos: cfg!(target_os = "macos"),
            files: FileDrops::default(),
        }
    }

    /// Translate with macOS conventions (Cmd is the command key) regardless of the build target.
    pub fn with_macos(mut self, macos: bool) -> Self {
        self.macos = macos;
        self
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn pointer_inside(&self) -> bool {
        self.inside
    }

    /// A button pressed inside the island is held.
    pub fn captured(&self) -> bool {
        self.captured != 0
    }

    /// Keyboard focus entered or left the island.
    pub fn set_focused(&mut self, focused: bool, out: &mut Vec<egui::Event>) {
        if self.focused != focused {
            self.focused = focused;
            out.push(egui::Event::WindowFocused(focused));
        }
    }

    pub fn egui_modifiers(&self) -> egui::Modifiers {
        modifiers(self.modifiers, self.macos)
    }

    fn local(&self, p: Point) -> egui::Pos2 {
        egui::pos2(p.x - self.rect.min.x, p.y - self.rect.min.y)
    }

    pub fn translate(&mut self, ev: &InputEvent, out: &mut Vec<egui::Event>) -> Routed {
        let before = out.len();
        let mut routed = Routed::default();
        match ev {
            InputEvent::Pointer(p) => routed.consumed = self.pointer(p, out),
            InputEvent::Wheel(w) => routed.consumed = self.wheel(w, out),
            InputEvent::Key(k) => {
                self.modifiers = k.modifiers;
                if self.focused {
                    routed.consumed = true;
                    routed.paste = self.key(k, out);
                }
            }
            InputEvent::Ime(ime) => {
                if self.focused {
                    routed.consumed = true;
                    if let Some(e) = ime_event(ime) {
                        out.push(egui::Event::Ime(e));
                    }
                }
            }
            InputEvent::Gesture(g) => {
                if self.inside {
                    routed.consumed = true;
                    match *g {
                        // egui-winit's mapping of winit's `PinchGesture` (positive = magnify).
                        GestureEvent::Pinch { delta, .. } => out.push(egui::Event::Zoom(delta.exp())),
                        GestureEvent::Rotate { delta_rad, .. } => out.push(egui::Event::Rotate(delta_rad)),
                        GestureEvent::DoubleTap => {}
                    }
                }
            }
            InputEvent::ModifiersChanged(m) => {
                self.modifiers = *m;
                out.push(egui::Event::ModifiersChanged(modifiers(*m, self.macos)));
            }
            InputEvent::FileHovered(path) => {
                if self.inside {
                    routed.consumed = true;
                    self.files.hovered.push(path.clone());
                }
            }
            InputEvent::FileDropped(path) => {
                if self.inside {
                    routed.consumed = true;
                    self.files.hovered.clear();
                    self.files.dropped.push(path.clone());
                }
            }
            InputEvent::FileHoverCancelled => self.files.hovered.clear(),
        }
        routed.queued = out.len() > before || routed.paste || matches!(ev, InputEvent::FileHovered(_) | InputEvent::FileDropped(_) | InputEvent::FileHoverCancelled);
        routed
    }

    fn pointer(&mut self, p: &PointerEvent, out: &mut Vec<egui::Event>) -> bool {
        self.modifiers = p.modifiers;
        let over = self.rect.contains(p.pos);
        let pos = self.local(p.pos);
        let mods = modifiers(p.modifiers, self.macos);
        if p.kind == PointerType::Touch {
            if let Some(phase) = touch_phase(p.phase) {
                if over || self.captured != 0 {
                    out.push(egui::Event::Touch {
                        device_id: egui::TouchDeviceId(0),
                        id: egui::TouchId(u64::from(p.pointer)),
                        phase,
                        pos,
                        force: Some(p.pressure),
                    });
                }
            }
        }
        match p.phase {
            PointerPhase::Move | PointerPhase::Enter => {
                if over || self.captured != 0 {
                    self.inside = over;
                    out.push(egui::Event::PointerMoved(pos));
                    true
                } else {
                    self.gone(out);
                    false
                }
            }
            PointerPhase::Down => {
                if !over {
                    return false;
                }
                self.inside = true;
                out.push(egui::Event::PointerMoved(pos));
                if let Some((button, bit)) = p.button.and_then(pointer_button) {
                    self.captured |= bit;
                    out.push(egui::Event::PointerButton { pos, button, pressed: true, modifiers: mods });
                }
                true
            }
            PointerPhase::Up => {
                let bit = p.button.and_then(pointer_button);
                let held = bit.is_some_and(|(_, b)| self.captured & b != 0);
                if !over && !held {
                    return false;
                }
                if let Some((button, b)) = bit {
                    self.captured &= !b;
                    out.push(egui::Event::PointerButton { pos, button, pressed: false, modifiers: mods });
                }
                if !over && self.captured == 0 {
                    self.gone(out);
                }
                true
            }
            PointerPhase::Leave => {
                if self.captured == 0 {
                    self.gone(out);
                }
                false
            }
            PointerPhase::Cancel => {
                // The OS took the pointer (a system gesture, a lost capture): egui must not keep a
                // button held, so release what it holds where it last saw the pointer.
                let held = std::mem::take(&mut self.captured);
                for (button, bit) in BUTTONS {
                    if held & bit != 0 {
                        out.push(egui::Event::PointerButton { pos, button, pressed: false, modifiers: mods });
                    }
                }
                self.gone(out);
                held != 0
            }
        }
    }

    fn gone(&mut self, out: &mut Vec<egui::Event>) {
        if self.inside {
            self.inside = false;
            out.push(egui::Event::PointerGone);
        }
    }

    fn wheel(&mut self, w: &WheelEvent, out: &mut Vec<egui::Event>) -> bool {
        self.modifiers = w.modifiers;
        if !self.rect.contains(w.pos) {
            return false;
        }
        if !self.inside {
            self.inside = true;
            out.push(egui::Event::PointerMoved(self.local(w.pos)));
        }
        // presto's wheel deltas have winit's sign (positive y: wheel away from the user), which is
        // what egui-winit passes to egui unchanged.
        let (unit, d) = match w.delta {
            ScrollDelta::Lines(d) => (egui::MouseWheelUnit::Line, d),
            ScrollDelta::Pixels(d) => (egui::MouseWheelUnit::Point, d),
            ScrollDelta::Pages(d) => (egui::MouseWheelUnit::Page, d),
        };
        let (phase, source) = match (w.momentum, w.phase) {
            (Some(m), _) => (gesture_phase(m), egui::MouseWheelSource::Momentum),
            (None, Some(p)) => (gesture_phase(p), egui::MouseWheelSource::Trackpad),
            (None, None) => (egui::TouchPhase::Move, egui::MouseWheelSource::Wheel),
        };
        out.push(egui::Event::MouseWheel {
            unit,
            delta: egui::vec2(d.x, d.y),
            phase,
            source,
            modifiers: modifiers(w.modifiers, self.macos),
        });
        true
    }

    /// Returns `true` for a paste shortcut (the caller reads the clipboard).
    fn key(&mut self, k: &KeyEvent, out: &mut Vec<egui::Event>) -> bool {
        let pressed = k.state == KeyState::Pressed;
        let mods = modifiers(k.modifiers, self.macos);
        if let Some(key) = k.key.and_then(egui_key) {
            if pressed {
                if is_cut(mods, key) {
                    out.push(egui::Event::Cut);
                    return false;
                }
                if is_copy(mods, key) {
                    out.push(egui::Event::Copy);
                    return false;
                }
                if is_paste(mods, key) {
                    return true;
                }
            }
            // presto's physical code is a USB HID usage, which egui has no names for.
            out.push(egui::Event::Key { key, physical_key: None, pressed, repeat: k.repeat, modifiers: mods });
        }
        if let Some(text) = &k.text {
            let is_cmd = mods.ctrl || mods.command || mods.mac_cmd;
            if pressed && !is_cmd && !text.is_empty() && text.chars().all(is_printable_char) {
                out.push(egui::Event::Text(text.clone()));
            }
        }
        false
    }
}

/// presto and egui name keys identically (`presto_base::key` copies egui's table).
pub fn egui_key(k: presto_base::key::Key) -> Option<egui::Key> {
    egui::Key::from_name(k.name())
}

pub fn modifiers(m: Modifiers, macos: bool) -> egui::Modifiers {
    let ctrl = m.contains(Modifiers::CTRL);
    let sup = m.contains(Modifiers::SUPER);
    egui::Modifiers {
        alt: m.contains(Modifiers::ALT) || m.contains(Modifiers::ALT_GR),
        ctrl,
        shift: m.contains(Modifiers::SHIFT),
        mac_cmd: macos && sup,
        command: if macos { sup } else { ctrl },
    }
}

const BUTTONS: [(egui::PointerButton, u32); 5] = [
    (egui::PointerButton::Primary, 1),
    (egui::PointerButton::Secondary, 2),
    (egui::PointerButton::Middle, 4),
    (egui::PointerButton::Extra1, 8),
    (egui::PointerButton::Extra2, 16),
];

/// The egui button and its capture bit (presto's `buttons` bits for the first three).
fn pointer_button(b: MouseButton) -> Option<(egui::PointerButton, u32)> {
    let i = match b {
        MouseButton::Primary => 0,
        MouseButton::Secondary => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(_) => return None,
    };
    Some(BUTTONS[i])
}

fn touch_phase(p: PointerPhase) -> Option<egui::TouchPhase> {
    Some(match p {
        PointerPhase::Down => egui::TouchPhase::Start,
        PointerPhase::Move => egui::TouchPhase::Move,
        PointerPhase::Up => egui::TouchPhase::End,
        PointerPhase::Cancel => egui::TouchPhase::Cancel,
        PointerPhase::Enter | PointerPhase::Leave => return None,
    })
}

fn gesture_phase(p: GesturePhase) -> egui::TouchPhase {
    match p {
        GesturePhase::Started => egui::TouchPhase::Start,
        GesturePhase::Moved => egui::TouchPhase::Move,
        GesturePhase::Ended => egui::TouchPhase::End,
        GesturePhase::Cancelled => egui::TouchPhase::Cancel,
    }
}

/// presto's IME events carry byte offsets; egui's preedit range is in chars. `DeleteSurrounding`
/// counts bytes of text the island can't see, so it can't become egui's char counts and is dropped
/// (egui's text edits never ask for surrounding-text deletion; only text-input-v3 sends it).
fn ime_event(e: &ImeEvent) -> Option<egui::ImeEvent> {
    match e {
        ImeEvent::Preedit { text, cursor } => {
            let chars = |byte: usize| text.get(..byte).map(|s| s.chars().count());
            let active_range_chars = cursor.and_then(|(a, b)| Some(chars(a)?..chars(b)?));
            Some(egui::ImeEvent::Preedit { text: text.clone(), active_range_chars })
        }
        ImeEvent::Commit(text) => Some(egui::ImeEvent::Commit(text.clone())),
        ImeEvent::Enabled | ImeEvent::Disabled | ImeEvent::DeleteSurrounding { .. } => None,
    }
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_in_private_use_area && !chr.is_ascii_control()
}

fn is_cut(m: egui::Modifiers, k: egui::Key) -> bool {
    k == egui::Key::Cut
        || (m.command_only() && k == egui::Key::X)
        || (cfg!(target_os = "windows") && m.shift && k == egui::Key::Delete)
}

fn is_copy(m: egui::Modifiers, k: egui::Key) -> bool {
    k == egui::Key::Copy
        || (m.command_only() && k == egui::Key::C)
        || (cfg!(target_os = "windows") && m.ctrl && k == egui::Key::Insert)
}

fn is_paste(m: egui::Modifiers, k: egui::Key) -> bool {
    k == egui::Key::Paste
        || (m.command_only() && k == egui::Key::V)
        || (cfg!(target_os = "windows") && m.shift && k == egui::Key::Insert)
}

/// egui's cursor for presto's (CSS names).
pub fn cursor(c: egui::CursorIcon) -> presto_base::cursor::CursorIcon {
    use egui::CursorIcon as E;
    use presto_base::cursor::CursorIcon as P;
    match c {
        E::Default => P::Default,
        E::None => P::None,
        E::ContextMenu => P::ContextMenu,
        E::Help => P::Help,
        E::PointingHand => P::Pointer,
        E::Progress => P::Progress,
        E::Wait => P::Wait,
        E::Cell => P::Cell,
        E::Crosshair => P::Crosshair,
        E::Text => P::Text,
        E::VerticalText => P::VerticalText,
        E::Alias => P::Alias,
        E::Copy => P::Copy,
        E::Move => P::Move,
        E::NoDrop => P::NoDrop,
        E::NotAllowed => P::NotAllowed,
        E::Grab => P::Grab,
        E::Grabbing => P::Grabbing,
        E::AllScroll => P::AllScroll,
        E::ResizeHorizontal => P::EwResize,
        E::ResizeNeSw => P::NeswResize,
        E::ResizeNwSe => P::NwseResize,
        E::ResizeVertical => P::NsResize,
        E::ResizeEast => P::EResize,
        E::ResizeSouthEast => P::SeResize,
        E::ResizeSouth => P::SResize,
        E::ResizeSouthWest => P::SwResize,
        E::ResizeWest => P::WResize,
        E::ResizeNorthWest => P::NwResize,
        E::ResizeNorth => P::NResize,
        E::ResizeNorthEast => P::NeResize,
        E::ResizeColumn => P::ColResize,
        E::ResizeRow => P::RowResize,
        E::ZoomIn => P::ZoomIn,
        E::ZoomOut => P::ZoomOut,
    }
}
