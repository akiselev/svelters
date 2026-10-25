//! `NSTextInputClient` semantics without AppKit: how each protocol method maps onto presto's
//! [`TextInputClient`] (architecture §11.3) or, for windows that only turned IME on
//! ([`ImeState`](presto_platform::ImeState)), onto [`SimpleIme`], which reports the composition as
//! [`ImeEvent`]s. Also the key-routing rule (IME first, architecture §11.2) and the UTF-16 helpers.
//!
//! Ranges are UTF-16 code units of the whole document, as `NSRange` is; `NSNotFound` is
//! [`NsRange::NOT_FOUND`].

use presto_base::event::ImeEvent;
use presto_base::geom::{Point, Rect};
use presto_base::input::{InputPurpose, TextInputClient, Utf16Range};
use std::ops::Range;

/// `NSNotFound` (`NSIntegerMax`).
pub const NS_NOT_FOUND: usize = isize::MAX as usize;

/// An `NSRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NsRange {
    pub location: usize,
    pub length: usize,
}

impl NsRange {
    pub const NOT_FOUND: NsRange = NsRange {
        location: NS_NOT_FOUND,
        length: 0,
    };

    pub fn new(location: usize, length: usize) -> Self {
        NsRange { location, length }
    }

    /// `None` for `NSNotFound` (AppKit's "no range": the selection or the marked text).
    pub fn to_range(self) -> Option<Utf16Range> {
        (self.location != NS_NOT_FOUND)
            .then(|| self.location..self.location.saturating_add(self.length))
    }

    pub fn from_range(r: Option<Utf16Range>) -> Self {
        match r {
            Some(r) => NsRange::new(r.start, r.end.saturating_sub(r.start)),
            None => NsRange::NOT_FOUND,
        }
    }
}

pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The byte offset of UTF-16 offset `u` in `s`, clamped to `s`, rounded down to a character boundary.
pub fn utf16_to_byte(s: &str, u: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > u {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

pub fn utf16_range_to_bytes(s: &str, r: &Range<usize>) -> Range<usize> {
    let start = utf16_to_byte(s, r.start);
    start..utf16_to_byte(s, r.end).max(start)
}

/// The substring of `s` in UTF-16 range `r` (clamped).
pub fn utf16_substring(s: &str, r: &Range<usize>) -> String {
    s[utf16_range_to_bytes(s, r)].to_owned()
}

/// Clamp a selection relative to `text` (setMarkedText's `selectedRange`) to `text`.
pub fn clamp_relative(r: Option<Utf16Range>, text: &str) -> Option<Utf16Range> {
    let len = utf16_len(text);
    r.map(|r| r.start.min(len)..r.end.min(len).max(r.start.min(len)))
}

/// How a key press travels (architecture §11.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRoute {
    /// Straight to the app as a key event.
    Direct,
    /// Through the input context first (`interpretKeyEvents`); what the IME doesn't take comes back as
    /// `insertText:` with the key's own text or as `doCommandBySelector:`, and is delivered as a key
    /// event then.
    ImeFirst,
}

/// Composition, dead keys and accents need the input context; Cmd and Ctrl chords and Option-as-Alt
/// presses go straight to key bindings (unless a composition is active, which owns every key).
pub fn route_key(
    text_target: bool,
    composing: bool,
    option_as_alt: bool,
    command_or_control: bool,
) -> KeyRoute {
    if !text_target {
        KeyRoute::Direct
    } else if composing {
        KeyRoute::ImeFirst
    } else if option_as_alt || command_or_control {
        KeyRoute::Direct
    } else {
        KeyRoute::ImeFirst
    }
}

/// What an `insertText:` during a key press means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    /// The IME passed the key through unchanged: deliver the key event (with its text), so key
    /// bindings see it before the focused editor types it.
    Key,
    /// The IME produced text (a commit, a dead-key result, an accent replacing the previous
    /// character): insert it through the text-input target.
    Text,
}

pub fn classify_insert(
    key_text: Option<&str>,
    inserted: &str,
    had_marked: bool,
    replacement: NsRange,
) -> InsertOutcome {
    if !had_marked && replacement.to_range().is_none() && key_text == Some(inserted) {
        InsertOutcome::Key
    } else {
        InsertOutcome::Text
    }
}

