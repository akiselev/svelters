//! The presto-platform API this backend implements: lane PLATFORM's traits and data, re-exported, plus
//! the additions this backend needs, marked "Addition" and listed in `plans/milestones/PRESTO-wayland.md`.
//! Additions to `PlatformCx` live in the [`WaylandCx`] extension trait, which [`crate::cx::Cx`] implements.

pub use presto_platform::{
    A11yEvent, AppHandler, Clipboard, ClipboardKind, HitKind, HitRegion, ImeState, Monitor, OsLayerCommitter, OsLayerDesc,
    PlatformCx, PlatformError, SurfaceTarget, Waker, WindowDesc, hit_test,
};

use presto_base::geom::{DeviceSize, Rect, Vec2};
use presto_base::ids::{FrameId, SurfaceId, WindowId};
use presto_base::input::TextInputClient;
use presto_base::time::{Span, Time};
use smallvec::SmallVec;

/// text-input-v3 caps surrounding text at 4000 bytes.
pub const SURROUNDING_MAX: usize = 4000;

/// Addition: what this backend offers beyond `PlatformCx`.
pub trait WaylandCx: PlatformCx {
    /// A popup (menu, tooltip, combo list) positioned against its parent; configured on return.
    fn create_popup(&mut self, desc: WindowDesc, popup: PopupDesc) -> WindowId;
    /// A toplevel that is a transient child (dialog) of `parent`.
    fn create_child_window(&mut self, desc: WindowDesc, parent: WindowId) -> WindowId;
    /// The window's (or one of its OS layers') frame source, handed to the compositor thread.
    fn frame_source(&mut self, id: WindowId, layer: Option<SurfaceId>) -> Option<Box<dyn FrameSource>>;
    /// Whether the app must draw the window's title bar and frame (no server-side decorations). The platform
    /// then resizes from a border along the window's edges by itself.
    fn client_side_decorations(&self, id: WindowId) -> bool;
    fn set_window_state(&mut self, id: WindowId, req: WindowStateRequest);
    /// Input hints for the window's text input (with `set_ime`).
    fn set_ime_hints(&mut self, id: WindowId, hints: ImeHints);
    /// Start dragging `data` from `window` (the pointer button must be held).
    fn start_drag(&mut self, window: WindowId, data: DragData) -> Result<DragId, PlatformError>;
    /// Tab tear-off: attach `window` to the running `drag`, with the pointer at `offset` inside it (window
    /// points); the OS then moves the window with the pointer until the drag ends.
    fn attach_window_to_drag(&mut self, drag: DragId, window: WindowId, offset: Vec2) -> Result<(), PlatformError>;
    /// Drags that ended since the last call. The platform calls `AppHandler::wake` when one ends.
    fn take_finished_drags(&mut self) -> Vec<(DragId, DragOutcome)>;
}

/// Addition: the IME state for `client` with the caret or clause at `area`, its surrounding text converted
/// from the client's UTF-8 offsets to the UTF-16 offsets `ImeState` holds.
pub fn ime_state_from_client(client: &dyn TextInputClient, area: Rect) -> ImeState {
    let surrounding = client.surrounding_text(SURROUNDING_MAX).map(|(text, sel)| {
        let u16_at = |b: usize| text.get(..b.min(text.len())).map_or(0, |s| s.encode_utf16().count());
        let range = u16_at(sel.start)..u16_at(sel.end);
        (text, range)
    });
    ImeState { area, purpose: client.purpose(), surrounding, ..Default::default() }
}

/// Addition: a popup's placement: `anchor` is a rect in the parent's window points; the popup opens on
/// `gravity`'s side of it and the OS may flip or slide it to stay on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct PopupDesc {
    pub parent: WindowId,
    pub anchor: Rect,
    pub gravity: PopupGravity,
    /// Take an explicit grab (menus): the popup closes when the user clicks outside it.
    pub grab: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PopupGravity {
    #[default]
    Below,
    Above,
    Right,
    Left,
}

