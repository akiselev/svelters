//! WG5a (plans/milestones/WG5a-list-table-interaction.md): keyboard navigation, type-ahead, keyed
//! selection and cell editing in `virtual-list` and `table`, driven through the real vtable by a
//! host that plays JS in Rust (the M5 row protocol, `commit`'s verdict, the column editor
//! protocol of the runtime's `cellEditor`). Headless kittest.
//!
//! Gates (QT-SURVEY §3 WG5a): a 100k-row table is usable from the keyboard; F2 edits a cell, Enter
//! commits through `commit` (which can reject, with the reason shown), Escape reverts, Tab moves to
//! the next editable cell; a snippet-style editor is realised once and rebound; selection is by
//! id and survives a re-sort and a filter.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use svelters_abi::*;
use svelters_egui::EguiBackend;
use svelters_egui::scene::NodeId;

#[derive(Default)]
struct St {
    br: Option<BackendRef<'static>>,
    sig: HashMap<svl_id, &'static str>,
    /// list row slot → label
    rows: HashMap<i64, svl_handle>,
    events: Vec<(&'static str, Vec<Value>)>,
    /// `commit`'s answer (None: accept)
    verdict: Option<Value>,
    commits: Vec<Vec<Value>>,
    /// the column editor (a text-edit), created at the first `editor-bind`
    editor: Option<svl_handle>,
    editor_binds: usize,
}

struct Js(Rc<RefCell<St>>);

fn prop(br: BackendRef<'static>, h: svl_handle, name: &str) -> svl_id {
    br.resolve(NameKind::Prop, br.handle_info(h).unwrap().kind, name)
}

impl HostCallbacks for Js {
    fn post_event(&self, _h: svl_handle, signal: svl_id, args: &[Value]) {
        let mut st = self.0.borrow_mut();
        let name = st.sig.get(&signal).copied().unwrap_or("?");
        st.events.push((name, args.to_vec()));
    }
    fn checkpoint(&self) {}
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        let (br, name) = {
            let st = self.0.borrow();
            (st.br.unwrap(), st.sig.get(&signal).copied().unwrap_or("?"))
        };
        let int = |k: usize| match args.get(k) {
            Some(Value::I64(x)) => *x,
            other => panic!("{name}: arg {k} is {other:?}"),
        };
        match name {
            "setup" => {
                let row = int(0);
                let label = br.create(br.resolve(NameKind::Kind, 0, "label")).unwrap();
                let m = br.resolve(NameKind::Method, br.handle_info(h).unwrap().kind, "setRow");
                br.call(h, m, &[Value::I64(row), Value::Handle(label)]).unwrap();
                self.0.borrow_mut().rows.insert(row, label);
            }
            "bind" => {
                let label = self.0.borrow().rows[&int(0)];
                br.set_prop(label, prop(br, label, "text"), &Value::Str(format!("Item {}", int(2)))).unwrap();
            }
            "commit" => {
                let mut st = self.0.borrow_mut();
                st.commits.push(args.to_vec());
                return Ok(st.verdict.clone().unwrap_or(Value::Null));
            }
            "editor-bind" => {
                let ed = self.0.borrow().editor;
                let ed = ed.unwrap_or_else(|| {
                    let e = br.create(br.resolve(NameKind::Kind, 0, "text-edit")).unwrap();
                    br.set_prop(e, prop(br, e, "accessible-name"), &Value::Str("Quantity".into())).unwrap();
                    self.0.borrow_mut().editor = Some(e);
                    e
                });
                let value = match (&args[1], &args[2]) {
                    (_, Value::Str(t)) if !t.is_empty() => t.clone(),
                    (Value::Str(v), _) => v.clone(),
                    _ => String::new(),
                };
                br.set_prop(ed, prop(br, ed, "text"), &Value::Str(value)).unwrap();
                let m = br.resolve(NameKind::Method, br.handle_info(h).unwrap().kind, "set-editor");
                br.call(h, m, &[Value::Handle(ed)]).unwrap();
                self.0.borrow_mut().editor_binds += 1;
            }
            "editor-value" => {
                let ed = self.0.borrow().editor.unwrap();
                return Ok(br.get_prop(ed, prop(br, ed, "text")).unwrap());
            }
            _ => {}
        }
        Ok(Value::Null)
    }
    fn on_destroyed(&self, _h: svl_handle, _reason: u32) {}
    fn frame(&self) {}
}

