//! WG5a: the egui side of item-view keyboard selection, on WG5b's shared [`KeySelect`]
//! (`views/selection.rs`, kept identical on both lanes).
//!
//! [`read_keys`] takes this pass's navigation keys and typed text from egui's input for the
//! focused view and consumes them (key handling stays local to the focused list or table: no
//! enclosing widget or focus navigation sees them). [`apply`] feeds them to the view's
//! [`KeySelect`] over its ordered row ids and folds the results into one [`Outcome`]. [`RowKeys`] is
//! the ids-and-labels store of `virtual-list` (JS ids) and `table` (`set-ids`).
//!
//! Additive to `selection.rs`: Ctrl+A selects all (multi), Enter activates the cursor row.
//!
//! Writer lane: WG5a. WG5b's `tree-view` can use the same `read_keys`/`apply` and handle
//! Left/Right itself before them.

use super::selection::{Change, KeySelect, Mode, Mods, Nav};
use std::collections::HashMap;

/// One input a view's selection takes.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Move(Nav, Mods),
    /// Space: select (single) or toggle (multi) the cursor row.
    Space,
    /// Ctrl+A.
    SelectAll,
    /// Enter: activate the cursor row.
    Activate,
    /// Typed text (type-ahead), with the input time in seconds.
    Text(String, f64),
}

/// What [`apply`] changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The cursor moved: scroll it into view.
    pub cursor: bool,
    /// The selected set changed: emit `selectionchange`.
    pub selection: bool,
    /// Enter on the cursor row (its position).
    pub activate: Option<usize>,
}

impl Outcome {
    pub fn add(&mut self, c: Change) {
        self.cursor |= c.cursor;
        self.selection |= c.selection;
    }
}

pub fn mods(m: egui::Modifiers) -> Mods {
    Mods { shift: m.shift, ctrl: m.command }
}

/// The keys a focused view takes this pass, consumed from egui's input. `text`: also take typed
/// text (type-ahead); a view that starts an edit on typing reads text itself and passes false.
pub fn read_keys(ui: &egui::Ui, text: bool) -> Vec<Input> {
    use egui::{Event, Key};
    let now = ui.input(|i| i.time);
    ui.ctx().input_mut(|i| {
        let mut out = Vec::new();
        i.events.retain(|e| {
            let taken = match e {
                Event::Key { key, pressed: true, modifiers, .. } => {
                    let m = mods(*modifiers);
                    match key {
                        Key::ArrowUp => Some(Input::Move(Nav::Up, m)),
                        Key::ArrowDown => Some(Input::Move(Nav::Down, m)),
                        Key::PageUp => Some(Input::Move(Nav::PageUp, m)),
                        Key::PageDown => Some(Input::Move(Nav::PageDown, m)),
                        Key::Home => Some(Input::Move(Nav::Home, m)),
                        Key::End => Some(Input::Move(Nav::End, m)),
                        Key::Space => Some(Input::Space),
                        Key::A if m.ctrl && !m.shift => Some(Input::SelectAll),
                        Key::Enter => Some(Input::Activate),
                        _ => None,
                    }
                }
                // Space arrives as a key (above) and as text
                Event::Text(t) if text && t != " " => Some(Input::Text(t.clone(), now)),
                _ => None,
            };
            match taken {
                Some(x) => {
                    out.push(x);
                    false
                }
                None => true,
            }
        });
        out
    })
}

/// While a view has focus, egui's focus navigation leaves its arrow keys to it (Tab and Escape
/// still move or drop focus).
pub fn lock_arrows(ui: &egui::Ui, id: egui::Id) {
    ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
}

/// Applies `inputs` to `sel` over the rows `ids` (in order); `page` rows per page move;
/// `label(pos)` is a row's type-ahead text.
pub fn apply(sel: &mut KeySelect, ids: &[u64], inputs: &[Input], page: usize, label: impl Fn(usize) -> Option<String>) -> Outcome {
    let mut out = Outcome::default();
    for i in inputs {
        match i {
            Input::Move(nav, m) => out.add(sel.nav(ids, *nav, *m, page)),
            Input::Space => {
                if sel.cursor.is_none() {
                    out.add(sel.nav(ids, Nav::Home, Mods::default(), page));
                } else {
                    out.add(sel.toggle_cursor());
                }
            }
            Input::SelectAll if sel.mode == Mode::Multi => {
                // hidden (filtered out) selected rows stay selected
                let all: Vec<u64> = ids.iter().copied().chain(sel.selected.iter().copied()).collect();
                out.selection |= sel.set(all);
            }
            Input::SelectAll => {}
            Input::Activate => out.activate = sel.cursor.and_then(|c| ids.iter().position(|&x| x == c)),
            Input::Text(t, now) => {
                // the label by id: one index per typed text (type-ahead scans every row once)
                let pos: HashMap<u64, usize> = ids.iter().enumerate().map(|(p, &k)| (k, p)).collect();
                out.add(sel.type_ahead(ids, t, *now, |id| pos.get(&id).and_then(|&p| label(p)).unwrap_or_default()));
            }
        }
    }
    out
}

