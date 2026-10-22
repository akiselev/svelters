//! WG6 (plans/milestones/WG6-widget-polish.md): tooltips on disabled widgets and the `tooltip`
//! element (QA5), `status-tip`, `elide` (QW13), `scroll-into-view` (QA13), `cursor` (QA14),
//! `key-sequence-edit` (QW10), combo type-ahead (QW9), file drops and drop actions (QA9).

mod common;

use common::Rig;
use egui_kittest::kittest::Queryable;
use std::sync::Arc;
use svelters_abi::Value;

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn events_of(rig: &Rig, log: &common::Log, h: u64, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|(eh, es, _)| *eh == h && *es == sig).map(|(_, _, a)| a.clone()).collect()
}

#[test]
fn wg6_disabled_widgets_show_their_tooltip_and_rich_tooltips() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    rig.leaf(b, "button", &[("text", s("Fillet")), ("enabled", Value::Bool(false)), ("tooltip", s("Select an edge first"))]);
    let rich = rig.leaf(b, "button", &[("text", s("Extrude"))]);
    let tip = rig.create("tooltip");
    rig.insert(rich, tip);
    rig.leaf(tip, "label", &[("text", s("Extrude a sketch profile"))]);
    // every tooltip shows at once (no hover delay in the test)
    rig.harness.ctx.memory_mut(|m| m.set_everything_is_visible(true));
    rig.run();
    rig.run();
    assert!(rig.harness.query_by_label("Select an edge first").is_some(), "a disabled button's tooltip shows");
    assert!(rig.harness.query_by_label("Extrude a sketch profile").is_some(), "the tooltip element shows");
}

#[test]
fn wg6_status_tip_and_cursor_follow_the_pointer() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let run = rig.leaf(b, "button", &[("text", s("Run")), ("status-tip", s("Run the simulation")), ("cursor", s("pointer"))]);
    let off = rig.leaf(b, "button", &[("text", s("Off")), ("enabled", Value::Bool(false)), ("status-tip", s("Needs a solver"))]);
    rig.run();
    assert!(rig.br().set_prop(run, rig.prop_id(run, "cursor"), &s("hand")).is_err(), "CSS names only");
    rig.harness.hover_at(rig.drawn(run).center());
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx).as_deref(), Some("Run the simulation"));
    assert_eq!(rig.harness.output().platform_output.cursor_icon, egui::CursorIcon::PointingHand);
    rig.harness.hover_at(rig.drawn(off).center());
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx).as_deref(), Some("Needs a solver"), "disabled too");
    rig.harness.hover_at(egui::pos2(700.0, 500.0));
    rig.run();
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx), None);
}

#[test]
fn wg6_label_elides_in_the_middle() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let path = "/home/user/projects/cadatomic/boards/main/power-supply.kicad_pcb";
    let l = rig.leaf(b, "label", &[("text", s(path)), ("elide", s("middle")), ("width", Value::F64(160.0))]);
    rig.run();
    assert_eq!(rig.get(l, "elide"), s("middle"));
    assert!(rig.drawn(l).width() <= 161.0, "{:?}", rig.drawn(l));
    // the full text is the accessible name; what is drawn has an ellipsis
    // (AccessKit: `label` the full text, `value` what is drawn; kittest's queries match the value)
    let node = format!("{:?}", rig.harness.get_by_role(egui::accesskit::Role::Label));
    assert!(node.contains(&format!("label: \"{path}\"")), "{node}");
    assert!(rig.harness.query_by_label("/home/user/p…y.kicad_pcb").is_some(), "{node}");
    rig.set(l, "elide", s("none"));
    rig.set(l, "wrap-mode", s("extend"));
    rig.run();
    assert!(rig.harness.query_by_label(path).is_some(), "elide none: the whole text again");
}

