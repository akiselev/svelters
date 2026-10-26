//! Table behaviour over skelter events: virtual rows, column resize, header sorting, keyboard
//! selection, recording.

use super::shim::testing::FixedText;
use super::*;
use skelter_base::event::{KeyLocation, PointerType};
use skelter_base::smallvec::SmallVec;

const MS: Span = Span::from_millis(1);

fn table(rows: i64) -> Table {
    let mut name = ColumnState::new("Name", 100.0);
    name.name = "name".into();
    let mut t = Table::new(vec![name, ColumnState::new("Value", 80.0), ColumnState::new("Note", 120.0)]);
    t.state.row_count = rows;
    t.state.overscan = 5;
    t.state.stretch_last = false;
    // header 24, rows 20: a 200-point body shows 10 rows
    t.set_viewport(Size::new(300.0, 224.0), Vec2::ZERO);
    t
}

fn ptr(phase: PointerPhase, x: f32, y: f32, button: Option<MouseButton>, modifiers: Modifiers) -> PointerEvent {
    PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button,
        buttons: 0,
        pos: Point::new(x, y),
        modifiers,
        pressure: 0.0,
        tilt: Vec2::ZERO,
        history: SmallVec::new(),
    }
}

fn click(t: &mut Table, x: f32, y: f32, m: Modifiers, now: Time, out: &mut Vec<Emit>) {
    t.pointer(&ptr(PointerPhase::Down, x, y, Some(MouseButton::Primary), m), now, false, out);
    t.pointer(&ptr(PointerPhase::Up, x, y, Some(MouseButton::Primary), m), now, false, out);
}

fn key(t: &mut Table, k: Key, m: Modifiers, out: &mut Vec<Emit>) -> bool {
    let ev = KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m };
    t.key(&ev, Time::ZERO, false, out)
}

fn signals(out: &[Emit]) -> Vec<&'static str> {
    out.iter().map(|e| e.signal).collect()
}

fn ids(v: &[u64]) -> Value {
    item_keys::ids_value(v.iter().copied())
}

#[test]
fn virtual_rows_ask_for_the_shown_range_with_overscan() {
    let mut t = table(1000);
    let mut out = Vec::new();
    let mut text = FixedText::default();
    let look = TableLook::default();
    t.prepare(&look, &mut text, &mut out);
    assert_eq!(out, vec![Emit::new("range", vec![Value::I64(0), Value::I64(14)])], "rows 0..10 and 5 more");
    out.clear();
    t.prepare(&look, &mut text, &mut out);
    assert!(out.is_empty(), "the same range is asked once");
    t.set_viewport(Size::new(300.0, 224.0), Vec2::new(0.0, 400.0));
    t.prepare(&look, &mut text, &mut out);
    assert_eq!(out, vec![Emit::new("range", vec![Value::I64(15), Value::I64(34)])], "scrolled to row 20");
    // only the window's rows draw text; the others draw empty
    let row = |a: &str| Value::List(vec![Value::Str(a.into()), Value::Str("v".into())]);
    t.state.set_rows(&[Value::I64(20), Value::List(vec![row("twenty"), row("twenty-one")])]).unwrap();
    let mut list = DisplayList::new();
    t.record(&look, &mut text, &mut list, Time::ZERO);
    let drawn = text.texts();
    assert!(drawn.contains(&"twenty") && drawn.contains(&"twenty-one"));
    assert_eq!(drawn.iter().filter(|s| **s == "v").count(), 2, "two rows of data in view");
    assert_eq!(drawn.iter().filter(|s| ["Name", "Value", "Note"].contains(s)).count(), 3, "the header");
}

#[test]
fn rows_outside_the_body_are_not_recorded() {
    let mut t = table(1000);
    let rows: Vec<Value> = (0..1000).map(|k| Value::List(vec![Value::Str(format!("r{k}"))])).collect();
    t.state.set_rows(&[Value::I64(0), Value::List(rows)]).unwrap();
    let mut text = FixedText::default();
    let mut list = DisplayList::new();
    t.record(&TableLook::default(), &mut text, &mut list, Time::ZERO);
    let cells: Vec<&str> = text.texts().into_iter().filter(|s| s.starts_with('r')).collect();
    assert_eq!(cells, (0..10).map(|k| format!("r{k}")).collect::<Vec<_>>(), "rows 0..10 only");
}

