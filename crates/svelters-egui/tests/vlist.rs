//! LS1/SG5: `virtual-list` on egui (`src/kinds/virtuallist.rs`), driven through the real vtable by
//! a host that plays `@svelters/lists`' part of the M5 protocol in Rust (setup → `setRow` of a box
//! with a label, bind → the label's text), and the realised-row tier from a Rust model (the
//! property-grid shape of ARCH-SCENE §10.2). Headless kittest.
//!
//! Gates: a 2k-row list creates ≤ 60 rows and none while scrolling; R22: a fling through the list
//! never draws an unbound row (and every drawn row shows its own item); scroll anchoring by key on
//! a splice above the viewport; `scrollTo`; selection and activation; set-size a11y; anchors of
//! off-screen rows.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use svelters_abi::*;
use svelters_egui::EguiBackend;
use svelters_egui::scene::{NodeId, ProviderId, RowPool, RowSource, SceneMut, Target, VNodeId, keys};

#[derive(Default)]
struct St {
    br: Option<BackendRef<'static>>,
    sig: HashMap<svl_id, &'static str>,
    /// row slot → (outlet, label)
    rows: HashMap<i64, (svl_handle, svl_handle)>,
    /// row slot → (pos, id) of its current binding
    bound: HashMap<i64, (i64, i64)>,
    events: Vec<(&'static str, Vec<Value>)>,
    syncs: usize,
}

/// The JS side of the protocol, in Rust.
struct Js(Rc<RefCell<St>>);

impl HostCallbacks for Js {
    fn post_event(&self, _h: svl_handle, signal: svl_id, args: &[Value]) {
        let mut st = self.0.borrow_mut();
        let name = st.sig.get(&signal).copied().unwrap_or("?");
        st.events.push((name, args.to_vec()));
    }
    fn checkpoint(&self) {}
    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        let (br, name) = {
            let mut st = self.0.borrow_mut();
            st.syncs += 1;
            (st.br.unwrap(), st.sig.get(&signal).copied().unwrap_or("?"))
        };
        let int = |k: usize| match args.get(k) {
            Some(Value::I64(x)) => *x,
            other => panic!("{name}: arg {k} is {other:?}"),
        };
        match name {
            "setup" => {
                let row = int(0);
                let bx = br.create(br.resolve(NameKind::Kind, 0, "box")).unwrap();
                let lk = br.resolve(NameKind::Kind, 0, "label");
                let label = br.create(lk).unwrap();
                br.insert(bx, label, None, 0).unwrap();
                let lk_list = br.handle_info(h).unwrap().kind;
                let m = br.resolve(NameKind::Method, lk_list, "setRow");
                br.call(h, m, &[Value::I64(row), Value::Handle(bx)]).unwrap();
                self.0.borrow_mut().rows.insert(row, (bx, label));
            }
            "bind" => {
                let (row, pos, id) = (int(0), int(1), int(2));
                let label = self.0.borrow().rows[&row].1;
                let lk = br.handle_info(label).unwrap().kind;
                br.set_prop(label, br.resolve(NameKind::Prop, lk, "text"), &Value::Str(format!("Item {id}"))).unwrap();
                self.0.borrow_mut().bound.insert(row, (pos, id));
            }
            "unbind" => {
                self.0.borrow_mut().bound.remove(&int(0));
            }
            _ => {}
        }
        Ok(Value::Null)
    }
    fn on_destroyed(&self, _h: svl_handle, _reason: u32) {}
    fn frame(&self) {}
}

struct ListRig {
    backend: EguiBackend,
    br: BackendRef<'static>,
    st: Rc<RefCell<St>>,
    harness: Harness<'static, EguiBackend>,
    list: svl_handle,
}

impl ListRig {
    fn new() -> ListRig {
        ListRig::with_kind("virtual-list")
    }