/// Addition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowStateRequest {
    Maximize,
    Unmaximize,
    Minimize,
    Fullscreen,
    Unfullscreen,
    /// Ask the OS to raise and focus the window (Wayland: `xdg_activation_v1`).
    Activate,
}

/// Addition: text-input hints beyond the purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ImeHints {
    pub multiline: bool,
    /// Turn off completion, spell checking and auto-capitalisation (code, identifiers).
    pub no_assist: bool,
    pub sensitive: bool,
}

/// Addition: a drag's payload: MIME type → bytes. Within the app the payload is usually one id under
/// presto's own MIME type.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DragData {
    pub items: Vec<(String, Vec<u8>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragOutcome {
    /// Dropped on a target that accepted it.
    Dropped,
    Cancelled,
}

/// Addition: the per-surface frame signal source the compositor thread waits on (`platform.md` §4: frame
/// callbacks and presentation feedback on Wayland, the DXGI waitable or compositor clock on Windows, the
/// display link on macOS). `Send`: created on the main thread, used on the compositor thread.
pub trait FrameSource: Send {
    /// Ask for (or stop asking for) vsync signals.
    fn subscribe(&mut self, on: bool);
    /// Call right before presenting `frame` at `size` (device pixels); `target` is the intended display time.
    fn before_present(&mut self, frame: FrameId, target: Option<Time>, size: DeviceSize);
    /// Call right after the present call returns.
    fn after_present(&mut self, frame: FrameId);
    /// Collect signals that arrived, without blocking.
    fn poll(&mut self, now: Time) -> SmallVec<[FrameSignal; 4]>;
    /// Block until a signal may be ready or `until` passes.
    fn wait(&mut self, until: Option<Time>);
}

/// What a [`FrameSource`] reports; maps one to one onto presto-frame's `Signal`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameSignal {
    Vsync { target: Time, interval: Span },
    Presented { frame: FrameId, at: Time, estimated: bool },
    /// The OS stopped (or resumed) asking for frames: hidden, minimised, fully covered.
    Visible(bool),
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::geom::Point;
    use presto_base::input::{InputPurpose, Utf16Range};

    struct Field(&'static str, std::ops::Range<usize>);

    impl TextInputClient for Field {
        fn text_for_range(&self, _: Utf16Range) -> Option<String> {
            None
        }
        fn selected_range(&self) -> Option<Utf16Range> {
            None
        }
        fn marked_range(&self) -> Option<Utf16Range> {
            None
        }
        fn replace_text_in_range(&mut self, _: Option<Utf16Range>, _: &str) {}
        fn replace_and_mark_text_in_range(&mut self, _: Option<Utf16Range>, _: &str, _: Option<Utf16Range>) {}
        fn unmark_text(&mut self) {}
        fn bounds_for_range(&self, _: Utf16Range) -> Option<Rect> {
            None
        }
        fn character_index_for_point(&self, _: Point) -> Option<usize> {
            None
        }
        fn surrounding_text(&self, _: usize) -> Option<(String, std::ops::Range<usize>)> {
            Some((self.0.to_owned(), self.1.clone()))
        }
        fn purpose(&self) -> InputPurpose {
            InputPurpose::Code
        }
    }

    #[test]
    fn from_client_converts_byte_offsets_to_utf16() {
        // "é😀x": é is 2 bytes / 1 unit, 😀 4 bytes / 2 units.
        let s = ime_state_from_client(&Field("é😀x", 2..6), Rect::default());
        assert_eq!(s.surrounding, Some(("é😀x".to_owned(), 1..3)));
        assert_eq!(s.purpose, InputPurpose::Code);
    }

    #[test]
    fn hit_test_defaults_to_client() {
        let regions = [HitRegion { rect: Rect::new(0.0, 0.0, 10.0, 10.0), kind: HitKind::Caption }];
        assert_eq!(hit_test(&regions, Point::new(5.0, 5.0)), HitKind::Caption);
        assert_eq!(hit_test(&regions, Point::new(50.0, 5.0)), HitKind::Client);
    }
}