#[test]
fn column_resize_drags_clamps_and_commits() {
    let mut t = table(10);
    t.columns[0].min_width = Some(40.0);
    let mut out = Vec::new();
    let t0 = Time(1_000_000_000);
    // the first column's right edge is at x = 100 in the header
    t.pointer(&ptr(PointerPhase::Move, 100.0, 10.0, None, Modifiers::default()), t0, false, &mut out);
    assert_eq!(t.cursor_icon(), CursorIcon::ColResize);
    assert!(t.pointer(&ptr(PointerPhase::Down, 100.0, 10.0, Some(MouseButton::Primary), Modifiers::default()), t0, false, &mut out));
    t.pointer(&ptr(PointerPhase::Move, 130.0, 10.0, None, Modifiers::default()), t0 + MS, false, &mut out);
    assert_eq!(t.geom().ws[0], 130.0, "the live width draws before the commit");
    assert_eq!(t.columns[0].width, Some(100.0), "the prop is written at the commit");
    t.pointer(&ptr(PointerPhase::Move, 0.0, 10.0, None, Modifiers::default()), t0 + Span::from_millis(2), false, &mut out);
    assert_eq!(t.geom().ws[0], 40.0, "clamped to min-width");
    t.pointer(&ptr(PointerPhase::Up, 0.0, 10.0, Some(MouseButton::Primary), Modifiers::default()), t0 + Span::from_millis(3), false, &mut out);
    assert_eq!(out[0], Emit::new("column-resized", vec![Value::I64(0), Value::F64(40.0)]));
    assert_eq!(out[1].signal, "header-state-changed");
    assert_eq!(t.columns[0].width, Some(40.0));
    assert_eq!(t.take_changed_columns(), vec![0]);
    assert!(t.take_changed().contains(&"header-state"));
    // Escape cancels a drag back to the width before
    out.clear();
    t.pointer(&ptr(PointerPhase::Down, 40.0, 10.0, Some(MouseButton::Primary), Modifiers::default()), t0 + Span::from_millis(4), false, &mut out);
    t.pointer(&ptr(PointerPhase::Move, 90.0, 10.0, None, Modifiers::default()), t0 + Span::from_millis(5), false, &mut out);
    assert!(key(&mut t, Key::Escape, Modifiers::default(), &mut out));
    assert_eq!(t.geom().ws[0], 40.0);
    assert!(out.is_empty());
}

#[test]
fn double_click_on_an_edge_fits_the_column() {
    let mut t = table(10);
    let row = |a: &str| Value::List(vec![Value::Str(a.into())]);
    t.state.set_rows(&[Value::I64(0), Value::List(vec![row("a much longer cell text"), row("x")])]).unwrap();
    let mut out = Vec::new();
    let t0 = Time(1_000_000_000);
    click(&mut t, 100.0, 10.0, Modifiers::default(), t0, &mut out);
    click(&mut t, 100.0, 10.0, Modifiers::default(), t0 + Span::from_millis(100), &mut out);
    assert!(out.is_empty(), "the fit is applied at prepare");
    let mut text = FixedText::default();
    t.prepare(&TableLook::default(), &mut text, &mut out);
    // 23 chars at 6.5 points + 2 × 4 padding = 157.5, rounded up
    let fitted = Emit::new("column-resized", vec![Value::I64(0), Value::F64(158.0)]);
    assert!(out.contains(&fitted), "{out:?}");
}