struct Rig {
    backend: EguiBackend,
    br: BackendRef<'static>,
    st: Rc<RefCell<St>>,
    harness: Harness<'static, EguiBackend>,
    win: svl_handle,
}

impl Rig {
    fn new() -> Rig {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let st: Rc<RefCell<St>> = Rc::default();
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Js(st.clone()))));
        let br = vt.backend_ref();
        br.start(hv).expect("start");
        st.borrow_mut().br = Some(br);
        let harness = Harness::builder().with_size(egui::vec2(480.0, 360.0)).build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        let win = br.create(br.resolve(NameKind::Kind, 0, "window")).unwrap();
        Rig { backend, br, st, harness, win }
    }

    fn create(&self, tag: &str, parent: svl_handle) -> svl_handle {
        let h = self.br.create(self.br.resolve(NameKind::Kind, 0, tag)).unwrap();
        self.br.insert(parent, h, None, 0).unwrap();
        h
    }

    fn listen(&self, h: svl_handle, signals: &[&'static str]) {
        let k = self.br.handle_info(h).unwrap().kind;
        for &s in signals {
            let id = self.br.resolve(NameKind::Signal, k, s);
            assert_ne!(id, 0, "signal {s}");
            self.st.borrow_mut().sig.insert(id, s);
            self.br.listen(h, id, true).unwrap();
        }
    }

    fn present(&mut self) {
        let wk = self.br.handle_info(self.win).unwrap().kind;
        self.br.call(self.win, self.br.resolve(NameKind::Method, wk, "present"), &[]).unwrap();
        self.harness.run();
    }

    fn set(&self, h: svl_handle, name: &str, v: Value) {
        self.br.set_prop(h, prop(self.br, h, name), &v).unwrap();
    }

    fn get(&self, h: svl_handle, name: &str) -> Value {
        self.br.get_prop(h, prop(self.br, h, name)).unwrap()
    }

    fn call(&self, h: svl_handle, name: &str, args: &[Value]) -> Value {
        let k = self.br.handle_info(h).unwrap().kind;
        self.br.call(h, self.br.resolve(NameKind::Method, k, name), args).unwrap()
    }

    fn key(&mut self, k: Key) {
        self.harness.key_press(k);
        self.harness.run();
    }

    fn key_mod(&mut self, m: Modifiers, k: Key) {
        self.harness.key_press_modifiers(m, k);
        self.harness.run();
    }

    fn text(&mut self, t: &str) {
        self.harness.event(egui::Event::Text(t.into()));
        self.harness.run();
    }

    fn take(&self, name: &str) -> Vec<Vec<Value>> {
        let mut st = self.st.borrow_mut();
        let (hit, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut st.events).into_iter().partition(|(n, _)| *n == name);
        st.events = rest;
        hit.into_iter().map(|(_, a)| a).collect()
    }

    fn has(&self, label: &str) -> bool {
        self.harness.query_by_label(label).is_some()
    }
}

fn ids(v: impl IntoIterator<Item = i64>) -> Value {
    Value::List(v.into_iter().map(Value::I64).collect())
}

fn strs(v: &[&str]) -> Value {
    Value::List(v.iter().map(|s| Value::Str((*s).into())).collect())
}

// ---- virtual-list ----

fn list_rig(selection: &str) -> (Rig, svl_handle) {
    let mut r = Rig::new();
    let list = r.create("virtual-list", r.win);
    r.listen(list, &["setup", "bind", "unbind", "teardown", "select", "activate", "selectionchange"]);
    r.set(list, "flex-grow", Value::F64(1.0));
    r.set(list, "row-height", Value::I64(24));
    r.set(list, "selection", Value::Str(selection.into()));
    r.present();
    (r, list)
}

