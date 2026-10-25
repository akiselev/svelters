//! Keyboard and pointer selection over a flat, ordered list of item ids (svelters-egui
//! `views/selection.rs`, WG5), shared by `table` and, once they adopt it, the views lane's
//! `virtual-list` and `tree-view`. Selection is keyed by id, not position, so it survives
//! filtering, sorting and splices: ids that leave the list stay selected and come back selected.
//!
//! - Up/Down move the cursor one row, PageUp/PageDown by `page` rows, Home/End to the ends.
//! - Plain moves select the cursor (single and multi); Shift extends a range from the anchor
//!   (multi); Ctrl (Cmd) moves the cursor without selecting (multi), and Space toggles it.
//! - Clicks: plain selects only that row, Ctrl toggles, Shift selects the range from the anchor.
//! - `toggle` (Qt `MultiSelection`): a click toggles its row with no modifier and leaves the
//!   others; moves only move the cursor; Space toggles.
//! - `range` (Qt `ContiguousSelection`): one contiguous run; Shift (click or move) selects the
//!   run from the anchor; Ctrl does what a plain input does.
//! - Type-ahead: typed characters within [`TYPEAHEAD_RESET`] seconds build a prefix; the cursor
//!   goes to the next row (from the cursor, wrapping) whose label starts with it, ignoring case.
//!   Repeating one character cycles through the rows starting with it.

use std::collections::BTreeSet;

/// Seconds of no typing after which type-ahead starts a new prefix.
pub const TYPEAHEAD_RESET: f64 = 1.0;

/// How the view selects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    None,
    #[default]
    Single,
    Multi,
    /// A click toggles, no modifier (Qt `MultiSelection`).
    Toggle,
    /// One contiguous run, Shift extends (Qt `ContiguousSelection`).
    Range,
}

impl Mode {
    pub const NAMES: &'static [&'static str] = &["none", "single", "multi", "toggle", "range"];

    pub fn parse(s: &str) -> Option<Mode> {
        Some(match s {
            "none" => Mode::None,
            "single" => Mode::Single,
            "multi" => Mode::Multi,
            "toggle" => Mode::Toggle,
            "range" => Mode::Range,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Mode::NAMES[self as usize]
    }

    /// More than one row can be selected (AccessKit `multiselectable`, Ctrl+A).
    pub fn is_multi(self) -> bool {
        matches!(self, Mode::Multi | Mode::Toggle | Mode::Range)
    }
}

/// A navigation key, decoded from the key event by the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// The modifiers that matter to selection (`ctrl` is Cmd on macOS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

/// What an input changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The cursor moved (scroll it into view).
    pub cursor: bool,
    /// The selected set changed (raise `selectionchange`).
    pub selection: bool,
}

/// The first (`forward`) or last item `enabled` among `len`.
pub fn edge_by(len: usize, forward: bool, enabled: impl Fn(usize) -> bool) -> Option<usize> {
    if forward { (0..len).find(|&i| enabled(i)) } else { (0..len).rev().find(|&i| enabled(i)) }
}

/// The enabled item `step` away from `from` among `len` (svelters-egui `RovingList::next_by`): no
/// item from `from` on stays at `from`, unless `wrap`; no `from` starts at the edge `step` points
/// away from.
pub fn next_by(len: usize, from: Option<usize>, step: i32, wrap: bool, enabled: impl Fn(usize) -> bool) -> Option<usize> {
    let n = len as i64;
    let Some(from) = from.filter(|f| (*f as i64) < n) else { return edge_by(len, step > 0, enabled) };
    let mut i = from as i64;
    for _ in 0..n {
        i += step as i64;
        if i < 0 || i >= n {
            if !wrap {
                return Some(from);
            }
            i = i.rem_euclid(n);
        }
        if enabled(i as usize) {
            return Some(i as usize);
        }
    }
    Some(from)
}

/// The selection state of one view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeySelect {
    pub mode: Mode,
    /// The focused row (keyboard cursor).
    pub cursor: Option<u64>,
    /// Where Shift ranges start.
    pub anchor: Option<u64>,
    pub selected: BTreeSet<u64>,
    typed: String,
    typed_at: f64,
}