/// The `NSTextInputClient` methods over either kind of text-input target. Rects and points are window
/// points (top-left origin); the view converts to and from screen coordinates.
pub trait ImeTarget {
    fn marked(&self) -> NsRange;
    fn selected(&self) -> NsRange;
    /// `attributedSubstringForProposedRange:actualRange:`: the text and the range it really covers.
    fn substring(&self, proposed: NsRange) -> Option<(String, NsRange)>;
    /// `firstRectForCharacterRange:actualRange:`.
    fn first_rect(&self, range: NsRange) -> Option<(Rect, NsRange)>;
    /// `characterIndexForPoint:`; [`NS_NOT_FOUND`] when outside the text.
    fn index_for_point(&self, p: Point) -> usize;
    fn insert(&mut self, text: &str, replacement: NsRange, events: &mut Vec<ImeEvent>);
    fn set_marked(
        &mut self,
        text: &str,
        selected: NsRange,
        replacement: NsRange,
        events: &mut Vec<ImeEvent>,
    );
    fn unmark(&mut self, events: &mut Vec<ImeEvent>);
    fn purpose(&self) -> InputPurpose;
}

/// A full text-input client: the IME edits the document directly (reconversion, accents replacing the
/// previous character, dictation, the Character Viewer).
pub struct ClientTarget<'a>(pub &'a mut dyn TextInputClient);

impl ImeTarget for ClientTarget<'_> {
    fn marked(&self) -> NsRange {
        NsRange::from_range(self.0.marked_range())
    }

    fn selected(&self) -> NsRange {
        NsRange::from_range(self.0.selected_range())
    }

    fn substring(&self, proposed: NsRange) -> Option<(String, NsRange)> {
        let range = proposed.to_range().filter(|r| !r.is_empty())?;
        let text = self.0.text_for_range(range.clone())?;
        let actual = NsRange::new(range.start, utf16_len(&text));
        Some((text, actual))
    }

    fn first_rect(&self, range: NsRange) -> Option<(Rect, NsRange)> {
        // NSNotFound asks about the selection (some input methods do).
        let range = range.to_range().or_else(|| self.0.selected_range())?;
        let rect = self.0.bounds_for_range(range.clone())?;
        Some((rect, NsRange::from_range(Some(range))))
    }

    fn index_for_point(&self, p: Point) -> usize {
        self.0.character_index_for_point(p).unwrap_or(NS_NOT_FOUND)
    }

    fn insert(&mut self, text: &str, replacement: NsRange, _events: &mut Vec<ImeEvent>) {
        self.0.replace_text_in_range(replacement.to_range(), text);
    }

    fn set_marked(
        &mut self,
        text: &str,
        selected: NsRange,
        replacement: NsRange,
        _events: &mut Vec<ImeEvent>,
    ) {
        if text.is_empty() {
            // An empty composition removes the marked text and ends the composition.
            self.0.replace_text_in_range(replacement.to_range(), "");
        } else {
            let sel = clamp_relative(selected.to_range(), text);
            self.0
                .replace_and_mark_text_in_range(replacement.to_range(), text, sel);
        }
    }

    fn unmark(&mut self, _events: &mut Vec<ImeEvent>) {
        self.0.unmark_text();
    }

    fn purpose(&self) -> InputPurpose {
        self.0.purpose()
    }
}

/// IME without a document: the composition lives here and is reported as [`ImeEvent::Preedit`] and
/// [`ImeEvent::Commit`] (winit's model). The IME sees a document that is just the marked text, so
/// reconversion and replacement ranges outside it are not possible.
#[derive(Clone, Debug, PartialEq)]
pub struct SimpleIme {
    pub area: Rect,
    pub purpose: InputPurpose,
    /// Per-character rectangles of the composition (window points), when the app gives them.
    pub composition_rects: Vec<Rect>,
    marked: Option<(String, Utf16Range)>,
}

impl SimpleIme {
    pub fn new(area: Rect, purpose: InputPurpose) -> Self {
        SimpleIme {
            area,
            purpose,
            composition_rects: Vec::new(),
            marked: None,
        }
    }

    pub fn marked_text(&self) -> Option<&str> {
        self.marked.as_ref().map(|(t, _)| t.as_str())
    }

