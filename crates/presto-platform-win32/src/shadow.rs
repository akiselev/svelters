//! The IME's view of the focused text field. presto-platform's contract gives the platform the field
//! as data ([`ImeState`]: caret area, purpose, per-character composition rectangles, the text around
//! the caret with its selection) and takes edits back as [`ImeEvent`]s, so the TSF store and
//! `WM_IME_REQUEST` work on this shadow copy: a [`TextInputClient`] over the surrounding text plus
//! the composition, which turns each edit into events.
//!
//! Offsets are UTF-16 units of the shadow (the surrounding window, not the whole document): TSF only
//! needs them to be consistent. The marked text (composition) is part of the shadow but not of the
//! app's text; the app sees it as `Preedit`. Edits that remove committed text around the selection
//! become `DeleteSurrounding` (bytes before and after the selection), then `Commit` replaces the
//! selection; edits elsewhere in the window cannot be expressed and only commit their text.

use presto_base::event::ImeEvent;
use presto_base::geom::{Point, Rect};
use presto_base::input::{InputPurpose, TextInputClient, Utf16Range};
use presto_platform::ImeState;

#[derive(Debug, Default)]
pub struct ShadowClient {
    text: Vec<u16>,
    selection: Utf16Range,
    marked: Option<Utf16Range>,
    area: Rect,
    composition_rects: Vec<Rect>,
    purpose: InputPurpose,
    events: Vec<ImeEvent>,
}

/// UTF-8 length of UTF-16 units.
fn utf8_len(units: &[u16]) -> usize {
    char::decode_utf16(units.iter().copied()).map(|c| c.map_or(3, char::len_utf8)).sum()
}

fn clamp(r: &Utf16Range, len: usize) -> Utf16Range {
    let start = r.start.min(len);
    start..r.end.clamp(start, len)
}

impl ShadowClient {
    /// Takes the app's latest field state. While a composition runs the shadow keeps its own text
    /// (the app's surrounding text has no preedit in it); only the geometry follows.
    pub fn set_state(&mut self, state: &ImeState) {
        self.area = state.area;
        self.composition_rects = state.composition_rects.clone();
        self.purpose = state.purpose;
        if self.marked.is_some() {
            return;
        }
        match &state.surrounding {
            Some((text, sel)) => {
                self.text = text.encode_utf16().collect();
                self.selection = clamp(sel, self.text.len());
            }
            None => {
                self.text.clear();
                self.selection = 0..0;
            }
        }
    }

    /// Drops the composition without committing it (the app cancelled it).
    pub fn cancel_composition(&mut self) {
        let Some(m) = self.marked.take() else { return };
        self.text.drain(m.clone());
        self.selection = m.start..m.start;
        self.events.push(ImeEvent::Preedit { text: String::new(), cursor: None });
    }

    /// The events the edits since the last call produced, in order.
    pub fn take_events(&mut self) -> Vec<ImeEvent> {
        std::mem::take(&mut self.events)
    }

    fn string(&self, r: &Utf16Range) -> String {
        String::from_utf16_lossy(&self.text[clamp(r, self.text.len())])
    }

    /// A shadow offset in the app's text, where the marked text does not exist.
    fn committed(&self, x: usize) -> usize {
        match &self.marked {
            Some(m) if x >= m.end => x - (m.end - m.start),
            Some(m) if x > m.start => m.start,
            _ => x,
        }
    }

    /// The app's selection in committed offsets: a composition sits at its caret.
    fn app_selection(&self) -> Utf16Range {
        match &self.marked {
            Some(m) => self.committed(m.start)..self.committed(m.start),
            None => self.selection.clone(),
        }
    }

    /// The app's text (without the marked text).
    fn committed_text(&self) -> Vec<u16> {
        match &self.marked {
            Some(m) => self.text[..m.start].iter().chain(&self.text[m.end..]).copied().collect(),
            None => self.text.clone(),
        }
    }

    /// Deletes the committed text `range` covers outside the app's selection, if `range` contains
    /// the selection (otherwise the edit is out of reach and nothing is deleted).
    fn delete_around_selection(&mut self, range: &Utf16Range) {
        let r = self.committed(range.start)..self.committed(range.end);
        let sel = self.app_selection();
        if r.start > sel.start || r.end < sel.end {
            return;
        }
        let committed = self.committed_text();
        let before = utf8_len(&committed[r.start..sel.start]);
        let after = utf8_len(&committed[sel.end..r.end]);
        if before > 0 || after > 0 {
            self.events.push(ImeEvent::DeleteSurrounding { before, after });
        }
    }