fn pos_of(ids: &[u64], id: Option<u64>) -> Option<usize> {
    id.and_then(|id| ids.iter().position(|x| *x == id))
}

impl KeySelect {
    pub fn new(mode: Mode) -> KeySelect {
        KeySelect { mode, ..KeySelect::default() }
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    /// Replaces the selection (the host's `selected` write); single mode keeps the first id.
    pub fn set(&mut self, ids: impl IntoIterator<Item = u64>) -> bool {
        let mut new: BTreeSet<u64> = ids.into_iter().collect();
        match self.mode {
            Mode::None => new.clear(),
            Mode::Single => new = new.into_iter().take(1).collect(),
            Mode::Multi | Mode::Toggle | Mode::Range => {}
        }
        let changed = new != self.selected;
        self.selected = new;
        changed
    }

    fn only(&mut self, id: u64) -> bool {
        if self.mode == Mode::None {
            return false;
        }
        let changed = !(self.selected.len() == 1 && self.selected.contains(&id));
        self.selected.clear();
        self.selected.insert(id);
        changed
    }

    fn range(&mut self, ids: &[u64], to: usize) -> bool {
        let from = pos_of(ids, self.anchor).unwrap_or(to);
        let (a, b) = if from <= to { (from, to) } else { (to, from) };
        let new: BTreeSet<u64> = ids[a..=b].iter().copied().collect();
        let changed = new != self.selected;
        self.selected = new;
        changed
    }

    fn land(&mut self, ids: &[u64], to: usize, m: Mods) -> Change {
        let id = ids[to];
        let moved = self.cursor != Some(id);
        self.cursor = Some(id);
        let selection = match self.mode {
            Mode::None | Mode::Toggle => false,
            Mode::Multi | Mode::Range if m.shift => self.range(ids, to),
            Mode::Multi if m.ctrl => false,
            _ => {
                self.anchor = Some(id);
                self.only(id)
            }
        };
        Change { cursor: moved, selection }
    }

    /// A navigation key over the visible `ids`; `page` is the rows a page moves.
    pub fn nav(&mut self, ids: &[u64], key: Nav, m: Mods, page: usize) -> Change {
        self.nav_by(ids, key, m, page, |_| true)
    }

    /// [`nav`](Self::nav) with the rows that can take the cursor (`enabled(pos)`, asked lazily).
    /// Up/Down/Home/End do not wrap.
    pub fn nav_by(&mut self, ids: &[u64], key: Nav, m: Mods, page: usize, enabled: impl Fn(usize) -> bool) -> Change {
        if ids.is_empty() {
            return Change::default();
        }
        let (len, last) = (ids.len(), ids.len() - 1);
        let to = match (pos_of(ids, self.cursor), key) {
            (_, Nav::Home) => edge_by(len, true, &enabled),
            (_, Nav::End) => edge_by(len, false, &enabled),
            // no cursor (or it was filtered away): any move starts at the top
            (None, _) => edge_by(len, true, &enabled),
            (Some(p), Nav::Up) => next_by(len, Some(p), -1, false, &enabled),
            (Some(p), Nav::Down) => next_by(len, Some(p), 1, false, &enabled),
            (Some(p), Nav::PageUp) => Some(p.saturating_sub(page.max(1))),
            (Some(p), Nav::PageDown) => Some((p + page.max(1)).min(last)),
        };
        match to {
            Some(to) => self.land(ids, to, m),
            None => Change::default(),
        }
    }

    /// Moves the cursor to `id` (a tree's parent or first child) as a plain navigation would.
    pub fn go_to(&mut self, ids: &[u64], id: u64, m: Mods) -> Change {
        match ids.iter().position(|x| *x == id) {
            Some(p) => self.land(ids, p, m),
            None => Change::default(),
        }
    }

    /// Space: toggles the cursor's row (multi), selects it (single).
    pub fn toggle_cursor(&mut self) -> Change {
        let Some(id) = self.cursor else { return Change::default() };
        match self.mode {
            Mode::None => Change::default(),
            Mode::Single => Change { cursor: false, selection: self.only(id) },
            Mode::Multi | Mode::Toggle => {
                if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
                self.anchor = Some(id);
                Change { cursor: false, selection: true }
            }
            Mode::Range => {
                self.anchor = Some(id);
                Change { cursor: false, selection: self.only(id) }
            }
        }
    }

    /// A click on the row `id`.
    pub fn click(&mut self, ids: &[u64], id: u64, m: Mods) -> Change {
        let moved = self.cursor != Some(id);
        self.cursor = Some(id);
        let selection = match self.mode {
            Mode::None => false,
            Mode::Toggle | Mode::Multi if self.mode == Mode::Toggle || m.ctrl => {
                if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
                self.anchor = Some(id);
                true
            }
            Mode::Multi | Mode::Range if m.shift => match pos_of(ids, Some(id)) {
                Some(p) => self.range(ids, p),
                None => false,
            },
            _ => {
                self.anchor = Some(id);
                self.only(id)
            }
        };
        Change { cursor: moved, selection }
    }

    /// Typed text at time `now` (seconds); `label(id)` is a row's type-ahead text. Moves the cursor
    /// (and selects as a plain move does) to the next matching row.
    pub fn type_ahead(&mut self, ids: &[u64], text: &str, now: f64, label: impl Fn(u64) -> String) -> Change {
        if ids.is_empty() || text.is_empty() {
            return Change::default();
        }
        if now - self.typed_at > TYPEAHEAD_RESET {
            self.typed.clear();
        }
        self.typed_at = now;
        self.typed.push_str(&text.to_lowercase());
        // one repeated character cycles through the rows that start with it
        let mut chars = self.typed.chars();
        let first = chars.next();
        let repeat = first.is_some() && chars.all(|c| Some(c) == first) && self.typed.chars().count() > 1;
        let prefix: String = if repeat { first.into_iter().collect() } else { self.typed.clone() };
        let start = pos_of(ids, self.cursor).map_or(0, |p| if repeat || prefix.chars().count() == 1 { p + 1 } else { p });
        let n = ids.len();
        let hit = (0..n).map(|k| (start + k) % n).find(|&p| label(ids[p]).to_lowercase().starts_with(&prefix));
        match hit {
            Some(p) => self.land(ids, p, Mods::default()),
            None => Change::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: [u64; 5] = [10, 20, 30, 40, 50];
    const SHIFT: Mods = Mods { shift: true, ctrl: false };
    const CTRL: Mods = Mods { shift: false, ctrl: true };

    fn sel(k: &KeySelect) -> Vec<u64> {
        k.selected.iter().copied().collect()
    }

    #[test]
    fn arrows_move_and_select_single() {
        let mut k = KeySelect::new(Mode::Single);
        assert_eq!(k.nav(&IDS, Nav::Down, Mods::default(), 3), Change { cursor: true, selection: true });
        assert_eq!(k.cursor, Some(10));
        k.nav(&IDS, Nav::Down, Mods::default(), 3);
        k.nav(&IDS, Nav::PageDown, Mods::default(), 3);
        assert_eq!((k.cursor, sel(&k)), (Some(50), vec![50]));
        k.nav(&IDS, Nav::Home, SHIFT, 3);
        assert_eq!(sel(&k), vec![10], "shift does not extend in single mode");
        assert_eq!(k.nav(&[], Nav::Down, Mods::default(), 3), Change::default());
    }

    #[test]
    fn multi_ranges_toggles_and_ctrl_moves() {
        let mut k = KeySelect::new(Mode::Multi);
        k.click(&IDS, 20, Mods::default());
        k.nav(&IDS, Nav::Down, SHIFT, 3);
        k.nav(&IDS, Nav::Down, SHIFT, 3);
        assert_eq!(sel(&k), vec![20, 30, 40]);
        k.nav(&IDS, Nav::Up, CTRL, 3);
        assert_eq!((k.cursor, sel(&k)), (Some(30), vec![20, 30, 40]), "ctrl moves without selecting");
        k.toggle_cursor();
        assert_eq!(sel(&k), vec![20, 40]);
        k.click(&IDS, 50, CTRL);
        assert_eq!(sel(&k), vec![20, 40, 50]);
        k.click(&IDS, 30, SHIFT);
        assert_eq!(sel(&k), vec![30, 40, 50], "a shift click ranges from the last anchor (50)");
    }

    #[test]
    fn toggle_and_range_modes() {
        let mut t = KeySelect::new(Mode::Toggle);
        t.click(&IDS, 20, Mods::default());
        t.click(&IDS, 40, Mods::default());
        assert_eq!(sel(&t), vec![20, 40], "a plain click toggles and keeps the others");
        t.click(&IDS, 20, Mods::default());
        assert_eq!(sel(&t), vec![40]);
        assert_eq!(t.nav(&IDS, Nav::Down, Mods::default(), 3), Change { cursor: true, selection: false }, "moves only move");
        t.toggle_cursor();
        assert_eq!(sel(&t), vec![30, 40], "space toggles the cursor (30)");
        let mut r = KeySelect::new(Mode::Range);
        r.click(&IDS, 20, Mods::default());
        r.click(&IDS, 40, SHIFT);
        assert_eq!(sel(&r), vec![20, 30, 40], "shift: the run from the anchor");
        r.click(&IDS, 50, CTRL);
        assert_eq!(sel(&r), vec![50], "ctrl does not add a second run");
        r.nav(&IDS, Nav::Up, SHIFT, 3);
        assert_eq!(sel(&r), vec![40, 50]);
        r.toggle_cursor();
        assert_eq!(sel(&r), vec![40], "space selects the cursor only");
        assert!(Mode::Range.is_multi() && Mode::Toggle.is_multi() && !Mode::Single.is_multi());
    }

    #[test]
    fn selection_is_by_id_and_survives_filtering() {
        let mut k = KeySelect::new(Mode::Multi);
        k.set([20, 40]);
        let filtered = [10, 30, 50];
        k.nav(&filtered, Nav::Down, CTRL, 3);
        assert_eq!(sel(&k), vec![20, 40], "hidden rows stay selected");
        assert!(k.is_selected(40));
        let mut s = KeySelect::new(Mode::Single);
        s.set([30, 20]);
        assert_eq!(sel(&s), vec![20]);
    }

    #[test]
    fn type_ahead_prefix_cycle_and_reset() {
        let labels = |id: u64| match id {
            10 => "alpha",
            20 => "beta",
            30 => "bravo",
            40 => "Charlie",
            _ => "bob",
        }
        .to_string();
        let mut k = KeySelect::new(Mode::Single);
        k.type_ahead(&IDS, "b", 0.0, labels);
        assert_eq!(k.cursor, Some(20));
        k.type_ahead(&IDS, "r", 0.2, labels);
        assert_eq!(k.cursor, Some(30), "the prefix grows: br");
        k.type_ahead(&IDS, "c", 2.0, labels);
        assert_eq!(k.cursor, Some(40), "a pause starts a new prefix, ignoring case");
        k.type_ahead(&IDS, "b", 4.0, labels);
        k.type_ahead(&IDS, "b", 4.1, labels);
        assert_eq!(k.cursor, Some(20), "repeating b cycles: 50 then (wrapping) 20");
        assert_eq!(sel(&k), vec![20]);
        assert_eq!(k.type_ahead(&IDS, "z", 9.0, labels), Change::default());
    }

    #[test]
    fn roving_steps_stop_at_the_ends_without_wrap() {
        assert_eq!(next_by(3, Some(2), 1, false, |_| true), Some(2));
        assert_eq!(next_by(3, Some(2), 1, true, |_| true), Some(0));
        assert_eq!(next_by(3, None, -1, false, |_| true), Some(2));
        assert_eq!(next_by(3, Some(0), 1, false, |i| i != 1), Some(2));
        assert_eq!(edge_by(0, true, |_| true), None);
    }
}