    fn preedit(&self) -> ImeEvent {
        match &self.marked {
            Some((text, sel)) => {
                let b = utf16_range_to_bytes(text, sel);
                ImeEvent::Preedit {
                    text: text.clone(),
                    cursor: Some((b.start, b.end)),
                }
            }
            None => ImeEvent::Preedit {
                text: String::new(),
                cursor: None,
            },
        }
    }
}

impl ImeTarget for SimpleIme {
    fn marked(&self) -> NsRange {
        match &self.marked {
            Some((t, _)) => NsRange::new(0, utf16_len(t)),
            None => NsRange::NOT_FOUND,
        }
    }

    fn selected(&self) -> NsRange {
        match &self.marked {
            Some((_, sel)) => NsRange::from_range(Some(sel.clone())),
            None => NsRange::new(0, 0),
        }
    }

    fn substring(&self, proposed: NsRange) -> Option<(String, NsRange)> {
        let (text, _) = self.marked.as_ref()?;
        let r = proposed.to_range().filter(|r| !r.is_empty())?;
        let len = utf16_len(text);
        if r.start >= len {
            return None;
        }
        let r = r.start..r.end.min(len);
        Some((utf16_substring(text, &r), NsRange::from_range(Some(r))))
    }

    fn first_rect(&self, range: NsRange) -> Option<(Rect, NsRange)> {
        let actual = if range.to_range().is_some() {
            range
        } else {
            self.selected()
        };
        let rect = actual
            .to_range()
            .and_then(|r| composition_rect(&self.composition_rects, r))
            .unwrap_or(self.area);
        Some((rect, actual))
    }

    fn index_for_point(&self, _p: Point) -> usize {
        NS_NOT_FOUND
    }

    fn insert(&mut self, text: &str, _replacement: NsRange, events: &mut Vec<ImeEvent>) {
        if self.marked.take().is_some() {
            events.push(self.preedit());
        }
        if !text.is_empty() {
            events.push(ImeEvent::Commit(text.to_owned()));
        }
    }

    fn set_marked(
        &mut self,
        text: &str,
        selected: NsRange,
        _replacement: NsRange,
        events: &mut Vec<ImeEvent>,
    ) {
        if text.is_empty() {
            if self.marked.take().is_none() {
                return;
            }
        } else {
            let len = utf16_len(text);
            let sel = clamp_relative(selected.to_range(), text).unwrap_or(len..len);
            self.marked = Some((text.to_owned(), sel));
        }
        events.push(self.preedit());
    }

    fn unmark(&mut self, events: &mut Vec<ImeEvent>) {
        if let Some((text, _)) = self.marked.take() {
            events.push(self.preedit());
            events.push(ImeEvent::Commit(text));
        }
    }

    fn purpose(&self) -> InputPurpose {
        self.purpose
    }
}

