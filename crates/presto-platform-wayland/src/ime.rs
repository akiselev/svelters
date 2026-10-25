//! `zwp_text_input_v3`: the Wayland IME bridge.
//!
//! **Outgoing.** While the focused window has an [`ImeState`] (an editor is focused) and the seat's text input
//! has entered that window's surface, the text input is enabled and carries the editor's surrounding text
//! (≤ 4000 bytes, cursor and anchor as byte offsets), content type (purpose and hints) and cursor rectangle,
//! each update closed with `commit`.
//!
//! **Incoming.** The IME sends `preedit_string`, `commit_string` and `delete_surrounding_text`, then `done`.
//! [`DoneBatch`] collects them and applies them in the protocol's order: replace the preedit, delete the
//! surrounding text, insert the commit string, then show the new preedit. They become presto
//! [`ImeEvent`]s, and [`apply_to_client`] applies the same batch to a [`TextInputClient`] for editors that take
//! the IME directly.

use crate::shim::{ImeHints, ImeState};
use presto_base::event::ImeEvent;
use presto_base::input::{InputPurpose, TextInputClient};
use smallvec::SmallVec;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{ContentHint, ContentPurpose, ZwpTextInputV3};

/// The pieces of one `done` batch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DoneBatch {
    pub preedit: Option<(String, Option<(usize, usize)>)>,
    pub commit: Option<String>,
    pub delete: Option<(usize, usize)>,
}

impl DoneBatch {
    pub fn preedit_string(&mut self, text: Option<String>, begin: i32, end: i32) {
        let text = text.unwrap_or_default();
        // Negative offsets mean "hide the cursor".
        let cursor = (begin >= 0 && end >= 0).then(|| {
            let (b, e) = (begin as usize, end as usize);
            (b.min(text.len()), e.min(text.len()))
        });
        self.preedit = Some((text, cursor));
    }

    pub fn commit_string(&mut self, text: Option<String>) {
        self.commit = Some(text.unwrap_or_default());
    }

    pub fn delete_surrounding(&mut self, before: u32, after: u32) {
        self.delete = Some((before as usize, after as usize));
    }

    /// The batch as presto events, given whether a preedit is showing now. Returns the events and whether a
    /// preedit shows afterwards.
    pub fn into_events(self, preedit_showing: bool) -> (SmallVec<[ImeEvent; 4]>, bool) {
        let mut out = SmallVec::new();
        let (text, cursor) = self.preedit.unwrap_or_default();
        let changes_text = self.delete.is_some() || self.commit.is_some();
        // 1. The current preedit goes away before text changes around it.
        if preedit_showing && (changes_text || text.is_empty()) {
            out.push(ImeEvent::Preedit { text: String::new(), cursor: None });
        }
        // 2. Delete around the cursor.
        if let Some((before, after)) = self.delete {
            out.push(ImeEvent::DeleteSurrounding { before, after });
        }
        // 3. Insert the commit string.
        if let Some(c) = self.commit.filter(|c| !c.is_empty()) {
            out.push(ImeEvent::Commit(c));
        }
        // 4. Show the new preedit.
        let showing = !text.is_empty();
        if showing {
            out.push(ImeEvent::Preedit { text, cursor });
        }
        (out, showing)
    }
}

