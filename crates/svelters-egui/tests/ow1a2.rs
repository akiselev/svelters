//! OW1a2 (plans/milestones/OW1a2-basic-widgets-2.md): radio-button, frame, collapsing-header, icon
//! and image as own widgets, OW1a's parity props (WIDGET-PARITY §2.1–§2.3, §2.9, §2.10, §2.16),
//! Roving, Expandable, and the default flip for OW1a's kinds.

mod common;

use common::Rig;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::kinds::KindRegistry;
use svelters_egui::native::{self, Native};
use svelters_egui::provider::StateBits;
use svelters_egui::scene::W;
use svelters_egui::widgets::{DEFAULT_OWN, KindSet};

const NEW: &[&str] = &["radio-button", "frame", "collapsing-header", "icon", "image"];
const OW1A: &[&str] = &[
    "checkbox", "switch", "toggle-button", "label", "hyperlink", "separator", "progress-bar", "spinner", "radio-button", "frame", "collapsing-header", "icon", "image",
];

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn rig() -> Rig {
    let rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    rig.backend.set_own_widgets(KindSet::Default);
    rig
}

fn own(rig: &Rig, h: svl_handle) -> bool {
    rig.backend.with_scene(|s| matches!(s.n(h).w, W::Ext(_)))
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

fn events(rig: &Rig, log: &common::Log, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

#[test]
fn ow1a2_the_default_is_own_for_every_ow1a_kind_and_none_keeps_the_wrappers() {
    let reg = KindRegistry::global();
    for tag in OW1A {
        assert_eq!(DEFAULT_OWN.contains(tag), *tag != "label", "{tag} flipped (label waits for RT2's runs, OW1a3)");
        let k = reg.tag_index(tag).expect(tag);
        let r = reg.replacement(k).unwrap_or_else(|| panic!("an own widget replaces <{tag}>"));
        assert_eq!(r.entry.replaces, Some(*tag));
    }
    let mut r = rig();
    let (_w, col) = r.window();
    let hs: Vec<_> = NEW.iter().map(|t| r.leaf(col, t, &[("tooltip", s(t))])).collect();
    r.run();
    for (t, h) in NEW.iter().zip(&hs) {
        assert!(own(&r, *h), "<{t}> is own by default");
    }
    r.backend.set_own_widgets(KindSet::None);
    let h = r.leaf(col, "radio-button", &[("text", s("Wrapped"))]);
    r.run();
    assert!(!own(&r, h), "`none` keeps the egui wrapper until VIS1c");
}

#[test]
fn ow1a2_radio_roving_moves_focus_and_the_check_and_is_one_tab_stop() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let before = rig.leaf(col, "button", &[("text", s("Before"))]);
    let mk = |rig: &Rig, t: &str, v: &str| rig.leaf(col, "radio-button", &[("text", s(t)), ("value", s(v)), ("group", s("b"))]);
    let (a, b, c) = (mk(&rig, "A", "a"), mk(&rig, "B", "b"), mk(&rig, "C", "c"));
    for h in [a, b, c] {
        rig.listen(h, "changed");
        rig.observe(h, "group", true);
    }
    rig.run();
    rig.run();
    assert!(state(&rig, b).contains(StateBits::CHECKED));
    // Tab from the button lands on the checked radio button, the group's one stop
    rig.click(before);
    rig.harness.key_press(egui::Key::Tab);
    rig.run();
    rig.run();
    assert!(state(&rig, b).contains(StateBits::FOCUS_VISIBLE), "{:?}", state(&rig, b));
    rig.take();
    // Down: C is focused and checked, B and A follow the group
    rig.harness.key_press(egui::Key::ArrowDown);
    rig.run();
    rig.run();
    let log = rig.take();
    assert_eq!(rig.get(c, "checked"), Value::Bool(true));
    assert_eq!(rig.get(b, "checked"), Value::Bool(false));
    assert!(state(&rig, c).contains(StateBits::FOCUSED), "focus moved: {:?}", state(&rig, c));
    assert_eq!(events(&rig, &log, c, "changed"), vec![vec![Value::Bool(true), s("c")]]);
    assert_eq!(events(&rig, &log, b, "changed"), vec![vec![Value::Bool(false), s("c")]]);
    for h in [a, b, c] {
        assert_eq!(rig.get(h, "group"), s("c"));
    }
    // Down again wraps to A
    rig.harness.key_press(egui::Key::ArrowDown);
    rig.run();
    rig.run();
    assert_eq!(rig.get(a, "checked"), Value::Bool(true));
    assert_eq!(rig.get(c, "checked"), Value::Bool(false));
    assert_eq!(rig.harness.get_by_role_and_label(Role::RadioButton, "A").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn ow1a2_checkbox_indeterminate_and_tristate() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let cb = rig.leaf(col, "checkbox", &[("text", s("Mixed")), ("indeterminate", Value::Bool(true))]);
    rig.observe(cb, "indeterminate", true);
    rig.observe(cb, "checked", true);
    rig.run();
    rig.run();
    assert!(state(&rig, cb).contains(StateBits::INDETERMINATE));
    assert_eq!(rig.harness.get_by_role_and_label(Role::CheckBox, "Mixed").accesskit_node().toggled(), Some(Toggled::Mixed));
    // a click on an indeterminate box checks it (Qt, GTK), and clears `indeterminate` (announced)
    rig.click(cb);
    rig.run();
    assert_eq!((rig.get(cb, "checked"), rig.get(cb, "indeterminate")), (Value::Bool(true), Value::Bool(false)));
    let ind = rig.prop_id(cb, "indeterminate");
    assert!(rig.take().changed.contains(&(cb, ind)));
    // tristate: checked → indeterminate → unchecked
    rig.set(cb, "tristate", Value::Bool(true));
    rig.run();
    rig.click(cb);
    rig.run();
    assert_eq!((rig.get(cb, "checked"), rig.get(cb, "indeterminate")), (Value::Bool(false), Value::Bool(true)));
    rig.click(cb);
    rig.run();
    assert_eq!((rig.get(cb, "checked"), rig.get(cb, "indeterminate")), (Value::Bool(false), Value::Bool(false)));
}

#[test]
fn ow1a2_progress_value_range_orientation_and_busy() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let p = rig.leaf(col, "progress-bar", &[("value", Value::F64(30.0)), ("min", Value::F64(20.0)), ("max", Value::F64(40.0)), ("format", s("%v of %m (%p%)")), ("align-self", s("start"))]);
    rig.run();
    rig.run();
    let n = rig.harness.get_by_role(Role::ProgressIndicator);
    assert_eq!(n.accesskit_node().data().numeric_value(), Some(30.0));
    assert_eq!(n.accesskit_node().data().max_numeric_value(), Some(40.0));
    assert!(rig.harness.query_by_label("30 of 40 (50%)").is_some(), "format: %v %m %p");
    let h_size = rig.drawn(p).size();
    rig.set(p, "orientation", s("vertical"));
    rig.set(p, "format", s(""));
    rig.run();
    rig.run();
    let v = rig.drawn(p).size();
    assert!(v.y > v.x && h_size.x > h_size.y, "vertical swaps the axes: {h_size:?} {v:?}");
    // Qt's busy form: min == max
    rig.set(p, "max", Value::F64(20.0));
    rig.run();
    rig.run();
    assert!(state(&rig, p).contains(StateBits::BUSY) && state(&rig, p).contains(StateBits::INDETERMINATE));
}

#[test]
fn ow1a2_label_for_focuses_its_buddy_and_elide_keeps_the_full_name() {
    let mut rig = rig();
    rig.backend.set_own_widgets(KindSet::All);
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", s("Name")), ("for", s("name-field"))]);
    let f = rig.leaf(col, "text-edit", &[("anchor", s("name-field")), ("hint-text", s("Your name"))]);
    let long = "/home/user/projects/a/very/long/path/to/the/part.step";
    let e = rig.leaf(col, "label", &[("text", s(long)), ("elide", s("middle")), ("width", Value::F64(80.0))]);
    rig.run();
    rig.run();
    rig.click(l);
    rig.run();
    rig.run();
    // typing reaches the buddy: the click on the label focused it
    rig.harness.event(egui::Event::Text("Ada".into()));
    rig.run();
    assert_eq!(rig.get(f, "text"), s("Ada"), "a click on the label focuses its buddy");
    assert!(rig.drawn(e).width() <= 80.5);
    assert!(rig.harness.query_by_label(long).is_some(), "the elided label keeps its full accessible name");
    let _ = e;
}

#[test]
fn ow1a2_collapsing_header_expands_by_keyboard_and_mirrors_expanded() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let h = rig.leaf(col, "collapsing-header", &[("text", s("Section"))]);
    let body = rig.leaf(h, "label", &[("text", s("Body"))]);
    rig.listen(h, "changed");
    rig.run();
    rig.click(h);
    rig.run();
    rig.run();
    assert!(state(&rig, h).contains(StateBits::EXPANDED), "`:expanded`");
    assert!(rig.harness.query_by_label("Body").is_some());
    assert!(rig.drawn(body).top() >= rig.drawn(h).top() + 18.0);
    rig.take();
    // Left closes it (the focused header claims the arrows)
    rig.harness.key_press(egui::Key::ArrowLeft);
    rig.run();
    rig.run();
    assert_eq!(rig.get(h, "open"), Value::Bool(false));
    assert!(!state(&rig, h).contains(StateBits::EXPANDED));
    rig.harness.key_press(egui::Key::ArrowRight);
    rig.run();
    rig.run();
    assert_eq!(rig.get(h, "open"), Value::Bool(true));
    let log = rig.take();
    assert_eq!(events(&rig, &log, h, "changed"), vec![vec![Value::Bool(false)], vec![Value::Bool(true)]]);
}