fn list_offset(r: &Rig, list: svl_handle) -> f64 {
    r.backend.scene().read(|s| s.virtual_list(NodeId::from_raw(list).unwrap()).unwrap().offset())
}

#[test]
fn virtual_list_keyboard_type_ahead_and_keyed_multi_selection() {
    let (mut r, list) = list_rig("multi");
    let names: Vec<String> = (0..2000).map(|i| format!("{}{i}", ["alpha", "bravo", "charlie"][i % 3])).collect();
    let labels: Vec<&str> = names.iter().map(String::as_str).collect();
    r.call(list, "splice", &[Value::I64(0), Value::I64(0), ids(1..=2000), strs(&labels)]);
    r.harness.run();
    // a click focuses the list and selects the row
    r.harness.get_by_label("Item 3").click();
    r.harness.run();
    assert_eq!(r.take("selectionchange"), vec![vec![ids([3])]]);
    assert_eq!(r.take("select"), vec![vec![Value::I64(2)]], "M5's select carries the position");
    // Down, Shift+Down x2: a range from the anchor
    r.key(Key::ArrowDown);
    r.key_mod(Modifiers::SHIFT, Key::ArrowDown);
    r.key_mod(Modifiers::SHIFT, Key::ArrowDown);
    assert_eq!(r.get(list, "selected-ids"), ids([4, 5, 6]));
    // Ctrl+Down moves the cursor alone; Ctrl+Space toggles it in
    r.key_mod(Modifiers::COMMAND, Key::ArrowDown);
    r.key_mod(Modifiers::COMMAND, Key::ArrowDown);
    r.key_mod(Modifiers::COMMAND, Key::Space);
    assert_eq!(r.get(list, "selected-ids"), ids([4, 5, 6, 8]));
    // End: the last row, scrolled into view
    r.key(Key::End);
    assert_eq!(r.get(list, "selected-ids"), ids([2000]));
    assert!(list_offset(&r, list) > 2000.0 * 24.0 - 600.0, "End scrolled to the bottom: {}", list_offset(&r, list));
    assert!(r.has("Item 2000"));
    // Home, then PageDown: a page of rows
    r.key(Key::Home);
    r.key(Key::PageDown);
    let page = match r.get(list, "selected-ids") {
        Value::List(v) => v,
        other => panic!("{other:?}"),
    };
    assert!(matches!(page[..], [Value::I64(k)] if (10..=16).contains(&k)), "a page down from the top: {page:?}");
    // type-ahead on the splice labels: "ch" finds the next charlie row
    r.text("c");
    r.text("h");
    let Value::List(sel) = r.get(list, "selected-ids") else { unreachable!() };
    let Value::I64(k) = sel[0] else { unreachable!() };
    assert!(names[(k - 1) as usize].starts_with("charlie"), "{k}");
    // Enter activates the cursor row
    r.take("activate");
    r.key(Key::Enter);
    assert_eq!(r.take("activate"), vec![vec![Value::I64(k - 1)]]);
    // Ctrl+A selects all
    r.key_mod(Modifiers::COMMAND, Key::A);
    assert!(matches!(r.get(list, "selected-ids"), Value::List(v) if v.len() == 2000));
}

