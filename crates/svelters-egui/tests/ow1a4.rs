//! OW1a4 (plans/milestones/OW1a4-actions-collapsing.md): the behaviour units as `use:` actions
//! (`svelters:behaviors/sv`, `use_units.rs`), and collapsing-header's `accordion`, `slot="end"`
//! header actions and `icon`. Headless (kittest, no JS: `common::Rig` attaches the behaviour node
//! the way `nativeBehavior` does: create, set params, listen, insert into the host).

mod common;

use common::{Log, Rig};
use egui::{Event, Key, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::kittest::Queryable;
use svelters_abi::*;
use svelters_egui::native::{self, Native};
use svelters_egui::provider::StateBits;
use svelters_egui::widgets::KindSet;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}
fn f(v: f64) -> Value {
    Value::F64(v)
}
fn b(v: bool) -> Value {
    Value::Bool(v)
}

fn rig() -> Rig {
    let rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    rig.backend.set_own_widgets(KindSet::Default);
    rig
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

fn events(rig: &Rig, log: &Log, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

/// A window with a column, and in it a 120x40 box carrying `use:<name>` with `params`, listening
/// to `listen`. Returns (rig, column, host box, behaviour node).
fn with_use(name: &str, params: &[(&str, Value)], listen: &[&str]) -> (Rig, svl_handle, svl_handle, svl_handle) {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let host = rig.leaf(col, "box", &[("width", f(120.0)), ("height", f(40.0))]);
    let u = rig.create(&format!("sv.{name}"));
    for (k, v) in params {
        rig.set(u, k, v.clone());
    }
    for e in listen {
        rig.listen(u, e);
    }
    rig.insert(host, u);
    rig.run();
    rig.run();
    rig.take();
    (rig, col, host, u)
}

/// Where `h` is on screen: its drawn rect, or its layout rect for a box (boxes record no `drawn`).
fn at(rig: &Rig, h: svl_handle) -> egui::Rect {
    let d = rig.drawn(h);
    if d.is_finite() { d } else { rig.backend.with_scene(|s| s.n(h).rect) }
}

fn click(rig: &mut Rig, h: svl_handle) {
    let c = at(rig, h).center();
    rig.harness.hover_at(c);
    press(rig, c, true);
    press(rig, c, false);
    rig.run();
}

fn press(rig: &mut Rig, pos: egui::Pos2, pressed: bool) {
    rig.harness.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
}

fn drag(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    rig.harness.hover_at(from);
    press(rig, from, true);
    rig.harness.step();
    for k in 1..=4 {
        rig.harness.hover_at(from + (to - from) * (k as f32 / 4.0));
        rig.harness.step();
    }
    press(rig, to, false);
    rig.run();
}

/// A record payload's fields, in declaration order.
fn rec(v: &[Value]) -> Vec<Value> {
    match v {
        [Value::Record { fields, .. }] => fields.clone(),
        other => panic!("not one record: {other:?}"),
    }
}

/// Whether `h` holds egui's keyboard focus.
fn has_focus(rig: &Rig, h: svl_handle) -> bool {
    let id = rig.backend.with_scene(|s| s.n(h).id);
    rig.harness.ctx.memory(|m| m.focused()) == Some(id)
}

/// Tab until `h` holds focus (at most 8 presses).
fn tab_to(rig: &mut Rig, h: svl_handle) {
    for _ in 0..8 {
        if has_focus(rig, h) {
            return;
        }
        rig.harness.key_press(Key::Tab);
        rig.run();
        rig.run();
    }
    panic!("never focused: {:?}", state(rig, h));
}

// ---- the module and its typings ----

const EXPORTS: &[&str] = &[
    "hoverable", "pressable", "focusable", "toggleable", "expandable", "roving", "rovingList", "typeahead", "anchored", "dismissable", "tooltipable", "draggable", "dragToValue", "steppable", "resizable", "scrollable",
];

#[test]
fn ow1a4_sv_behaviours_module_and_typings_export_every_unit() {
    let modules = svelters_egui::spec::behavior_modules();
    let (_, js) = modules.iter().find(|(ns, _)| *ns == "sv").expect("svelters:behaviors/sv");
    let dts = svelters_egui::spec::behaviors_typings();
    assert!(dts.contains("declare module 'svelters:behaviors/sv'"));
    for e in EXPORTS {
        assert!(js.contains(&format!("export const {e} = nativeBehavior('sv.{e}'")), "{e} in behaviors/sv.js");
        assert!(dts.contains(&format!("export const {e}: Action<")), "{e} in behaviors.d.ts");
    }
    // params are camelCase, events single words (`on<event>` callbacks)
    assert!(dts.contains("autoRepeat"), "{dts}");
    assert!(dts.contains("onpress"), "{dts}");
}

// ---- one test per export ----

#[test]
fn ow1a4_use_hoverable_sets_hover_and_reports_enter_and_leave() {
    let (mut rig, _c, host, u) = with_use("hoverable", &[], &["hover"]);
    rig.harness.hover_at(at(&rig, host).center());
    rig.run();
    rig.run();
    assert!(state(&rig, host).contains(StateBits::HOVERED));
    rig.harness.hover_at(pos2(at(&rig, host).right() + 300.0, 500.0));
    rig.run();
    rig.run();
    assert!(!state(&rig, host).contains(StateBits::HOVERED));
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "hover"), vec![vec![b(true)], vec![b(false)]]);
}

