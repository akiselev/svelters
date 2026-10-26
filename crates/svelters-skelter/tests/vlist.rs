//! A25 and A9b.b: `virtual-list` and `tree-view` on skelter with svelters-egui's members, driven
//! through the real vtable by a host that plays `@svelters/lists`' part of the row protocol in Rust
//! (as svelters-egui's `tests/vlist.rs` does): `setup` → `setRow` of a box holding a label, `bind`
//! → the label's text. Skelter queues the row requests with the frame's events, so a frame and a
//! pump bring the rows in.

use skelter_base::geom::{Scale, Size};
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::time::Time;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use svelters_abi::*;
use svelters_skelter::SkelterBackend;

#[derive(Default)]
struct St {
    br: Option<BackendRef<'static>>,
    sig: HashMap<svl_id, &'static str>,
    /// row slot → (outlet, label)
    rows: HashMap<i64, (svl_handle, svl_handle)>,
    /// row slot → (pos, id) of its binding
    bound: HashMap<i64, (i64, i64)>,
    events: Vec<(&'static str, Vec<Value>)>,
    setups: usize,
    /// As `@svelters/lists` does it: `setup` gives an empty outlet, the row's content is mounted
    /// into it at its first `bind`.
    late_content: bool,
}

/// The JS side of the protocol, in Rust.
struct Js(Rc<RefCell<St>>);

impl Js {
    fn handle(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        let (br, name) = {
            let st = self.0.borrow();
            (st.br.unwrap(), st.sig.get(&signal).copied().unwrap_or("?"))
        };
        self.0.borrow_mut().events.push((name, args.to_vec()));
        let int = |k: usize| match args.get(k) {
            Some(Value::I64(x)) => *x,
            other => panic!("{name}: arg {k} is {other:?}"),
        };
        match name {
            "setup" => {
                let row = int(0);
                let late = self.0.borrow().late_content;
                let bx = br.create(br.resolve(NameKind::Kind, 0, "box")).unwrap();
                let label = if late { 0 } else { br.create(br.resolve(NameKind::Kind, 0, "label")).unwrap() };
                if !late {
                    br.insert(bx, label, None, 0).unwrap();
                }
                let lk = br.handle_info(h).unwrap().kind;
                br.call(h, br.resolve(NameKind::Method, lk, "setRow"), &[Value::I64(row), Value::Handle(bx)]).unwrap();
                let mut st = self.0.borrow_mut();
                st.rows.insert(row, (bx, label));
                st.setups += 1;
            }
            "bind" => {
                let (row, pos, id) = (int(0), int(1), int(2));
                let (bx, mut label) = self.0.borrow().rows[&row];
                if label == 0 {
                    // the row component mounts into the outlet: a box with padding, then the label
                    let inner = br.create(br.resolve(NameKind::Kind, 0, "box")).unwrap();
                    label = br.create(br.resolve(NameKind::Kind, 0, "label")).unwrap();
                    br.insert(inner, label, None, 0).unwrap();
                    br.insert(bx, inner, None, 0).unwrap();
                    self.0.borrow_mut().rows.insert(row, (bx, label));
                }
                let lk = br.handle_info(label).unwrap().kind;
                br.set_prop(label, br.resolve(NameKind::Prop, lk, "text"), &Value::Str(format!("Item {id}"))).unwrap();
                self.0.borrow_mut().bound.insert(row, (pos, id));
            }
            "unbind" => {
                self.0.borrow_mut().bound.remove(&int(0));
            }
            _ => {}
        }
    }
}

impl HostCallbacks for Js {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.handle(h, signal, args);
    }
    fn checkpoint(&self) {}
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        self.handle(h, signal, args);
        Ok(Value::Null)
    }
    fn on_destroyed(&self, _h: svl_handle, _reason: u32) {}
    fn frame(&self) {}
}

struct ListRig {
    backend: SkelterBackend,
    br: BackendRef<'static>,
    st: Rc<RefCell<St>>,
    list: svl_handle,
}

const ROW_H: f64 = 24.0;
const VIEW_H: f32 = 240.0;

impl ListRig {
    fn new(kind: &str) -> ListRig {
        ListRig::new_with(kind, false)
    }

    /// Frames and pumps until the rows settle; the nodes whose lists the commits sent (and did not
    /// remove since).
    fn run_drawn(&self) -> std::collections::HashSet<NodeId> {
        let mut drawn = std::collections::HashSet::new();
        for _ in 0..6 {
            if let Some(c) = self.backend.with_doc(|d| d.frame(WindowId(1), Time(0))) {
                for n in &c.removed {
                    drawn.remove(n);
                }
                drawn.extend(c.lists.iter().map(|(n, _)| *n));
            }
            self.backend.pump();
        }
        drawn
    }

