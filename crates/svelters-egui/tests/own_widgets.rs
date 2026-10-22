//! OW0 (ARCH-WIDGETS §6.2, plans/milestones/OW0-widget-foundation.md): svelters' own widgets
//! behind the per-kind switch. Every behaviour test runs on both implementations (the egui
//! wrapper and the own widget) until the kind's default flips; the registry's routing of a
//! replaced kind's row is checked once.

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::kinds::KindRegistry;
use svelters_egui::native::{self, Native};
use svelters_egui::provider::StateBits;
use svelters_egui::scene::W;
use svelters_egui::widgets::KindSet;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// The two implementations of `button`: (name, switch).
fn impls() -> [(&'static str, KindSet); 2] {
    [("egui", KindSet::None), ("own", KindSet::tags(["button"]))]
}

fn rig(set: &KindSet) -> Rig {
    let rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk); // a press focuses a button (FX1)
    rig.backend.set_own_widgets(set.clone());
    rig
}

fn own(rig: &Rig, h: svl_handle) -> bool {
    rig.backend.with_scene(|s| matches!(s.n(h).w, W::Ext(_)))
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

fn key(rig: &mut Rig, k: egui::Key) {
    rig.harness.key_press(k);
    rig.run();
}

#[test]
fn ow0_button_replaces_the_builtin_row() {
    let reg = KindRegistry::global();
    let k = reg.tag_index("button").expect("button");
    let r = reg.replacement(k).expect("the own button replaces <button>");
    assert_eq!(r.entry.id, "sv.Button");
    assert_eq!(r.entry.replaces, Some("button"));
    assert!(reg.provider_index("sv.Button").is_none(), "an own widget is not an element of its own");
    // every own prop and signal of the row routes to the built-in id
    for (i, p) in r.entry.props.iter().enumerate() {
        assert_eq!(reg.ext_prop_id(k, i), reg.find_prop(k, p.name).unwrap().0, "{}", p.name);
    }
    for (i, e) in r.entry.events.iter().enumerate() {
        assert_eq!(reg.ext_signal_id(k, i), reg.find_signal(k, e.name).unwrap().0, "{}", e.name);
    }
}

#[test]
fn ow0_switch_picks_the_implementation_and_props_round_trip() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let b = rig.leaf(col, "button", &[]);
        assert_eq!(own(&rig, b), name == "own", "{name}");
        // defaults are the row's
        assert_eq!(rig.get(b, "text"), s(""), "{name}");
        assert_eq!(rig.get(b, "frame"), Value::Bool(true), "{name}");
        assert_eq!(rig.get(b, "font-weight"), Value::I64(400), "{name}");
        let props = [
            ("text", s("Save")),
            ("selected", Value::Bool(true)),
            ("frame", Value::Bool(false)),
            ("small", Value::Bool(true)),
            ("font-weight", Value::I64(600)),
            ("default", Value::Bool(true)),
            ("cancel", Value::Bool(true)),
            ("tooltip", s("Saves")),
        ];
        for (p, v) in &props {
            rig.set(b, p, v.clone());
        }
        for (p, v) in &props {
            assert_eq!(&rig.get(b, p), v, "{name}: {p}");
        }
        // null resets to the row's default
        rig.br().set_prop(b, rig.prop_id(b, "text"), &Value::Null).unwrap();
        assert_eq!(rig.get(b, "text"), s(""), "{name}");
        rig.set(b, "text", s("Save"));
        rig.run();
        assert_eq!(rig.backend.with_scene(|s| s.n(b).text().to_string()), "Save", "{name}: Node::text");
        assert!(rig.drawn(b).width() > 0.0, "{name}: drawn");
    }
}

#[test]
fn ow0_button_clicks_keys_and_accesskit_on_both() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let b = rig.leaf(col, "button", &[("text", s("Apply"))]);
        let off = rig.leaf(col, "button", &[("text", s("Off")), ("enabled", Value::Bool(false))]);
        for h in [b, off] {
            rig.listen(h, "clicked");
        }
        rig.run();
        let node = rig.harness.query_by_role_and_label(Role::Button, "Apply");
        assert!(node.is_some(), "{name}: a Button named by its text");

        // a click: one `clicked`, with no arguments (the row's arity)
        rig.click(off);
        assert!(events(&rig, off, "clicked").is_empty(), "{name}: disabled");
        rig.click(b);
        assert_eq!(events(&rig, b, "clicked"), vec![Vec::<Value>::new()], "{name}");

        // Space and Enter on the focused button
        assert!(rig.harness.query_by_label("Apply").unwrap().is_focused(), "{name}: a press focuses it (GTK)");
        rig.take();
        key(&mut rig, egui::Key::Space);
        key(&mut rig, egui::Key::Enter);
        assert_eq!(events(&rig, b, "clicked").len(), 2, "{name}: Space and Enter");

        // AccessKit Click goes through the same path
        rig.take();
        rig.harness.get_by_label("Apply").click_accesskit();
        rig.run();
        assert_eq!(events(&rig, b, "clicked").len(), 1, "{name}: AccessKit click");
    }
}

