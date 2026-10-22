//! WG1 (plans/milestones/WG1-sc1-widgets.md): the SC1 widget kinds, each rendered, driven by the
//! interaction that changes its bound value, and found by role and label. Headless (kittest, no
//! JS: the backend behind its real vtable with a recording host, `common::Rig`).

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::kinds::KINDS;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// Grid: children fill the cells row by row; `auto` fits the labels, `1fr` takes the rest.
#[test]
fn grid_places_children_in_columns() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let g = rig.leaf(col, "grid", &[("column-gap", Value::F64(10.0)), ("row-gap", Value::F64(4.0))]);
    let la = rig.leaf(g, "label", &[("text", s("Width"))]);
    let ta = rig.leaf(g, "text-edit", &[("hint-text", s("Width value"))]);
    let lb = rig.leaf(g, "label", &[("text", s("Height of box"))]);
    let tb = rig.leaf(g, "text-edit", &[("hint-text", s("Height value"))]);
    rig.run();
    let (ra, rta, rb, rtb) = (rig.drawn(la), rig.drawn(ta), rig.drawn(lb), rig.drawn(tb));
    // one column for labels (as wide as the widest), one for the fields, which fill the rest
    assert!((rta.left() - rtb.left()).abs() < 0.5, "{rta:?} {rtb:?}");
    assert!(rta.left() >= rb.right() + 9.0, "the field column starts after the widest label: {rb:?} {rta:?}");
    // row 2 below row 1
    assert!(rb.top() > ra.top() && rtb.top() >= rta.bottom() + 3.0, "{rta:?} {rtb:?}");
    // found by role and label; typing into a cell works
    rig.harness.get_by_role_and_label(Role::TextInput, "Height value");
    rig.type_into(tb, "20 mm");
    assert_eq!(rig.get(tb, "text"), s("20 mm"));
    // a new template relayouts; a bad one is refused and keeps the old one
    rig.set(g, "grid-template-columns", s("1fr 1fr"));
    rig.run();
    let half = rig.drawn(ta).left();
    assert!(half > rta.left() + 20.0, "1fr 1fr moves the field column right: {half} vs {}", rta.left());
    let id = rig.prop_id(g, "grid-template-columns");
    assert!(rig.br().set_prop(g, id, &s("1fr bogus")).is_err());
    assert_eq!(rig.get(g, "grid-template-columns"), s("1fr 1fr"));
    for ok in ["", "auto", "repeat(3, 1fr)", "minmax(40px, 1fr) 100", "fit-content(50%) max-content min-content 25%"] {
        assert!(svelters_egui::layout::parse_grid_template(ok).is_ok(), "{ok}");
    }
    for bad in ["1fx", "repeat(0, 1fr)", "minmax(1fr, 2fr)", "(", "fit-content(1fr)"] {
        assert!(svelters_egui::layout::parse_grid_template(bad).is_err(), "{bad}");
    }
}

/// Collapsing header: closed hides its children; a click opens it (announced, `changed`), the
/// children are laid out below the header and indented; a host write opens it without an echo.
#[test]
fn collapsing_header_toggles_open_and_lays_out_its_body() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let h = rig.leaf(col, "collapsing-header", &[("text", s("Placement"))]);
    let x = rig.leaf(h, "text-edit", &[("hint-text", s("X offset"))]);
    let after = rig.leaf(col, "label", &[("text", s("After"))]);
    rig.observe(h, "open", true);
    rig.listen(h, "changed");
    rig.run();
    let header = rig.harness.get_by_role_and_label(Role::DisclosureTriangle, "Placement");
    assert_eq!(header.accesskit_node().data().is_expanded(), Some(false));
    assert!(rig.harness.query_by_label("X offset").is_none(), "closed: the body is not drawn");
    let closed_after = rig.drawn(after).top();

    rig.click(h);
    rig.run();
    let l = rig.take();
    let open = rig.prop_id(h, "open");
    assert_eq!(l.changed, vec![(h, open)]);
    assert_eq!(l.events.iter().map(|e| e.2.clone()).collect::<Vec<_>>(), vec![vec![Value::Bool(true)]]);
    assert_eq!(rig.get(h, "open"), Value::Bool(true));
    let field = rig.harness.get_by_role_and_label(Role::TextInput, "X offset");
    let _ = field;
    assert_eq!(rig.harness.get_by_role_and_label(Role::DisclosureTriangle, "Placement").accesskit_node().data().is_expanded(), Some(true));
    let (hd, xr) = (rig.drawn(h), rig.drawn(x));
    assert!(xr.top() >= hd.bottom() - 0.5, "the body is below the header: {hd:?} {xr:?}");
    assert!(xr.left() >= hd.left() + 10.0, "and indented: {hd:?} {xr:?}");
    assert!(rig.drawn(after).top() > closed_after + 10.0, "the next sibling moved down");

    // closing again
    rig.click(h);
    rig.run();
    assert_eq!(rig.get(h, "open"), Value::Bool(false));
    assert!(rig.harness.query_by_label("X offset").is_none());
    assert!((rig.drawn(after).top() - closed_after).abs() < 0.5);
    rig.take();

    // a host write: shown, never announced
    rig.set(h, "open", Value::Bool(true));
    rig.run();
    assert!(rig.harness.query_by_label("X offset").is_some());
    assert!(rig.take().changed.is_empty());
}