    /// The labels of the rows at `first..` are drawn in the list's viewport, with a size.
    fn check_drawn(&self, drawn: &std::collections::HashSet<NodeId>, first: usize) {
        let n = self.node();
        let text = skelter_core::PropId::of("label", "text", "string");
        self.backend.with_doc(|d| {
            // rows are placed in the list (their containing block), scrolled with its content
            let view = d.world_rect(n).unwrap();
            let rows = d.virtual_rows(n).to_vec();
            let mut seen = 0;
            for &(pos, row) in &rows {
                let mut label = d.children(row)[0];
                if d.get_prop(label, text).is_none() {
                    label = d.children(label)[0];
                }
                let r = d.world_rect(label).unwrap();
                if r.min.y >= view.min.y && r.max.y <= view.max.y {
                    assert!(r.width() > 0.0 && r.height() > 0.0, "row {pos}'s label has a size: {r:?}");
                    assert!(drawn.contains(&label), "row {pos}'s label is drawn");
                    assert!(pos >= first, "row {pos} in view after scrolling to {first}");
                    seen += 1;
                }
            }
            assert!(seen >= 8, "{seen} rows drawn in the viewport");
        });
    }

    fn new_with(kind: &str, late_content: bool) -> ListRig {
        let backend = SkelterBackend::builtin();
        // text shaping, as the app installs it (labels measure their text)
        svelters_skelter::app::install_widget_services(&backend, skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())));
        let vt: &'static BackendVtable = Box::leak(Box::new(BackendVtable::new(backend.clone())));
        let st: Rc<RefCell<St>> = Rc::default();
        st.borrow_mut().late_content = late_content;
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Js(st.clone()))));
        let br = vt.backend_ref();
        br.start(hv).expect("start");
        st.borrow_mut().br = Some(br);
        let root = br.create(br.resolve(NameKind::Kind, 0, "box")).unwrap();
        backend.attach_presented(root, WindowId(1), Size::new(300.0, VIEW_H), Scale(1.0)).unwrap();
        let list = br.create(br.resolve(NameKind::Kind, 0, kind)).unwrap();
        let lk = br.handle_info(list).unwrap().kind;
        let sigs: &[&'static str] = if kind == "tree-view" { &["setup", "bind", "unbind", "teardown"] } else { &["setup", "bind", "unbind", "teardown", "selectionchange"] };
        for &s in sigs {
            let id = br.resolve(NameKind::Signal, lk, s);
            assert_ne!(id, 0, "signal {s}");
            st.borrow_mut().sig.insert(id, s);
            br.listen(list, id, true).unwrap();
        }
        let r = ListRig { backend, br, st, list };
        // (the compiler turns `flex-grow` into a style write: the inline style here)
        let ln = r.backend.node(list).unwrap();
        r.backend.with_doc(|d| d.set_style(ln, skelter_core::StyleInput { inline: Some("flex-grow: 1".into()), ..Default::default() }));
        r.set("row-height", Value::I64(ROW_H as i64));
        br.insert(root, list, None, 0).unwrap();
        r
    }

    fn set(&self, name: &str, v: Value) {
        let k = self.br.handle_info(self.list).unwrap().kind;
        let p = self.br.resolve(NameKind::Prop, k, name);
        assert_ne!(p, 0, "prop {name}");
        self.br.set_prop(self.list, p, &v).unwrap();
    }

    fn call(&self, name: &str, args: &[Value]) -> Value {
        let k = self.br.handle_info(self.list).unwrap().kind;
        let m = self.br.resolve(NameKind::Method, k, name);
        assert_ne!(m, 0, "method {name}");
        self.br.call(self.list, m, args).unwrap()
    }

    fn splice(&self, pos: i64, removed: i64, ids: impl IntoIterator<Item = i64>) {
        self.call("splice", &[Value::I64(pos), Value::I64(removed), Value::List(ids.into_iter().map(Value::I64).collect())]);
    }

    /// Frames and pumps until the rows settle (JS answers the requests after each frame).
    fn run(&self) {
        for _ in 0..4 {
            self.backend.with_doc(|d| d.frame(WindowId(1), Time(0)));
            self.backend.pump();
        }
    }

    fn node(&self) -> NodeId {
        self.backend.node(self.list).unwrap()
    }

    fn offset(&self) -> f64 {
        self.backend.with_doc(|d| d.scroll_offset(self.node()).y as f64)
    }

    /// Every realised row shows its own item (`Item {id}`); returns how many there are.
    fn check_rows(&self, ids: &[i64]) -> usize {
        let n = self.node();
        let text = skelter_core::PropId::of("label", "text", "string");
        self.backend.with_doc(|d| {
            let rows = d.virtual_rows(n).to_vec();
            for &(pos, row) in &rows {
                let mut label = d.children(row)[0];
                if d.get_prop(label, text).is_none() && !d.children(label).is_empty() {
                    label = d.children(label)[0];
                }
                assert_eq!(d.get_prop(label, text), Some(skelter_core::Value::Str(format!("Item {}", ids[pos]).into())), "row at {pos}");
                let r = d.layout_rect(row).unwrap();
                assert_eq!(r.min.y as f64, pos as f64 * ROW_H, "row {pos} placed at its offset");
            }
            rows.len()
        })
    }
}