#[test]
fn ow1a4_use_pressable_reports_down_up_and_a_click_with_active_while_held() {
    let (mut rig, _c, host, u) = with_use("pressable", &[], &["press", "down", "up"]);
    let c = at(&rig, host).center();
    rig.harness.hover_at(c);
    press(&mut rig, c, true);
    rig.harness.step();
    rig.harness.step();
    assert!(state(&rig, host).contains(StateBits::PRESSED), "`:active` while held");
    press(&mut rig, c, false);
    rig.harness.step();
    rig.harness.step();
    assert!(!state(&rig, host).contains(StateBits::PRESSED));
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "down").len(), 1);
    assert_eq!(events(&rig, &log, u, "up"), vec![vec![b(true)]]);
    let p = events(&rig, &log, u, "press");
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(rec(&p[0]), vec![f(1.0), s("primary")]);
}

#[test]
fn ow1a4_use_focusable_is_a_tab_stop_with_focus_visible() {
    let (mut rig, _c, host, u) = with_use("focusable", &[], &["focus", "blur"]);
    tab_to(&mut rig, host);
    let st = state(&rig, host);
    assert!(st.contains(StateBits::FOCUS_VISIBLE), "keyboard focus shows the ring: {st:?}");
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "focus"), vec![vec![s("keyboard")]]);
}

#[test]
fn ow1a4_use_toggleable_toggles_on_click_and_sets_checked() {
    let (mut rig, _c, host, u) = with_use("toggleable", &[], &["toggle"]);
    click(&mut rig, host);
    rig.run();
    rig.run();
    assert!(state(&rig, host).contains(StateBits::CHECKED));
    assert_eq!(rig.get(u, "checked"), b(true), "uncontrolled: the param follows");
    click(&mut rig, host);
    rig.run();
    rig.run();
    assert!(!state(&rig, host).contains(StateBits::CHECKED));
    let log = rig.take();
    let t: Vec<_> = events(&rig, &log, u, "toggle").iter().map(|e| rec(e)).collect();
    assert_eq!(t, vec![vec![b(true), b(false)], vec![b(false), b(false)]]);
}

#[test]
fn ow1a4_use_expandable_expands_on_click_and_by_arrows() {
    let (mut rig, _c, host, u) = with_use("expandable", &[], &["expand"]);
    click(&mut rig, host);
    rig.run();
    rig.run();
    assert!(state(&rig, host).contains(StateBits::EXPANDED), "`:expanded`");
    tab_to(&mut rig, host);
    rig.harness.key_press(Key::ArrowLeft);
    rig.run();
    rig.run();
    assert!(!state(&rig, host).contains(StateBits::EXPANDED));
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "expand"), vec![vec![b(true)], vec![b(false)]]);
}

#[test]
fn ow1a4_use_roving_moves_focus_among_its_siblings() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let mut hosts = Vec::new();
    let mut us = Vec::new();
    for _ in 0..3 {
        let h = rig.leaf(col, "box", &[("width", f(60.0)), ("height", f(20.0))]);
        let u = rig.create("sv.roving");
        rig.listen(u, "rove");
        rig.insert(h, u);
        hosts.push(h);
        us.push(u);
    }
    rig.run();
    rig.run();
    tab_to(&mut rig, hosts[0]);
    rig.take();
    rig.harness.key_press(Key::ArrowDown);
    for _ in 0..3 {
        rig.run();
    }
    assert!(has_focus(&rig, hosts[1]));
    let log = rig.take();
    assert_eq!(events(&rig, &log, us[0], "rove"), vec![vec![f(1.0)]]);
    // End goes to the last member
    rig.harness.key_press(Key::End);
    for _ in 0..3 {
        rig.run();
    }
    assert!(has_focus(&rig, hosts[2]));
}

