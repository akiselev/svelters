//! rmegui lane S2 (`plans/cloud/rmegui/LANE-S2.md`): skelter bugs cadatomic's rmegui wave 1 found on the
//! native shell, on a real skelter-core document with skelter-widgets' kinds, read through AccessKit's
//! consumer (and AT-SPI's translation layer on Linux):
//! - FQ-66: a `table` exposes its header, realised rows and cells, with actions.
//! - SV59: `setLabels` renames a list's realised options;
//! - SV60: `aria-activedescendant` / `aria-controls` resolve when their target appears later;
//! - a label stretched past its text that JS listens to for clicks takes them on its whole box.
//! - FQ-67: a widget re-enabled by its `disabled` prop is enabled for AccessKit too.
//! - a select's `option-anchors` resolve with svelters-automation's `anchor=` selector.

use skelter::CoreDocument;
use skelter_base::accesskit::{Action, Role};
use skelter_base::geom::Size;
use skelter_base::ids::NodeId;
use skelter_base::key::Key;
use skelter_core::{Document, KindId, KindRegistry, StyleInput, Value, common};
use skelter_test::Harness;
use std::sync::Arc;

const BOX: KindId = KindId::of("box");
const TABLE: KindId = KindId::of("table");
const COLUMN: KindId = KindId::of("table-column");

fn text() -> skelter_text::TextSystem {
    skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing()))
}

/// A 400 × 300 window on a document with skelter-widgets' tier 0 kinds, the table, `virtual-list`
/// and `text-edit`.
fn harness() -> Harness<CoreDocument> {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    skelter_widgets::host::register::<skelter_widgets::views::virtual_list::VirtualList>(&mut reg).unwrap();
    skelter_widgets::host::register::<skelter_widgets::views::select::Select>(&mut reg).unwrap();
    skelter_widgets::table::kinds::register(&mut reg).unwrap();
    reg.register::<skelter_widgets::text::TextEdit>().unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let doc = CoreDocument::with_widgets(doc, BOX, text(), skelter_widgets::host::os_policy(), skelter_widgets::theme::Theme::light_modern());
    Harness::<CoreDocument>::builder().with_size(Size::new(400.0, 300.0)).with_consumer(true).build(doc)
}

fn add(d: &mut Document, parent: NodeId, kind: KindId, css: &str) -> NodeId {
    let n = d.create(kind);
    d.set_style(n, StyleInput { inline: Some(css.into()), ..Default::default() });
    d.insert(parent, n, None, None).unwrap();
    n
}

fn prop(d: &mut Document, n: NodeId, kind: KindId, name: &str, v: Value) {
    let p = d.registry().get(kind).unwrap().spec.prop_named(name).unwrap();
    d.set_prop(n, p, v).unwrap();
}

fn call(d: &mut Document, n: NodeId, kind: KindId, name: &str, args: &[Value]) {
    let m = d.registry().get(kind).unwrap().spec.method_named(name).unwrap();
    d.call(n, m, args).unwrap();
}

fn list(items: impl IntoIterator<Item = Value>) -> Value {
    Value::List(items.into_iter().collect::<Vec<_>>().into())
}

/// The tree changed outside input: a key the document ignores brings the frame.
fn settle(h: &mut Harness<CoreDocument>) {
    h.key_press(Key::F24);
    h.run();
}