#[test]
fn ow0_default_and_cancel_buttons_on_both() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let field = rig.leaf(col, "text-edit", &[("accessible-name", s("name"))]);
        let ok = rig.leaf(col, "button", &[("text", s("OK")), ("default", Value::Bool(true))]);
        let cancel = rig.leaf(col, "button", &[("text", s("Cancel")), ("cancel", Value::Bool(true))]);
        for h in [ok, cancel] {
            rig.listen(h, "clicked");
        }
        rig.run();
        rig.click(field);
        rig.take();
        key(&mut rig, egui::Key::Enter);
        assert_eq!((events(&rig, ok, "clicked").len(), events(&rig, cancel, "clicked").len()), (1, 0), "{name}: Enter");
        rig.take();
        key(&mut rig, egui::Key::Escape);
        assert_eq!((events(&rig, ok, "clicked").len(), events(&rig, cancel, "clicked").len()), (0, 1), "{name}: Escape");
    }
}

#[test]
fn ow0_own_button_states_hover_press_focus_visible() {
    let mut rig = rig(&KindSet::tags(["button"]));
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "button", &[("text", s("A"))]);
    let b = rig.leaf(col, "button", &[("text", s("B")), ("selected", Value::Bool(true))]);
    rig.run();
    assert!(state(&rig, b).contains(StateBits::SELECTED), "the prop mirror");

    rig.harness.hover_at(rig.drawn(a).center());
    rig.run();
    assert!(state(&rig, a).contains(StateBits::HOVERED));
    assert!(!state(&rig, b).contains(StateBits::HOVERED));

    // held: pressed; released: not
    let pos = rig.drawn(a).center();
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    rig.run();
    assert!(state(&rig, a).contains(StateBits::PRESSED));
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    rig.run();
    rig.run();
    let st = state(&rig, a);
    assert!(!st.contains(StateBits::PRESSED));
    // focused by the press (GTK focus-on-click) but no focus ring (FX1, `:focus-visible`)
    assert!(st.contains(StateBits::FOCUSED), "{st:?}");
    assert!(!st.contains(StateBits::FOCUS_VISIBLE), "{st:?}");

    // Tab: focus moves by keyboard, the ring shows
    key(&mut rig, egui::Key::Tab);
    rig.run();
    let st = state(&rig, b);
    assert!(st.contains(StateBits::FOCUSED) && st.contains(StateBits::FOCUS_VISIBLE), "{st:?}");
    assert!(!state(&rig, a).contains(StateBits::FOCUSED));
}

#[test]
fn ow0_own_button_is_sized_by_its_style() {
    let mut rig = rig(&KindSet::tags(["button"]));
    let (_w, col) = rig.window();
    let b = rig.leaf(col, "button", &[("text", s("Width"))]);
    let small = rig.leaf(col, "button", &[("text", s("Width")), ("small", Value::Bool(true))]);
    rig.run();
    rig.run();
    let (r, rs) = (rig.drawn(b), rig.drawn(small));
    // content + padding (button-pad-x/y) + border (stroke) from the UA table: VS Code's 13px text
    assert!(r.height() >= 20.0 && r.height() <= 30.0, "{r:?}");
    assert!(rs.height() < r.height(), "small drops the vertical padding: {rs:?} vs {r:?}");
}

// ---- OW0b ----

use svelters_egui::paint::{self, PaintArgs, PaintCx, PartInfo, StyleView, TraceKind};
use svelters_egui::provider::{AvailableSpace, MeasureCx, Widget, WidgetCx};

/// A test-only own widget: replaces `sides` (only in this test binary; no own widget replaces it,
/// and its row props are all `Common`) with an extra prop and an extra event the built-in row lacks
/// (the superset rule).
#[svelters::widget(namespace = "t", replaces = "sides", caps = PAINTS_BOX)]
pub struct TestSides {
    /// An extra prop: not in the row.
    #[prop(default = 2.0)]
    thickness: f64,
    /// An extra event.
    #[event]
    poked: svelters_egui::provider::Event<bool>,
}

impl Widget for TestSides {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(10.0, self.thickness as f32)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        ui.interact(cx.rect(), cx.id(), egui::Sense::hover())
    }
}

fn node_raw(rig: &Rig, h: svl_handle) -> u64 {
    rig.backend.with_scene(|s| s.n(h).handle.to_raw())
}

