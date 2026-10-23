//! OW5b: the own `table` header (plans/milestones/OW5b-table.md): FM1's QW4 sort, column menu and
//! `header-state`, Resizable column edges, `fit-columns`, `visible` columns, a11y with the sort.

use egui::Role;
use egui::accesskit::SortDirection;
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::{Backend, Value};
use svelters_egui::embed::{EmbedConfig, SlotId, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::kinds;
use svelters_egui::scene::{NodeId, ProviderId, SceneHandle};
use svelters_host::HostConfig;

const APP: &str = "ow5b.test";

struct Rig {
    h: EmbedHarness,
    scene: SceneHandle,
    t: NodeId,
    cols: Vec<NodeId>,
}

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

impl Rig {
    fn new() -> Rig {
        let cfg = EmbedConfig { host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() };
        let mut eh = EmbedHarness::new(egui::vec2(600.0, 300.0), cfg).unwrap();
        let e = eh.embed.clone();
        let slot: SlotId = e.create_slot("ow5b", SlotOptions::fill());
        eh.set_ui(move |e, ui| {
            e.show_slot(ui, slot);
        });
        let scene = e.backend().scene();
        let slot = NodeId::from_raw(slot.handle()).unwrap();
        let mut r = Rig { h: eh, scene, t: slot, cols: Vec::new() };
        r.t = r.add(slot, "table", &[("flex-grow", Value::F64(1.0)), ("row-count", Value::I64(20)), ("sortable", Value::Bool(true)), ("accessible-name", s("Parts"))]);
        for (title, name, w) in [("Name", "name", 120.0), ("Value", "", 100.0), ("Footprint", "fp", 160.0)] {
            let c = r.add(r.t, "table-column", &[("title", s(title)), ("name", s(name)), ("width", Value::F64(w))]);
            r.cols.push(c);
        }
        r.h.run();
        let rows = Value::List((0..20).map(|k| Value::List((0..3).map(|c| s(&format!("r{k}c{c}"))).collect())).collect());
        r.call("set-rows", &[Value::I64(0), rows]);
        r.h.run();
        r
    }

    fn add(&self, parent: NodeId, tag: &str, props: &[(&str, Value)]) -> NodeId {
        let props: Vec<(String, Value)> = props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let tag = tag.to_string();
        self.scene
            .write(ProviderId::named(APP), move |m| {
                let n = m.create_tag(&tag).unwrap();
                for (k, v) in &props {
                    m.node(n).unwrap().set_by_name(k, v).unwrap_or_else(|e| panic!("{tag}.{k}: {e:?}"));
                }
                m.insert(parent, n, None).unwrap();
                n
            })
            .unwrap()
    }

    fn set(&self, n: NodeId, name: &str, v: Value) {
        let name = name.to_string();
        self.scene.write(ProviderId::named(APP), move |m| m.node(n).unwrap().set_by_name(&name, &v).unwrap()).unwrap();
    }

    fn get(&self, n: NodeId, name: &str) -> Value {
        self.scene.read(|s| s.get(n).unwrap().value_of(name).unwrap())
    }

    fn call(&self, name: &str, args: &[Value]) -> Value {
        let b = self.h.embed.backend();
        let (m, _) = kinds::find_method(kinds::tag_index("table").unwrap(), name).unwrap();
        b.call(self.t.to_raw(), m, args).unwrap()
    }

    fn header(&self, title: &str) -> Option<egui::Rect> {
        self.h.harness.query_by_role_and_label(Role::ColumnHeader, title).map(|n| n.rect())
    }

    fn sort_of(&self, title: &str) -> Option<SortDirection> {
        self.h.harness.query_by_role_and_label(Role::ColumnHeader, title).and_then(|n| n.accesskit_node().sort_direction())
    }

    fn events(&mut self, evs: Vec<egui::Event>) {
        for e in evs {
            self.h.harness.event(e);
            self.h.harness.step();
        }
        self.h.run();
    }

    fn button(&mut self, at: egui::Pos2, button: egui::PointerButton) {
        let m = egui::Modifiers::NONE;
        self.events(vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton { pos: at, button, pressed: true, modifiers: m },
            egui::Event::PointerButton { pos: at, button, pressed: false, modifiers: m },
        ]);
    }

    fn header_state(&self) -> serde_json::Value {
        let Value::Str(st) = self.get(self.t, "header-state") else { panic!("a string") };
        serde_json::from_str(&st).unwrap()
    }
}