/// Applies a batch's events to an editor in order (the bridge for editors implementing [`TextInputClient`]).
/// Offsets in `DeleteSurrounding` are UTF-8 bytes around the selection, as text-input-v3 sends them.
pub fn apply_to_client(client: &mut dyn TextInputClient, events: &[ImeEvent]) {
    for ev in events {
        match ev {
            ImeEvent::Preedit { text, cursor } => {
                if text.is_empty() {
                    if client.marked_range().is_some() {
                        client.replace_text_in_range(client.marked_range(), "");
                    }
                    continue;
                }
                let sel = cursor.and_then(|(b, e)| Some(utf16_len(text.get(..b)?)..utf16_len(text.get(..e)?)));
                client.replace_and_mark_text_in_range(client.marked_range(), text, sel);
            }
            ImeEvent::Commit(text) => client.replace_text_in_range(client.marked_range(), text),
            ImeEvent::DeleteSurrounding { before, after } => {
                let Some(sel) = client.selected_range() else { continue };
                let start16 = utf16_back_from(client, sel.start, *before);
                let end16 = utf16_forward_from(client, sel.end, *after);
                if start16 < sel.start {
                    client.replace_text_in_range(Some(start16..sel.start), "");
                }
                let shift = sel.start - start16;
                if end16 > sel.end {
                    client.replace_text_in_range(Some(sel.end - shift..end16 - shift), "");
                }
            }
            ImeEvent::Enabled | ImeEvent::Disabled => {}
        }
    }
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The UTF-16 offset `bytes` UTF-8 bytes before `from` (UTF-16), using the client's text.
fn utf16_back_from(client: &dyn TextInputClient, from: usize, bytes: usize) -> usize {
    if bytes == 0 {
        return from;
    }
    // Each UTF-8 byte is at most one UTF-16 unit, so `bytes` units back cover the span.
    let lo = from.saturating_sub(bytes);
    let Some(text) = client.text_for_range(lo..from) else { return from };
    let mut taken = 0usize;
    let mut units = 0usize;
    for c in text.chars().rev() {
        if taken + c.len_utf8() > bytes {
            break;
        }
        taken += c.len_utf8();
        units += c.len_utf16();
    }
    from - units
}

fn utf16_forward_from(client: &dyn TextInputClient, from: usize, bytes: usize) -> usize {
    if bytes == 0 {
        return from;
    }
    let Some(text) = client.text_for_range(from..from + bytes) else { return from };
    let mut taken = 0usize;
    let mut units = 0usize;
    for c in text.chars() {
        if taken + c.len_utf8() > bytes {
            break;
        }
        taken += c.len_utf8();
        units += c.len_utf16();
    }
    from + units
}

/// text-input-v3's content purpose and hints for an editor.
pub fn content_type(purpose: InputPurpose, hints: ImeHints) -> (ContentHint, ContentPurpose) {
    let mut h = ContentHint::empty();
    let p = match purpose {
        InputPurpose::Text => ContentPurpose::Normal,
        InputPurpose::Number => ContentPurpose::Number,
        InputPurpose::Password => {
            h |= ContentHint::HiddenText | ContentHint::SensitiveData;
            ContentPurpose::Password
        }
        InputPurpose::Url => ContentPurpose::Url,
        InputPurpose::Email => ContentPurpose::Email,
        InputPurpose::Phone => ContentPurpose::Phone,
        InputPurpose::Code => {
            h |= ContentHint::Latin;
            ContentPurpose::Normal
        }
        InputPurpose::Search => ContentPurpose::Normal,
    };
    if hints.multiline {
        h |= ContentHint::Multiline;
    }
    if hints.sensitive {
        h |= ContentHint::SensitiveData;
    }
    if !hints.no_assist && !matches!(purpose, InputPurpose::Code | InputPurpose::Password) {
        h |= ContentHint::Completion | ContentHint::Spellcheck | ContentHint::AutoCapitalization;
    }
    (h, p)
}

/// Clamps surrounding text to the protocol's 4000 bytes on character boundaries, keeping the selection
/// inside; returns the text and the cursor and anchor byte offsets.
pub fn clamp_surrounding(text: &str, sel: std::ops::Range<usize>) -> (String, i32, i32) {
    const MAX: usize = ImeState::SURROUNDING_MAX;
    let sel = sel.start.min(text.len())..sel.end.min(text.len());
    if text.len() <= MAX {
        return (text.to_owned(), sel.end as i32, sel.start as i32);
    }
    // A window around the selection's end (the caret).
    let caret = sel.end;
    let mut start = caret.saturating_sub(MAX / 2);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let mut end = (start + MAX).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let anchor = sel.start.clamp(start, end);
    (text[start..end].to_owned(), (caret - start) as i32, (anchor - start) as i32)
}

/// Sends an editor's state to the text input and commits it.
pub fn send_state(ti: &ZwpTextInputV3, state: &ImeState) {
    if let Some((text, sel)) = &state.surrounding {
        let (text, cursor, anchor) = clamp_surrounding(text, sel.clone());
        ti.set_surrounding_text(text, cursor, anchor);
    }
    let (hint, purpose) = content_type(state.purpose, state.hints);
    ti.set_content_type(hint, purpose);
    let r = state.area;
    ti.set_cursor_rectangle(
        r.min.x.floor() as i32,
        r.min.y.floor() as i32,
        (r.max.x - r.min.x).ceil().max(1.0) as i32,
        (r.max.y - r.min.y).ceil().max(1.0) as i32,
    );
    ti.commit();
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::geom::{Point, Rect};
    use presto_base::input::Utf16Range;

    #[test]
    fn preedit_only() {
        let mut b = DoneBatch::default();
        b.preedit_string(Some("かな".into()), 0, 6);
        let (ev, showing) = b.into_events(false);
        assert!(showing);
        assert_eq!(ev.as_slice(), [ImeEvent::Preedit { text: "かな".into(), cursor: Some((0, 6)) }]);
    }

    #[test]
    fn commit_replaces_the_preedit() {
        let mut b = DoneBatch::default();
        b.commit_string(Some("仮名".into()));
        let (ev, showing) = b.into_events(true);
        assert!(!showing);
        assert_eq!(
            ev.as_slice(),
            [ImeEvent::Preedit { text: String::new(), cursor: None }, ImeEvent::Commit("仮名".into())]
        );
    }

    #[test]
    fn protocol_order_delete_commit_preedit() {
        let mut b = DoneBatch::default();
        b.preedit_string(Some("b".into()), -1, -1);
        b.commit_string(Some("a".into()));
        b.delete_surrounding(3, 0);
        let (ev, _) = b.into_events(true);
        assert_eq!(
            ev.as_slice(),
            [
                ImeEvent::Preedit { text: String::new(), cursor: None },
                ImeEvent::DeleteSurrounding { before: 3, after: 0 },
                ImeEvent::Commit("a".into()),
                ImeEvent::Preedit { text: "b".into(), cursor: None },
            ]
        );
    }

    #[test]
    fn empty_done_clears_a_showing_preedit_only() {
        let (ev, showing) = DoneBatch::default().into_events(true);
        assert_eq!(ev.as_slice(), [ImeEvent::Preedit { text: String::new(), cursor: None }]);
        assert!(!showing);
        assert!(DoneBatch::default().into_events(false).0.is_empty());
    }

    #[test]
    fn preedit_cursor_is_clamped() {
        let mut b = DoneBatch::default();
        b.preedit_string(Some("ab".into()), 1, 9);
        assert_eq!(b.preedit, Some(("ab".into(), Some((1, 2)))));
    }

    #[test]
    fn content_types() {
        let (h, p) = content_type(InputPurpose::Password, ImeHints::default());
        assert_eq!(p, ContentPurpose::Password);
        assert!(h.contains(ContentHint::HiddenText | ContentHint::SensitiveData));
        assert!(!h.contains(ContentHint::Completion));
        let (h, p) = content_type(InputPurpose::Code, ImeHints { multiline: true, ..Default::default() });
        assert_eq!(p, ContentPurpose::Normal);
        assert!(h.contains(ContentHint::Latin | ContentHint::Multiline));
        assert!(!h.contains(ContentHint::Spellcheck));
        let (h, _) = content_type(InputPurpose::Text, ImeHints::default());
        assert!(h.contains(ContentHint::Completion | ContentHint::Spellcheck | ContentHint::AutoCapitalization));
        let (h, _) = content_type(InputPurpose::Text, ImeHints { no_assist: true, ..Default::default() });
        assert!(h.is_empty());
    }

    #[test]
    fn surrounding_text_is_clamped_around_the_caret() {
        let short = clamp_surrounding("hello", 1..3);
        assert_eq!(short, ("hello".into(), 3, 1));
        let long = "é".repeat(3000); // 6000 bytes
        let (text, cursor, anchor) = clamp_surrounding(&long, 5000..5000);
        assert!(text.len() <= 4000);
        assert!(text.is_char_boundary(cursor as usize));
        assert_eq!(cursor, anchor);
        assert_eq!(&long[5000 - cursor as usize..5000], &text[..cursor as usize]);
    }

    /// A plain-string editor for the client bridge.
    struct Editor {
        text: String, // ASCII and BMP only: UTF-16 offsets computed by encode_utf16
        sel: Utf16Range,
        marked: Option<Utf16Range>,
    }

    impl Editor {
        fn byte(&self, u16: usize) -> usize {
            let mut units = 0;
            for (i, c) in self.text.char_indices() {
                if units >= u16 {
                    return i;
                }
                units += c.len_utf16();
            }
            self.text.len()
        }
    }

    impl TextInputClient for Editor {
        fn text_for_range(&self, r: Utf16Range) -> Option<String> {
            Some(self.text[self.byte(r.start)..self.byte(r.end)].to_owned())
        }
        fn selected_range(&self) -> Option<Utf16Range> {
            Some(self.sel.clone())
        }
        fn marked_range(&self) -> Option<Utf16Range> {
            self.marked.clone()
        }
        fn replace_text_in_range(&mut self, range: Option<Utf16Range>, text: &str) {
            let r = range.unwrap_or(self.sel.clone());
            let (a, b) = (self.byte(r.start), self.byte(r.end));
            self.text.replace_range(a..b, text);
            let caret = r.start + utf16_len(text);
            self.sel = caret..caret;
            self.marked = None;
        }
        fn replace_and_mark_text_in_range(&mut self, range: Option<Utf16Range>, text: &str, new_sel: Option<Utf16Range>) {
            let r = range.unwrap_or(self.sel.clone());
            let (a, b) = (self.byte(r.start), self.byte(r.end));
            self.text.replace_range(a..b, text);
            self.marked = Some(r.start..r.start + utf16_len(text));
            let s = new_sel.unwrap_or(utf16_len(text)..utf16_len(text));
            self.sel = r.start + s.start..r.start + s.end;
        }
        fn unmark_text(&mut self) {
            self.marked = None;
        }
        fn bounds_for_range(&self, _: Utf16Range) -> Option<Rect> {
            Some(Rect::new(0.0, 0.0, 1.0, 1.0))
        }
        fn character_index_for_point(&self, _: Point) -> Option<usize> {
            None
        }
        fn surrounding_text(&self, _: usize) -> Option<(String, std::ops::Range<usize>)> {
            Some((self.text.clone(), self.byte(self.sel.start)..self.byte(self.sel.end)))
        }
    }

    #[test]
    fn client_bridge_composes_and_commits() {
        let mut e = Editor { text: "ab".into(), sel: 2..2, marked: None };
        apply_to_client(&mut e, &[ImeEvent::Preedit { text: "ka".into(), cursor: Some((2, 2)) }]);
        assert_eq!((e.text.as_str(), e.marked.clone()), ("abka", Some(2..4)));
        apply_to_client(
            &mut e,
            &[ImeEvent::Preedit { text: String::new(), cursor: None }, ImeEvent::Commit("か".into())],
        );
        assert_eq!((e.text.as_str(), e.marked.clone(), e.sel.clone()), ("abか", None, 3..3));
    }

    #[test]
    fn client_bridge_deletes_surrounding_bytes() {
        let mut e = Editor { text: "xéyz".into(), sel: 2..2, marked: None }; // caret after "xé"
        // é is two UTF-8 bytes: delete it and "y" after the caret.
        apply_to_client(&mut e, &[ImeEvent::DeleteSurrounding { before: 2, after: 1 }]);
        assert_eq!((e.text.as_str(), e.sel.clone()), ("xz", 1..1));
    }
}
