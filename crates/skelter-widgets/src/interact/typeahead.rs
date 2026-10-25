//! `Typeahead`: printable keys typed within a timeout build a prefix; the kind moves to the next
//! item whose label starts with it ([`find`]). Typing one letter again and again cycles through the
//! items starting with it (ARIA APG, Qt `keyboardSearch`).

use super::{InputCx, Intents, StateBits, Unit};

/// What [`Typeahead`] is given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeaheadParams {
    /// Seconds after the last key before a new prefix starts (1 s).
    pub timeout: f64,
}

impl Default for TypeaheadParams {
    fn default() -> Self {
        TypeaheadParams { timeout: 1.0 }
    }
}

/// What [`Typeahead`] reports: the prefix typed so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Search {
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct Typeahead {
    buf: String,
    last: f64,
}

impl Typeahead {
    /// Adds `text` typed at `now`; returns the prefix to search.
    pub fn push(&mut self, p: &TypeaheadParams, text: &str, now: f64) -> Option<String> {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if text.is_empty() || (self.buf.is_empty() && text.trim().is_empty()) {
            return None;
        }
        if now - self.last > p.timeout {
            self.buf.clear();
        }
        self.last = now;
        self.buf.push_str(&text);
        Some(self.buf.clone())
    }

    /// Ends the current prefix (the kind closed, the pointer picked).
    pub fn reset(&mut self) {
        self.buf.clear();
    }

    /// Whether a prefix is being typed (a Space then belongs to it, not to activation).
    pub fn typing(&self, p: &TypeaheadParams, now: f64) -> bool {
        !self.buf.is_empty() && now - self.last <= p.timeout
    }
}

impl Unit for Typeahead {
    type Params = TypeaheadParams;
    type Intent = Search;
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &TypeaheadParams, cx: &mut InputCx<'_>, out: &mut Intents<Search>) {
        if !cx.enabled || !cx.resp.has_focus {
            return;
        }
        let now = cx.now;
        for t in cx.input.text() {
            if let Some(text) = self.push(p, t, now) {
                out.push(Search { text });
            }
        }
    }
}

/// The item for prefix `query` among `labels` (case-insensitive), searching from `current`: the
/// current item itself while it still matches a longer prefix, else the next one that matches,
/// wrapping. A prefix of one repeated letter (`"ff"`) cycles through the items starting with it.
/// `enabled` (same length, or empty for all) skips items that cannot be picked.
pub fn find<'a>(labels: impl IntoIterator<Item = &'a str>, enabled: &[bool], query: &str, current: Option<usize>) -> Option<usize> {
    let labels: Vec<String> = labels.into_iter().map(|l| l.trim_start().to_lowercase()).collect();
    let q = query.to_lowercase();
    let mut chars = q.chars();
    let first = chars.next()?;
    let repeated = chars.all(|c| c == first);
    let n = labels.len();
    let ok = |i: usize| enabled.get(i).copied().unwrap_or(true);
    // one letter repeated: the next item after the current one starting with it
    let (needle, start) = if repeated && q.chars().count() > 1 {
        (first.to_string(), current.map_or(0, |c| c + 1))
    } else if q.chars().count() == 1 {
        (q.clone(), current.map_or(0, |c| c + 1))
    } else {
        (q.clone(), current.unwrap_or(0))
    };
    (0..n).map(|k| (start + k) % n).find(|&i| ok(i) && labels[i].starts_with(&needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::Sense;
    use crate::interact::tracker::testing::Rig;

    #[test]
    fn prefixes_cycle_and_time_out() {
        let labels = ["Alpha", "Beta", "beer", "Gamma", "Bravo"];
        assert_eq!(find(labels, &[], "b", None), Some(1));
        assert_eq!(find(labels, &[], "b", Some(1)), Some(2), "one letter: the next one");
        assert_eq!(find(labels, &[], "bb", Some(2)), Some(4), "repeated: cycles");
        assert_eq!(find(labels, &[], "be", Some(1)), Some(1), "a longer prefix keeps a matching current");
        assert_eq!(find(labels, &[], "bee", Some(1)), Some(2));
        assert_eq!(find(labels, &[true, false, true, true, true], "b", None), Some(2), "skips disabled");
        assert_eq!(find(labels, &[], "z", None), None);
        let mut t = Typeahead::default();
        let p = TypeaheadParams::default();
        assert_eq!(t.push(&p, "b", 0.0).as_deref(), Some("b"));
        assert_eq!(t.push(&p, "e", 0.5).as_deref(), Some("be"));
        assert_eq!(t.push(&p, "g", 2.0).as_deref(), Some("g"), "after the timeout a new prefix");
        assert!(t.typing(&p, 2.5) && !t.typing(&p, 3.5));
        let mut u = Typeahead::default();
        assert_eq!(u.push(&p, " ", 0.0), None, "a leading space is not a search");
    }

    #[test]
    fn typed_text_on_the_focused_host() {
        let mut rig = Rig::new(Sense::FOCUSABLE);
        let mut u = Typeahead::default();
        let p = TypeaheadParams::default();
        let mut out = Intents::default();
        rig.text("a", |cx| u.input(&p, cx, &mut out));
        assert!(out.is_empty(), "not focused");
        rig.focus(true, |_| {});
        rig.text("a", |cx| u.input(&p, cx, &mut out));
        rig.text("b", |cx| u.input(&p, cx, &mut out));
        assert_eq!(out.0, vec![Search { text: "a".into() }, Search { text: "ab".into() }]);
    }
}