/// svelters-egui's `a_2k_row_list_binds_its_window_and_flings_without_unbound_rows`: `splice` and
/// `count`, the rows of the window set up once and bound, and scrolling through the list rebinds
/// the same slots (no row created while scrolling).
#[test]
fn virtual_list_binds_its_window_and_recycles_while_scrolling() {
    let r = ListRig::new("virtual-list");
    let ids: Vec<i64> = (1..=2000).collect();
    r.splice(0, 0, ids.iter().copied());
    assert_eq!(r.call("count", &[]), Value::I64(2000));
    r.run();
    let setups = r.st.borrow().setups;
    assert!(setups > 8 && setups <= 30, "{setups} row slots for a {VIEW_H}-point viewport of {ROW_H}-point rows");
    let shown = r.check_rows(&ids);
    assert!(shown >= 10, "{shown} rows shown");
    // scroll through the list: the slots are rebound, none created
    let n = r.node();
    for k in 1..=20 {
        r.backend.with_doc(|d| d.set_scroll_offset(n, skelter_base::geom::Vec2::new(0.0, k as f32 * 2000.0)));
        r.run();
        r.check_rows(&ids);
    }
    assert!(r.offset() > 39_000.0, "scrolled: {}", r.offset());
    assert_eq!(r.st.borrow().setups, setups, "no row created while scrolling");
    assert!(r.st.borrow().events.iter().filter(|e| e.0 == "bind").count() > 200, "rows were rebound");
}

/// svelters-egui's `scroll_to_anchoring_and_set_size`: `scrollTo`, `rowAt`, scroll anchoring by key
/// on a splice above the first visible row, and a splice of what is in view rebinds without stale
/// rows.
#[test]
fn virtual_list_scroll_to_row_at_and_anchoring() {
    let r = ListRig::new("virtual-list");
    let mut ids: Vec<i64> = (1..=500).collect();
    r.splice(0, 0, ids.iter().copied());
    r.run();
    r.call("scrollTo", &[Value::I64(300)]);
    r.run();
    let off = r.offset();
    assert!(off > 0.0 && off <= 300.0 * ROW_H && off + VIEW_H as f64 >= 301.0 * ROW_H, "row 300 in view at offset {off}");
    assert_ne!(r.call("rowAt", &[Value::I64(300)]), Value::I64(-1));
    assert_eq!(r.call("rowAt", &[Value::I64(10)]), Value::I64(-1));
    r.check_rows(&ids);
    // `scrollTo` with a hint
    r.call("scrollTo", &[Value::I64(100), Value::Str("top".into())]);
    r.run();
    assert_eq!(r.offset(), 100.0 * ROW_H);
    // insert 10 items at the top: the first visible item keeps its place on screen
    let off = r.offset();
    let first = (off / ROW_H).floor() as usize;
    let first_id = ids[first];
    let y_before = first as f64 * ROW_H - off;
    r.splice(0, 0, 9001..=9010);
    ids.splice(0..0, 9001..=9010);
    r.run();
    let at = ids.iter().position(|&x| x == first_id).unwrap();
    assert_eq!(at as f64 * ROW_H - r.offset(), y_before, "anchored by key: offset {off} → {}", r.offset());
    r.check_rows(&ids);
    // remove what is in view: rows are rebound, none stale
    r.splice(at as i64, 20, std::iter::empty());
    ids.drain(at..at + 20);
    r.run();
    assert!(r.check_rows(&ids) >= 10);
    assert_eq!(r.call("count", &[]), Value::I64(ids.len() as i64));
}

/// egui's `selected` (a position) on the list: a host write selects that item and raises nothing;
/// it reads back.
#[test]
fn virtual_list_selected_is_a_position() {
    let r = ListRig::new("virtual-list");
    r.splice(0, 0, 1..=50);
    r.run();
    r.st.borrow_mut().events.clear();
    r.set("selected", Value::I64(7));
    r.run();
    let k = r.br.handle_info(r.list).unwrap().kind;
    assert_eq!(r.br.get_prop(r.list, r.br.resolve(NameKind::Prop, k, "selected")).unwrap(), Value::I64(7));
    assert!(!r.st.borrow().events.iter().any(|e| e.0 == "selectionchange"), "a host write never raises");
}

