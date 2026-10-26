//! The data device: clipboard, primary selection, drag and drop, and tab tear-off.
//!
//! - **Clipboard and primary selection** (`wl_data_device`, `zwp_primary_selection_v1`). Setting needs the
//!   serial of a recent input event. Reading asks the selection's owner for UTF-8 text and reads the pipe with
//!   a timeout; when we own the selection the text comes straight from memory (reading our own pipe from the
//!   thread that must answer it would deadlock).
//! - **Drop target.** File drags (`text/uri-list`) become `FileHovered`/`FileDropped`/`FileHoverCancelled`.
//!   During our own drags the compositor holds the pointer grab, so the data device's enter, motion, drop and
//!   leave come back as pointer events (enter, move with the primary button held, up, leave) on the window
//!   under the pointer: the pressed node keeps receiving the pointer across windows (architecture §11.4).
//! - **Drag source and tear-off.** `start_drag` offers the payload's MIME types; with
//!   `xdg-toplevel-drag-v1` a window can be attached to the running drag and the compositor moves it with the
//!   pointer (KWin 6.7, Mutter 51, Jay; `platform.md` §3.3).

use crate::app::{Pending, State};
use crate::seat::latest_press;
use crate::shim::{DragData, DragId, DragOutcome, PlatformError};
use crate::time;
use skelter_base::event::{InputEvent, MouseButton, PointerEvent, PointerPhase, PointerType};
use skelter_base::geom::{Point, Vec2};
use skelter_base::ids::WindowId;
use smallvec::SmallVec;
use smithay_client_toolkit::data_device_manager::data_device::{DataDevice, DataDeviceHandler};
use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use smithay_client_toolkit::data_device_manager::data_source::{CopyPasteSource, DataSourceHandler, DragSource};
use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, WritePipe};
use smithay_client_toolkit::primary_selection::PrimarySelectionManagerState;
use smithay_client_toolkit::primary_selection::device::{PrimarySelectionDevice, PrimarySelectionDeviceHandler};
use smithay_client_toolkit::primary_selection::selection::{PrimarySelectionSource, PrimarySelectionSourceHandler};
use smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::DndAction;
use smithay_client_toolkit::{delegate_data_device, delegate_primary_selection};
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::path::PathBuf;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy, QueueHandle};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;
use wayland_protocols::xdg::toplevel_drag::v1::client::xdg_toplevel_drag_v1::XdgToplevelDragV1;

/// skelter's own drag payload type: drags carrying it are internal (tabs, list items).
pub const SKELTER_DRAG_MIME: &str = "application/x-skelter-drag";
/// Text MIME types, preferred first.
pub const TEXT_MIMES: [&str; 5] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "TEXT", "STRING"];
const URI_LIST: &str = "text/uri-list";
/// How long a paste waits for the selection's owner.
const READ_TIMEOUT_MS: i32 = 500;

struct SeatDevices {
    seat: WlSeat,
    data: Option<DataDevice>,
    primary: Option<PrimarySelectionDevice>,
}

struct ActiveDrag {
    id: DragId,
    source: DragSource,
    data: DragData,
    toplevel_drag: Option<XdgToplevelDragV1>,
}

enum DropKind {
    Files(Vec<PathBuf>),
    Internal,
    Other,
}

struct DropTarget {
    window: WindowId,
    kind: DropKind,
    offer: DragOffer,
}

pub struct DataState {
    manager: Option<DataDeviceManagerState>,
    primary_manager: Option<PrimarySelectionManagerState>,
    devices: Vec<SeatDevices>,
    clipboard: Option<(CopyPasteSource, String)>,
    primary: Option<(PrimarySelectionSource, String)>,
    drag: Option<ActiveDrag>,
    target: Option<DropTarget>,
}

impl DataState {
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<State>) -> Self {
        DataState {
            manager: DataDeviceManagerState::bind(globals, qh).ok(),
            primary_manager: PrimarySelectionManagerState::bind(globals, qh).ok(),
            devices: Vec::new(),
            clipboard: None,
            primary: None,
            drag: None,
            target: None,
        }
    }
}

pub fn add_seat(state: &mut State, qh: &QueueHandle<State>, seat: &WlSeat) {
    let d = &mut state.data;
    let data = d.manager.as_ref().map(|m| m.get_data_device(qh, seat));
    let primary = d.primary_manager.as_ref().map(|m| m.get_selection_device(qh, seat));
    d.devices.push(SeatDevices { seat: seat.clone(), data, primary });
}

pub fn remove_seat(state: &mut State, seat: &WlSeat) {
    state.data.devices.retain(|d| &d.seat != seat);
}