#[test]
fn wg6_scroll_into_view_reveals_a_node_below_the_fold() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let sa = rig.create("scroll-area");
    rig.set(sa, "height", Value::F64(120.0));
    rig.insert(b, sa);
    let col = rig.create("box");
    rig.insert(sa, col);
    let rows: Vec<_> = (0..40).map(|k| rig.leaf(col, "button", &[("text", s(&format!("Row {k}")))])).collect();
    rig.run();
    let view = rig.drawn(sa);
    let last = rows[39];
    let m = rig.br().resolve(svelters_abi::NameKind::Method, rig.kind(last), "scroll-into-view");
    assert!(rig.br().call(last, m, &[s("sideways")]).is_err());
    rig.br().call(last, m, &[]).unwrap();
    rig.run();
    rig.run();
    let r = rig.drawn(last);
    assert!(view.expand(1.0).contains_rect(r), "row 39 at {r:?} is inside the scroll area {view:?}");
}

#[test]
fn wg6_key_sequence_edit_records_a_shortcut() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let k = rig.leaf(b, "key-sequence-edit", &[("value", s("Ctrl+S"))]);
    rig.observe(k, "value", true);
    rig.listen(k, "changed");
    rig.run();
    assert!(rig.harness.query_by_label("Ctrl+S").is_some());
    rig.click(k);
    assert!(rig.harness.query_by_label("Press a shortcut…").is_some(), "recording");
    rig.harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::K);
    rig.run();
    assert_eq!(rig.get(k, "value"), s("Ctrl+Shift+K"));
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, k, "changed"), vec![vec![s("Ctrl+Shift+K")]]);
    // Backspace clears; Escape cancels
    rig.click(k);
    rig.harness.key_press(egui::Key::Backspace);
    rig.run();
    assert_eq!(rig.get(k, "value"), s(""));
    assert!(rig.harness.query_by_label("None").is_some());
}

#[test]
fn wg6_combo_box_type_ahead() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let opts = Value::List(["Aluminium", "Brass", "Bronze", "Copper", "Steel"].iter().map(|o| s(o)).collect());
    let c = rig.leaf(b, "combo-box", &[("options", opts), ("selected", Value::I64(0)), ("accessible-name", s("Material"))]);
    rig.listen(c, "changed");
    rig.run();
    rig.harness.key_press(egui::Key::Tab);
    rig.run();
    rig.harness.event(egui::Event::Text("br".into()));
    rig.run();
    assert_eq!(rig.get(c, "selected"), Value::I64(1), "br: Brass");
    rig.harness.event(egui::Event::Text("o".into()));
    rig.run();
    assert_eq!(rig.get(c, "selected"), Value::I64(2), "bro: Bronze");
}

#[derive(Debug)]
struct File(std::path::PathBuf);

impl egui::DroppedFile for File {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        Ok(vec![])
    }
}

#[test]
fn wg6_files_dropped_from_the_os_arrive_as_a_uri_list() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let t = rig.leaf(b, "button", &[("text", s("Drop STEP here")), ("drop-types", s("text/uri-list"))]);
    let other = rig.leaf(b, "button", &[("text", s("Parts only")), ("drop-types", s("cad/part"))]);
    for h in [t, other] {
        rig.listen(h, "drag-enter");
        rig.listen(h, "drag-leave");
        rig.listen(h, "drop");
    }
    rig.run();
    let at = rig.drawn(t).center();
    rig.harness.hover_at(at);
    rig.harness.input_mut().hovered_files.push(egui::HoveredFile { path: Some("/tmp/a.step".into()), ..Default::default() });
    rig.harness.step();
    rig.harness.step();
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, t, "drag-enter"), vec![vec![s("text/uri-list")]]);
    // winit: the hover ends with the drop
    rig.harness.input_mut().hovered_files.clear();
    rig.harness.input_mut().dropped_files.push(Arc::new(File("/tmp/a b.step".into())));
    rig.harness.step();
    rig.run();
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, t, "drag-leave").len(), 1);
    let drops = events_of(&rig, &log, t, "drop");
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0][0], s("text/uri-list"));
    assert_eq!(drops[0][1], s("file:///tmp/a%20b.step"));
    assert_eq!(drops[0][4], s("copy"));
    assert!(events_of(&rig, &log, other, "drop").is_empty(), "cad/part does not take files");
}