    /// OW5a: a `virtual-list` or a `tree-view` (the same row protocol).
    fn with_kind(kind: &str) -> ListRig {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let st: Rc<RefCell<St>> = Rc::default();
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Js(st.clone()))));
        let br = vt.backend_ref();
        br.start(hv).expect("start");
        st.borrow_mut().br = Some(br);
        let harness = Harness::builder().with_size(egui::vec2(480.0, 360.0)).build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        let w = br.create(br.resolve(NameKind::Kind, 0, "window")).unwrap();
        let list = br.create(br.resolve(NameKind::Kind, 0, kind)).unwrap();
        let lk = br.handle_info(list).unwrap().kind;
        let sigs: &[&'static str] = if kind == "tree-view" {
            &["setup", "bind", "unbind", "teardown", "activate", "expand", "collapse", "row-clicked", "row-context"]
        } else {
            &["setup", "bind", "unbind", "teardown", "select", "activate", "selectionchange", "row-clicked", "row-context"]
        };
        for &s in sigs {
            let id = br.resolve(NameKind::Signal, lk, s);
            assert_ne!(id, 0, "signal {s}");
            st.borrow_mut().sig.insert(id, s);
            br.listen(list, id, true).unwrap();
        }
        let mut r = ListRig { backend, br, st, harness, list };
        r.set(list, "flex-grow", Value::F64(1.0));
        r.set(list, "row-height", Value::I64(24));
        r.br.insert(w, list, None, 0).unwrap();
        let wk = r.br.handle_info(w).unwrap().kind;
        r.br.call(w, r.br.resolve(NameKind::Method, wk, "present"), &[]).unwrap();
        r.harness.run();
        r
    }

    fn set(&self, h: svl_handle, name: &str, v: Value) {
        let k = self.br.handle_info(h).unwrap().kind;
        self.br.set_prop(h, self.br.resolve(NameKind::Prop, k, name), &v).unwrap();
    }

    fn call(&self, name: &str, args: &[Value]) -> Value {
        let k = self.br.handle_info(self.list).unwrap().kind;
        self.br.call(self.list, self.br.resolve(NameKind::Method, k, name), args).unwrap()
    }

    fn splice(&self, pos: i64, removed: i64, ids: impl IntoIterator<Item = i64>) {
        self.call("splice", &[Value::I64(pos), Value::I64(removed), Value::List(ids.into_iter().map(Value::I64).collect())]);
    }

    fn id(&self) -> NodeId {
        NodeId::from_raw(self.list).unwrap()
    }

    fn stats(&self) -> svelters_egui::kinds::ListStats {
        self.backend.scene().read(|s| s.virtual_list(self.id()).unwrap().stats())
    }

    fn slots(&self) -> usize {
        self.backend.scene().read(|s| s.virtual_list(self.id()).unwrap().slot_count())
    }

    fn offset(&self) -> f64 {
        self.backend.scene().read(|s| s.virtual_list(self.id()).unwrap().offset())
    }

    /// Every row the list shows has its own item's text (no stale or unbound content).
    fn check_rows(&self, ids: &[i64]) -> usize {
        let scene = self.backend.scene();
        scene.read(|s| {
            let rows = s.list_rows(self.id());
            for &(n, pos) in &rows {
                let label = s.get(n).unwrap().children().next().expect("a label in the row");
                assert_eq!(label.get(&keys::label::TEXT).map(String::as_str), Some(format!("Item {}", ids[pos]).as_str()), "row at {pos}");
            }
            rows.len()
        })
    }

    fn list_rect(&self) -> egui::Rect {
        self.backend.scene().read(|s| s.get(self.id()).unwrap().drawn().unwrap())
    }

    fn wheel(&mut self, dy: f32) {
        let c = self.list_rect().center();
        self.harness.event(egui::Event::PointerMoved(c));
        self.harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

/// 2k rows: ≤ 60 rows' nodes, none created while scrolling, and (R22) a fling never draws an
/// unbound or stale row.
#[test]
fn a_2k_row_list_binds_its_window_and_flings_without_unbound_rows() {
    let mut r = ListRig::new();
    let ids: Vec<i64> = (1..=2000).collect();
    r.splice(0, 0, ids.iter().copied());
    assert_eq!(r.call("count", &[]), Value::I64(2000));
    r.harness.run();
    let slots = r.slots();
    assert!(slots > 10 && slots <= 60, "{slots} row slots for a 344-point viewport of 24-point rows");
    assert_eq!(r.stats().setups as usize, slots);
    let shown = r.check_rows(&ids);
    assert!(shown >= 14, "{shown} rows shown");
    assert!(r.harness.query_by_label("Item 1").is_some(), "rows' content is in the a11y tree");
    let base = r.stats();
    // the fling: a big wheel delta per frame for many frames, through the whole list
    let mut frames = 0;
    while r.offset() < 2000.0 * 24.0 - 400.0 && frames < 400 {
        r.wheel(-600.0);
        r.harness.step();
        frames += 1;
        r.check_rows(&ids);
        let st = r.stats();
        assert_eq!(st.unbound_drawn, base.unbound_drawn, "frame {frames}: an unbound row was drawn at offset {}", r.offset());
    }
    assert!(r.offset() > 40_000.0, "the fling went through the list: offset {}", r.offset());
    let st = r.stats();
    assert_eq!(st.setups, base.setups, "no row created while scrolling");
    assert_eq!(r.slots(), slots);
    assert!(st.binds > base.binds + 1000, "rows were rebound: {st:?}");
    assert!(r.harness.query_by_label("Item 2000").is_some() || r.harness.query_by_label("Item 1999").is_some());
}

/// `scrollTo`, scroll anchoring by key on a splice above the first visible row, `rowAt`, and the
/// rows' set-size a11y.
#[test]
fn scroll_to_anchoring_and_set_size() {
    let mut r = ListRig::new();
    let mut ids: Vec<i64> = (1..=500).collect();
    r.splice(0, 0, ids.iter().copied());
    r.harness.run();
    r.call("scrollTo", &[Value::I64(300)]);
    r.harness.run();
    let off = r.offset();
    assert!(off > 0.0 && off <= 300.0 * 24.0 && off + 344.0 >= 301.0 * 24.0, "row 300 in view at offset {off}");
    assert_ne!(r.call("rowAt", &[Value::I64(300)]), Value::I64(-1));
    assert_eq!(r.call("rowAt", &[Value::I64(10)]), Value::I64(-1));
    r.check_rows(&ids);
    // insert 10 items at the top: the first visible item keeps its place on screen
    let first = (off / 24.0).floor() as usize;
    let first_id = ids[first];
    let y_before = first as f64 * 24.0 - off;
    r.splice(0, 0, 9001..=9010);
    ids.splice(0..0, 9001..=9010);
    r.harness.run();
    let off2 = r.offset();
    let at = ids.iter().position(|&x| x == first_id).unwrap();
    assert_eq!(at as f64 * 24.0 - off2, y_before, "anchored by key: offset {off} → {off2}");
    r.check_rows(&ids);
    // remove what is in view: rows are rebound, none stale
    r.splice(at as i64, 20, std::iter::empty());
    ids.drain(at..at + 20);
    r.harness.run();
    r.check_rows(&ids);
    // set size and position on the rows' a11y nodes
    let label = format!("Item {}", ids[at]);
    // the row is labelled by its content: the query finds the row, not the label in it
    let n = r.harness.get_by_label(&label);
    let node = n.accesskit_node();
    assert_eq!(node.size_of_set(), Some(ids.len()));
    assert_eq!(node.position_in_set(), Some(at + 1));
}

/// A click selects (`select(pos)`), a double click activates; `selected` set by the host does not
/// echo.
#[test]
fn select_and_activate() {
    let mut r = ListRig::new();
    r.splice(0, 0, 1..=50);
    r.harness.run();
    let rect = r.backend.scene().read(|s| {
        let (n, _) = s.list_rows(r.id()).into_iter().find(|&(_, p)| p == 3).unwrap();
        s.get(n).unwrap().drawn().unwrap()
    });
    let p = egui::pos2(rect.max.x - 20.0, rect.center().y);
    r.harness.event(egui::Event::PointerMoved(p));
    r.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    r.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    r.harness.run();
    assert!(r.st.borrow().events.contains(&("select", vec![Value::I64(3)])), "{:?}", r.st.borrow().events);
    let k = r.br.handle_info(r.list).unwrap().kind;
    assert_eq!(r.br.get_prop(r.list, r.br.resolve(NameKind::Prop, k, "selected")).unwrap(), Value::I64(3));
    r.st.borrow_mut().events.clear();
    r.set(r.list, "selected", Value::I64(7));
    r.harness.run();
    assert!(r.st.borrow().events.is_empty(), "a host write never raises select");
}

// ---- realised rows from a Rust model (the property-grid shape) ----

/// 2k parameters, each a row with a name label and a value label (a property grid's shape: the
/// editor kind is the template).
struct Params {
    names: Vec<String>,
    built: Rc<RefCell<u64>>,
}

impl RowSource for Params {
    fn len(&self) -> usize {
        self.names.len()
    }
    fn key(&self, row: usize) -> u64 {
        row as u64
    }
    fn uniform_height(&self) -> Option<f32> {
        Some(24.0)
    }
    fn build(&mut self, s: &mut SceneMut<'_>, _template: u32) -> NodeId {
        *self.built.borrow_mut() += 1;
        let row = s.create_tag("box").unwrap();
        s.node(row).unwrap().set(&keys::container::FLEX_DIRECTION, "row").unwrap();
        let name = s.create_tag("label").unwrap();
        let value = s.create_tag("label").unwrap();
        s.insert(row, name, None).unwrap();
        s.insert(row, value, None).unwrap();
        row
    }
    fn bind(&mut self, s: &mut SceneMut<'_>, node: NodeId, row: usize) {
        let kids: Vec<NodeId> = s.read().get(node).unwrap().children().map(|n| n.id()).collect();
        s.node(kids[0]).unwrap().set(&keys::label::TEXT, self.names[row].clone()).unwrap();
        s.node(kids[1]).unwrap().set(&keys::label::TEXT, format!("{} mm", row * 10)).unwrap();
    }
    fn anchor(&self, key: u64) -> Option<String> {
        Some(format!("row[param:{}]", self.names[key as usize]))
    }
    fn find_anchor(&self, a: &str) -> Option<u64> {
        let name = a.strip_prefix("row[param:")?.strip_suffix(']')?;
        self.names.iter().position(|n| n == name).map(|p| p as u64)
    }
}

#[test]
fn a_rust_row_source_realises_2k_rows_with_60_nodes() {
    let mut r = ListRig::new();
    let built = Rc::new(RefCell::new(0));
    let names: Vec<String> = (0..2000).map(|i| format!("p{i}")).collect();
    let list = r.id();
    let scene = r.backend.scene();
    let src = Params { names, built: built.clone() };
    scene.write(ProviderId::named("ls1.test"), |m| m.set_row_source(list, src, RowPool::new())).unwrap().unwrap();
    r.set(r.list, "anchor", Value::Str("grid".into()));
    let nodes_before = scene.read(|s| s.len());
    r.harness.run();
    let b0 = *built.borrow();
    assert!(b0 > 10 && b0 <= 60, "{b0} rows built");
    assert!(scene.read(|s| s.len()) - nodes_before <= 60 * 3, "≤ 60 rows' nodes");
    let check = |r: &ListRig| {
        r.backend.scene().read(|s| {
            for (n, pos) in s.list_rows(list) {
                let name = s.get(n).unwrap().children().next().unwrap();
                assert_eq!(name.get(&keys::label::TEXT).map(String::as_str), Some(format!("p{pos}").as_str()));
            }
        })
    };
    check(&r);
    let base = r.stats();
    let mut frames = 0;
    while r.offset() < 2000.0 * 24.0 - 400.0 && frames < 400 {
        r.wheel(-500.0);
        r.harness.step();
        frames += 1;
        check(&r);
        assert_eq!(r.stats().unbound_drawn, base.unbound_drawn, "frame {frames}");
    }
    assert!(r.offset() > 40_000.0);
    assert_eq!(*built.borrow(), b0, "no row built while scrolling");
    // anchors resolve for off-screen rows (row 10 is far above the viewport now)
    let root = scene.read(|s| s.get(list).unwrap().root());
    let t = scene.read(|s| s.target_by_anchor(root, "grid/row[param:p10]"));
    assert_eq!(t, Some(Target::Virtual(VNodeId { host: list, key: 10 })));
    assert_eq!(scene.read(|s| s.target_anchor(t.unwrap())).as_deref(), Some("grid/row[param:p10]"));
    let (_, rect) = scene.read(|s| s.target_rect(t.unwrap())).expect("a laid-out list has row rects");
    assert!(rect.max.y < r.list_rect().min.y, "row 10 is above the viewport: {rect:?}");
    // include_virtual: the list's rows as targets
    let n = scene.read(|s| s.query().tag("virtual-list").include_virtual().targets().count());
    assert_eq!(n, 2001);
    // scroll back to it from Rust: bound before the next layout
    scene.write(ProviderId::named("ls1.test"), |m| m.scroll_list_to(list, 10)).unwrap().unwrap();
    r.harness.run();
    assert!(scene.read(|s| s.list_rows(list).iter().any(|&(_, p)| p == 10)));
    check(&r);
    assert_eq!(*built.borrow(), b0);
}

// ---- end to end: `<ListView>` from @svelters/lists on the real QuickJS host ----

fn bundle(name: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name).join("dist/app.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

/// `fixtures/lists-egui`: 2k items in a `<ListView>`; the list binds its window through JS, a fling
/// draws no unbound row and every drawn row shows its item, keyed splices keep rows bound, and a
/// click selects through `bind:selected`.
#[test]
fn list_view_runs_on_egui() {
    use svelters_egui::harness::TestApp;
    let mut app = TestApp::load(bundle("lists-egui")).unwrap();
    app.run();
    assert!(app.errors().is_empty(), "{:?}", app.errors());
    let scene = app.backend.scene();
    let list = NodeId::from_raw(app.backend.handles_of("virtual-list")[0]).unwrap();
    let text_of = |s: svelters_egui::scene::SceneRef<'_>, n: NodeId| -> String {
        // outlet → row component's box → first label
        let mut d = s.get(n).unwrap().descendants();
        d.find(|x| x.tag() == "label").map(|l| l.text().to_string()).unwrap_or_default()
    };
    let check = |app: &TestApp, first: usize| {
        app.backend.scene().read(|s| {
            let rows = s.list_rows(list);
            assert!(!rows.is_empty());
            for (n, pos) in rows {
                let want = if pos < first { "Item new".to_string() } else { format!("Item {}", pos + 1 - first) };
                assert_eq!(text_of(s, n), want, "row at {pos}");
            }
        })
    };
    check(&app, 0);
    let v = scene.read(|s| s.virtual_list(list).unwrap().len());
    assert_eq!(v, 2000);
    let slots = scene.read(|s| s.virtual_list(list).unwrap().slot_count());
    assert!(slots <= 60, "{slots} row slots");
    let base = scene.read(|s| s.virtual_list(list).unwrap().stats());
    let rect = scene.read(|s| s.get(list).unwrap().drawn().unwrap());
    for _ in 0..80 {
        app.harness.event(egui::Event::PointerMoved(rect.center()));
        app.harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -500.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
        app.harness.step();
        check(&app, 0);
        let st = scene.read(|s| s.virtual_list(list).unwrap().stats());
        assert_eq!(st.unbound_drawn, base.unbound_drawn, "an unbound row was drawn");
    }
    let st = scene.read(|s| s.virtual_list(list).unwrap().stats());
    assert_eq!(st.setups, base.setups, "no row slot created while scrolling");
    assert!(scene.read(|s| s.virtual_list(list).unwrap().offset()) > 10_000.0);
    // keyed splices from JS: prepend keeps the first visible row in place (anchoring)
    let off = scene.read(|s| s.virtual_list(list).unwrap().offset());
    app.harness.get_by_label("prepend").click();
    app.run();
    let off2 = scene.read(|s| s.virtual_list(list).unwrap().offset());
    assert_eq!(off2 - off, 24.0, "anchored by key");
    check(&app, 1);
    // scrollEnd through bind:this
    app.harness.get_by_label("scrollEnd").click();
    app.run();
    assert!(app.harness.query_by_label("Item 2000").is_some());
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}

/// EV3: a focused row of a selectable list claims the navigation keys (R11), so a host keymap
/// leaves them; `focus_info` names the list.
#[test]
fn a_focused_row_claims_the_navigation_keys() {
    let mut r = ListRig::new();
    r.splice(0, 0, 1..=50);
    r.harness.run();
    r.harness.key_press(egui::Key::Tab);
    r.harness.run();
    let ctx = r.harness.ctx.clone();
    let info = r.backend.focus_info(&ctx);
    assert_eq!(info.kind, Some("virtual-list"), "{info:?}");
    assert_eq!(info.node, Some(r.id()));
    let k = |m, key| egui::KeyboardShortcut::new(m, key);
    assert!(r.backend.shortcut_override(&ctx, &k(egui::Modifiers::NONE, egui::Key::ArrowDown)));
    assert!(r.backend.shortcut_override(&ctx, &k(egui::Modifiers::NONE, egui::Key::PageUp)));
    assert!(!r.backend.shortcut_override(&ctx, &k(egui::Modifiers::COMMAND, egui::Key::Z)));
}

// ---- OW5a: own rows (DESIGN R6, R8, R14), the Scrollable bar, EV1 gates, tree rows ----

impl ListRig {
    /// Row `pos`'s drawn rect and node.
    fn row(&self, pos: usize) -> (NodeId, egui::Rect) {
        self.backend.scene().read(|s| {
            let (n, _) = s.list_rows(self.id()).into_iter().find(|&(_, p)| p == pos).expect("row drawn");
            (n, s.get(n).unwrap().drawn().unwrap())
        })
    }

    fn press(&mut self, p: egui::Pos2, button: egui::PointerButton) {
        self.harness.event(egui::Event::PointerMoved(p));
        self.harness.event(egui::Event::PointerButton { pos: p, button, pressed: true, modifiers: egui::Modifiers::NONE });
        self.harness.event(egui::Event::PointerButton { pos: p, button, pressed: false, modifiers: egui::Modifiers::NONE });
        self.harness.run();
    }

    fn key(&mut self, k: egui::Key) {
        self.harness.key_press(k);
        self.harness.run();
    }

    fn events(&self, name: &str) -> Vec<Vec<Value>> {
        self.st.borrow().events.iter().filter(|(n, _)| *n == name).map(|(_, a)| a.clone()).collect()
    }

    /// The fill of the `::part(row)` box traced across row `rect` (transparent: none).
    fn row_fill(&self, rect: egui::Rect) -> egui::Color32 {
        let t = svelters_egui::paint::trace::last(&self.harness.ctx);
        t.of(self.id().to_raw(), Some("row"))
            .filter(|e| e.kind == svelters_egui::paint::TraceKind::Box && (e.rect.center().y - rect.center().y).abs() < 1.0)
            .map(|e| e.fill)
            .last()
            .unwrap_or(egui::Color32::TRANSPARENT)
    }

    fn row_ring(&self, rect: egui::Rect) -> bool {
        let t = svelters_egui::paint::trace::last(&self.harness.ctx);
        t.of(self.id().to_raw(), Some("row")).any(|e| e.kind == svelters_egui::paint::TraceKind::Outline && (e.rect.center().y - rect.center().y).abs() < 1.0)
    }
}

/// R6: rest, hover, active selection and inactive selection are four distinct fills of
/// `::part(row)`, and R8's ring is on the cursor row for keyboard focus only; the row node
/// mirrors `:selected` and `:current`.
#[test]
fn ow5a_rows_show_r6_states_and_a_keyboard_only_ring() {
    use svelters_egui::provider::StateBits;
    let mut r = ListRig::new();
    svelters_egui::paint::trace::enable(&r.harness.ctx, true);
    r.splice(0, 0, 1..=50);
    r.harness.run();
    let (_, r3) = r.row(3);
    let rest = r.row_fill(r3);
    assert_eq!(rest.a(), 0, "a row at rest is flat and unfilled");
    r.harness.event(egui::Event::PointerMoved(r3.center()));
    r.harness.run();
    let hover = r.row_fill(r3);
    assert!(hover.a() > 0 && hover != rest, "hover fill");
    r.press(r3.center(), egui::PointerButton::Primary);
    r.harness.event(egui::Event::PointerMoved(egui::pos2(-10.0, -10.0)));
    r.harness.run();
    let active = r.row_fill(r3);
    assert!(active.a() > 0 && active != hover, "active selection fill: {active:?} vs {hover:?}");
    assert!(!r.row_ring(r3), "R8: a click shows no ring");
    assert_eq!(r.events("row-clicked"), vec![vec![Value::I64(3), Value::I64(4)]]);
    // Down: the cursor row moves and shows the ring (keyboard)
    r.key(egui::Key::ArrowDown);
    r.harness.run();
    let (n4, r4) = r.row(4);
    assert!(r.row_ring(r4), "R8: the keyboard cursor row shows its ring");
    let st = r.backend.scene().read(|s| s.get(n4).unwrap().state());
    assert!(st.contains(StateBits::SELECTED) && st.contains(StateBits::CURRENT), "the row node mirrors its row: {st:?}");
    // focus elsewhere: the selection stays, in the inactive colour, without a ring
    r.harness.ctx.memory_mut(|m| m.request_focus(egui::Id::unique("elsewhere")));
    r.harness.run();
    r.harness.run();
    let inactive = r.row_fill(r4);
    assert!(inactive.a() > 0 && inactive != active && inactive != hover, "inactive selection fill: {inactive:?}");
    assert!(!r.row_ring(r4));
    // a secondary click asks for the row's context menu
    r.press(r3.center(), egui::PointerButton::Secondary);
    assert_eq!(r.events("row-context"), vec![vec![Value::I64(3), Value::I64(4)]]);
}

/// P1 and the per-row state: `::part(row):nth-child(even)` stripes by logical position and
/// `::part(row):selected` beats it on the selected row only (the rules see the row's state, not
/// the list's).
#[test]
fn ow5a_part_row_rules_match_each_rows_position_and_state() {
    let mut r = ListRig::new();
    svelters_egui::paint::trace::enable(&r.harness.ctx, true);
    let d = r.backend.add_sheet(
        "ow5a",
        "virtual-list::part(row):nth-child(even) { background-color: #102030 } virtual-list::part(row):selected { background-color: #00ff00 }",
    );
    assert!(d.is_empty(), "{d:?}");
    r.splice(0, 0, 1..=50);
    r.harness.run();
    let stripe = egui::Color32::from_rgb(0x10, 0x20, 0x30);
    // :nth-child(even) is 1-based: positions 1, 3, 5 …
    assert_eq!(r.row_fill(r.row(1).1), stripe);
    assert_eq!(r.row_fill(r.row(3).1), stripe);
    assert_ne!(r.row_fill(r.row(2).1), stripe);
    r.press(r.row(2).1.center(), egui::PointerButton::Primary);
    r.harness.event(egui::Event::PointerMoved(egui::pos2(-10.0, -10.0)));
    r.harness.run();
    assert_eq!(r.row_fill(r.row(2).1), egui::Color32::from_rgb(0, 255, 0), "the selected row");
    assert_eq!(r.row_fill(r.row(3).1), stripe, "its neighbour keeps its stripe");
}

/// OW2's Scrollable is the list's bar: dragging its thumb scrolls the list (the virtualiser binds
/// the new window), and AccessKit sees a `ScrollBar`.
#[test]
fn ow5a_the_bar_is_a_scrollable_thumb() {
    let mut r = ListRig::new();
    svelters_egui::paint::trace::enable(&r.harness.ctx, true);
    r.splice(0, 0, 1..=2000);
    r.harness.run();
    let lr = r.list_rect();
    r.harness.event(egui::Event::PointerMoved(lr.center()));
    r.harness.run();
    let t = svelters_egui::paint::trace::last(&r.harness.ctx);
    let thumb = t.of(r.id().to_raw(), Some("thumb-y")).last().expect("the thumb is drawn while hovered").rect;
    assert!(thumb.max.x <= lr.max.x + 0.5 && thumb.width() <= 12.0, "{thumb:?} in {lr:?}");
    let from = thumb.center();
    let to = from + egui::vec2(0.0, 100.0);
    r.harness.event(egui::Event::PointerMoved(from));
    r.harness.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    r.harness.step();
    for k in 1..=10 {
        r.harness.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 10.0)));
        r.harness.step();
    }
    r.harness.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    r.harness.run();
    assert!(r.offset() > 2000.0, "the thumb drag scrolled: {}", r.offset());
    assert_eq!(r.stats().unbound_drawn, 0, "rows were bound for the new offset");
    assert!(r.harness.query_all_by_role(egui::accesskit::Role::ScrollBar).next().is_some());
}

