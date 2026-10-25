//! Seat input: pointer, keyboard, touch, tablet tools, pinch gestures, text input, cursors and key repeat.
//!
//! Each `wl_seat` capability gets its device, dispatched here into the pure assemblers of `frames.rs` and the
//! xkb state of `xkb.rs`; the assembled events are queued for the window they hit. Presses on the window's
//! client-side decorations are turned into xdg-shell requests first (`csd.rs`). Cursors use
//! `wp_cursor_shape_v1` where the compositor has it and the user's xcursor theme otherwise.

use crate::app::State;
use crate::csd::{self, CsdAction};
use crate::frames::{
    Assembled, Axis, AxisSource, PinchAssembler, PointerAssembler, RawPointer, RawTool, RawTouch, ToolAssembler, TouchAssembler,
};
use crate::ime::{self, DoneBatch};
use crate::keymap;
use crate::shim::HitKind;
use crate::time;
use crate::xkb::Xkb;
use presto_base::cursor::CursorIcon;
use presto_base::event::{ImeEvent, InputEvent, KeyEvent, KeyState, Modifiers, PointerPhase, WindowEvent};
use presto_base::geom::{Point, Vec2};
use presto_base::ids::WindowId;
use presto_base::time::{Span, Time};
use smithay_client_toolkit::seat::Capability;
use wayland_client::protocol::wl_keyboard::{self, KeymapFormat, WlKeyboard};
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::protocol::wl_touch::{self, WlTouch};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, event_created_child};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_pinch_v1::{self, ZwpPointerGesturePinchV1};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_pad_group_v2::{self, ZwpTabletPadGroupV2},
    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2,
    zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
    zwp_tablet_pad_v2::{self, ZwpTabletPadV2},
    zwp_tablet_seat_v2::{self, ZwpTabletSeatV2},
    zwp_tablet_tool_v2::{self, ZwpTabletToolV2},
    zwp_tablet_v2::ZwpTabletV2,
};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{self, ZwpTextInputV3};

/// User data of every seat-owned object: the seat's key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeatKey(pub u32);

pub struct PointerDev {
    pub pointer: WlPointer,
    pub asm: PointerAssembler<WindowId>,
    pub enter_serial: u32,
    pub shape: Option<WpCursorShapeDeviceV1>,
    pub themed: Option<ThemedCursor>,
    pub pinch: Option<(ZwpPointerGesturePinchV1, PinchAssembler, Option<WindowId>)>,
    /// The cursor last set, to skip redundant requests.
    pub shown: Option<CursorIcon>,
    /// The CSD region under the pointer, when it overrides the cursor.
    pub csd_hover: Option<HitKind>,
}

pub struct KeyboardDev {
    pub keyboard: WlKeyboard,
    pub xkb: Option<Xkb>,
    pub focus: Option<WindowId>,
    pub repeat: Option<Repeat>,
    pub rate: i32,
    pub delay: i32,
    /// The compositor sends `repeated` key states itself (wl_keyboard v10): no client-side repeat.
    pub server_repeat: bool,
}

pub struct Repeat {
    pub evdev: u32,
    pub next: Time,
    pub interval: Span,
}

pub struct Tool {
    pub tool: ZwpTabletToolV2,
    pub asm: ToolAssembler<WindowId>,
    pub shape: Option<WpCursorShapeDeviceV1>,
}

pub struct TextInput {
    pub ti: ZwpTextInputV3,
    pub focus: Option<WindowId>,
    pub enabled: bool,
    pub batch: DoneBatch,
    pub preedit: bool,
}

pub struct Seat {
    pub key: SeatKey,
    pub seat: WlSeat,
    pub pointer: Option<PointerDev>,
    pub keyboard: Option<KeyboardDev>,
    pub touch: Option<(WlTouch, TouchAssembler<WindowId>)>,
    pub tablet_seat: Option<ZwpTabletSeatV2>,
    pub tools: Vec<Tool>,
    pub text_input: Option<TextInput>,
    pub modifiers: Modifiers,
    /// The latest press serial and a global order, for move/resize/popup grabs/drags.
    pub last_press: Option<(u32, u64)>,
}