/// Toggle buttons: plain ones toggle; `radio` ones form a segmented group (a click checks, never
/// unchecks), with role `RadioButton`.
#[test]
fn toggle_buttons_toggle_and_radio_ones_stay_checked() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let row = rig.leaf(col, "box", &[("flex-direction", s("row"))]);
    let opts: Vec<_> = ["+X", "+Y", "+Z"].iter().map(|t| rig.leaf(row, "toggle-button", &[("text", s(t)), ("radio", Value::Bool(true))])).collect();
    rig.set(opts[0], "checked", Value::Bool(true));
    let plain = rig.leaf(col, "toggle-button", &[("text", s("Snap"))]);
    for &b in opts.iter().chain([&plain]) {
        rig.observe(b, "checked", true);
        rig.listen(b, "changed");
    }
    rig.run();
    let x = rig.harness.get_by_role_and_label(Role::RadioButton, "+X");
    assert_eq!(x.accesskit_node().data().toggled(), Some(egui::accesskit::Toggled::True));
    // segments sit side by side
    assert!(rig.drawn(opts[1]).left() >= rig.drawn(opts[0]).right() - 0.5);

    rig.harness.get_by_role_and_label(Role::RadioButton, "+Z").click();
    rig.run();
    let l = rig.take();
    let checked = rig.prop_id(opts[2], "checked");
    assert_eq!(l.changed, vec![(opts[2], checked)]);
    assert_eq!(l.events[0].2, vec![Value::Bool(true)]);
    assert_eq!(rig.get(opts[2], "checked"), Value::Bool(true));
    // a radio click on a checked one keeps it checked: no change
    rig.click(opts[2]);
    assert!(rig.take().changed.is_empty());
    assert_eq!(rig.get(opts[2], "checked"), Value::Bool(true));

    // a plain toggle flips both ways
    let snap = rig.harness.get_by_role_and_label(Role::Button, "Snap");
    assert_eq!(snap.accesskit_node().data().toggled(), Some(egui::accesskit::Toggled::False));
    rig.click(plain);
    rig.click(plain);
    let l = rig.take();
    assert_eq!(l.changed.len(), 2);
    assert_eq!(l.events.iter().map(|e| e.2.clone()).collect::<Vec<_>>(), vec![vec![Value::Bool(true)], vec![Value::Bool(false)]]);
}

/// Combo box: shows the selected option; picking a row in the popup changes `selected`.
#[test]
fn combo_box_picks_an_option() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let opts = Value::List(vec![s("Alpha"), s("Beta"), s("Gamma")]);
    let cb = rig.leaf(col, "combo-box", &[("options", opts.clone()), ("accessible-name", s("Plane"))]);
    rig.observe(cb, "selected", true);
    rig.listen(cb, "changed");
    rig.run();
    assert_eq!(rig.get(cb, "options"), opts);
    let combo = rig.harness.get_by_role_and_label(Role::ComboBox, "Plane");
    combo.click();
    rig.run();
    rig.harness.get_by_label("Gamma").click();
    rig.run();
    let l = rig.take();
    let sel = rig.prop_id(cb, "selected");
    assert_eq!(l.changed, vec![(cb, sel)]);
    assert_eq!(l.events[0].2, vec![Value::I64(2)]);
    assert_eq!(rig.get(cb, "selected"), Value::I64(2));
    // a static attribute's form: whitespace-separated
    rig.set(cb, "options", s("one two"));
    assert_eq!(rig.get(cb, "options"), Value::List(vec![s("one"), s("two")]));
}