#[test]
fn a_sortable_header_sorts_and_saves_its_state() {
    let mut r = Rig::new();
    assert!(r.h.harness.query_by_role_and_label(Role::Table, "Parts").is_some());
    assert_eq!(r.sort_of("Value"), None);
    // a click sorts ascending, a second click flips
    r.h.harness.get_by_role_and_label(Role::ColumnHeader, "Value").click();
    r.h.run();
    assert_eq!((r.get(r.t, "sort-column"), r.get(r.t, "sort-order")), (Value::I64(1), s("ascending")));
    assert_eq!(r.sort_of("Value"), Some(SortDirection::Ascending), "a11y: ColumnHeader with its sort");
    let st = r.header_state();
    assert_eq!((st["sort"]["column"].as_str(), st["widths"]["name"].as_f64()), (Some("1"), Some(120.0)), "{st}");
    r.h.harness.get_by_role_and_label(Role::ColumnHeader, "Value").click();
    r.h.run();
    assert_eq!(r.get(r.t, "sort-order"), s("descending"));
    assert_eq!(r.sort_of("Value"), Some(SortDirection::Descending));
}

#[test]
fn the_column_menu_hides_columns_and_header_state_restores_them() {
    let mut r = Rig::new();
    let at = r.header("Name").unwrap().center();
    r.button(at, egui::PointerButton::Secondary);
    // the menu: the hideable columns as check items, all checked
    let fp = r.h.harness.get_by_role_and_label(Role::MenuItemCheckBox, "Footprint");
    assert_eq!(fp.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    fp.click();
    r.h.run();
    assert_eq!(r.get(r.t, "hidden-columns"), Value::List(vec![s("fp")]));
    assert!(r.header("Footprint").is_none(), "hidden");
    assert!(r.h.harness.query_by_role(Role::MenuItemCheckBox).is_none(), "a pick closes the menu");
    assert_eq!(r.header_state()["hidden"][0].as_str(), Some("fp"));
    // restoring a saved state: widths, hidden columns, sort
    r.set(r.t, "header-state", s(r#"{"widths":{"name":200},"hidden":[],"sort":{"column":"fp","order":"descending"}}"#));
    r.h.run();
    assert!(r.header("Footprint").is_some());
    assert_eq!(r.get(r.cols[0], "width"), Value::F64(200.0), "bind:width carries the restored width");
    assert_eq!((r.get(r.t, "sort-column"), r.sort_of("Footprint")), (Value::I64(2), Some(SortDirection::Descending)));
    // `visible={false}` takes a column out as well
    r.set(r.cols[1], "visible", Value::Bool(false));
    r.h.run();
    assert!(r.header("Value").is_none() && r.header("Name").is_some());
}

#[test]
fn header_edges_resize_and_fit_columns() {
    let mut r = Rig::new();
    let name = r.header("Name").unwrap();
    let edge = egui::pos2(name.right(), name.center().y);
    let m = egui::Modifiers::NONE;
    let b = egui::PointerButton::Primary;
    r.events(vec![
        egui::Event::PointerMoved(edge),
        egui::Event::PointerButton { pos: edge, button: b, pressed: true, modifiers: m },
        egui::Event::PointerMoved(edge + egui::vec2(20.0, 0.0)),
        egui::Event::PointerMoved(edge + egui::vec2(40.0, 0.0)),
        egui::Event::PointerButton { pos: edge + egui::vec2(40.0, 0.0), button: b, pressed: false, modifiers: m },
    ]);
    assert_eq!(r.get(r.cols[0], "width"), Value::F64(160.0), "dragged 40 to the right");
    assert_eq!(r.header("Name").unwrap().width().round(), 160.0);
    assert_eq!(r.header_state()["widths"]["name"].as_f64(), Some(160.0));
    // min-width clamps a drag
    r.set(r.cols[0], "min-width", Value::F64(150.0));
    let name = r.header("Name").unwrap();
    let edge = egui::pos2(name.right(), name.center().y);
    r.events(vec![
        egui::Event::PointerMoved(edge),
        egui::Event::PointerButton { pos: edge, button: b, pressed: true, modifiers: m },
        egui::Event::PointerMoved(edge - egui::vec2(60.0, 0.0)),
        egui::Event::PointerButton { pos: edge - egui::vec2(60.0, 0.0), button: b, pressed: false, modifiers: m },
    ]);
    assert_eq!(r.get(r.cols[0], "width"), Value::F64(150.0));
    // fit-columns: the Value column fits "Value" and "r19c1", far below its 100
    r.call("fit-columns", &[Value::I64(1)]);
    r.h.run();
    let Value::F64(w) = r.get(r.cols[1], "width") else { panic!() };
    assert!((20.0..80.0).contains(&w), "fitted: {w}");
}