/// EV1: a Rust filter that prevents `selectionchange` keeps the selection; one that prevents
/// `activate` stops the activation.
#[test]
fn ow5a_filters_prevent_the_lists_default_actions() {
    use svelters_egui::events::{FilterScope, Flow, SceneEvent};
    use svelters_egui::scene::SceneRef;
    let mut r = ListRig::new();
    r.splice(0, 0, 1..=20);
    r.harness.run();
    let _f = r.backend.scene().register_filter(FilterScope::All, |ev: &SceneEvent, _s: SceneRef<'_>| {
        if matches!(ev.name(), "selectionchange" | "activate") { Flow::Prevent } else { Flow::Pass }
    });
    r.press(r.row(2).1.center(), egui::PointerButton::Primary);
    let k = r.br.handle_info(r.list).unwrap().kind;
    assert_eq!(r.br.get_prop(r.list, r.br.resolve(NameKind::Prop, k, "selected-ids")).unwrap(), Value::List(vec![]), "prevented: no selection");
    assert!(r.events("selectionchange").is_empty());
    r.key(egui::Key::Enter);
    assert!(r.events("activate").is_empty(), "prevented: no activation");
}

/// Tree rows (WG5b's `tree-view`, own): TreeItem roles with level and expanded, the twistie
/// (R14) toggles through Expandable, `expand-on="click"` toggles on a row click, and
/// `root-decorated={false}` hides level-0 twisties.
#[test]
fn ow5a_tree_rows_expand_through_their_twistie_and_expand_on() {
    let mut r = ListRig::with_kind("tree-view");
    svelters_egui::paint::trace::enable(&r.harness.ctx, true);
    r.splice(0, 0, [1, 2, 3]);
    let strs = |v: &[&str]| Value::List(v.iter().map(|x| Value::Str((*x).into())).collect());
    let ints = |v: &[i64]| Value::List(v.iter().map(|x| Value::I64(*x)).collect());
    // 1 (expandable, collapsed) and 2 (expandable, expanded) at level 0, 3 a leaf at level 1
    r.call("setNodes", &[ints(&[1, 2, 3]), ints(&[0, 0, 1]), ints(&[1, 3, 0]), strs(&["one", "two", "three"]), strs(&["a", "b", "c"])]);
    r.harness.run();
    let items: Vec<_> = r.harness.query_all_by_role(egui::accesskit::Role::TreeItem).collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].accesskit_node().data().is_expanded(), Some(false));
    assert_eq!(items[1].accesskit_node().data().is_expanded(), Some(true));
    assert_eq!(items[2].accesskit_node().data().level(), Some(2));
    assert_eq!(items[2].accesskit_node().data().is_expanded(), None, "a leaf has no expanded state");
    let t = svelters_egui::paint::trace::last(&r.harness.ctx);
    let twisties: Vec<_> = t.of(r.id().to_raw(), Some("twistie")).collect();
    assert_eq!(twisties.len(), 2, "R14: a twistie on each expandable row");
    // a click on the first row's twistie raises expand(1)
    let tw = twisties.iter().find(|e| (e.rect.center().y - r.row(0).1.center().y).abs() < 1.0).unwrap().rect;
    r.press(tw.center(), egui::PointerButton::Primary);
    assert_eq!(r.events("expand"), vec![vec![Value::I64(1)]]);
    // a click elsewhere on the row only selects …
    r.press(r.row(1).1.center() + egui::vec2(60.0, 0.0), egui::PointerButton::Primary);
    assert!(r.events("collapse").is_empty());
    // … until `expand-on="click"`
    r.set(r.list, "expand-on", Value::Str("click".into()));
    r.harness.run();
    r.press(r.row(1).1.center() + egui::vec2(60.0, 0.0), egui::PointerButton::Primary);
    assert_eq!(r.events("collapse"), vec![vec![Value::I64(2)]]);
    // root-decorated={false}: no twistie on level 0
    r.set(r.list, "root-decorated", Value::Bool(false));
    r.harness.run();
    r.harness.run();
    let t = svelters_egui::paint::trace::last(&r.harness.ctx);
    assert_eq!(t.of(r.id().to_raw(), Some("twistie")).count(), 0);
}