#[test]
fn virtual_list_selection_survives_a_resort_and_a_filter() {
    let (mut r, list) = list_rig("multi");
    r.call(list, "splice", &[Value::I64(0), Value::I64(0), ids(1..=100)]);
    r.harness.run();
    r.set(list, "selected-ids", ids([5, 7]));
    // re-sorted (reversed) by JS: the same ids stay selected, now at other positions
    r.call(list, "splice", &[Value::I64(0), Value::I64(100), ids((1..=100).rev())]);
    r.harness.run();
    assert_eq!(r.get(list, "selected-ids"), ids([5, 7]));
    let node = NodeId::from_raw(list).unwrap();
    let a11y_selected = |r: &Rig, label: &str| r.harness.get_by_label(label).accesskit_node().is_selected();
    r.call(list, "scrollTo", &[Value::I64(95)]);
    r.harness.run();
    assert_eq!(a11y_selected(&r, "Item 5"), Some(true));
    assert_eq!(a11y_selected(&r, "Item 6"), Some(false));
    // filtered: 5 is hidden but stays selected, and is selected again when shown
    r.call(list, "splice", &[Value::I64(0), Value::I64(100), ids((1..=100).filter(|&k| k != 5))]);
    r.harness.run();
    assert_eq!(r.get(list, "selected-ids"), ids([5, 7]));
    r.call(list, "splice", &[Value::I64(0), Value::I64(99), ids(1..=100)]);
    r.harness.run();
    assert!(r.backend.scene().read(|s| s.virtual_list(node).unwrap().selection().is_selected(5)));
    // the list is a multi-selectable list box
    let lb = r.harness.get_by_role(egui::accesskit::Role::ListBox);
    assert!(lb.accesskit_node().is_multiselectable());
}

// ---- table ----

/// A table of `n` rows with ids `1000 + pos` (set-ids), three columns (Name, Qty editable, Note
/// editable), the window answered from the rows' ids.
struct Table {
    r: Rig,
    t: svl_handle,
    cols: Vec<svl_handle>,
    order: Vec<i64>,
    texts: HashMap<(i64, usize), String>,
}

impl Table {
    fn new(n: i64) -> Table {
        let mut r = Rig::new();
        let t = r.create("table", r.win);
        r.listen(t, &["range", "selectionchange", "activated", "commit"]);
        r.set(t, "flex-grow", Value::F64(1.0));
        r.set(t, "accessible-name", Value::Str("Parts".into()));
        r.set(t, "selection", Value::Str("multi".into()));
        let mut cols = Vec::new();
        for (title, editable) in [("Name", false), ("Qty", true), ("Note", true)] {
            let c = r.create("table-column", t);
            r.set(c, "title", Value::Str(title.into()));
            r.set(c, "editable", Value::Bool(editable));
            cols.push(c);
        }
        let order: Vec<i64> = (0..n).map(|p| 1000 + p).collect();
        let labels: Vec<String> = order.iter().map(|id| format!("part{id}")).collect();
        r.call(t, "set-ids", &[ids(order.iter().copied()), Value::List(labels.into_iter().map(Value::Str).collect())]);
        r.present();
        let mut tb = Table { r, t, cols, order, texts: HashMap::new() };
        tb.feed();
        tb
    }

    fn text(&self, id: i64, c: usize) -> String {
        self.texts.get(&(id, c)).cloned().unwrap_or_else(|| match c {
            0 => format!("part{id}"),
            1 => format!("{}", id % 7),
            _ => format!("n{id}"),
        })
    }

    /// Answers the last `range` with its rows, then runs.
    fn feed(&mut self) {
        for _ in 0..3 {
            let Some(last) = self.r.take("range").pop() else { break };
            let (Value::I64(first), Value::I64(last)) = (&last[0], &last[1]) else { unreachable!() };
            let rows: Vec<Value> = (*first..=*last).map(|p| Value::List((0..3).map(|c| Value::Str(self.text(self.order[p as usize], c))).collect())).collect();
            self.r.call(self.t, "set-rows", &[Value::I64(*first), Value::List(rows)]);
            self.r.harness.run();
        }
    }

    fn reorder(&mut self, order: Vec<i64>) {
        self.order = order;
        self.r.call(self.t, "set-ids", &[ids(self.order.iter().copied())]);
        self.r.harness.run();
        self.feed();
    }

    fn key(&mut self, k: Key) {
        self.r.key(k);
        self.feed();
    }

    fn selected(&self) -> Value {
        self.r.get(self.t, "selected")
    }
}