#[test]
fn header_clicks_sort_and_flip_on_a_sortable_table() {
    let mut t = table(10);
    let mut out = Vec::new();
    let t0 = Time(1_000_000_000);
    click(&mut t, 50.0, 10.0, Modifiers::default(), t0, &mut out);
    assert_eq!(signals(&out), vec!["header-clicked"], "not sortable: no sort");
    t.state.sortable = true;
    out.clear();
    click(&mut t, 140.0, 10.0, Modifiers::default(), t0 + Span::from_millis(1000), &mut out);
    assert_eq!(signals(&out), vec!["header-clicked", "sort-changed", "header-state-changed"]);
    assert_eq!(out[1].args, vec![Value::I64(1), Value::Str("ascending".into())]);
    out.clear();
    click(&mut t, 140.0, 10.0, Modifiers::default(), t0 + Span::from_millis(2000), &mut out);
    assert_eq!(out[1].args, vec![Value::I64(1), Value::Str("descending".into())]);
    // a quick second click is a double click: no second sort
    out.clear();
    click(&mut t, 140.0, 10.0, Modifiers::default(), t0 + Span::from_millis(2100), &mut out);
    assert_eq!(signals(&out), vec!["header-double-clicked"]);
    out.clear();
    t.pointer(&ptr(PointerPhase::Down, 140.0, 10.0, Some(MouseButton::Secondary), Modifiers::default()), t0, false, &mut out);
    assert_eq!(out, vec![Emit::new("header-context", vec![Value::I64(1)])]);
}

#[test]
fn row_clicks_select_by_id_and_double_clicks_activate() {
    let mut t = table(5);
    t.state.set_ids(&[ids(&[50, 40, 30, 20, 10])]).unwrap();
    t.state.set_prop("selection", &Value::Str("multi".into())).unwrap();
    let mut out = Vec::new();
    let t0 = Time(1_000_000_000);
    // row 1 (id 40), column 1
    click(&mut t, 150.0, 24.0 + 25.0, Modifiers::default(), t0, &mut out);
    assert_eq!(signals(&out), vec!["changed", "selectionchange"]);
    assert_eq!(out[1].args, vec![ids(&[40])]);
    assert_eq!((t.state.sel.cursor, t.state.cur_col), (Some(40), 1));
    out.clear();
    click(&mut t, 150.0, 24.0 + 65.0, Modifiers::SHIFT, t0 + Span::from_millis(1000), &mut out);
    assert_eq!(out[1].args, vec![ids(&[20, 30, 40])], "shift ranges from the anchor");
    out.clear();
    click(&mut t, 150.0, 24.0 + 65.0, Modifiers::default(), t0 + Span::from_millis(1100), &mut out);
    assert_eq!(out, vec![Emit::new("activated", vec![Value::I64(3)])], "a double click activates the row (position)");
    out.clear();
    click(&mut t, 150.0, 24.0 + 300.0, Modifiers::default(), t0 + Span::from_millis(3000), &mut out);
    assert!(out.is_empty(), "below the last row");
}

