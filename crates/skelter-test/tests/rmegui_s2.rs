//! rmegui lane S2 (`plans/cloud/rmegui/LANE-S2.md`): skelter bugs cadatomic's rmegui wave 1 found on the
//! native shell, on a real skelter-core document with skelter-widgets' kinds, read through AccessKit's
//! consumer (and AT-SPI's translation layer on Linux):
//! - FQ-66: a `table` exposes its header, realised rows and cells, with actions.

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
