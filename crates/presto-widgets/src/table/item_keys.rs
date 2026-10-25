//! Item-view keyboard input on [`KeySelect`] (svelters-egui `views/item_keys.rs`, WG5a).
//!
//! [`input_of`] decodes one presto key event for a focused list or table into an [`Input`];
//! [`apply`] feeds inputs to the view's [`KeySelect`] over its ordered row ids and folds the results
//! into one [`Outcome`]. [`RowKeys`] is the ids-and-labels store of `table`'s `set-ids` (and
//! `virtual-list`'s JS ids). Additive to `selection.rs`: Ctrl+A selects all (multi), Enter
//! activates the cursor row.

use super::selection::{Change, KeySelect, Mods, Nav};
use super::shim::Value;
use presto_base::event::{KeyEvent, KeyState, Modifiers};
use presto_base::key::Key;
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

/// The selection modifiers of `m` (Ctrl is Cmd on macOS).
pub fn mods(m: Modifiers, mac: bool) -> Mods {
    Mods { shift: m.contains(Modifiers::SHIFT), ctrl: m.primary(mac) }
}

/// The selection input key event `ev` is, at time `now` (seconds). `text`: typed text counts
/// (type-ahead); a view that starts an edit on typing reads text itself and passes false.
pub fn input_of(ev: &KeyEvent, now: f64, mac: bool, text: bool) -> Option<Input> {
    if ev.state != KeyState::Pressed {
        return None;
    }
    let m = mods(ev.modifiers, mac);
    let input = match ev.key {
        Some(Key::ArrowUp) => Input::Move(Nav::Up, m),
        Some(Key::ArrowDown) => Input::Move(Nav::Down, m),
        Some(Key::PageUp) => Input::Move(Nav::PageUp, m),
        Some(Key::PageDown) => Input::Move(Nav::PageDown, m),
        Some(Key::Home) => Input::Move(Nav::Home, m),
        Some(Key::End) => Input::Move(Nav::End, m),
        Some(Key::Space) => Input::Space,
        Some(Key::A) if m.ctrl && !m.shift => Input::SelectAll,
        Some(Key::Enter) => Input::Activate,
        _ => return typed(ev, mac).filter(|_| text).map(|t| Input::Text(t, now)),
    };
    Some(input)
}

/// The printable text key event `ev` types (none with Ctrl/Cmd or Alt held, and none for Space,
/// which is a key).
pub fn typed(ev: &KeyEvent, mac: bool) -> Option<String> {
    if ev.modifiers.primary(mac) || (ev.modifiers.contains(Modifiers::ALT) && !ev.modifiers.contains(Modifiers::ALT_GR)) {
        return None;
    }
    ev.text.as_ref().filter(|t| !t.is_empty() && t.as_str() != " " && !t.chars().any(char::is_control)).cloned()
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
            Input::SelectAll if sel.mode.is_multi() => {
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

/// Row ids in order with optional type-ahead labels (`table`'s `set-ids`).
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

/// A list of ids (a list of non-negative integers).
pub fn ids_of(v: &Value, what: &str) -> Result<Vec<u64>, String> {
    match v {
        Value::List(xs) => xs
            .iter()
            .map(|x| match x {
                Value::I64(i) if *i >= 0 => Ok(*i as u64),
                Value::F64(f) if *f >= 0.0 && f.fract() == 0.0 => Ok(*f as u64),
                other => Err(format!("{what}: expected a non-negative integer id, got {other:?}")),
            })
            .collect(),
        other => Err(format!("{what}: expected a list of ids, got {other:?}")),
    }
}

/// Ids as a list of `I64`.
pub fn ids_value(ids: impl IntoIterator<Item = u64>) -> Value {
    Value::List(ids.into_iter().map(|k| Value::I64(k as i64)).collect())
}

/// A list of strings (labels).
pub fn labels_of(v: Option<&Value>, what: &str) -> Result<Option<Vec<String>>, String> {
    match v {
        None => Ok(None),
        Some(Value::List(xs)) => Ok(Some(
            xs.iter()
                .map(|x| match x {
                    Value::Str(s) => s.clone(),
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
    use crate::table::selection::Mode;
    use presto_base::event::KeyLocation;

    fn key(k: Key, m: Modifiers) -> KeyEvent {
        KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m }
    }

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
    fn key_events_decode_with_the_platform_modifier() {
        assert_eq!(input_of(&key(Key::ArrowDown, Modifiers::SHIFT), 0.0, false, true), Some(Input::Move(Nav::Down, Mods { shift: true, ctrl: false })));
        assert_eq!(input_of(&key(Key::A, Modifiers::CTRL), 0.0, false, true), Some(Input::SelectAll));
        assert_eq!(input_of(&key(Key::A, Modifiers::SUPER), 0.0, true, true), Some(Input::SelectAll), "Cmd+A on macOS");
        assert_eq!(input_of(&key(Key::A, Modifiers::CTRL), 0.0, true, true), None, "Ctrl+A is not select-all on macOS");
        let mut t = key(Key::B, Modifiers::default());
        t.text = Some("b".into());
        assert_eq!(input_of(&t, 1.5, false, true), Some(Input::Text("b".into(), 1.5)));
        assert_eq!(input_of(&t, 1.5, false, false), None);
        let mut up = key(Key::ArrowDown, Modifiers::default());
        up.state = KeyState::Released;
        assert_eq!(input_of(&up, 0.0, false, true), None);
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
        assert!(ids_of(&Value::List(vec![Value::I64(-1)]), "x").is_err());
    }
}