    fn splice(&mut self, range: &Utf16Range, units: &[u16]) {
        self.text.splice(range.clone(), units.iter().copied());
    }

    fn preedit_event(&self) -> ImeEvent {
        let Some(m) = &self.marked else { return ImeEvent::Preedit { text: String::new(), cursor: None } };
        let marked = &self.text[m.clone()];
        let rel = |x: usize| utf8_len(&marked[..x.saturating_sub(m.start).min(marked.len())]);
        ImeEvent::Preedit {
            text: String::from_utf16_lossy(marked),
            cursor: Some((rel(self.selection.start), rel(self.selection.end))),
        }
    }
}

impl TextInputClient for ShadowClient {
    fn text_for_range(&self, range: Utf16Range) -> Option<String> {
        Some(self.string(&range))
    }

    fn selected_range(&self) -> Option<Utf16Range> {
        Some(self.selection.clone())
    }

    fn marked_range(&self) -> Option<Utf16Range> {
        self.marked.clone()
    }

    fn replace_text_in_range(&mut self, range: Option<Utf16Range>, text: &str) {
        let len = self.text.len();
        let r = clamp(&range.or_else(|| self.marked.clone()).unwrap_or_else(|| self.selection.clone()), len);
        if self.marked.is_some() {
            self.events.push(ImeEvent::Preedit { text: String::new(), cursor: None });
        }
        self.delete_around_selection(&r);
        self.events.push(ImeEvent::Commit(text.to_owned()));
        let units: Vec<u16> = text.encode_utf16().collect();
        self.splice(&r, &units);
        self.marked = None;
        let caret = r.start + units.len();
        self.selection = caret..caret;
    }

    fn replace_and_mark_text_in_range(&mut self, range: Option<Utf16Range>, text: &str, new_selected: Option<Utf16Range>) {
        let len = self.text.len();
        let r = clamp(&range.or_else(|| self.marked.clone()).unwrap_or_else(|| self.selection.clone()), len);
        // Committed text the composition takes over (reconversion) leaves the app's text.
        let outside = match &self.marked {
            Some(m) => r.start < m.start || r.end > m.end,
            None => true,
        };
        if outside {
            self.delete_around_selection(&r);
        }
        let units: Vec<u16> = text.encode_utf16().collect();
        let old = self.marked.clone();
        self.splice(&r, &units);
        let new_end = r.start + units.len();
        let marked = match old {
            Some(m) => {
                let end = if m.end >= r.end { m.end - (r.end - r.start) + units.len() } else { new_end };
                m.start.min(r.start)..end.max(new_end)
            }
            None => r.start..new_end,
        };
        let sel = new_selected.map(|s| clamp(&s, units.len())).unwrap_or(units.len()..units.len());
        self.selection = r.start + sel.start..r.start + sel.end;
        self.marked = if marked.is_empty() { None } else { Some(marked) };
        let ev = self.preedit_event();
        self.events.push(ev);
    }

    fn unmark_text(&mut self) {
        let Some(m) = self.marked.take() else { return };
        let text = String::from_utf16_lossy(&self.text[m]);
        self.events.push(ImeEvent::Preedit { text: String::new(), cursor: None });
        self.events.push(ImeEvent::Commit(text));
    }

    fn bounds_for_range(&self, range: Utf16Range) -> Option<Rect> {
        if let Some(m) = &self.marked
            && !self.composition_rects.is_empty()
            && range.start >= m.start
            && range.start <= m.end
        {
            let first = (range.start - m.start).min(self.composition_rects.len() - 1);
            let last = (range.end.max(range.start + 1) - 1 - m.start).clamp(first, self.composition_rects.len() - 1);
            let rects = &self.composition_rects[first..=last];
            return Some(rects.iter().skip(1).fold(rects[0], |a, b| a.union(b)));
        }
        Some(self.area)
    }

    fn character_index_for_point(&self, _p: Point) -> Option<usize> {
        None
    }