#[test]
fn ow1a4_use_roving_list_moves_the_highlight_over_enabled_items() {
    let enabled = Value::List(vec![b(true), b(false), b(true)]);
    let (mut rig, _c, host, u) = with_use("rovingList", &[("enabled", enabled), ("current", f(0.0))], &["move"]);
    tab_to(&mut rig, host);
    rig.take();
    rig.harness.key_press(Key::ArrowDown);
    rig.run();
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "move"), vec![vec![f(2.0)]], "item 1 is disabled");
}

#[test]
fn ow1a4_use_typeahead_builds_a_prefix() {
    let (mut rig, _c, host, u) = with_use("typeahead", &[], &["search"]);
    tab_to(&mut rig, host);
    rig.take();
    for t in ["a", "b"] {
        rig.harness.event(Event::Text(t.into()));
        rig.run();
    }
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "search"), vec![vec![s("a")], vec![s("ab")]]);
}

#[test]
fn ow1a4_use_anchored_reports_the_placement_beside_the_anchor() {
    let anchor = Value::List(vec![f(100.0), f(100.0), f(50.0), f(20.0)]);
    let (mut rig, _c, _host, u) = with_use("anchored", &[("gap", f(4.0))], &["placed"]);
    rig.set(u, "anchor", anchor);
    rig.run();
    rig.run();
    let log = rig.take();
    let p = events(&rig, &log, u, "placed");
    assert_eq!(p.len(), 1, "once per change: {p:?}");
    let r = rec(&p[0]);
    assert_eq!(&r[..3], &[s("bottom"), f(100.0), f(124.0)]);
    assert_eq!(&r[3..], &[f(120.0), f(40.0)], "the host's size");
}

#[test]
fn ow1a4_use_dismissable_asks_to_close_on_escape_and_outside_press() {
    let (mut rig, _c, host, u) = with_use("dismissable", &[], &["dismiss"]);
    rig.harness.key_press(Key::Escape);
    rig.run();
    let out = pos2(at(&rig, host).right() + 200.0, at(&rig, host).bottom() + 200.0);
    rig.harness.hover_at(out);
    press(&mut rig, out, true);
    rig.run();
    press(&mut rig, out, false);
    rig.run();
    // a press inside does not
    let c = at(&rig, host).center();
    rig.harness.hover_at(c);
    press(&mut rig, c, true);
    rig.run();
    press(&mut rig, c, false);
    rig.run();
    let log = rig.take();
    let d = events(&rig, &log, u, "dismiss");
    assert_eq!(d.len(), 2, "{d:?}");
    assert_eq!(rec(&d[0]), vec![s("escape"), Value::Null, Value::Null]);
    assert_eq!(rec(&d[1])[0], s("outside-press"));
}

#[test]
fn ow1a4_use_tooltipable_shows_after_the_delay_and_hides_on_leave() {
    let (mut rig, _c, host, u) = with_use("tooltipable", &[("delay", f(0.05))], &["tip"]);
    rig.harness.hover_at(at(&rig, host).center());
    for _ in 0..20 {
        rig.harness.step();
    }
    rig.harness.hover_at(pos2(at(&rig, host).right() + 300.0, 500.0));
    for _ in 0..3 {
        rig.harness.step();
    }
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "tip"), vec![vec![b(true)], vec![b(false)]]);
}

#[test]
fn ow1a4_use_draggable_reports_start_move_end_with_dragging() {
    let (mut rig, _c, host, u) = with_use("draggable", &[], &["drag"]);
    let c = at(&rig, host).center();
    rig.harness.hover_at(c);
    press(&mut rig, c, true);
    rig.harness.step();
    for k in 1..=4 {
        rig.harness.hover_at(c + vec2(10.0 * k as f32, 0.0));
        rig.harness.step();
    }
    rig.harness.step();
    assert!(state(&rig, host).contains(StateBits::DRAGGING), "`:dragging`");
    press(&mut rig, c + vec2(40.0, 0.0), false);
    rig.run();
    let log = rig.take();
    let d: Vec<_> = events(&rig, &log, u, "drag").iter().map(|e| rec(e)).collect();
    assert_eq!(d.first().map(|e| e[0].clone()), Some(s("start")));
    assert_eq!(d.last().map(|e| e[0].clone()), Some(s("end")));
    assert!(d.iter().any(|e| e[0] == s("move") && e[1] == f(40.0)), "{d:?}");
    assert!(!state(&rig, host).contains(StateBits::DRAGGING));
}