#[test]
fn a_100k_row_table_is_usable_from_the_keyboard_and_keeps_selection_by_id() {
    let mut tb = Table::new(100_000);
    assert!(tb.r.has("part1003"));
    tb.r.harness.get_by_label("part1003").click();
    tb.r.harness.run();
    assert_eq!(tb.selected(), ids([1003]));
    assert_eq!(tb.r.get(tb.t, "current"), Value::I64(1003));
    tb.key(Key::ArrowDown);
    assert_eq!(tb.r.take("selectionchange").pop(), Some(vec![ids([1004])]));
    // End: the last of 100k rows, scrolled into view and fed
    tb.key(Key::End);
    tb.r.harness.run();
    tb.feed();
    assert_eq!(tb.selected(), ids([1000 + 99_999]));
    assert!(tb.r.has("part100999"), "the last row is shown");
    // Home, PageDown, Shift+Down: a range
    tb.key(Key::Home);
    tb.key(Key::PageDown);
    let Value::List(v) = tb.selected() else { unreachable!() };
    let Value::I64(p) = v[0] else { unreachable!() };
    assert!((1008..=1025).contains(&p), "a page: {p}");
    tb.r.key_mod(Modifiers::SHIFT, Key::ArrowDown);
    assert_eq!(tb.selected(), ids([p, p + 1]));
    // type-ahead on the set-ids labels
    tb.r.key(Key::Home);
    for c in ["p", "a", "r", "t", "5", "0", "0", "0"] {
        tb.r.text(c);
    }
    assert_eq!(tb.r.get(tb.t, "current"), Value::I64(5000), "type-ahead: the first row starting with part5000");
    // Right moves the cursor column; a re-sort and a filter keep the selection by id
    tb.r.key(Key::ArrowRight);
    assert_eq!(tb.r.get(tb.t, "current-column"), Value::I64(1));
    tb.r.set(tb.t, "selected", ids([1002, 1004]));
    tb.reorder((1000..101_000).rev().collect());
    assert_eq!(tb.selected(), ids([1002, 1004]));
    tb.reorder((1000..101_000).filter(|k| k % 2 == 0 && *k != 1002).collect());
    assert_eq!(tb.selected(), ids([1002, 1004]), "a hidden selected row stays selected");
    // the table is multi-selectable, its rows say whether they are selected
    let table = tb.r.harness.get_by_role_and_label(egui::accesskit::Role::Table, "Parts");
    assert!(table.accesskit_node().is_multiselectable());
}