/// FQ-66: the table's header row of column headers, and its realised rows with cells, named from
/// the cell text, with row and column index and selected state. `Click` on a cell selects its row
/// as a pointer click does; on a header, it reports `header-clicked`.
#[test]
fn a_table_exposes_rows_and_cells() {
    let mut h = harness();
    let w = h.window();
    let data = [["R1", "10k"], ["C3", "100n"], ["U2", "LM358"]];
    let table = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let t = add(d, root, TABLE, "width: 300px; height: 200px");
        d.set_prop(t, common::ANCHOR, Value::from("parts")).unwrap();
        for title in ["Ref", "Value"] {
            let c = d.create(COLUMN);
            d.insert(t, c, None, None).unwrap();
            prop(d, c, COLUMN, "title", Value::from(title));
            prop(d, c, COLUMN, "width", Value::F64(100.0));
        }
        prop(d, t, TABLE, "row-count", Value::I64(data.len() as i64));
        let rows = list(data.iter().map(|r| list(r.iter().map(|c| Value::from(*c)))));
        call(d, t, TABLE, "set-rows", &[Value::I64(0), rows]);
        t
    };
    settle(&mut h);
    settle(&mut h);
    let c = h.consumer();
    let t = c.by_author_id("parts").expect("the table");
    assert_eq!(t.role(), Role::Table);
    let kids: Vec<_> = t.children().collect();
    assert_eq!(kids.len(), 1 + data.len(), "the header row and a row per realised row");
    let head = &kids[0];
    assert_eq!(head.role(), Role::Row);
    let heads: Vec<(Role, Option<String>, Option<usize>)> = head.children().map(|n| (n.role(), n.data().label().map(str::to_owned), n.data().column_index())).collect();
    assert_eq!(heads, [(Role::ColumnHeader, Some("Ref".into()), Some(0)), (Role::ColumnHeader, Some("Value".into()), Some(1))]);
    for (pos, row) in kids[1..].iter().enumerate() {
        assert_eq!(row.role(), Role::Row);
        assert_eq!(row.data().row_index(), Some(pos));
        assert_eq!(row.data().label(), Some(data[pos].join(" ").as_str()), "a row is named from its cells");
        assert_eq!(row.data().is_selected(), Some(false));
        let cells: Vec<(Role, Option<String>, Option<usize>, Option<usize>)> =
            row.children().map(|n| (n.role(), n.data().label().map(str::to_owned), n.data().row_index(), n.data().column_index())).collect();
        assert_eq!(cells, [(Role::Cell, Some(data[pos][0].into()), Some(pos), Some(0)), (Role::Cell, Some(data[pos][1].into()), Some(pos), Some(1))]);
        assert!(row.data().supports_action(Action::Click));
    }
    // AccessKit's Click on the second row's value cell selects that row, as a click does
    // (virtual children in order: the header row 0, its headers 1–2, row 0 at 3 with cells 4–5, row 1 at 6)
    let cell = skelter_core::a11y::virtual_id(table, 8);
    assert_eq!(kids[2].children().nth(1).unwrap().data().label(), Some("100n"));
    h.accesskit_action(skelter_base::accesskit::ActionRequest { action: Action::Click, target_tree: skelter_base::accesskit::TreeId::ROOT, target_node: cell, data: None });
    h.run();
    let selected = |h: &Harness<CoreDocument>| -> Vec<Option<bool>> { h.consumer().by_author_id("parts").unwrap().children().skip(1).map(|r| r.data().is_selected()).collect() };
    assert_eq!(selected(&h), [Some(false), Some(true), Some(false)]);
    let sel = d_prop(&h, table, "selected");
    assert_eq!(sel, Some(list([Value::I64(1)])), "the table's own selection, for `bind:selected`");

    #[cfg(target_os = "linux")]
    {
        use accesskit_atspi_common::State;
        let c = h.consumer();
        let at = c.atspi().unwrap();
        let rows: Vec<_> = c.by_author_id("parts").unwrap().children().skip(1).collect();
        assert!(at.node(rows[1].id()).state().contains(State::Selected), "the selected row on AT-SPI");
        assert!(!at.node(rows[0].id()).state().contains(State::Selected));
    }
}

fn d_prop(h: &Harness<CoreDocument>, n: NodeId, name: &str) -> Option<Value> {
    let d = &h.state().doc;
    let p = d.registry().get(TABLE).unwrap().spec.prop_named(name).unwrap();
    d.get_prop(n, p)
}