    fn surrounding_text(&self, max_bytes: usize) -> Option<(String, std::ops::Range<usize>)> {
        let s = String::from_utf16_lossy(&self.text);
        if s.len() > max_bytes {
            return None;
        }
        let start = utf8_len(&self.text[..self.selection.start]);
        let end = utf8_len(&self.text[..self.selection.end]);
        Some((s, start..end))
    }

    fn purpose(&self) -> InputPurpose {
        self.purpose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(text: &str, sel: Utf16Range) -> ShadowClient {
        let mut c = ShadowClient::default();
        c.set_state(&ImeState { surrounding: Some((text.into(), sel)), ..ImeState::default() });
        c
    }

    #[test]
    fn composition_then_commit() {
        let mut c = field("ab", 2..2);
        c.replace_and_mark_text_in_range(None, "に", Some(1..1));
        assert_eq!(c.take_events(), vec![ImeEvent::Preedit { text: "に".into(), cursor: Some((3, 3)) }]);
        c.replace_and_mark_text_in_range(None, "にほ", Some(2..2));
        assert_eq!(c.marked_range(), Some(2..4));
        assert_eq!(c.take_events(), vec![ImeEvent::Preedit { text: "にほ".into(), cursor: Some((6, 6)) }]);
        c.replace_text_in_range(None, "日本");
        assert_eq!(
            c.take_events(),
            vec![ImeEvent::Preedit { text: String::new(), cursor: None }, ImeEvent::Commit("日本".into())]
        );
        assert_eq!(c.text_for_range(0..10).unwrap(), "ab日本");
        assert_eq!(c.selected_range(), Some(4..4));
    }

    #[test]
    fn unmark_commits_the_marked_text() {
        let mut c = field("", 0..0);
        c.replace_and_mark_text_in_range(None, "한", None);
        c.take_events();
        c.unmark_text();
        assert_eq!(c.take_events(), vec![ImeEvent::Preedit { text: String::new(), cursor: None }, ImeEvent::Commit("한".into())]);
        assert_eq!(c.marked_range(), None);
    }

    #[test]
    fn reconversion_deletes_around_the_selection() {
        // "東京" selected, reconverted: the composition takes over the selection itself.
        let mut c = field("x東京y", 1..3);
        c.replace_and_mark_text_in_range(Some(1..3), "とうきょう", None);
        assert_eq!(c.take_events(), vec![ImeEvent::Preedit { text: "とうきょう".into(), cursor: Some((15, 15)) }]);
        // A TIP replacing the word around a caret: the bytes before and after the caret go first.
        let mut c = field("hello", 3..3);
        c.replace_text_in_range(Some(1..5), "EY");
        assert_eq!(
            c.take_events(),
            vec![ImeEvent::DeleteSurrounding { before: 2, after: 2 }, ImeEvent::Commit("EY".into())]
        );
        assert_eq!(c.text_for_range(0..10).unwrap(), "hEY");
    }

    #[test]
    fn state_updates_wait_for_the_composition() {
        let mut c = field("ab", 2..2);
        c.replace_and_mark_text_in_range(None, "k", None);
        c.set_state(&ImeState { surrounding: Some(("zz".into(), 0..0)), ..ImeState::default() });
        assert_eq!(c.text_for_range(0..10).unwrap(), "abk", "the composition keeps the shadow");
        c.unmark_text();
        c.set_state(&ImeState { surrounding: Some(("abk".into(), 3..3)), ..ImeState::default() });
        assert_eq!(c.selected_range(), Some(3..3));
    }

    #[test]
    fn per_character_bounds() {
        let mut c = field("", 0..0);
        c.set_state(&ImeState {
            composition_rects: vec![Rect::new(0.0, 0.0, 10.0, 20.0), Rect::new(10.0, 0.0, 20.0, 20.0)],
            area: Rect::new(0.0, 0.0, 1.0, 20.0),
            surrounding: Some((String::new(), 0..0)),
            ..ImeState::default()
        });
        c.replace_and_mark_text_in_range(None, "ab", None);
        assert_eq!(c.bounds_for_range(1..2), Some(Rect::new(10.0, 0.0, 20.0, 20.0)));
        assert_eq!(c.bounds_for_range(0..2), Some(Rect::new(0.0, 0.0, 20.0, 20.0)));
    }
}