#[test]
fn ow1a2_checkable_frame_disables_its_content_when_unchecked() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let f = rig.leaf(col, "frame", &[("label", s("Options")), ("checkable", Value::Bool(true))]);
    let b = rig.leaf(f, "button", &[("text", s("Inner"))]);
    rig.listen(b, "clicked");
    rig.listen(f, "changed");
    rig.run();
    rig.run();
    assert_eq!(rig.get(f, "checked"), Value::Bool(true));
    assert!(state(&rig, f).contains(StateBits::CHECKED));
    // a click on the caption row unchecks it
    let fr = rig.drawn(f);
    // on the caption row (a click on the content's empty space does not toggle)
    let pos = fr.min + egui::vec2(14.0, 14.0);
    rig.harness.hover_at(pos);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    rig.run();
    rig.run();
    assert_eq!(rig.get(f, "checked"), Value::Bool(false), "{fr:?}");
    rig.take();
    rig.click(b);
    rig.run();
    let log = rig.take();
    assert!(events(&rig, &log, b, "clicked").is_empty(), "the content is disabled");
}

#[test]
fn ow1a2_toggle_button_group_is_exclusive() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let mk = |rig: &Rig, t: &str| rig.leaf(col, "toggle-button", &[("text", s(t)), ("group", s("align"))]);
    let (l, c) = (mk(&rig, "Left"), mk(&rig, "Center"));
    rig.listen(l, "changed");
    rig.run();
    rig.click(l);
    rig.run();
    rig.click(c);
    rig.run();
    rig.run();
    assert_eq!((rig.get(l, "checked"), rig.get(c, "checked")), (Value::Bool(false), Value::Bool(true)));
    let log = rig.take();
    assert_eq!(events(&rig, &log, l, "changed"), vec![vec![Value::Bool(true)], vec![Value::Bool(false)]]);
    // a click on the checked one keeps it (an exclusive set has one on)
    rig.click(c);
    rig.run();
    assert_eq!(rig.get(c, "checked"), Value::Bool(true));
}