/// SV59: `<ListView label>` sends the row labels with `splice` and, when only a label changed,
/// with `setLabels`: the realised options are renamed. (`anchor` is the runtime's: it writes the row
/// outlet's `anchor`, which `list_rows_are_options_and_a_combobox_names_the_selected_one` covers.)
#[test]
fn set_labels_renames_the_realised_options() {
    const LIST: KindId = KindId::of("virtual-list");
    let mut h = harness();
    let w = h.window();
    let view = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let view = add(d, root, LIST, "width: 300px; height: 200px");
        prop(d, view, LIST, "row-height", Value::F64(20.0));
        prop(d, view, LIST, "uniform-rows", Value::Bool(true));
        call(d, view, LIST, "splice", &[Value::I64(0), Value::I64(0), list_of([1, 2]), list([Value::from("Top"), Value::from("Bottom")])]);
        for k in 0..2 {
            let row = add(d, view, BOX, "");
            d.set_prop(row, common::ANCHOR, Value::from(format!("layer:{k}"))).unwrap();
            call(d, view, LIST, "setRow", &[Value::I64(k), Value::I64(row.0 as i64)]);
        }
        view
    };
    settle(&mut h);
    let names = |h: &Harness<CoreDocument>| -> Vec<Option<String>> { (0..2).map(|k| h.consumer().by_author_id(&format!("layer:{k}")).unwrap().data().label().map(str::to_owned)).collect() };
    assert_eq!(names(&h), [Some("Top".into()), Some("Bottom".into())]);
    call(&mut h.state_mut().doc, view, LIST, "setLabels", &[list([Value::from("Top copper"), Value::from("Bottom")])]);
    settle(&mut h);
    assert_eq!(names(&h), [Some("Top copper".into()), Some("Bottom".into())]);
}

fn list_of<const N: usize>(ids: [i64; N]) -> Value {
    list(ids.into_iter().map(Value::I64))
}

/// SV60: a field's `aria-activedescendant` and `aria-controls` name anchors that appear only later
/// (a row realised after the field set them): the field goes out again once they resolve, and again
/// when its target leaves, without any change of its own.
#[test]
fn references_resolve_when_their_target_appears() {
    use skelter_base::accesskit::NodeId as AxId;
    let mut h = harness();
    let w = h.window();
    {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let field = add(d, root, KindId::of("text-edit"), "width: 300px; height: 24px");
        d.set_prop(field, common::ANCHOR, Value::from("query")).unwrap();
        d.set_prop(field, common::ARIA_LABEL, Value::from("Command")).unwrap();
        d.set_prop(field, common::ROLE, Value::from("combobox")).unwrap();
        d.set_prop(field, common::ARIA_CONTROLS, Value::from("list")).unwrap();
        d.set_prop(field, common::ARIA_ACTIVEDESCENDANT, Value::from("opt-3")).unwrap();
    }
    settle(&mut h);
    let refs = |h: &Harness<CoreDocument>| -> (Option<AxId>, Vec<AxId>) {
        let c = h.consumer();
        let f = c.by_author_id("query").unwrap();
        (f.data().active_descendant(), f.data().controls().to_vec())
    };
    assert_eq!(refs(&h), (None, vec![]), "nothing to name yet");
    // the list and its option appear later; the field is not touched
    let (list, opt) = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let list = add(d, root, BOX, "width: 300px; height: 100px");
        d.set_prop(list, common::ANCHOR, Value::from("list")).unwrap();
        let opt = add(d, list, BOX, "width: 300px; height: 20px");
        d.set_prop(opt, common::ANCHOR, Value::from("opt-3")).unwrap();
        (list, opt)
    };
    settle(&mut h);
    assert_eq!(refs(&h), (Some(AxId(opt.0)), vec![AxId(list.0)]), "both resolved");
    // the option goes: the field drops it
    h.state_mut().doc.remove(list, opt).unwrap();
    settle(&mut h);
    assert_eq!(refs(&h), (None, vec![AxId(list.0)]));
}