static PRESS_ORDER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Seat {
    fn note_press(&mut self, serial: u32) {
        self.last_press = Some((serial, PRESS_ORDER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    }
}

fn seat_mut(state: &mut State, key: SeatKey) -> Option<&mut Seat> {
    state.seats.iter_mut().find(|s| s.key == key)
}

/// The seat and serial of the most recent press on any seat.
pub fn latest_press(state: &State) -> Option<(WlSeat, u32)> {
    state.seats.iter().filter_map(|s| s.last_press.map(|(serial, order)| (order, s.seat.clone(), serial))).max_by_key(|x| x.0).map(|(_, seat, serial)| (seat, serial))
}

pub fn add_seat(state: &mut State, qh: &QueueHandle<State>, seat: WlSeat) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let key = SeatKey(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let text_input = state.protocols.text_input.as_ref().map(|m| TextInput {
        ti: m.get_text_input(&seat, qh, key),
        focus: None,
        enabled: false,
        batch: DoneBatch::default(),
        preedit: false,
    });
    let tablet_seat = state.protocols.tablet.as_ref().map(|m| m.get_tablet_seat(&seat, qh, key));
    crate::data::add_seat(state, qh, &seat);
    state.seats.push(Seat {
        key,
        seat,
        pointer: None,
        keyboard: None,
        touch: None,
        tablet_seat,
        tools: Vec::new(),
        text_input,
        modifiers: Modifiers::default(),
        last_press: None,
    });
}

pub fn remove_seat(state: &mut State, seat: &WlSeat) {
    crate::data::remove_seat(state, seat);
    if let Some(i) = state.seats.iter().position(|s| &s.seat == seat) {
        let s = state.seats.remove(i);
        release_pointer(s.pointer);
        if let Some(k) = s.keyboard {
            if k.keyboard.version() >= 3 {
                k.keyboard.release();
            }
        }
        if let Some((t, _)) = s.touch {
            if t.version() >= 3 {
                t.release();
            }
        }
        for t in s.tools {
            if let Some(sh) = t.shape {
                sh.destroy();
            }
            t.tool.destroy();
        }
        if let Some(ts) = s.tablet_seat {
            ts.destroy();
        }
        if let Some(ti) = s.text_input {
            ti.ti.destroy();
        }
    }
}

fn release_pointer(p: Option<PointerDev>) {
    let Some(p) = p else { return };
    if let Some(sh) = p.shape {
        sh.destroy();
    }
    if let Some((pinch, _, _)) = p.pinch {
        pinch.destroy();
    }
    if p.pointer.version() >= 3 {
        p.pointer.release();
    }
}

pub fn add_capability(state: &mut State, qh: &QueueHandle<State>, seat: &WlSeat, cap: Capability) {
    let shape_mgr = state.protocols.cursor_shape.clone();
    let gestures = state.protocols.gestures.clone();
    let Some(s) = state.seats.iter_mut().find(|s| &s.seat == seat) else { return };
    let key = s.key;
    match cap {
        Capability::Pointer if s.pointer.is_none() => {
            let pointer = seat.get_pointer(qh, key);
            let shape = shape_mgr.as_ref().map(|m| m.get_pointer(&pointer, qh, ()));
            let pinch = gestures.as_ref().map(|g| (g.get_pinch_gesture(&pointer, qh, key), PinchAssembler::default(), None));
            let framed = pointer.version() >= 5;
            s.pointer = Some(PointerDev {
                pointer,
                asm: PointerAssembler::new(framed),
                enter_serial: 0,
                shape,
                themed: None,
                pinch,
                shown: None,
                csd_hover: None,
            });
        }
        Capability::Keyboard if s.keyboard.is_none() => {
            s.keyboard = Some(KeyboardDev {
                keyboard: seat.get_keyboard(qh, key),
                xkb: Xkb::new(),
                focus: None,
                repeat: None,
                rate: 25,
                delay: 600,
                server_repeat: false,
            });
        }
        Capability::Touch if s.touch.is_none() => {
            s.touch = Some((seat.get_touch(qh, key), TouchAssembler::default()));
        }
        _ => {}
    }
}

pub fn remove_capability(state: &mut State, seat: &WlSeat, cap: Capability) {
    let Some(s) = state.seats.iter_mut().find(|s| &s.seat == seat) else { return };
    match cap {
        Capability::Pointer => release_pointer(s.pointer.take()),
        Capability::Keyboard => {
            if let Some(k) = s.keyboard.take() {
                if k.keyboard.version() >= 3 {
                    k.keyboard.release();
                }
            }
        }
        Capability::Touch => {
            if let Some((t, _)) = s.touch.take() {
                if t.version() >= 3 {
                    t.release();
                }
            }
        }
        _ => {}
    }
}

// ---- pointer ----

fn axis(a: WEnum<wl_pointer::Axis>) -> Option<Axis> {
    match a {
        WEnum::Value(wl_pointer::Axis::VerticalScroll) => Some(Axis::Vertical),
        WEnum::Value(wl_pointer::Axis::HorizontalScroll) => Some(Axis::Horizontal),
        _ => None,
    }
}

impl Dispatch<WlPointer, SeatKey> for State {
    fn event(state: &mut Self, _: &WlPointer, event: wl_pointer::Event, key: &SeatKey, _: &Connection, _: &QueueHandle<Self>) {
        let now = time::now();
        let raw = match event {
            wl_pointer::Event::Enter { serial, surface, surface_x, surface_y } => {
                let Some(id) = state.window_of(&surface) else { return };
                if let Some(p) = seat_mut(state, *key).and_then(|s| s.pointer.as_mut()) {
                    p.enter_serial = serial;
                    p.shown = None;
                }
                RawPointer::Enter { surface: id, pos: Point::new(surface_x as f32, surface_y as f32) }
            }
            wl_pointer::Event::Leave { surface, .. } => {
                let Some(id) = state.window_of(&surface) else { return };
                RawPointer::Leave { surface: id }
            }
            wl_pointer::Event::Motion { time: ms, surface_x, surface_y } => {
                RawPointer::Motion { time: time::from_event_ms(ms, now), pos: Point::new(surface_x as f32, surface_y as f32) }
            }
            wl_pointer::Event::Button { serial, time: ms, button, state: bstate } => {
                let pressed = matches!(bstate, WEnum::Value(wl_pointer::ButtonState::Pressed));
                if pressed {
                    if let Some(s) = seat_mut(state, *key) {
                        s.note_press(serial);
                    }
                }
                RawPointer::Button { time: time::from_event_ms(ms, now), button, pressed }
            }
            wl_pointer::Event::Axis { time: ms, axis: a, value } => {
                let Some(axis) = axis(a) else { return };
                RawPointer::Axis { time: time::from_event_ms(ms, now), axis, value }
            }
            wl_pointer::Event::AxisSource { axis_source } => {
                let src = match axis_source {
                    WEnum::Value(wl_pointer::AxisSource::Wheel) => AxisSource::Wheel,
                    WEnum::Value(wl_pointer::AxisSource::Finger) => AxisSource::Finger,
                    WEnum::Value(wl_pointer::AxisSource::Continuous) => AxisSource::Continuous,
                    WEnum::Value(wl_pointer::AxisSource::WheelTilt) => AxisSource::WheelTilt,
                    _ => return,
                };
                RawPointer::AxisSource(src)
            }
            wl_pointer::Event::AxisStop { time: ms, axis: a } => {
                let Some(axis) = axis(a) else { return };
                RawPointer::AxisStop { time: time::from_event_ms(ms, now), axis }
            }
            wl_pointer::Event::AxisDiscrete { axis: a, discrete } => {
                let Some(axis) = axis(a) else { return };
                RawPointer::AxisDiscrete { axis, discrete }
            }
            wl_pointer::Event::AxisValue120 { axis: a, value120 } => {
                let Some(axis) = axis(a) else { return };
                RawPointer::AxisValue120 { axis, value120 }
            }
            wl_pointer::Event::Frame => RawPointer::Frame,
            _ => return,
        };
        let Some(seat) = seat_mut(state, *key) else { return };
        let mods = seat.modifiers;
        let Some(p) = seat.pointer.as_mut() else { return };
        let out = p.asm.push(raw, mods, now);
        if !out.is_empty() {
            deliver_pointer(state, *key, out, now);
        }
    }
}

/// Routes assembled pointer events through the window's decorations, then to the app.
fn deliver_pointer(state: &mut State, key: SeatKey, events: impl IntoIterator<Item = Assembled<WindowId>>, now: Time) {
    for a in events {
        let mut consumed = false;
        if let InputEvent::Pointer(p) = &a.event {
            let Some(w) = state.windows.get_mut(&a.surface) else { continue };
            let regions_active = w.toplevel().is_some() && !w.hit_regions.is_empty();
            if regions_active {
                let action = match (p.phase, p.button) {
                    (PointerPhase::Down, Some(presto_base::event::MouseButton::Primary)) => {
                        let (act, eat) = w.csd.press(&w.hit_regions, p.pos, true, now);
                        consumed = eat;
                        act
                    }
                    (PointerPhase::Down, Some(presto_base::event::MouseButton::Secondary)) => {
                        let (act, eat) = w.csd.press(&w.hit_regions, p.pos, false, now);
                        consumed = eat;
                        act
                    }
                    (PointerPhase::Up, Some(presto_base::event::MouseButton::Primary)) => w.csd.release(&w.hit_regions, p.pos),
                    _ => None,
                };
                let hover = match p.phase {
                    PointerPhase::Leave => None,
                    _ => csd::hit(&w.hit_regions, p.pos).filter(|k| csd::cursor_for(*k).is_some()),
                };
                if let Some(action) = action {
                    run_csd_action(state, key, a.surface, action, p.pos);
                }
                if let Some(ptr) = seat_mut(state, key).and_then(|s| s.pointer.as_mut()) {
                    if ptr.csd_hover != hover {
                        ptr.csd_hover = hover;
                        ptr.shown = None;
                    }
                }
            }
            if matches!(p.phase, PointerPhase::Enter | PointerPhase::Move) {
                update_cursor(state, key, a.surface);
            }
        }
        if !consumed {
            state.push_input(a.surface, a.time, a.event);
        }
    }
}

fn run_csd_action(state: &mut State, key: SeatKey, id: WindowId, action: CsdAction, pos: Point) {
    let Some(seat) = state.seats.iter().find(|s| s.key == key) else { return };
    let wl_seat = seat.seat.clone();
    let serial = seat.last_press.map(|(s, _)| s).unwrap_or(0);
    let Some(w) = state.windows.get(&id) else { return };
    let Some(top) = w.toplevel() else { return };
    match action {
        CsdAction::Move => top.move_(&wl_seat, serial),
        CsdAction::Resize(kind) => {
            if let Some(edge) = csd::resize_edge(kind) {
                top.resize(&wl_seat, serial, edge);
            }
        }
        CsdAction::WindowMenu => top.show_window_menu(&wl_seat, serial, (pos.x as i32, pos.y as i32)),
        CsdAction::ToggleMaximize => {
            if w.states.contains(smithay_client_toolkit::reexports::csd_frame::WindowState::MAXIMIZED) {
                top.unset_maximized();
            } else {
                top.set_maximized();
            }
        }
        CsdAction::Minimize => top.set_minimized(),
        CsdAction::Close => state.push_window(id, WindowEvent::CloseRequested),
    }
}

/// Shows the cursor `window` wants (or its CSD region's) for the seat's pointer.
pub fn update_cursor(state: &mut State, key: SeatKey, window: WindowId) {
    let Some(want) = state.windows.get(&window).map(|w| w.cursor) else { return };
    let (conn, qh) = (state.conn.clone(), state.qh.clone());
    let shm = state.shm.as_ref().map(|s| s.wl_shm().clone());
    let compositor = state.compositor.wl_compositor().clone();
    let scale = state.windows.get(&window).map_or(1, |w| w.scale().ceil() as i32);
    let Some(p) = seat_mut(state, key).and_then(|s| s.pointer.as_mut()) else { return };
    if p.asm.focus() != Some(window) {
        return;
    }
    let icon = p.csd_hover.and_then(csd::cursor_for).unwrap_or(want);
    if p.shown == Some(icon) {
        return;
    }
    p.shown = Some(icon);
    if icon == CursorIcon::None {
        p.pointer.set_cursor(p.enter_serial, None, 0, 0);
        return;
    }
    if let Some(shape) = &p.shape {
        shape.set_shape(p.enter_serial, cursor_shape(icon));
        return;
    }
    let Some(shm) = shm else { return };
    let themed = p.themed.get_or_insert_with(|| ThemedCursor::new(&conn, &qh, shm, &compositor));
    themed.show(&p.pointer, p.enter_serial, icon, scale);
}

pub fn cursor_shape(c: CursorIcon) -> Shape {
    match c {
        CursorIcon::Default | CursorIcon::None => Shape::Default,
        CursorIcon::ContextMenu => Shape::ContextMenu,
        CursorIcon::Help => Shape::Help,
        CursorIcon::Pointer => Shape::Pointer,
        CursorIcon::Progress => Shape::Progress,
        CursorIcon::Wait => Shape::Wait,
        CursorIcon::Cell => Shape::Cell,
        CursorIcon::Crosshair => Shape::Crosshair,
        CursorIcon::Text => Shape::Text,
        CursorIcon::VerticalText => Shape::VerticalText,
        CursorIcon::Alias => Shape::Alias,
        CursorIcon::Copy => Shape::Copy,
        CursorIcon::Move => Shape::Move,
        CursorIcon::NoDrop => Shape::NoDrop,
        CursorIcon::NotAllowed => Shape::NotAllowed,
        CursorIcon::Grab => Shape::Grab,
        CursorIcon::Grabbing => Shape::Grabbing,
        CursorIcon::EResize => Shape::EResize,
        CursorIcon::NResize => Shape::NResize,
        CursorIcon::NeResize => Shape::NeResize,
        CursorIcon::NwResize => Shape::NwResize,
        CursorIcon::SResize => Shape::SResize,
        CursorIcon::SeResize => Shape::SeResize,
        CursorIcon::SwResize => Shape::SwResize,
        CursorIcon::WResize => Shape::WResize,
        CursorIcon::EwResize => Shape::EwResize,
        CursorIcon::NsResize => Shape::NsResize,
        CursorIcon::NeswResize => Shape::NeswResize,
        CursorIcon::NwseResize => Shape::NwseResize,
        CursorIcon::ColResize => Shape::ColResize,
        CursorIcon::RowResize => Shape::RowResize,
        CursorIcon::AllScroll => Shape::AllScroll,
        CursorIcon::ZoomIn => Shape::ZoomIn,
        CursorIcon::ZoomOut => Shape::ZoomOut,
    }
}

/// The xcursor names to try for a cursor: its CSS name (freedesktop cursor-spec themes have them), then the
/// legacy X11 name.
pub fn xcursor_names(c: CursorIcon) -> &'static [&'static str] {
    match c {
        CursorIcon::Default | CursorIcon::None => &["default", "left_ptr"],
        CursorIcon::ContextMenu => &["context-menu", "left_ptr"],
        CursorIcon::Help => &["help", "question_arrow"],
        CursorIcon::Pointer => &["pointer", "hand2"],
        CursorIcon::Progress => &["progress", "left_ptr_watch"],
        CursorIcon::Wait => &["wait", "watch"],
        CursorIcon::Cell => &["cell", "plus"],
        CursorIcon::Crosshair => &["crosshair", "cross"],
        CursorIcon::Text => &["text", "xterm"],
        CursorIcon::VerticalText => &["vertical-text", "xterm"],
        CursorIcon::Alias => &["alias", "dnd-link"],
        CursorIcon::Copy => &["copy", "dnd-copy"],
        CursorIcon::Move => &["move", "fleur"],
        CursorIcon::NoDrop => &["no-drop", "dnd-none"],
        CursorIcon::NotAllowed => &["not-allowed", "crossed_circle"],
        CursorIcon::Grab => &["grab", "openhand", "hand1"],
        CursorIcon::Grabbing => &["grabbing", "closedhand", "fleur"],
        CursorIcon::EResize => &["e-resize", "right_side"],
        CursorIcon::NResize => &["n-resize", "top_side"],
        CursorIcon::NeResize => &["ne-resize", "top_right_corner"],
        CursorIcon::NwResize => &["nw-resize", "top_left_corner"],
        CursorIcon::SResize => &["s-resize", "bottom_side"],
        CursorIcon::SeResize => &["se-resize", "bottom_right_corner"],
        CursorIcon::SwResize => &["sw-resize", "bottom_left_corner"],
        CursorIcon::WResize => &["w-resize", "left_side"],
        CursorIcon::EwResize => &["ew-resize", "sb_h_double_arrow"],
        CursorIcon::NsResize => &["ns-resize", "sb_v_double_arrow"],
        CursorIcon::NeswResize => &["nesw-resize", "fd_double_arrow"],
        CursorIcon::NwseResize => &["nwse-resize", "bd_double_arrow"],
        CursorIcon::ColResize => &["col-resize", "sb_h_double_arrow"],
        CursorIcon::RowResize => &["row-resize", "sb_v_double_arrow"],
        CursorIcon::AllScroll => &["all-scroll", "fleur"],
        CursorIcon::ZoomIn => &["zoom-in"],
        CursorIcon::ZoomOut => &["zoom-out"],
    }
}

/// The xcursor theme fallback (`XCURSOR_THEME`, `XCURSOR_SIZE`) for compositors without cursor-shape-v1.
pub struct ThemedCursor {
    conn: Connection,
    shm: wayland_client::protocol::wl_shm::WlShm,
    surface: WlSurface,
    theme: Option<(i32, wayland_cursor::CursorTheme)>,
}

impl ThemedCursor {
    fn new(conn: &Connection, qh: &QueueHandle<State>, shm: wayland_client::protocol::wl_shm::WlShm, compositor: &wayland_client::protocol::wl_compositor::WlCompositor) -> Self {
        ThemedCursor { conn: conn.clone(), shm, surface: compositor.create_surface(qh, smithay_client_toolkit::compositor::SurfaceData::new(None, 1)), theme: None }
    }

    fn show(&mut self, pointer: &WlPointer, serial: u32, icon: CursorIcon, scale: i32) {
        let scale = scale.max(1);
        let base = std::env::var("XCURSOR_SIZE").ok().and_then(|s| s.parse::<u32>().ok()).unwrap_or(24);
        if self.theme.as_ref().is_none_or(|(s, _)| *s != scale) {
            let name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".to_owned());
            match wayland_cursor::CursorTheme::load_from_name(&self.conn, self.shm.clone(), &name, base * scale as u32) {
                Ok(t) => self.theme = Some((scale, t)),
                Err(_) => return,
            }
        }
        let Some((_, theme)) = self.theme.as_mut() else { return };
        let Some(name) = xcursor_names(icon).iter().find(|n| theme.get_cursor(n).is_some()) else { return };
        let Some(cursor) = theme.get_cursor(name) else { return };
        let image = &cursor[0];
        let (hx, hy) = image.hotspot();
        self.surface.set_buffer_scale(scale);
        self.surface.attach(Some(image), 0, 0);
        let (w, h) = image.dimensions();
        self.surface.damage_buffer(0, 0, w as i32, h as i32);
        self.surface.commit();
        pointer.set_cursor(serial, Some(&self.surface), hx as i32 / scale, hy as i32 / scale);
    }
}

impl Drop for ThemedCursor {
    fn drop(&mut self) {
        self.surface.destroy();
    }
}

// ---- keyboard ----

impl Dispatch<WlKeyboard, SeatKey> for State {
    fn event(state: &mut Self, _: &WlKeyboard, event: wl_keyboard::Event, key: &SeatKey, _: &Connection, _: &QueueHandle<Self>) {
        let now = time::now();
        let entered = match &event {
            wl_keyboard::Event::Enter { surface, .. } => state.window_of(surface),
            _ => None,
        };
        let Some(seat) = seat_mut(state, *key) else { return };
        let Some(kb) = seat.keyboard.as_mut() else { return };
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format == WEnum::Value(KeymapFormat::XkbV1) {
                    if let Some(x) = kb.xkb.as_mut() {
                        let _ = x.set_keymap(fd, size);
                    }
                }
            }
            wl_keyboard::Event::Enter { .. } => kb.focus = entered,
            wl_keyboard::Event::Leave { .. } => {
                kb.focus = None;
                kb.repeat = None;
                if let Some(x) = kb.xkb.as_mut() {
                    x.reset_compose();
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                kb.rate = rate;
                kb.delay = delay;
            }
            wl_keyboard::Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                let Some(x) = kb.xkb.as_mut() else { return };
                x.update_mask(mods_depressed, mods_latched, mods_locked, group);
                let mods = x.modifiers();
                let focus = kb.focus;
                if seat.modifiers != mods {
                    seat.modifiers = mods;
                    if let Some(w) = focus {
                        state.push_input(w, now, InputEvent::ModifiersChanged(mods));
                    }
                }
            }
            wl_keyboard::Event::Key { serial, time: ms, key: evdev, state: kstate } => {
                let (pressed, repeated) = match kstate {
                    WEnum::Value(wl_keyboard::KeyState::Pressed) => (true, false),
                    WEnum::Value(wl_keyboard::KeyState::Repeated) => (true, true),
                    _ => (false, false),
                };
                if repeated {
                    kb.server_repeat = true;
                }
                let Some(focus) = kb.focus else { return };
                let os_time = time::from_event_ms(ms, now);
                if pressed && !repeated {
                    seat.note_press(serial);
                }
                let mods = seat.modifiers;
                let Some(kb) = seat.keyboard.as_mut() else { return };
                let Some(ev) = key_event(kb, evdev, pressed, repeated, mods) else { return };
                if pressed && !repeated && !kb.server_repeat && kb.rate > 0 && kb.xkb.as_ref().is_some_and(|x| x.repeats(evdev)) {
                    kb.repeat = Some(Repeat {
                        evdev,
                        next: now + Span::from_millis(kb.delay as i64),
                        interval: Span::from_nanos(1_000_000_000 / kb.rate as i64),
                    });
                } else if !pressed && kb.repeat.as_ref().is_some_and(|r| r.evdev == evdev) {
                    kb.repeat = None;
                }
                state.push_input(focus, os_time, InputEvent::Key(ev));
            }
            _ => {}
        }
    }
}