/// Separators fill their rect along their orientation.
#[test]
fn separators_fill_along_their_orientation() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "label", &[("text", s("Above"))]);
    let hs = rig.leaf(col, "separator", &[]);
    let row = rig.leaf(col, "box", &[("flex-direction", s("row")), ("height", Value::F64(40.0))]);
    rig.leaf(row, "label", &[("text", s("Left"))]);
    let vs = rig.leaf(row, "separator", &[("orientation", s("vertical"))]);
    rig.run();
    let (rh, rv) = (rig.drawn(hs), rig.drawn(vs));
    let width = rig.backend.with_scene(|sc| sc.n(col).rect.width());
    assert!(rh.width() >= width - 1.0 && rh.height() <= 7.0, "{rh:?} in {width}");
    assert!(rh.top() >= rig.drawn(a).bottom() - 0.5);
    assert!(rv.height() >= 39.0 && rv.width() <= 7.0, "{rv:?}");
}

/// Scroll area: content taller than the area scrolls; scrolling writes `scroll-y` (announced); a
/// host write scrolls without an echo, and one past the end is clamped and announced.
#[test]
fn scroll_area_scrolls_and_binds_its_offset() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let sa = rig.leaf(col, "scroll-area", &[("height", Value::F64(100.0)), ("gap", Value::F64(2.0))]);
    let rows: Vec<_> = (0..30).map(|i| rig.leaf(sa, "label", &[("text", s(&format!("Row {i}")))])).collect();
    rig.observe(sa, "scroll-y", true);
    rig.listen(sa, "changed");
    rig.run();
    let area = rig.drawn(sa);
    assert!((area.height() - 100.0).abs() < 0.5, "{area:?}");
    // laid out at full size (overflowing), drawn only where visible
    assert!(rig.backend.with_scene(|sc| sc.n(rows[29]).rect.bottom()) > 300.0);
    assert!(rig.harness.query_by_label("Row 0").is_some());
    let top3 = rig.drawn(rows[3]).top();

    rig.harness.get_by_label("Row 1").scroll_down();
    rig.run();
    let l = rig.take();
    let y = rig.prop_id(sa, "scroll-y");
    assert!(l.changed.contains(&(sa, y)), "{:?}", l.changed);
    let Value::F64(off) = rig.get(sa, "scroll-y") else { panic!() };
    assert!(off > 10.0, "{off}");
    // (a row scrolled out is not drawn, so its `drawn` is stale: look at one still visible)
    let r = (off / 19.0).ceil() as usize + 1;
    assert!(rig.drawn(rows[r]).top() < rig.backend.with_scene(|sc| sc.n(rows[r]).rect.top()) + area.top() - rig.backend.with_scene(|sc| sc.n(sa).rect.top()) - 10.0, "row {r} moved up");

    rig.set(sa, "scroll-y", Value::F64(40.0));
    rig.run();
    assert!(rig.take().changed.is_empty(), "a host write within range is not echoed");
    assert!((rig.drawn(rows[3]).top() - (top3 - 40.0)).abs() < 1.0, "{:?} vs {top3}", rig.drawn(rows[3]));

    rig.set(sa, "scroll-y", Value::F64(1.0e6));
    rig.run();
    let l = rig.take();
    assert_eq!(l.changed, vec![(sa, y)], "clamped: announced");
    let Value::F64(max) = rig.get(sa, "scroll-y") else { panic!() };
    assert!(max < 1.0e5 && max > 100.0, "{max}");
}

/// Icon: a glyph from the family's name table; with `frame` an icon button named by its tooltip;
/// `font-weight` picks the registered family.
#[test]
fn icons_resolve_names_and_icon_buttons_click() {
    svelters_egui::fonts::register_icons("icons", [("gear", "*"), ("plus", "+")]);
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let plain = rig.leaf(col, "icon", &[("name", s("plus"))]);
    let btn = rig.leaf(col, "icon", &[("name", s("gear")), ("frame", Value::Bool(true)), ("tooltip", s("Settings"))]);
    rig.listen(btn, "clicked");
    rig.run();
    // a frameless icon is an image named by its glyph name
    rig.harness.get_by_role_and_label(Role::Image, "plus");
    let b = rig.harness.get_by_role_and_label(Role::Button, "Settings");
    b.click();
    rig.run();
    let l = rig.take();
    assert_eq!(l.events.len(), 1);
    assert_eq!(l.events[0].0, btn);
    assert!(rig.drawn(plain).width() > 0.0);
}