#[test]
fn ow1a4_use_drag_to_value_scrubs_and_commits() {
    let (mut rig, _c, host, u) = with_use("dragToValue", &[("value", f(10.0)), ("step", f(1.0))], &["scrub", "commit"]);
    let c = at(&rig, host).center();
    drag(&mut rig, c, c + vec2(20.0, 0.0));
    let log = rig.take();
    assert!(!events(&rig, &log, u, "scrub").is_empty());
    let commit = events(&rig, &log, u, "commit");
    assert_eq!(commit.len(), 1, "{commit:?}");
    let Value::F64(v) = commit[0][0] else { panic!() };
    assert!(v > 10.0, "dragged right raises it: {v}");
    assert_eq!(rig.get(u, "value"), f(v));
}

#[test]
fn ow1a4_use_steppable_steps_by_arrows_and_clamps() {
    let (mut rig, _c, host, u) = with_use("steppable", &[("value", f(1.0)), ("max", f(2.0))], &["change"]);
    tab_to(&mut rig, host);
    rig.take();
    for _ in 0..3 {
        rig.harness.key_press(Key::ArrowUp);
        rig.run();
    }
    let log = rig.take();
    assert_eq!(events(&rig, &log, u, "change"), vec![vec![f(2.0)]], "clamped at max");
    assert_eq!(rig.get(u, "value"), f(2.0));
}

#[test]
fn ow1a4_use_resizable_resizes_and_commits() {
    let (mut rig, _c, host, u) = with_use("resizable", &[("size", f(100.0)), ("max", f(400.0))], &["resize", "commit"]);
    let c = at(&rig, host).center();
    drag(&mut rig, c, c + vec2(30.0, 0.0));
    let log = rig.take();
    assert!(!events(&rig, &log, u, "resize").is_empty());
    assert_eq!(events(&rig, &log, u, "commit"), vec![vec![f(130.0)]]);
}

#[test]
fn ow1a4_use_scrollable_scrolls_a_virtual_content_by_wheel() {
    let content = Value::List(vec![f(120.0), f(1000.0)]);
    let (mut rig, _c, host, u) = with_use("scrollable", &[("content", content)], &["scroll"]);
    rig.harness.hover_at(at(&rig, host).center());
    rig.harness.event(Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -50.0), modifiers: Modifiers::NONE, phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Wheel });
    for _ in 0..10 {
        rig.harness.step();
    }
    let log = rig.take();
    let sc = events(&rig, &log, u, "scroll");
    let last = rec(sc.last().expect("scrolled"));
    let Value::F64(y) = last[1] else { panic!() };
    assert!(y > 0.0 && y <= 960.0, "{sc:?}");
}

#[test]
fn ow1a4_use_unit_bits_leave_with_the_behaviour() {
    let (mut rig, _c, host, u) = with_use("toggleable", &[("checked", b(true))], &[]);
    rig.run();
    assert!(state(&rig, host).contains(StateBits::CHECKED));
    rig.br().destroy(u);
    rig.run();
    rig.run();
    assert!(!state(&rig, host).contains(StateBits::CHECKED), "detached clears the unit's bits");
}

// ---- collapsing-header ----

#[test]
fn ow1a4_accordion_opening_one_closes_the_others() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "collapsing-header", &[("text", s("One")), ("accordion", s("g")), ("open", b(true))]);
    let _ = rig.leaf(a, "label", &[("text", s("Body one"))]);
    let b2 = rig.leaf(col, "collapsing-header", &[("text", s("Two")), ("accordion", s("g"))]);
    let _ = rig.leaf(b2, "label", &[("text", s("Body two"))]);
    let other = rig.leaf(col, "collapsing-header", &[("text", s("Other")), ("open", b(true))]);
    for h in [a, b2, other] {
        rig.listen(h, "changed");
    }
    rig.run();
    rig.run();
    rig.take();
    let row = rig.drawn(b2);
    let p = pos2(row.left() + 40.0, row.top() + 8.0);
    rig.harness.hover_at(p);
    press(&mut rig, p, true);
    press(&mut rig, p, false);
    for _ in 0..3 {
        rig.run();
    }
    assert_eq!(rig.get(b2, "open"), b(true));
    assert_eq!(rig.get(a, "open"), b(false), "the group's other header closed");
    assert_eq!(rig.get(other, "open"), b(true), "another group (none) is untouched");
    assert!(!state(&rig, a).contains(StateBits::EXPANDED));
    let log = rig.take();
    assert_eq!(events(&rig, &log, a, "changed"), vec![vec![b(false)]]);
    assert_eq!(events(&rig, &log, b2, "changed"), vec![vec![b(true)]]);
    assert!(rig.harness.query_by_label("Body one").is_none());
}