fn key_event(kb: &mut KeyboardDev, evdev: u32, pressed: bool, repeat: bool, mods: Modifiers) -> Option<KeyEvent> {
    let x = kb.xkb.as_mut()?;
    if !x.has_keymap() {
        return None;
    }
    let (keysym, text) = if pressed {
        let m = x.press(evdev)?;
        (m.keysym, m.text)
    } else {
        (x.keysym(evdev), None)
    };
    Some(KeyEvent {
        key: keymap::logical_key(keysym, evdev),
        physical: keymap::physical_code(evdev),
        text,
        state: if pressed { KeyState::Pressed } else { KeyState::Released },
        repeat,
        location: keymap::key_location(evdev),
        modifiers: mods,
    })
}

/// Fires due client-side key repeats; returns whether any fired.
pub fn tick_repeat(state: &mut State, now: Time) -> bool {
    let mut fired = Vec::new();
    for seat in &mut state.seats {
        let mods = seat.modifiers;
        let Some(kb) = seat.keyboard.as_mut() else { continue };
        let Some(focus) = kb.focus else { continue };
        let Some(r) = kb.repeat.as_ref() else { continue };
        if r.next > now {
            continue;
        }
        let evdev = r.evdev;
        let interval = r.interval;
        let next = if now - r.next > interval + interval { now + interval } else { r.next + interval };
        if let Some(ev) = key_event(kb, evdev, true, true, mods) {
            fired.push((focus, ev));
        }
        if let Some(r) = kb.repeat.as_mut() {
            r.next = next;
        }
    }
    let any = !fired.is_empty();
    for (w, ev) in fired {
        state.push_input(w, now, InputEvent::Key(ev));
    }
    any
}