/// The union of the per-character rectangles in `r` (UTF-16 offsets into the composition, one
/// rectangle per unit); for an empty range, the caret edge at `r.start`.
pub fn composition_rect(rects: &[Rect], r: Range<usize>) -> Option<Rect> {
    if rects.is_empty() {
        return None;
    }
    if r.is_empty() {
        return Some(match rects.get(r.start) {
            Some(c) => Rect {
                min: c.min,
                max: Point::new(c.min.x, c.max.y),
            },
            None => {
                let last = rects[rects.len() - 1];
                Rect {
                    min: Point::new(last.max.x, last.min.y),
                    max: last.max,
                }
            }
        });
    }
    let end = r.end.min(rects.len());
    let mut it = rects.get(r.start..end)?.iter();
    let first = *it.next()?;
    Some(it.fold(first, |a, b| a.union(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document with a selection and an optional composition, as an editor implements the trait.
    #[derive(Default)]
    struct Doc {
        text: Vec<u16>,
        sel: Utf16Range,
        marked: Option<Utf16Range>,
    }

    impl Doc {
        fn new(s: &str, caret: usize) -> Self {
            Doc {
                text: s.encode_utf16().collect(),
                sel: caret..caret,
                marked: None,
            }
        }
        fn string(&self) -> String {
            String::from_utf16(&self.text).unwrap()
        }
        fn splice(&mut self, r: Utf16Range, s: &str) -> Utf16Range {
            let new: Vec<u16> = s.encode_utf16().collect();
            let n = new.len();
            self.text.splice(r.clone(), new);
            r.start..r.start + n
        }
    }

    impl TextInputClient for Doc {
        fn text_for_range(&self, r: Utf16Range) -> Option<String> {
            let end = r.end.min(self.text.len());
            (r.start <= end).then(|| String::from_utf16(&self.text[r.start..end]).unwrap())
        }
        fn selected_range(&self) -> Option<Utf16Range> {
            Some(self.sel.clone())
        }
        fn marked_range(&self) -> Option<Utf16Range> {
            self.marked.clone()
        }
        fn replace_text_in_range(&mut self, r: Option<Utf16Range>, s: &str) {
            let r = r.or(self.marked.clone()).unwrap_or(self.sel.clone());
            let new = self.splice(r, s);
            self.sel = new.end..new.end;
            self.marked = None;
        }
        fn replace_and_mark_text_in_range(
            &mut self,
            r: Option<Utf16Range>,
            s: &str,
            sel: Option<Utf16Range>,
        ) {
            let r = r.or(self.marked.clone()).unwrap_or(self.sel.clone());
            let new = self.splice(r, s);
            self.sel = match sel {
                Some(rel) => new.start + rel.start..new.start + rel.end,
                None => new.end..new.end,
            };
            self.marked = Some(new);
        }
        fn unmark_text(&mut self) {
            self.marked = None;
        }
        fn bounds_for_range(&self, r: Utf16Range) -> Option<Rect> {
            // 10 points per unit on one line.
            let x0 = r.start as f32 * 10.0;
            let x1 = r.end as f32 * 10.0;
            Some(Rect {
                min: Point::new(x0, 0.0),
                max: Point::new(x1.max(x0 + 1.0), 20.0),
            })
        }
        fn character_index_for_point(&self, p: Point) -> Option<usize> {
            let i = (p.x / 10.0) as usize;
            (i <= self.text.len()).then_some(i)
        }
        fn surrounding_text(&self, _max_bytes: usize) -> Option<(String, Range<usize>)> {
            None
        }
    }

    #[test]
    fn ns_range_round_trip() {
        assert_eq!(NsRange::NOT_FOUND.to_range(), None);
        assert_eq!(NsRange::new(3, 2).to_range(), Some(3..5));
        assert_eq!(NsRange::from_range(Some(4..4)), NsRange::new(4, 0));
        assert_eq!(NsRange::from_range(None), NsRange::NOT_FOUND);
    }

    #[test]
    fn utf16_helpers() {
        let s = "a😀é";
        assert_eq!(utf16_len(s), 4);
        assert_eq!(utf16_to_byte(s, 1), 1);
        assert_eq!(
            utf16_to_byte(s, 2),
            1,
            "inside the surrogate pair rounds down"
        );
        assert_eq!(utf16_to_byte(s, 3), 5);
        assert_eq!(utf16_to_byte(s, 99), s.len());
        assert_eq!(utf16_substring(s, &(1..3)), "😀");
        assert_eq!(clamp_relative(Some(2..9), "abc"), Some(2..3));
        assert_eq!(clamp_relative(Some(7..9), "abc"), Some(3..3));
    }

    #[test]
    fn routing() {
        assert_eq!(route_key(false, false, false, false), KeyRoute::Direct);
        assert_eq!(route_key(true, false, false, false), KeyRoute::ImeFirst);
        assert_eq!(
            route_key(true, false, false, true),
            KeyRoute::Direct,
            "Cmd/Ctrl chords"
        );
        assert_eq!(
            route_key(true, false, true, false),
            KeyRoute::Direct,
            "Option as Alt"
        );
        assert_eq!(
            route_key(true, true, true, true),
            KeyRoute::ImeFirst,
            "a composition owns keys"
        );
    }

    #[test]
    fn insert_classification() {
        let none = NsRange::NOT_FOUND;
        assert_eq!(
            classify_insert(Some("j"), "j", false, none),
            InsertOutcome::Key
        );
        assert_eq!(
            classify_insert(Some("å"), "å", false, none),
            InsertOutcome::Key
        );
        // Dead key then a: the IME commits "ä".
        assert_eq!(
            classify_insert(Some("a"), "ä", true, none),
            InsertOutcome::Text
        );
        // Same text but a composition was active (Brazilian `"` then space commits `"`).
        assert_eq!(
            classify_insert(Some(" "), " ", true, none),
            InsertOutcome::Text
        );
        // Press-and-hold accent replacing the previous character.
        assert_eq!(
            classify_insert(Some("e"), "é", false, NsRange::new(4, 1)),
            InsertOutcome::Text
        );
        assert_eq!(
            classify_insert(Some("e"), "e", false, NsRange::new(4, 1)),
            InsertOutcome::Text
        );
        // Emoji from the Character Viewer (no key).
        assert_eq!(
            classify_insert(None, "😀", false, none),
            InsertOutcome::Text
        );
    }

    #[test]
    fn client_composition_and_commit() {
        let mut doc = Doc::new("ab", 2);
        let mut ev = Vec::new();
        {
            let mut t = ClientTarget(&mut doc);
            assert_eq!(t.marked(), NsRange::NOT_FOUND);
            assert_eq!(t.selected(), NsRange::new(2, 0));
            // Japanese: "k" then "か".
            t.set_marked("k", NsRange::new(1, 0), NsRange::NOT_FOUND, &mut ev);
            assert_eq!(t.marked(), NsRange::new(2, 1));
            t.set_marked("か", NsRange::new(1, 0), NsRange::NOT_FOUND, &mut ev);
            assert_eq!(t.marked(), NsRange::new(2, 1));
            assert_eq!(t.selected(), NsRange::new(3, 0));
            t.insert("火", NsRange::NOT_FOUND, &mut ev);
            assert_eq!(t.marked(), NsRange::NOT_FOUND);
        }
        assert_eq!(doc.string(), "ab火");
        assert_eq!(doc.sel, 3..3);
        assert!(ev.is_empty(), "a full client reports nothing as events");
    }

    #[test]
    fn client_selection_inside_marked_text_is_clamped() {
        let mut doc = Doc::new("", 0);
        let mut ev = Vec::new();
        ClientTarget(&mut doc).set_marked("한", NsRange::new(0, 5), NsRange::NOT_FOUND, &mut ev);
        assert_eq!(doc.sel, 0..1);
    }

    #[test]
    fn client_empty_marked_text_ends_the_composition() {
        let mut doc = Doc::new("x", 1);
        let mut ev = Vec::new();
        let mut t = ClientTarget(&mut doc);
        t.set_marked("´", NsRange::new(1, 0), NsRange::NOT_FOUND, &mut ev);
        t.set_marked("", NsRange::new(0, 0), NsRange::NOT_FOUND, &mut ev);
        assert_eq!(t.marked(), NsRange::NOT_FOUND);
        assert_eq!(doc.string(), "x");
    }

    #[test]
    fn client_press_and_hold_replaces_the_previous_character() {
        let mut doc = Doc::new("caf", 3);
        // The key path typed "e"; the accent menu then replaces it.
        doc.replace_text_in_range(None, "e");
        let mut ev = Vec::new();
        ClientTarget(&mut doc).insert("é", NsRange::new(3, 1), &mut ev);
        assert_eq!(doc.string(), "café");
        assert_eq!(doc.sel, 4..4);
    }

    #[test]
    fn client_reconversion_ranges() {
        let mut doc = Doc::new("日本語テキスト", 7);
        let t = ClientTarget(&mut doc);
        assert_eq!(
            t.substring(NsRange::new(0, 3)),
            Some(("日本語".to_owned(), NsRange::new(0, 3)))
        );
        // A proposal past the end is cut to the text that exists.
        assert_eq!(
            t.substring(NsRange::new(5, 10)),
            Some(("スト".to_owned(), NsRange::new(5, 2)))
        );
        assert_eq!(t.substring(NsRange::new(2, 0)), None);
        assert_eq!(t.substring(NsRange::NOT_FOUND), None);
    }

    #[test]
    fn client_geometry() {
        let mut doc = Doc::new("hello", 5);
        let t = ClientTarget(&mut doc);
        let (r, actual) = t.first_rect(NsRange::new(1, 2)).unwrap();
        assert_eq!((r.min.x, r.max.x), (10.0, 30.0));
        assert_eq!(actual, NsRange::new(1, 2));
        // NSNotFound means the selection (the caret).
        let (r, actual) = t.first_rect(NsRange::NOT_FOUND).unwrap();
        assert_eq!(r.min.x, 50.0);
        assert_eq!(actual, NsRange::new(5, 0));
        assert_eq!(t.index_for_point(Point::new(24.0, 5.0)), 2);
        assert_eq!(t.index_for_point(Point::new(500.0, 5.0)), NS_NOT_FOUND);
    }

    #[test]
    fn simple_ime_reports_events() {
        let area = Rect {
            min: Point::new(5.0, 5.0),
            max: Point::new(6.0, 25.0),
        };
        let mut ime = SimpleIme::new(area, InputPurpose::Text);
        let mut ev = Vec::new();
        assert_eq!(ime.marked(), NsRange::NOT_FOUND);
        assert_eq!(
            ime.selected(),
            NsRange::new(0, 0),
            "never NSNotFound: dictation needs a selection"
        );
        ime.set_marked("にほ", NsRange::new(2, 0), NsRange::NOT_FOUND, &mut ev);
        assert_eq!(
            ev,
            vec![ImeEvent::Preedit {
                text: "にほ".into(),
                cursor: Some((6, 6))
            }]
        );
        assert_eq!(ime.marked(), NsRange::new(0, 2));
        assert_eq!(
            ime.substring(NsRange::new(1, 4)),
            Some(("ほ".into(), NsRange::new(1, 1)))
        );
        assert_eq!(
            ime.first_rect(NsRange::new(0, 1)),
            Some((area, NsRange::new(0, 1)))
        );
        ev.clear();
        ime.insert("日本", NsRange::NOT_FOUND, &mut ev);
        assert_eq!(
            ev,
            vec![
                ImeEvent::Preedit {
                    text: String::new(),
                    cursor: None
                },
                ImeEvent::Commit("日本".into())
            ]
        );
        assert_eq!(ime.marked_text(), None);
    }

    #[test]
    fn simple_ime_uses_per_character_rects() {
        let rect = |x0: f32| Rect {
            min: Point::new(x0, 0.0),
            max: Point::new(x0 + 10.0, 20.0),
        };
        let mut ime = SimpleIme::new(rect(100.0), InputPurpose::Text);
        ime.composition_rects = vec![rect(0.0), rect(10.0), rect(20.0)];
        let mut ev = Vec::new();
        ime.set_marked("あいう", NsRange::new(3, 0), NsRange::NOT_FOUND, &mut ev);
        let (r, _) = ime.first_rect(NsRange::new(1, 2)).unwrap();
        assert_eq!((r.min.x, r.max.x), (10.0, 30.0));
        let (r, _) = ime.first_rect(NsRange::new(3, 0)).unwrap();
        assert_eq!(
            (r.min.x, r.max.x),
            (30.0, 30.0),
            "caret after the last character"
        );
        assert_eq!(composition_rect(&[], 0..1), None);
        assert_eq!(composition_rect(&ime.composition_rects, 5..6), None);
    }

    #[test]
    fn simple_ime_unmark_commits_and_empty_mark_cancels() {
        let mut ime = SimpleIme::new(Rect::ZERO, InputPurpose::Text);
        let mut ev = Vec::new();
        ime.set_marked("´", NsRange::NOT_FOUND, NsRange::NOT_FOUND, &mut ev);
        assert_eq!(
            ev,
            vec![ImeEvent::Preedit {
                text: "´".into(),
                cursor: Some((2, 2))
            }]
        );
        ev.clear();
        ime.unmark(&mut ev);
        assert_eq!(
            ev,
            vec![
                ImeEvent::Preedit {
                    text: String::new(),
                    cursor: None
                },
                ImeEvent::Commit("´".into())
            ]
        );
        ev.clear();
        ime.set_marked("x", NsRange::new(0, 1), NsRange::NOT_FOUND, &mut ev);
        ev.clear();
        ime.set_marked("", NsRange::new(0, 0), NsRange::NOT_FOUND, &mut ev);
        assert_eq!(
            ev,
            vec![ImeEvent::Preedit {
                text: String::new(),
                cursor: None
            }]
        );
        ev.clear();
        ime.set_marked("", NsRange::new(0, 0), NsRange::NOT_FOUND, &mut ev);
        assert!(ev.is_empty(), "nothing to cancel");
        ime.insert("", NsRange::NOT_FOUND, &mut ev);
        assert!(ev.is_empty());
    }
}