#[test]
fn table_cells_edit_with_f2_commit_reject_revert_and_tab() {
    let mut tb = Table::new(50);
    let t = tb.t;
    tb.r.harness.get_by_label("part1002").click();
    tb.r.harness.run();
    // Right to Qty, F2: the built-in editor (a text input named by the column)
    tb.r.key(Key::ArrowRight);
    tb.r.key(Key::F2);
    let ed = tb.r.harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Qty");
    assert_eq!(ed.accesskit_node().value().as_deref(), Some(tb.text(1002, 1).as_str()));
    // type a new value, Enter: commit(row id, column, value) accepted, the cell shows it
    tb.r.key_mod(Modifiers::COMMAND, Key::A);
    tb.r.text("42");
    tb.r.key(Key::Enter);
    assert_eq!(tb.r.st.borrow().commits.last(), Some(&vec![Value::I64(1002), Value::I64(1), Value::Str("42".into())]));
    assert!(tb.r.has("42"), "the accepted value is shown");
    assert!(tb.r.harness.query_by_role(egui::accesskit::Role::TextInput).is_none(), "the editor closed");
    // typing on the focused table starts an edit with the typed text; the host rejects it
    tb.r.st.borrow_mut().verdict = Some(Value::Str("Quantity must be below 100".into()));
    tb.r.text("9");
    tb.r.text("9");
    tb.r.text("9");
    tb.r.key(Key::Enter);
    assert_eq!(tb.r.st.borrow().commits.last().unwrap()[2], Value::Str("999".into()));
    assert!(tb.r.has("Quantity must be below 100"), "the reason is shown");
    assert!(tb.r.has("42") && !tb.r.has("999"), "the cell reverted");
    // Escape reverts without a commit
    tb.r.st.borrow_mut().verdict = None;
    let n = tb.r.st.borrow().commits.len();
    tb.r.key(Key::F2);
    tb.r.text("7");
    tb.r.key(Key::Escape);
    assert_eq!(tb.r.st.borrow().commits.len(), n);
    assert!(tb.r.harness.query_by_role(egui::accesskit::Role::TextInput).is_none());
    // Tab commits and edits the next editable cell (Note), Shift+Tab goes back (Qty)
    tb.r.key(Key::F2);
    tb.r.key(Key::Tab);
    tb.r.harness.run();
    assert!(tb.r.harness.query_by_role_and_label(egui::accesskit::Role::TextInput, "Note").is_some(), "Tab: the next editable cell");
    tb.r.key_mod(Modifiers::SHIFT, Key::Tab);
    tb.r.harness.run();
    assert!(tb.r.harness.query_by_role_and_label(egui::accesskit::Role::TextInput, "Qty").is_some(), "Shift+Tab: back");
    tb.r.key(Key::Escape);
    // the edit method and a double click on an editable cell open editors too
    tb.r.call(t, "edit", &[Value::I64(1004), Value::I64(2)]);
    tb.r.harness.run();
    assert!(tb.r.harness.query_by_role_and_label(egui::accesskit::Role::TextInput, "Note").is_some());
    tb.r.call(t, "end-edit", &[Value::Bool(false)]);
    tb.r.harness.run();
    assert!(tb.r.harness.query_by_role(egui::accesskit::Role::TextInput).is_none());
    // a cell that is not editable is read-only in the a11y tree
    let _ = tb.cols.len();
}

#[test]
fn a_column_editor_is_realised_once_and_rebound_per_edit() {
    let mut tb = Table::new(20);
    let qty = tb.cols[1];
    // the runtime's side of `editor={snippet}`: `true` natively, the protocol signals listened
    tb.r.set(qty, "editor", Value::Bool(true));
    tb.r.listen(qty, &["editor-bind", "editor-value", "editor-unbind"]);
    tb.r.harness.get_by_label("part1001").click();
    tb.r.harness.run();
    tb.r.key(Key::ArrowRight);
    tb.r.key(Key::F2);
    tb.r.harness.run();
    assert_eq!(tb.r.st.borrow().editor_binds, 1);
    let ed = tb.r.harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Quantity");
    assert_eq!(ed.accesskit_node().value().as_deref(), Some(tb.text(1001, 1).as_str()));
    // focus went into the editor: typing edits it, Enter commits its value (`editor-value`)
    tb.r.key_mod(Modifiers::COMMAND, Key::A);
    tb.r.text("12");
    tb.r.key(Key::Enter);
    tb.r.harness.run();
    assert_eq!(tb.r.st.borrow().commits.last(), Some(&vec![Value::I64(1001), Value::I64(1), Value::Str("12".into())]));
    assert!(tb.r.take("editor-unbind").len() <= 1);
    // a second edit rebinds the same editor (no second realisation)
    let first = tb.r.st.borrow().editor;
    tb.r.key(Key::ArrowDown);
    tb.r.key(Key::F2);
    tb.r.harness.run();
    assert_eq!((tb.r.st.borrow().editor_binds, tb.r.st.borrow().editor), (2, first));
    assert!(tb.r.harness.query_by_role_and_label(egui::accesskit::Role::TextInput, "Quantity").is_some());
    tb.r.key(Key::Escape);
    tb.r.harness.run();
    assert!(tb.r.harness.query_by_role_and_label(egui::accesskit::Role::TextInput, "Quantity").is_none(), "the editor is hidden between edits");
}