#[test]
fn ow1a2_icon_and_image_name_and_click() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let i = rig.leaf(col, "icon", &[("glyph", s("★")), ("tooltip", s("Favourite")), ("frame", Value::Bool(true))]);
    let img = rig.leaf(col, "image", &[("source", s("missing.png"))]);
    rig.listen(i, "clicked");
    rig.run();
    rig.run();
    assert!(rig.harness.query_by_role_and_label(Role::Button, "Favourite").is_some(), "a framed icon is a button named by its tooltip");
    rig.click(i);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, i, "clicked").len(), 1);
    assert!(rig.drawn(img).width() >= 16.0, "a source that does not load keeps the placeholder's size");
}

/// OW-RUST's findings: the indicator kinds paint no host box (the base sheet's `checkbox` rule
/// styles the indicator, never a full-row box), a pointer press focuses without a ring (R8), and
/// an unnamed separator passes kittest's accessibility check (it is no input).
#[test]
fn ow1a2_indicator_hosts_paint_no_box_and_a_click_shows_no_ring() {
    use svelters_egui::paint::{self, TraceKind};
    let mut rig = rig();
    paint::trace::enable(&rig.harness.ctx, true);
    let (_w, col) = rig.window();
    let cb = rig.leaf(col, "checkbox", &[("text", s("Snap"))]);
    let rb = rig.leaf(col, "radio-button", &[("text", s("One"))]);
    let sw = rig.leaf(col, "switch", &[("text", s("On"))]);
    let _sep = rig.leaf(col, "separator", &[]);
    rig.run();
    rig.run();
    let t = paint::trace::last(&rig.harness.ctx);
    for h in [cb, rb, sw] {
        let n = rig.backend.with_scene(|s| s.n(h).handle.to_raw());
        assert!(t.find(n, None, TraceKind::Box).is_none(), "no host box behind the indicator kind");
    }
    rig.click(cb);
    rig.run();
    rig.run();
    let st = state(&rig, cb);
    assert!(st.contains(StateBits::FOCUSED) && !st.contains(StateBits::FOCUS_VISIBLE), "{st:?}");
}