fn device_for<'a>(d: &'a DataState, seat: &WlSeat) -> Option<&'a SeatDevices> {
    d.devices.iter().find(|x| &x.seat == seat)
}

/// The device of the seat with the latest input, else the first.
fn current_device(state: &State) -> Option<(&SeatDevices, Option<u32>)> {
    match latest_press(state) {
        Some((seat, serial)) => device_for(&state.data, &seat).map(|d| (d, Some(serial))),
        None => state.data.devices.first().map(|d| (d, None)),
    }
}

/// Reads a pipe to the end, waiting at most `READ_TIMEOUT_MS` for each chunk.
fn read_pipe(mut pipe: impl Read + AsFd) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let ready = {
            let mut fds = [rustix::event::PollFd::new(&pipe, rustix::event::PollFlags::IN)];
            let ts = rustix::event::Timespec { tv_sec: 0, tv_nsec: READ_TIMEOUT_MS as i64 * 1_000_000 };
            matches!(rustix::event::poll(&mut fds, Some(&ts)), Ok(n) if n > 0)
        };
        if !ready {
            return None;
        }
        match pipe.read(&mut buf) {
            Ok(0) => return Some(out),
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted || e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return None,
        }
    }
}

/// Writes `bytes` on a helper thread, so a slow reader never blocks the event loop.
fn write_async(mut pipe: WritePipe, bytes: Vec<u8>) {
    std::thread::spawn(move || {
        let _ = pipe.write_all(&bytes);
    });
}

fn pick_text_mime(offered: &[String]) -> Option<String> {
    TEXT_MIMES.iter().find(|m| offered.iter().any(|o| o == *m)).map(|m| (*m).to_owned())
}

pub fn get_clipboard(state: &mut State) -> Option<String> {
    if let Some((_, text)) = &state.data.clipboard {
        return Some(text.clone());
    }
    let (dev, _) = current_device(state)?;
    let offer = dev.data.as_ref()?.data().selection_offer()?;
    let mime = offer.with_mime_types(pick_text_mime)?;
    let pipe = offer.receive(mime).ok()?;
    let _ = state.conn.flush();
    read_pipe(pipe).map(|b| String::from_utf8_lossy(&b).into_owned())
}

pub fn set_clipboard(state: &mut State, text: String) -> Result<(), PlatformError> {
    let manager = state.data.manager.as_ref().ok_or(PlatformError::Os("the compositor lacks wl_data_device_manager".into()))?;
    let (dev, serial) = current_device(state).ok_or(PlatformError::Os("clipboard without a seat".into()))?;
    let serial = serial.ok_or(PlatformError::Os("setting the clipboard before any input event".into()))?;
    let device = dev.data.as_ref().ok_or(PlatformError::Os("the compositor lacks wl_data_device".into()))?;
    let source = manager.create_copy_paste_source(&state.qh, TEXT_MIMES);
    source.set_selection(device, serial);
    state.data.clipboard = Some((source, text));
    Ok(())
}

pub fn has_primary(state: &State) -> bool {
    state.data.primary_manager.is_some()
}

pub fn get_primary(state: &mut State) -> Option<String> {
    if let Some((_, text)) = &state.data.primary {
        return Some(text.clone());
    }
    let (dev, _) = current_device(state)?;
    let offer = dev.primary.as_ref()?.data().selection_offer()?;
    let mime = offer.with_mime_types(pick_text_mime)?;
    let pipe = offer.receive(mime).ok()?;
    let _ = state.conn.flush();
    read_pipe(pipe).map(|b| String::from_utf8_lossy(&b).into_owned())
}

pub fn set_primary(state: &mut State, text: String) -> Result<(), PlatformError> {
    let manager = state.data.primary_manager.as_ref().ok_or(PlatformError::Os("the compositor lacks zwp_primary_selection_device_manager_v1".into()))?;
    let (dev, serial) = current_device(state).ok_or(PlatformError::Os("primary selection without a seat".into()))?;
    let serial = serial.ok_or(PlatformError::Os("setting the primary selection before any input event".into()))?;
    let device = dev.primary.as_ref().ok_or(PlatformError::Os("the compositor lacks zwp_primary_selection_device_v1".into()))?;
    let source = manager.create_selection_source(&state.qh, TEXT_MIMES);
    source.set_selection(device, serial);
    state.data.primary = Some((source, text));
    Ok(())
}

/// Gives up the primary selection: `unset_selection` with the latest input serial.
pub fn clear_primary(state: &mut State) -> bool {
    let Some((dev, Some(serial))) = current_device(state) else { return false };
    let Some(device) = dev.primary.as_ref() else { return false };
    device.unset_selection(serial);
    state.data.primary = None;
    true
}