/// Row ids in order with optional type-ahead labels (`virtual-list`'s JS ids, `table`'s `set-ids`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowKeys {
    pub ids: Vec<u64>,
    /// Empty, or one per id.
    pub labels: Vec<String>,
}

impl RowKeys {
    /// Replaces `removed` ids at `pos` with `ids` (and their labels: `labels` when given, else empty).
    pub fn splice(&mut self, pos: usize, removed: usize, ids: &[u64], labels: Option<&[String]>) {
        let old = self.ids.len();
        self.ids.splice(pos..pos + removed, ids.iter().copied());
        if labels.is_some() || !self.labels.is_empty() {
            self.labels.resize(old, String::new());
            let add = (0..ids.len()).map(|k| labels.and_then(|l| l.get(k).cloned()).unwrap_or_default());
            self.labels.splice(pos..pos + removed, add);
        }
    }

    pub fn label(&self, pos: usize) -> Option<String> {
        self.labels.get(pos).filter(|s| !s.is_empty()).cloned()
    }
}

/// A list of ids (`Value::List` of integers; null is empty).
pub fn ids_of(v: &svelters_abi::Value, what: &str) -> Result<Vec<u64>, String> {
    use svelters_abi::Value;
    match v {
        Value::Null => Ok(Vec::new()),
        Value::List(xs) => xs
            .iter()
            .map(|x| match x {
                Value::I64(i) if *i >= 0 => Ok(*i as u64),
                Value::U64(u) => Ok(*u),
                Value::F64(f) if *f >= 0.0 && f.fract() == 0.0 => Ok(*f as u64),
                other => Err(format!("{what}: expected a non-negative integer id, got {other:?}")),
            })
            .collect(),
        other => Err(format!("{what}: expected a list of ids, got {other:?}")),
    }
}

/// Ids as a `Value::List` of `I64`.
pub fn ids_value(ids: impl IntoIterator<Item = u64>) -> svelters_abi::Value {
    svelters_abi::Value::List(ids.into_iter().map(|k| svelters_abi::Value::I64(k as i64)).collect())
}

/// A list of strings (labels); null is none.
pub fn labels_of(v: Option<&svelters_abi::Value>, what: &str) -> Result<Option<Vec<String>>, String> {
    use svelters_abi::Value;
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::List(xs)) => Ok(Some(
            xs.iter()
                .map(|x| match x {
                    Value::Str(s) => s.clone(),
                    Value::Null => String::new(),
                    Value::I64(i) => i.to_string(),
                    Value::F64(f) => f.to_string(),
                    other => format!("{other:?}"),
                })
                .collect(),
        )),
        Some(other) => Err(format!("{what}: expected a list of strings, got {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_moves_selects_all_activates_and_types_ahead() {
        let ids: Vec<u64> = vec![5, 6, 7, 8];
        let labels = ["apple", "Banana", "berry", "cherry"];
        let mut s = KeySelect::new(Mode::Multi);
        let none = Mods::default();
        let o = apply(&mut s, &ids, &[Input::Move(Nav::Down, none), Input::Move(Nav::Down, none)], 2, |p| Some(labels[p].into()));
        assert_eq!((s.cursor, o.cursor, o.selection), (Some(6), true, true));
        let o = apply(&mut s, &ids, &[Input::SelectAll, Input::Activate], 2, |_| None);
        assert_eq!((s.selected.len(), o.activate), (4, Some(1)));
        apply(&mut s, &ids, &[Input::Text("c".into(), 0.0)], 2, |p| Some(labels[p].into()));
        assert_eq!(s.cursor, Some(8));
        let mut s = KeySelect::new(Mode::Single);
        let o = apply(&mut s, &ids, &[Input::Space], 2, |_| None);
        assert_eq!((s.cursor, o.selection), (Some(5), true), "Space with no cursor starts at the top");
    }

    #[test]
    fn row_keys_splice_keeps_labels_aligned() {
        let mut r = RowKeys::default();
        r.splice(0, 0, &[1, 2, 3], None);
        assert!(r.labels.is_empty());
        r.splice(1, 1, &[7, 8], Some(&["seven".into(), "eight".into()]));
        assert_eq!(r.ids, vec![1, 7, 8, 3]);
        assert_eq!(r.labels, vec!["", "seven", "eight", ""]);
        assert_eq!((r.label(1).as_deref(), r.label(0)), (Some("seven"), None));
        assert_eq!(ids_of(&ids_value([3, 4]), "x").unwrap(), vec![3, 4]);
        assert!(ids_of(&svelters_abi::Value::List(vec![svelters_abi::Value::I64(-1)]), "x").is_err());
    }
}
