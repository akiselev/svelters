//! The commit's IME area: the focused text kind's caret bounds, sent when they move.

use super::*;
use crate::cx::RecordCx;
use crate::kind::{Kind, KindSpec};
use crate::{FocusReason, KindRegistry};
use skelter_base::geom::Rect;
use skelter_base::input::{TextInputClient, Utf16Range};
use std::ops::Range;
use std::sync::Arc;

static EDIT: KindSpec = KindSpec { focusable: true, ..KindSpec::new("edit") };

#[derive(Default)]
struct Edit {
    caret: usize,
}

impl TextInputClient for Edit {
    fn text_for_range(&self, _range: Utf16Range) -> Option<String> {
        None
    }
    fn selected_range(&self) -> Option<Utf16Range> {
        Some(self.caret..self.caret)
    }
    fn marked_range(&self) -> Option<Utf16Range> {
        None
    }
    fn replace_text_in_range(&mut self, _range: Option<Utf16Range>, text: &str) {
        self.caret += text.len();
    }
    fn replace_and_mark_text_in_range(&mut self, _range: Option<Utf16Range>, _text: &str, _new_selected: Option<Utf16Range>) {}
    fn unmark_text(&mut self) {}
    fn bounds_for_range(&self, r: Utf16Range) -> Option<Rect> {
        Some(Rect::new(r.start as f32 * 8.0, 0.0, r.start as f32 * 8.0 + 1.0, 16.0))
    }
    fn character_index_for_point(&self, _p: Point) -> Option<usize> {
        None
    }
    fn surrounding_text(&self, _max_bytes: usize) -> Option<(String, Range<usize>)> {
        None
    }
}

impl Kind for Edit {
    fn spec() -> &'static KindSpec {
        &EDIT
    }
    fn record(&self, cx: &mut RecordCx) {
        cx.draw_box();
    }
    fn text_input(&mut self) -> Option<&mut dyn TextInputClient> {
        Some(self)
    }
}

#[test]
fn the_ime_area_follows_the_caret() {
    let mut r = KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<Edit>().unwrap();
    let mut d = Document::new(Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    let e = child(&mut d, root, KindId::of("edit"), "width: 100px; height: 20px");
    assert_eq!(frame(&mut d).unwrap().ime_area, None, "nothing focused");
    d.focus(e, FocusReason::Pointer);
    assert_eq!(frame(&mut d).unwrap().ime_area, Some(Rect::new(0.0, 0.0, 1.0, 16.0)));
    assert!(frame(&mut d).is_none_or(|c| c.ime_area.is_none()), "unchanged: not resent");
    d.with_text_input(W, |c| c.replace_text_in_range(None, "ab"));
    assert_eq!(frame(&mut d).unwrap().ime_area, Some(Rect::new(16.0, 0.0, 17.0, 16.0)));
}