pub fn next_repeat(state: &State) -> Option<Time> {
    state.seats.iter().filter_map(|s| s.keyboard.as_ref()?.repeat.as_ref().map(|r| r.next)).min()
}

// ---- touch ----

impl Dispatch<WlTouch, SeatKey> for State {
    fn event(state: &mut Self, _: &WlTouch, event: wl_touch::Event, key: &SeatKey, _: &Connection, _: &QueueHandle<Self>) {
        let now = time::now();
        let raw = match event {
            wl_touch::Event::Down { serial, time: ms, surface, id, x, y } => {
                let Some(w) = state.window_of(&surface) else { return };
                if let Some(s) = seat_mut(state, *key) {
                    s.note_press(serial);
                }
                RawTouch::Down { time: time::from_event_ms(ms, now), surface: w, id, pos: Point::new(x as f32, y as f32) }
            }
            wl_touch::Event::Up { time: ms, id, .. } => RawTouch::Up { time: time::from_event_ms(ms, now), id },
            wl_touch::Event::Motion { time: ms, id, x, y } => {
                RawTouch::Motion { time: time::from_event_ms(ms, now), id, pos: Point::new(x as f32, y as f32) }
            }
            wl_touch::Event::Frame => RawTouch::Frame,
            wl_touch::Event::Cancel => RawTouch::Cancel,
            _ => return,
        };
        let Some(seat) = seat_mut(state, *key) else { return };
        let mods = seat.modifiers;
        let Some((_, asm)) = seat.touch.as_mut() else { return };
        for a in asm.push(raw, mods) {
            state.push_input(a.surface, a.time, a.event);
        }
    }
}