/// `font-weight` maps to the family registered for the nearest weight, only when the `Context`
/// has it; a weight without one keeps the regular face.
#[test]
fn font_weight_maps_to_registered_families() {
    use egui::{FontDefinitions, FontFamily};
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let regular = rig.leaf(col, "label", &[("text", s("Wide text here"))]);
    let bold = rig.leaf(col, "label", &[("text", s("Wide text here")), ("font-weight", Value::I64(700))]);
    rig.run();
    assert_eq!(rig.drawn(regular).width(), rig.drawn(bold).width(), "no family registered: the regular face");

    // a "bold" family drawn with egui's monospace font, registered for 600 of Proportional
    let mut defs = FontDefinitions::default();
    let mono = defs.families[&FontFamily::Monospace].clone();
    defs.families.insert(FontFamily::Name("bold".into()), mono);
    rig.harness.ctx.set_fonts(defs);
    svelters_egui::fonts::register_weight(FontFamily::Proportional, 600, FontFamily::Name("bold".into()));
    rig.run();
    // remeasure: a weight write invalidates the label
    rig.set(bold, "font-weight", Value::I64(650));
    rig.run();
    assert_ne!(rig.drawn(regular).width(), rig.drawn(bold).width(), "650 → the 600 family (monospace)");
    let id = rig.prop_id(bold, "font-weight");
    assert!(rig.br().set_prop(bold, id, &Value::I64(5000)).is_err());
}

/// `invalid` on any kind: AccessKit `invalid`, and the app's flag only (the widget never sets it).
#[test]
fn invalid_reaches_accesskit() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let te = rig.leaf(col, "text-edit", &[("hint-text", s("Length")), ("invalid", Value::Bool(true))]);
    let ok = rig.leaf(col, "text-edit", &[("hint-text", s("Width"))]);
    rig.run();
    let n = rig.harness.get_by_role_and_label(Role::TextInput, "Length");
    assert_eq!(n.accesskit_node().data().invalid(), Some(egui::accesskit::Invalid::True));
    assert_eq!(rig.harness.get_by_label("Width").accesskit_node().data().invalid(), None);
    rig.set(te, "invalid", Value::Bool(false));
    rig.run();
    assert_eq!(rig.harness.get_by_label("Length").accesskit_node().data().invalid(), None);
    assert_eq!(rig.get(ok, "invalid"), Value::Bool(false));
}

/// Every in-window kind shows its `tooltip` on hover (containers too), and every input is named
/// without `accessible-name` when it has a caption, hint or tooltip (kittest fails unnamed inputs).
#[test]
fn every_kind_has_a_tooltip_and_inputs_are_named() {
    for k in KINDS.iter() {
        let (Some(tag), false) = (k.tag, k.toplevel) else { continue };
        let mut rig = Rig::new();
        rig.harness.ctx.all_styles_mut(|st| st.interaction.tooltip_delay = 0.0);
        let (_w, col) = rig.window();
        // a leaf whose drawn rect gives the root's screen origin (containers draw nothing)
        let probe = rig.leaf(col, "label", &[("text", s("probe"))]);
        let tip = format!("tip of {tag}");
        let h = rig.leaf(col, tag, &[("tooltip", s(&tip)), ("width", Value::F64(120.0)), ("height", Value::F64(30.0))]);
        if k.text_content.is_some() {
            rig.set(h, "text", s(tag));
        }
        rig.run();
        let origin = rig.drawn(probe).min - rig.backend.with_scene(|sc| sc.n(probe).rect.min);
        // a leaf's response rect (a label is as wide as its text), else the node's laid-out rect
        let pos = if rig.drawn(h).is_positive() { rig.drawn(h).center() } else { rig.backend.with_scene(|sc| sc.n(h).rect.translate(origin).center()) };
        rig.harness.hover_at(pos);
        rig.run();
        rig.harness.hover_at(pos);
        rig.run();
        assert!(rig.harness.query_by_label(&tip).is_some() || rig.harness.query_by_label_contains(&tip).is_some(), "<{tag}>: no tooltip shown");
    }
}