pub fn start_drag(state: &mut State, window: WindowId, data: DragData) -> Result<DragId, PlatformError> {
    let manager = state.data.manager.as_ref().ok_or(PlatformError::Os("the compositor lacks wl_data_device_manager".into()))?;
    let (dev, serial) = current_device(state).ok_or(PlatformError::Os("drag without a seat".into()))?;
    let serial = serial.ok_or(PlatformError::Os("drag without a pointer press".into()))?;
    let device = dev.data.as_ref().ok_or(PlatformError::Os("the compositor lacks wl_data_device".into()))?;
    let origin = state.windows.get(&window).ok_or(PlatformError::Os("drag from a closed window".into()))?.surface.clone();
    let mimes: Vec<String> = data.items.iter().map(|(m, _)| m.clone()).collect();
    let source = manager.create_drag_and_drop_source(&state.qh, mimes, DndAction::Copy | DndAction::Move);
    // The toplevel drag must exist before the drag starts.
    let toplevel_drag = state.protocols.toplevel_drag.as_ref().map(|m| m.get_xdg_toplevel_drag(source.inner(), &state.qh, ()));
    source.start_drag(device, &origin, None, serial);
    let id = DragId(state.next_drag);
    state.next_drag += 1;
    if let Some(old) = state.data.drag.take() {
        finish_drag(state, old, DragOutcome::Cancelled);
    }
    state.data.drag = Some(ActiveDrag { id, source, data, toplevel_drag });
    Ok(id)
}

/// Tab tear-off: the compositor moves `window` with the pointer, which sits at `offset` inside it.
pub fn attach_window(state: &mut State, drag: DragId, window: WindowId, offset: Vec2) -> Result<(), PlatformError> {
    let d = state.data.drag.as_ref().filter(|d| d.id == drag).ok_or(PlatformError::Os("attach to a finished drag".into()))?;
    let td = d.toplevel_drag.as_ref().ok_or(PlatformError::Os("the compositor lacks xdg_toplevel_drag_manager_v1".into()))?;
    let w = state.windows.get(&window).ok_or(PlatformError::Os("attach a closed window".into()))?;
    let top = w.toplevel().ok_or(PlatformError::Os("attach a popup".into()))?;
    td.attach(top.xdg_toplevel(), offset.x.round() as i32, offset.y.round() as i32);
    Ok(())
}

fn finish_drag(state: &mut State, d: ActiveDrag, outcome: DragOutcome) {
    if let Some(td) = d.toplevel_drag {
        td.destroy();
    }
    drop(d.source);
    state.pending.push_back(Pending::DragEnded(d.id, outcome));
}

/// File paths in a `text/uri-list` (RFC 2483): `file://` URIs only, percent-decoded, comments skipped.
pub fn parse_uri_list(list: &str) -> Vec<PathBuf> {
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("file://"))
        .filter_map(|rest| {
            // Skip the authority (empty or "localhost").
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            path.starts_with('/').then(|| percent_decode(path))
        })
        .map(|bytes| {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(bytes))
        })
        .collect()
}

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn internal_pointer(phase: PointerPhase, pos: Point, held: bool) -> InputEvent {
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer: crate::frames::MOUSE_POINTER,
        kind: PointerType::Mouse,
        button: matches!(phase, PointerPhase::Up).then_some(MouseButton::Primary),
        buttons: if held { 1 } else { 0 },
        pos,
        modifiers: Default::default(),
        pressure: if held { 0.5 } else { 0.0 },
        tilt: Vec2::default(),
        history: SmallVec::new(),
    })
}