// ---- pinch ----

impl Dispatch<ZwpPointerGesturePinchV1, SeatKey> for State {
    fn event(
        state: &mut Self,
        _: &ZwpPointerGesturePinchV1,
        event: zwp_pointer_gesture_pinch_v1::Event,
        key: &SeatKey,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let now = time::now();
        let begin_window = match &event {
            zwp_pointer_gesture_pinch_v1::Event::Begin { surface, .. } => state.window_of(surface),
            _ => None,
        };
        let Some(p) = seat_mut(state, *key).and_then(|s| s.pointer.as_mut()) else { return };
        let Some((_, asm, target)) = p.pinch.as_mut() else { return };
        let (gestures, t) = match event {
            zwp_pointer_gesture_pinch_v1::Event::Begin { time: ms, .. } => {
                *target = begin_window;
                (asm.begin(), ms)
            }
            zwp_pointer_gesture_pinch_v1::Event::Update { time: ms, scale, rotation, .. } => (asm.update(scale as f32, rotation as f32), ms),
            zwp_pointer_gesture_pinch_v1::Event::End { time: ms, cancelled, .. } => (asm.end(cancelled != 0), ms),
            _ => return,
        };
        let Some(w) = *target else { return };
        let os_time = time::from_event_ms(t, now);
        for g in gestures {
            state.push_input(w, os_time, InputEvent::Gesture(g));
        }
    }
}