/// A `label` stretched by layout (here `width`) with a `click` listener takes a click beyond its
/// text, as a button does; one nobody listens to still lets it through to its parent.
#[test]
fn a_stretched_label_with_a_click_listener_takes_clicks_on_its_whole_box() {
    use skelter_core::{ListenFlags, common::CLICK};
    let mut h = harness();
    let w = h.window();
    let (parent, label) = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let parent = add(d, root, BOX, "width: 300px; height: 24px");
        let label = add(d, parent, KindId::of("label"), "width: 300px; height: 24px");
        prop(d, label, KindId::of("label"), "text", Value::from("Hi"));
        d.listen(parent, CLICK, ListenFlags::EMPTY);
        (parent, label)
    };
    settle(&mut h);
    h.state_mut().take_js_events();
    let click_target = |h: &mut Harness<CoreDocument>| {
        h.click_at(skelter_base::geom::Point::new(280.0, 12.0));
        h.run();
        h.state_mut().take_js_events().into_iter().find(|e| e.signal == CLICK).map(|e| e.target)
    };
    assert_eq!(click_target(&mut h), Some(parent), "unlistened: the click past the text is the parent's");
    h.state_mut().doc.listen(label, CLICK, ListenFlags::EMPTY);
    assert_eq!(click_target(&mut h), Some(label), "listened: the whole box is the label's");
}

/// FQ-67: a button (and a checkbox) disabled and then re-enabled through the common `disabled` prop:
/// the AccessKit node follows both ways. The widget's own mirror of `disabled` was synced from the
/// node's state before core updated it, so a re-enabled button stayed disabled.
#[test]
fn a_reenabled_widget_is_enabled_for_accesskit() {
    let mut h = harness();
    let w = h.window();
    let (button, check) = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let button = add(d, root, KindId::of("button"), "width: 80px; height: 24px");
        prop(d, button, KindId::of("button"), "text", Value::from("Place"));
        let check = add(d, root, KindId::of("checkbox"), "width: 80px; height: 24px");
        prop(d, check, KindId::of("checkbox"), "text", Value::from("Snap"));
        (button, check)
    };
    settle(&mut h);
    let disabled = |h: &Harness<CoreDocument>| (h.get_by_label("Place").is_disabled(), h.get_by_label("Snap").is_disabled());
    assert_eq!(disabled(&h), (false, false));
    for n in [button, check] {
        h.state_mut().doc.set_prop(n, common::DISABLED, Value::Bool(true)).unwrap();
    }
    settle(&mut h);
    assert_eq!(disabled(&h), (true, true));
    for n in [button, check] {
        h.state_mut().doc.set_prop(n, common::DISABLED, Value::Bool(false)).unwrap();
    }
    settle(&mut h);
    assert_eq!(disabled(&h), (false, false), "re-enabled");
}

/// A select's options carry the anchors of `option-anchors` (AccessKit author ids), so
/// svelters-automation's `anchor=` selector finds an option as cadatomic's tours address it
/// (`ecad.lib.part.new.footprint:LED_0805`), and AccessKit's `Click` on it picks it.
#[test]
fn select_options_resolve_by_anchor() {
    use skelter_test::SkelterScene;
    use svelters_automation::source::query;
    const SELECT: KindId = KindId::of("select");
    let mut h = harness();
    let w = h.window();
    let sel = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let sel = add(d, root, SELECT, "width: 200px; height: 24px");
        d.set_prop(sel, common::ARIA_LABEL, Value::from("Footprint")).unwrap();
        prop(d, sel, SELECT, "options", list([Value::from("LED_0603"), Value::from("LED_0805")]));
        prop(d, sel, SELECT, "option-anchors", list([Value::from("fp:LED_0603"), Value::from("fp:LED_0805")]));
        sel
    };
    settle(&mut h);
    let opt = {
        let scene = SkelterScene::new(h.a11y()).with_document(&h.state().doc);
        let found = query(&scene, "anchor=fp:LED_0805").unwrap();
        assert_eq!(found.len(), 1, "the option by its anchor");
        found[0]
    };
    h.accesskit_action(skelter_base::accesskit::ActionRequest { action: Action::Click, target_tree: opt.0, target_node: opt.1, data: None });
    h.run();
    assert_eq!(d_prop_of(&h, sel, SELECT, "selected"), Some(Value::I64(1)));
}

fn d_prop_of(h: &Harness<CoreDocument>, n: NodeId, kind: KindId, name: &str) -> Option<Value> {
    let d = &h.state().doc;
    let p = d.registry().get(kind).unwrap().spec.prop_named(name).unwrap();
    d.get_prop(n, p)
}