impl DataDeviceHandler for State {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice, x: f64, y: f64, surface: &WlSurface) {
        let Some(window) = self.window_of(surface) else { return };
        let Some(offer) = device.data::<smithay_client_toolkit::data_device_manager::data_device::DataDeviceData>().and_then(|d| d.drag_offer()) else {
            return;
        };
        let mimes = offer.with_mime_types(|m| m.to_vec());
        let now = time::now();
        let pos = Point::new(x as f32, y as f32);
        let kind = if self.data.drag.is_some() && mimes.iter().any(|m| m == SKELTER_DRAG_MIME) {
            offer.accept_mime_type(offer.serial, Some(SKELTER_DRAG_MIME.to_owned()));
            offer.set_actions(DndAction::Copy | DndAction::Move, DndAction::Move);
            self.push_input(window, now, internal_pointer(PointerPhase::Enter, pos, true));
            DropKind::Internal
        } else if mimes.iter().any(|m| m == URI_LIST) {
            offer.accept_mime_type(offer.serial, Some(URI_LIST.to_owned()));
            offer.set_actions(DndAction::Copy, DndAction::Copy);
            let bytes: Option<Vec<u8>> = offer.receive(URI_LIST.to_owned()).ok().and_then(|p| {
                let _ = self.conn.flush();
                read_pipe(p)
            });
            let paths = bytes.map(|b| parse_uri_list(&String::from_utf8_lossy(&b))).unwrap_or_default();
            for p in &paths {
                self.push_input(window, now, InputEvent::FileHovered(p.clone()));
            }
            DropKind::Files(paths)
        } else {
            offer.accept_mime_type(offer.serial, None);
            DropKind::Other
        };
        self.data.target = Some(DropTarget { window, kind, offer });
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        let Some(t) = self.data.target.take() else { return };
        let now = time::now();
        match t.kind {
            DropKind::Files(_) => self.push_input(t.window, now, InputEvent::FileHoverCancelled),
            DropKind::Internal => {
                self.push_input(t.window, now, internal_pointer(PointerPhase::Leave, Point::new(t.offer.x as f32, t.offer.y as f32), true))
            }
            DropKind::Other => {}
        }
    }

    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, x: f64, y: f64) {
        let Some(t) = self.data.target.as_mut() else { return };
        t.offer.x = x;
        t.offer.y = y;
        if matches!(t.kind, DropKind::Internal) {
            let w = t.window;
            self.push_input(w, time::now(), internal_pointer(PointerPhase::Move, Point::new(x as f32, y as f32), true));
        }
    }

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        // Read lazily on paste.
    }

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        let Some(t) = self.data.target.take() else { return };
        let now = time::now();
        let pos = Point::new(t.offer.x as f32, t.offer.y as f32);
        match t.kind {
            DropKind::Files(paths) => {
                for p in paths {
                    self.push_input(t.window, now, InputEvent::FileDropped(p));
                }
                t.offer.finish();
            }
            DropKind::Internal => {
                self.push_input(t.window, now, internal_pointer(PointerPhase::Up, pos, false));
                t.offer.finish();
            }
            DropKind::Other => {}
        }
        t.offer.destroy();
    }
}

impl DataOfferHandler for State {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl DataSourceHandler for State {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}

    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource, mime: String, fd: WritePipe) {
        if let Some((s, text)) = &self.data.clipboard {
            if s.inner() == source {
                return write_async(fd, text.clone().into_bytes());
            }
        }
        if let Some(d) = &self.data.drag {
            if d.source.inner() == source {
                if let Some((_, bytes)) = d.data.items.iter().find(|(m, _)| *m == mime) {
                    write_async(fd, bytes.clone());
                }
            }
        }
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self.data.clipboard.as_ref().is_some_and(|(s, _)| s.inner() == source) {
            self.data.clipboard = None;
        }
        if self.data.drag.as_ref().is_some_and(|d| d.source.inner() == source) {
            let d = self.data.drag.take().unwrap();
            finish_drag(self, d, DragOutcome::Cancelled);
        }
    }

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self.data.drag.as_ref().is_some_and(|d| d.source.inner() == source) {
            let d = self.data.drag.take().unwrap();
            finish_drag(self, d, DragOutcome::Dropped);
        }
    }

    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl PrimarySelectionDeviceHandler for State {
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ZwpPrimarySelectionDeviceV1) {}
}

impl PrimarySelectionSourceHandler for State {
    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &ZwpPrimarySelectionSourceV1, _: String, pipe: WritePipe) {
        if let Some((s, text)) = &self.data.primary {
            if s.inner() == source {
                write_async(pipe, text.clone().into_bytes());
            }
        }
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &ZwpPrimarySelectionSourceV1) {
        if self.data.primary.as_ref().is_some_and(|(s, _)| s.inner() == source) {
            self.data.primary = None;
        }
    }
}

delegate_data_device!(State);
delegate_primary_selection!(State);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_lists_give_file_paths() {
        let list = "# comment\r\nfile:///home/me/a%20b.txt\r\nfile://localhost/tmp/x\r\nhttps://example.com/y\r\n\r\nfile:///caf%C3%A9\n";
        let paths = parse_uri_list(list);
        assert_eq!(paths, [PathBuf::from("/home/me/a b.txt"), PathBuf::from("/tmp/x"), PathBuf::from("/café")]);
    }

    #[test]
    fn malformed_escapes_pass_through() {
        assert_eq!(percent_decode("/a%2"), b"/a%2");
        assert_eq!(percent_decode("/a%zz"), b"/a%zz");
        assert_eq!(percent_decode("/%41"), b"/A");
    }

    #[test]
    fn text_mime_preference() {
        let offered = vec!["text/plain".to_owned(), "UTF8_STRING".to_owned()];
        assert_eq!(pick_text_mime(&offered).as_deref(), Some("UTF8_STRING"));
        assert_eq!(pick_text_mime(&["image/png".to_owned()]), None);
    }
}