/// A9b.b: cadatomic's `lists.js` drives `tree-view` as svelters-egui's: JS flattens the tree,
/// `splice(pos, removed, ids)` gives the visible rows, `setNodes(ids, levels, flags, labels)` their
/// facts, and the rows come through the same row protocol; `selection` and `setSelection`.
#[test]
fn tree_view_splice_set_nodes_and_rows() {
    let r = ListRig::new("tree-view");
    // 1 (expanded) ─ 11, 12; 2
    r.call("setNodes", &[
        Value::List(vec![Value::I64(1), Value::I64(11), Value::I64(12), Value::I64(2)]),
        Value::List(vec![Value::I64(0), Value::I64(1), Value::I64(1), Value::I64(0)]),
        Value::List(vec![Value::I64(3), Value::I64(0), Value::I64(0), Value::I64(1)]),
        Value::List(vec![Value::Str("One".into()), Value::Str("Eleven".into()), Value::Str("Twelve".into()), Value::Str("Two".into())]),
    ]);
    let mut ids = vec![1, 11, 12, 2];
    r.splice(0, 0, ids.iter().copied());
    assert_eq!(r.call("count", &[]), Value::I64(4));
    r.run();
    assert_eq!(r.check_rows(&ids), 4);
    let n = r.node();
    let tree_rows = r.backend.with_doc(|d| {
        d.with_kind(n, |k| {
            let t = k.as_any().and_then(|a| a.downcast_ref::<skelter_widgets::views::tree::TreeView>()).expect("skelter-widgets' tree");
            t.rows().iter().map(|r| (r.id, r.depth, r.expandable, r.expanded)).collect::<Vec<_>>()
        })
    });
    assert_eq!(tree_rows, Some(vec![(1, 0, true, true), (11, 1, false, false), (12, 1, false, false), (2, 0, true, false)]), "the facts setNodes gave");
    // collapsing 1 in JS: its children spliced out
    r.call("setNodes", &[Value::List(vec![Value::I64(1)]), Value::List(vec![Value::I64(0)]), Value::List(vec![Value::I64(1)])]);
    r.splice(1, 2, std::iter::empty());
    ids.drain(1..3);
    r.run();
    assert_eq!(r.call("count", &[]), Value::I64(2));
    assert_eq!(r.check_rows(&ids), 2);
    assert_ne!(r.call("rowAt", &[Value::I64(1)]), Value::I64(-1));
    // a row's content (its label) starts after its disclosure triangle, at every level
    r.backend.with_doc(|d| {
        d.with_kind(n, |k| {
            let t = k.as_any().and_then(|a| a.downcast_ref::<skelter_widgets::views::tree::TreeView>()).unwrap();
            for &(pos, row) in d.virtual_rows(n) {
                let label = d.children(row)[0];
                let lr = d.layout_rect(row).unwrap();
                let label_x = lr.min.x + d.layout_rect(label).unwrap().min.x;
                if let Some(tri) = t.disclosure_rect(pos) {
                    assert!(label_x >= tri.max.x, "row {pos}: the label at {label_x} starts after its chevron {tri:?}");
                }
                assert!(label_x > 0.0, "row {pos} is indented past its level's triangle");
            }
        });
    });
    // selection
    r.call("setSelection", &[Value::List(vec![Value::I64(2)])]);
    assert_eq!(r.call("selection", &[]), Value::List(vec![Value::I64(2)]));
}

/// A25b: rows as `@svelters/lists` makes them (an empty outlet at `setup`, the row component mounted
/// into it at the first `bind`) lay out, draw and scroll: each visible row's label is drawn (its
/// list sent) inside the viewport with a size, and `scrollTo` and the wheel bring later rows in.
#[test]
fn rows_mounted_at_bind_lay_out_draw_and_scroll() {
    let r = ListRig::new_with("virtual-list", true);
    let ids: Vec<i64> = (1..=1000).collect();
    r.splice(0, 0, ids.iter().copied());
    let drawn = r.run_drawn();
    let shown = r.check_rows(&ids);
    assert!(shown >= 10, "{shown} rows realised");
    r.check_drawn(&drawn, 0);
    r.call("scrollTo", &[Value::I64(600), Value::Str("top".into())]);
    let drawn = r.run_drawn();
    assert_eq!(r.offset(), 600.0 * ROW_H);
    r.check_rows(&ids);
    r.check_drawn(&drawn, 600);
}

