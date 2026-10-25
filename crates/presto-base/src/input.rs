//! The text-input client contract (architecture §11.3): implemented by editing kinds (quire-text's
//! editors), used by the platform layer's IME bridges (IMM32/TSF, NSTextInputClient, text-input-v3).
//! Offsets are UTF-16 code units of the whole document, as TSF, macOS and UIA expect.

use crate::geom::{Point, Rect};
use std::ops::Range;

pub type Utf16Range = Range<usize>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InputPurpose {
    #[default]
    Text,
    Number,
    Password,
    Url,
    Email,
    Phone,
    Code,
    Search,
}

pub trait TextInputClient {
    fn text_for_range(&self, range: Utf16Range) -> Option<String>;
    /// The selection; an empty range is the caret.
    fn selected_range(&self) -> Option<Utf16Range>;
    /// The composition (marked text), if any.
    fn marked_range(&self) -> Option<Utf16Range>;
    /// Replace `range` (or the selection, or the marked text) with `text`, ending any composition.
    fn replace_text_in_range(&mut self, range: Option<Utf16Range>, text: &str);
    /// Replace and mark: the IME's composition update.
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Utf16Range>,
        text: &str,
        new_selected: Option<Utf16Range>,
    );
    fn unmark_text(&mut self);
    /// Window-space bounds of `range` (per-character candidate placement).
    fn bounds_for_range(&self, range: Utf16Range) -> Option<Rect>;
    fn character_index_for_point(&self, p: Point) -> Option<usize>;
    /// Text around the caret, at most `max_bytes` UTF-8 bytes, and the selection inside it (bytes).
    fn surrounding_text(&self, max_bytes: usize) -> Option<(String, Range<usize>)>;
    fn purpose(&self) -> InputPurpose {
        InputPurpose::Text
    }
}