#[test]
fn ow1a4_header_actions_in_slot_end_sit_in_the_header_and_show_on_hover() {
    let mut rig = rig();
    let (_w, col) = rig.window();
    let h = rig.leaf(col, "collapsing-header", &[("text", s("Explorer")), ("width", f(300.0))]);
    let body = rig.leaf(h, "label", &[("text", s("Body"))]);
    let end = rig.br().resolve(NameKind::Slot, rig.kind(h), "end");
    assert_eq!(end, 2, "`end` is collapsing-header's second slot");
    let act = rig.create("button");
    rig.set(act, "text", s("Refresh"));
    rig.listen(act, "clicked");
    rig.br().insert(h, act, None, end).unwrap();
    rig.listen(h, "changed");
    rig.run();
    rig.run();
    // not hovered: the action is not drawn
    rig.harness.hover_at(pos2(600.0, 590.0));
    rig.run();
    rig.run();
    assert!(rig.harness.query_by_label("Refresh").is_none(), "hidden until hover");
    let hr = rig.drawn(h);
    rig.harness.hover_at(pos2(hr.left() + 40.0, hr.top() + 8.0));
    rig.run();
    rig.run();
    assert!(rig.harness.query_by_label("Refresh").is_some(), "shown on hover");
    let ar = rig.drawn(act);
    assert!(ar.top() >= hr.top() - 0.5 && ar.bottom() <= hr.top() + 30.0, "in the header row: {ar:?} {hr:?}");
    assert!((hr.right() - ar.right()).abs() < 8.0, "right-aligned: {ar:?} {hr:?}");
    // a click on the action is the action's, not the header's
    rig.click(act);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, act, "clicked").len(), 1);
    assert!(events(&rig, &log, h, "changed").is_empty());
    assert_eq!(rig.get(h, "open"), b(false));
    // open: the body is laid out below without room for the action
    rig.set(h, "open", b(true));
    rig.run();
    rig.run();
    assert!(rig.drawn(body).top() >= hr.top() + 18.0, "{:?} {hr:?}", rig.drawn(body));
    assert!(rig.drawn(body).bottom() < rig.drawn(h).bottom() + 0.5);
}

#[test]
fn ow1a4_header_icon_paints_through_the_icon_primitive() {
    use svelters_egui::paint::{self, TraceKind};
    let mut rig = rig();
    paint::trace::enable(&rig.harness.ctx, true);
    let (_w, col) = rig.window();
    let plain = rig.leaf(col, "collapsing-header", &[("text", s("Plain"))]);
    let h = rig.leaf(col, "collapsing-header", &[("text", s("Plain")), ("icon", s("codicons:folder"))]);
    rig.run();
    rig.run();
    let id = |h| rig.backend.with_scene(|s| s.n(h).handle.to_raw());
    let (n, np) = (id(h), id(plain));
    let t = paint::trace::last(&rig.harness.ctx);
    let icon = t.entries.iter().any(|e| e.node == n && e.part == Some("icon") && e.kind != TraceKind::Box);
    assert!(icon, "the icon part is painted: {:?}", t.of(n, None).collect::<Vec<_>>());
    let label = |n| t.find(n, Some("label"), TraceKind::Text).map(|e| e.rect.left()).expect("caption");
    assert!(label(n) > label(np) + 10.0, "the caption moves past the icon");
}

#[test]
fn ow1a4_twistie_is_an_r14_chevron_that_follows_expanded() {
    use svelters_egui::paint::{self, TraceKind};
    let mut rig = rig();
    paint::trace::enable(&rig.harness.ctx, true);
    let (_w, col) = rig.window();
    let h = rig.leaf(col, "collapsing-header", &[("text", s("Section"))]);
    rig.run();
    rig.run();
    let n = rig.backend.with_scene(|s| s.n(h).handle.to_raw());
    let tw = |rig: &Rig| paint::trace::last(&rig.harness.ctx).find(n, Some("twistie"), TraceKind::Path).map(|e| e.rect);
    tw(&rig).expect("R14: the header shows a chevron");
    assert!(!state(&rig, h).contains(StateBits::EXPANDED));
    rig.set(h, "open", b(true));
    rig.run();
    rig.run();
    assert!(state(&rig, h).contains(StateBits::EXPANDED), "`:expanded`");
    tw(&rig).expect("still a chevron (pointing down)");
}

