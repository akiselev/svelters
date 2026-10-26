//! The skelter-platform API this backend implements, re-exported, plus the helpers this backend adds. The
//! popup, child-window, window-state, IME-hint, drag and layer-frame-source calls that started here as a
//! `WaylandCx` extension trait are `PlatformCx` methods now (with `AppHandler::drag_ended`), so apps need
//! no downcast.

pub use skelter_platform::{
    A11yEvent, AppHandler, Clipboard, ClipboardKind, DragData, DragId, DragOutcome, FrameSource, FrameWait, HitKind,
    HitRegion, ImeHints, ImeState, Monitor, OsLayerCommitter, OsLayerDesc, PlatformCx, PlatformError, PopupDesc,
    PopupGravity, Presented, SurfaceTarget, Waker, WindowDesc, WindowStateRequest, hit_test,
};

use skelter_base::geom::Rect;
use skelter_base::input::TextInputClient;

/// text-input-v3 caps surrounding text at 4000 bytes.
pub const SURROUNDING_MAX: usize = 4000;

/// The IME state for `client` with the caret or clause at `area`. The client's surrounding text and
/// selection are UTF-16 offsets, as `ImeState` holds them; offsets past the text are clamped to its end.
pub fn ime_state_from_client(client: &dyn TextInputClient, area: Rect) -> ImeState {
    let surrounding = client.surrounding_text(SURROUNDING_MAX).map(|(text, sel)| {
        let units = text.encode_utf16().count();
        let range = sel.start.min(units)..sel.end.min(units);
        (text, range)
    });
    ImeState { area, purpose: client.purpose(), surrounding, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::geom::Point;
    use skelter_base::input::{InputPurpose, Utf16Range};

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
    fn from_client_keeps_utf16_offsets() {
        // "é😀x": é is 1 unit, 😀 2 units; 4 units in all.
        let s = ime_state_from_client(&Field("é😀x", 1..3), Rect::default());
        assert_eq!(s.surrounding, Some(("é😀x".to_owned(), 1..3)));
        assert_eq!(s.purpose, InputPurpose::Code);
        let past = ime_state_from_client(&Field("é😀x", 3..9), Rect::default());
        assert_eq!(past.surrounding, Some(("é😀x".to_owned(), 3..4)));
    }

    #[test]
    fn hit_test_defaults_to_client() {
        let regions = [HitRegion { rect: Rect::new(0.0, 0.0, 10.0, 10.0), kind: HitKind::Caption }];
        assert_eq!(hit_test(&regions, Point::new(5.0, 5.0)), HitKind::Caption);
        assert_eq!(hit_test(&regions, Point::new(50.0, 5.0)), HitKind::Client);
    }
}