#[test]
fn ow0b_extra_props_and_events_of_an_own_widget() {
    let reg = KindRegistry::global();
    let k = reg.tag_index("sides").unwrap();
    let r = reg.replacement(k).expect("the test sides");
    assert_eq!(r.extra_props.len(), 1);
    assert_eq!(r.extra_props[0].name, "thickness");
    let (id, def) = reg.find_prop(k, "thickness").expect("an extra prop resolves by name");
    assert_eq!(reg.prop_def(k, id).map(|d| d.name), Some("thickness"));
    assert!(matches!(def.default, svelters_egui::kinds::Def::F64(v) if v == 2.0));
    assert!(reg.find_signal(k, "poked").is_some(), "an extra event is a signal of the kind");
    assert!(reg.spec_kind(k).props.iter().any(|p| p.name == "thickness"), "the spec lists it");
    assert!(reg.kind(k).props.iter().all(|p| p.name != "thickness"), "the row itself is untouched");
    for (name, set) in [("egui", KindSet::None), ("own", KindSet::tags(["sides"]))] {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let h = rig.leaf(col, "sides", &[]);
        rig.set(h, "thickness", Value::F64(5.0));
        rig.run();
        let want = if name == "own" { Value::F64(5.0) } else { Value::F64(2.0) };
        assert_eq!(rig.get(h, "thickness"), want, "{name}: the wrapper ignores an extra prop");
        rig.set(h, "align-items", s("center"));
        assert_eq!(rig.get(h, "align-items"), s("center"), "{name}: a row prop kept in Common");
    }
}

#[test]
fn ow0b_paint_trace_names_the_box_label_and_focus_ring() {
    let mut rig = rig(&KindSet::Default);
    paint::trace::enable(&rig.harness.ctx, true);
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "button", &[("text", s("Alpha"))]);
    let b = rig.leaf(col, "button", &[("text", s("Beta"))]);
    rig.run();
    assert!(own(&rig, a), "button is own by default");
    let t = paint::trace::last(&rig.harness.ctx);
    let (na, nb) = (node_raw(&rig, a), node_raw(&rig, b));
    let bx = t.find(na, None, TraceKind::Box).expect("the button's box");
    assert_eq!(bx.rect, rig.drawn(a));
    assert!(bx.fill.a() > 0, "R1: a raised button is never transparent");
    assert!(bx.stroke.width >= 1.0, "R1: a boundary");
    let label = t.find(na, Some("label"), TraceKind::Text).expect("the label part");
    assert_eq!(label.font_size, 12.0, "VIS1: 12px button text");
    assert!(t.find(na, None, TraceKind::Outline).is_none(), "no ring at rest");
    // keyboard focus: the ring (R8: 2px, outside the box)
    for _ in 0..2 {
        rig.harness.key_press(egui::Key::Tab);
        rig.run();
    }
    let t = paint::trace::last(&rig.harness.ctx);
    let focused = [na, nb].into_iter().find(|n| t.find(*n, None, TraceKind::Outline).is_some()).expect("a focus ring after Tab");
    let ring = t.find(focused, None, TraceKind::Outline).unwrap();
    assert_eq!(ring.stroke.width, 2.0);
    let bx = t.find(focused, None, TraceKind::Box).unwrap();
    assert!(ring.rect.contains_rect(bx.rect) && ring.rect != bx.rect, "offset +1, outside the box");
    // a pointer press focuses without a ring (FX1)
    rig.click(a);
    let t = paint::trace::last(&rig.harness.ctx);
    assert!(t.find(na, None, TraceKind::Outline).is_none());
    assert!(state(&rig, a).contains(StateBits::FOCUSED));
}

#[test]
fn ow0b_disabled_button_keeps_its_shape_and_its_reason() {
    let mut rig = rig(&KindSet::Default);
    let (_w, col) = rig.window();
    let off = rig.leaf(col, "button", &[("text", s("Save")), ("enabled", Value::Bool(false)), ("tooltip", s("Nothing to save"))]);
    rig.listen(off, "clicked");
    rig.run();
    rig.harness.hover_at(rig.drawn(off).center());
    rig.run();
    // R9: hoverable while disabled
    assert!(state(&rig, off).contains(StateBits::HOVERED) && state(&rig, off).contains(StateBits::DISABLED), "{:?} at {:?}", state(&rig, off), rig.drawn(off));
    let n = rig.harness.query_by_role_and_label(Role::Button, "Save").expect("a11y");
    assert!(n.accesskit_node().data().is_disabled());
    rig.click(off);
    assert!(events(&rig, off, "clicked").is_empty());
}