#[test]
fn keyboard_moves_selects_copies_and_activates() {
    let mut t = table(100);
    t.state.set_prop("selection", &Value::Str("multi".into())).unwrap();
    t.set_focus(true, true);
    let mut out = Vec::new();
    let mut text = FixedText::default();
    let look = TableLook::default();
    t.prepare(&look, &mut text, &mut out);
    out.clear();
    assert!(key(&mut t, Key::ArrowDown, Modifiers::default(), &mut out));
    assert_eq!((t.state.sel.cursor, signals(&out)), (Some(0), vec!["changed", "selectionchange"]));
    key(&mut t, Key::ArrowDown, Modifiers::SHIFT, &mut out);
    key(&mut t, Key::ArrowDown, Modifiers::SHIFT, &mut out);
    assert_eq!(t.state.sel.selected.iter().copied().collect::<Vec<_>>(), vec![0, 1, 2]);
    out.clear();
    key(&mut t, Key::ArrowDown, Modifiers::CTRL, &mut out);
    assert!(out.is_empty(), "ctrl moves the cursor alone");
    assert_eq!(t.state.sel.cursor, Some(3));
    key(&mut t, Key::C, Modifiers::CTRL, &mut out);
    assert_eq!(out, vec![Emit::new("copy", vec![ids(&[0, 1, 2])])]);
    out.clear();
    key(&mut t, Key::Enter, Modifiers::default(), &mut out);
    assert_eq!(out, vec![Emit::new("activated", vec![Value::I64(3)])]);
    // a page is the rows in view (10 shown: 9 rows); the cursor scrolls into view
    key(&mut t, Key::PageDown, Modifiers::default(), &mut out);
    assert_eq!(t.state.sel.cursor, Some(12));
    t.prepare(&look, &mut text, &mut out);
    assert_eq!(t.take_scroll_request(), Some(Vec2::new(0.0, 13.0 * 20.0 - 200.0)));
    key(&mut t, Key::End, Modifiers::default(), &mut out);
    assert_eq!(t.state.sel.cursor, Some(99));
    key(&mut t, Key::A, Modifiers::CTRL, &mut out);
    assert_eq!(t.state.sel.selected.len(), 100);
    // Left/Right move the cursor column among the shown columns, without wrapping
    assert!(key(&mut t, Key::ArrowRight, Modifiers::default(), &mut out));
    key(&mut t, Key::ArrowRight, Modifiers::default(), &mut out);
    key(&mut t, Key::ArrowRight, Modifiers::default(), &mut out);
    assert_eq!(t.state.cur_col, 2);
    t.state.hidden = vec!["name".into()];
    t.prepare(&look, &mut text, &mut out);
    assert_eq!(t.state.cur_col, 1, "kept among the shown columns");
    assert!(t.take_changed().contains(&"current-column"));
}

#[test]
fn header_state_restores_at_prepare() {
    let mut t = table(10);
    t.state.set_prop("header-state", &Value::Str(r#"{"widths":{"name":150,"1":60},"hidden":["2"],"sort":{"column":"name","order":"descending"}}"#.into())).unwrap();
    let mut out = Vec::new();
    t.prepare(&TableLook::default(), &mut FixedText::default(), &mut out);
    assert_eq!((t.columns[0].width, t.columns[1].width), (Some(150.0), Some(60.0)));
    assert_eq!(t.shown(), vec![0, 1]);
    assert_eq!((t.state.sort_column, t.state.sort_order), (0, "descending"));
    assert!(signals(&out).contains(&"sort-changed") && signals(&out).contains(&"columns-changed"));
    assert!(!signals(&out).contains(&"header-state-changed"), "a restored state is not echoed");
}

#[test]
fn record_draws_selection_ring_and_sort_arrow() {
    let mut t = table(3);
    t.state.sort_column = 0;
    t.state.sort_order = "ascending";
    t.state.set_prop("selected", &ids(&[1])).unwrap();
    t.state.set_prop("current", &Value::I64(1)).unwrap();
    t.set_focus(true, true);
    let look = TableLook::default();
    let mut list = DisplayList::new();
    t.record(&look, &mut FixedText::default(), &mut list, Time::ZERO);
    let fills: Vec<&skelter_base::display::Quad> = list.items.iter().filter_map(|p| if let Prim::Quad(q) = p { Some(q) } else { None }).collect();
    let row1 = Rect::new(0.0, 44.0, 300.0, 64.0);
    assert!(fills.iter().any(|q| q.rect == row1 && q.fill == skelter_base::color::Fill::Solid(look.selected)), "row 1 selected (focused)");
    assert!(fills.iter().any(|q| q.border_color == look.focus_ring && q.rect == Rect::new(0.0, 44.0, 100.0, 64.0)), "the cursor cell's ring");
    let lines = list.items.iter().filter(|p| matches!(p, Prim::Line(l) if l.color == look.sort_arrow)).count();
    assert_eq!(lines, 2, "one chevron on the sorted column");
    let clips = list.items.iter().filter(|p| matches!(p, Prim::PushClip(_))).count();
    let pops = list.items.iter().filter(|p| matches!(p, Prim::PopClip)).count();
    assert_eq!(clips, pops, "clips are balanced");
}