// ---- tablet ----

impl Dispatch<ZwpTabletSeatV2, SeatKey> for State {
    fn event(state: &mut Self, _: &ZwpTabletSeatV2, event: zwp_tablet_seat_v2::Event, key: &SeatKey, _: &Connection, qh: &QueueHandle<Self>) {
        if let zwp_tablet_seat_v2::Event::ToolAdded { id } = event {
            let shape = state.protocols.cursor_shape.as_ref().map(|m| m.get_tablet_tool_v2(&id, qh, ()));
            let Some(seat) = seat_mut(state, *key) else { return };
            let index = seat.tools.len() as u32;
            seat.tools.push(Tool { tool: id, asm: ToolAssembler::new(index), shape });
        }
    }

    event_created_child!(State, ZwpTabletSeatV2, [
        zwp_tablet_seat_v2::EVT_TABLET_ADDED_OPCODE => (ZwpTabletV2, ()),
        zwp_tablet_seat_v2::EVT_TOOL_ADDED_OPCODE => (ZwpTabletToolV2, ()),
        zwp_tablet_seat_v2::EVT_PAD_ADDED_OPCODE => (ZwpTabletPadV2, ()),
    ]);
}

impl Dispatch<ZwpTabletToolV2, ()> for State {
    fn event(state: &mut Self, tool: &ZwpTabletToolV2, event: zwp_tablet_tool_v2::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        let now = time::now();
        let raw = match event {
            zwp_tablet_tool_v2::Event::ProximityIn { surface, .. } => {
                let Some(w) = state.window_of(&surface) else { return };
                RawTool::ProximityIn { surface: w }
            }
            zwp_tablet_tool_v2::Event::ProximityOut => RawTool::ProximityOut,
            zwp_tablet_tool_v2::Event::Down { serial } => {
                if let Some(s) = state.seats.iter_mut().find(|s| s.tools.iter().any(|t| &t.tool == tool)) {
                    s.note_press(serial);
                }
                RawTool::Down
            }
            zwp_tablet_tool_v2::Event::Up => RawTool::Up,
            zwp_tablet_tool_v2::Event::Motion { x, y } => RawTool::Motion { pos: Point::new(x as f32, y as f32) },
            zwp_tablet_tool_v2::Event::Pressure { pressure } => RawTool::Pressure(pressure as f32 / 65535.0),
            zwp_tablet_tool_v2::Event::Tilt { tilt_x, tilt_y } => RawTool::Tilt(Vec2::new(tilt_x as f32, tilt_y as f32)),
            zwp_tablet_tool_v2::Event::Button { button, state: bstate, .. } => RawTool::Button {
                button,
                pressed: matches!(bstate, WEnum::Value(zwp_tablet_tool_v2::ButtonState::Pressed)),
            },
            zwp_tablet_tool_v2::Event::Frame { time: ms } => RawTool::Frame { time: time::from_event_ms(ms, now) },
            zwp_tablet_tool_v2::Event::Removed => {
                for s in &mut state.seats {
                    if let Some(i) = s.tools.iter().position(|t| &t.tool == tool) {
                        let t = s.tools.remove(i);
                        if let Some(sh) = t.shape {
                            sh.destroy();
                        }
                        t.tool.destroy();
                    }
                }
                return;
            }
            _ => return,
        };
        let Some(seat) = state.seats.iter_mut().find(|s| s.tools.iter().any(|t| &t.tool == tool)) else { return };
        let mods = seat.modifiers;
        let Some(t) = seat.tools.iter_mut().find(|t| &t.tool == tool) else { return };
        for a in t.asm.push(raw, mods) {
            state.push_input(a.surface, a.time, a.event);
        }
    }
}