#[test]
fn ow0b_press_release_and_repeat() {
    let mut rig = rig(&KindSet::Default);
    let (_w, col) = rig.window();
    let b = rig.leaf(col, "button", &[("text", s("Step")), ("repeat", Value::Bool(true))]);
    for e in ["press", "release", "clicked"] {
        rig.listen(b, e);
    }
    rig.run();
    let pos = rig.drawn(b).center();
    rig.harness.hover_at(pos);
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    rig.harness.step();
    assert_eq!(events(&rig, b, "press").len(), 1);
    // held (kittest steps 0.25 s of input time): a repeat after 300 ms, then every 100 ms
    for _ in 0..8 {
        rig.harness.step();
    }
    let repeats = events(&rig, b, "clicked").len();
    assert!(repeats >= 2, "{repeats} repeats");
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    rig.run();
    assert_eq!(events(&rig, b, "release").len(), 1);
}

/// A part painter selected in CSS (`background-image: paint(name)`), called by the box primitive
/// with no scene node (as a foreign painter would: OW-RUST, berth's `ChromePainter`). The
/// engine's parse of `paint()` is OW-CSS's; the value is read here from a UA table.
#[test]
fn ow0b_css_selected_part_painter() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use svelters_egui::paint::{Ua, UaRule, UaValue};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    paint::register_painter("t.stripes", |cx: &mut PaintCx, _s: &StyleView, r: egui::Rect, part: &PartInfo<'_>, _a: &PaintArgs<'_>| {
        assert_eq!(part.tag, "chrome-tab");
        cx.painter.rect_filled(r.shrink(2.0), 0.0, egui::Color32::from_rgb(1, 2, 3));
        CALLS.fetch_add(1, Ordering::Relaxed);
    });
    const UA: Ua = Ua(&[UaRule::base(&[
        ("background-color", UaValue::Token("button-bg")),
        ("background-image", UaValue::Keyword("paint(t.stripes)")),
        ("border-width", UaValue::Px(1.0)),
        ("border-color", UaValue::Token("button-border-color")),
    ])]);
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let painter = ui.painter().clone();
        let mut cx = PaintCx::new(&painter);
        cx.tag = "chrome-tab";
        let s = StyleView::ua_only(UA, StateBits::HOVERED, true);
        let r = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(80.0, 24.0));
        paint::paint_box(&mut cx, &s, r);
    });
    out.textures_delta.clear();
    assert_eq!(CALLS.load(Ordering::Relaxed), 1, "the painter ran once for the box");
    let fills: Vec<egui::Color32> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) => Some(r.fill),
            _ => None,
        })
        .collect();
    let stripe = fills.iter().position(|f| *f == egui::Color32::from_rgb(1, 2, 3)).expect("the painter's shape");
    // between the background and the border (painted last, with a transparent fill)
    assert!(stripe > 0 && stripe < fills.len() - 1, "{fills:?}");
    // an unknown painter falls back to the default paint
    const NONE_UA: Ua = Ua(&[UaRule::base(&[("background-image", UaValue::Keyword("paint(t.missing)"))])]);
    let mut out2 = ctx.run_ui(egui::RawInput::default(), |ui| {
        let painter = ui.painter().clone();
        let r = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(10.0, 10.0));
        paint::paint_box(&mut PaintCx::new(&painter), &StyleView::ua_only(NONE_UA, StateBits::NONE, true), r);
    });
    out2.textures_delta.clear();
    assert_eq!(CALLS.load(Ordering::Relaxed), 1);
}

#[test]
fn ow0b_policy_override_reaches_the_units() {
    let mut rig = rig(&KindSet::Default);
    svelters_egui::native::override_policy(&rig.harness.ctx, |p| p.focus_on_click = false);
    assert!(!svelters_egui::native::policy_of(&rig.harness.ctx).focus_on_click);
    let (_w, col) = rig.window();
    let b = rig.leaf(col, "button", &[("text", s("Plain"))]);
    rig.run();
    rig.click(b);
    assert!(!state(&rig, b).contains(StateBits::FOCUSED), "no focus on click once the app overrides it");
    svelters_egui::native::clear_policy_override(&rig.harness.ctx);
}

/// `onclicked` from compiled Svelte reaches JS through `WidgetCx::dispatch` (EV1's seam; a direct
/// emit until EV1 lands): the EV3 dialog fixture on the own button.
#[test]
fn ow0b_svelte_onclicked_reaches_js_through_dispatch() {
    use svelters_egui::harness::TestApp;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/src/FocusDialog.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    let pos = app.harness.get_by_label("Open").rect().center();
    app.harness.hover_at(pos);
    for pressed in [true, false] {
        app.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    app.run();
    assert!(app.harness.query_by_label("OK").is_some(), "the handler opened the dialog: {:?}", app.errors());
    app.harness.get_by_label("Cancel").click_accesskit();
    app.run();
    assert!(app.harness.query_by_label("Result: cancel").is_some(), "{:?}", app.errors());
}