impl Dispatch<ZwpTabletV2, ()> for State {
    fn event(_: &mut Self, _: &ZwpTabletV2, _: <ZwpTabletV2 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpTabletPadV2, ()> for State {
    fn event(_: &mut Self, _: &ZwpTabletPadV2, _: zwp_tablet_pad_v2::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}

    event_created_child!(State, ZwpTabletPadV2, [
        zwp_tablet_pad_v2::EVT_GROUP_OPCODE => (ZwpTabletPadGroupV2, ()),
    ]);
}

impl Dispatch<ZwpTabletPadGroupV2, ()> for State {
    fn event(_: &mut Self, _: &ZwpTabletPadGroupV2, _: zwp_tablet_pad_group_v2::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}

    event_created_child!(State, ZwpTabletPadGroupV2, [
        zwp_tablet_pad_group_v2::EVT_RING_OPCODE => (ZwpTabletPadRingV2, ()),
        zwp_tablet_pad_group_v2::EVT_STRIP_OPCODE => (ZwpTabletPadStripV2, ()),
    ]);
}

impl Dispatch<ZwpTabletPadRingV2, ()> for State {
    fn event(_: &mut Self, _: &ZwpTabletPadRingV2, _: <ZwpTabletPadRingV2 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpTabletPadStripV2, ()> for State {
    fn event(_: &mut Self, _: &ZwpTabletPadStripV2, _: <ZwpTabletPadStripV2 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

// ---- text input ----

impl Dispatch<ZwpTextInputV3, SeatKey> for State {
    fn event(state: &mut Self, _: &ZwpTextInputV3, event: zwp_text_input_v3::Event, key: &SeatKey, _: &Connection, _: &QueueHandle<Self>) {
        let now = time::now();
        let entered = match &event {
            zwp_text_input_v3::Event::Enter { surface } => state.window_of(surface),
            _ => None,
        };
        let ime_state = entered.and_then(|w| state.windows.get(&w)).and_then(|w| w.ime.clone());
        let Some(seat) = seat_mut(state, *key) else { return };
        let Some(ti) = seat.text_input.as_mut() else { return };
        let mut out: Vec<(WindowId, ImeEvent)> = Vec::new();
        match event {
            zwp_text_input_v3::Event::Enter { .. } => {
                ti.focus = entered;
                if let (Some(w), Some(s)) = (entered, ime_state) {
                    ti.ti.enable();
                    ti.enabled = true;
                    ime::send_state(&ti.ti, &s);
                    out.push((w, ImeEvent::Enabled));
                }
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                if let (Some(w), true) = (ti.focus, ti.enabled) {
                    if ti.preedit {
                        out.push((w, ImeEvent::Preedit { text: String::new(), cursor: None }));
                    }
                    out.push((w, ImeEvent::Disabled));
                    // The compositor disabled it with the leave; this keeps our request count in step.
                    ti.ti.disable();
                    ti.ti.commit();
                }
                ti.focus = None;
                ti.enabled = false;
                ti.preedit = false;
                ti.batch = DoneBatch::default();
            }
            zwp_text_input_v3::Event::PreeditString { text, cursor_begin, cursor_end } => {
                ti.batch.preedit_string(text, cursor_begin, cursor_end);
            }
            zwp_text_input_v3::Event::CommitString { text } => ti.batch.commit_string(text),
            zwp_text_input_v3::Event::DeleteSurroundingText { before_length, after_length } => {
                ti.batch.delete_surrounding(before_length, after_length);
            }
            zwp_text_input_v3::Event::Done { .. } => {
                let batch = std::mem::take(&mut ti.batch);
                let (events, showing) = batch.into_events(ti.preedit);
                ti.preedit = showing;
                if let Some(w) = ti.focus {
                    out.extend(events.into_iter().map(|e| (w, e)));
                }
            }
            _ => {}
        }
        for (w, e) in out {
            state.push_input(w, now, InputEvent::Ime(e));
        }
    }
}

/// Applies `set_ime` for `window` to every seat whose text input is on it.
pub fn update_ime(state: &mut State, window: WindowId) {
    let now = time::now();
    let ime_state = state.windows.get(&window).and_then(|w| w.ime.clone());
    let mut out = Vec::new();
    for seat in &mut state.seats {
        let Some(ti) = seat.text_input.as_mut() else { continue };
        if ti.focus != Some(window) {
            continue;
        }
        match &ime_state {
            Some(s) => {
                if !ti.enabled {
                    ti.ti.enable();
                    ti.enabled = true;
                    out.push(ImeEvent::Enabled);
                }
                ime::send_state(&ti.ti, s);
            }
            None if ti.enabled => {
                ti.ti.disable();
                ti.ti.commit();
                ti.enabled = false;
                if ti.preedit {
                    ti.preedit = false;
                    out.push(ImeEvent::Preedit { text: String::new(), cursor: None });
                }
                out.push(ImeEvent::Disabled);
            }
            None => {}
        }
    }
    for e in out {
        state.push_input(window, now, InputEvent::Ime(e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cursor_has_a_shape_and_xcursor_names() {
        use CursorIcon::*;
        let all = [
            Default, ContextMenu, Help, Pointer, Progress, Wait, Cell, Crosshair, Text, VerticalText, Alias, Copy, Move, NoDrop,
            NotAllowed, Grab, Grabbing, EResize, NResize, NeResize, NwResize, SResize, SeResize, SwResize, WResize, EwResize,
            NsResize, NeswResize, NwseResize, ColResize, RowResize, AllScroll, ZoomIn, ZoomOut, None,
        ];
        for c in all {
            assert!(!xcursor_names(c).is_empty(), "{c:?}");
            let _ = cursor_shape(c);
        }
        assert_eq!(cursor_shape(NwseResize), Shape::NwseResize);
        assert_eq!(xcursor_names(Text)[0], "text");
    }
}
