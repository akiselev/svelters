//! OW2 (plans/milestones/OW2-scroll-panels.md): svelters' own `scroll-area`, `scrollbar`,
//! `split-view`, `panel` and window chrome, on the Scrollable and Resizable units. Headless
//! (kittest, no JS: `common::Rig`).

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, TouchPhase, Vec2, pos2, vec2};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::scene::W;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn f(v: &Value) -> f64 {
    match v {
        Value::F64(x) => *x,
        Value::I64(x) => *x as f64,
        other => panic!("{other:?}"),
    }
}

fn wheel(rig: &mut Rig, at: egui::Pos2, delta: Vec2, mods: Modifiers) {
    rig.harness.hover_at(at);
    rig.harness.step();
    rig.harness.event(Event::MouseWheel { unit: MouseWheelUnit::Point, delta, phase: TouchPhase::Move, source: MouseWheelSource::Wheel, modifiers: mods });
    rig.harness.run_steps(30);
}

/// A tall scroll area of 30 labels in the rig's column; returns (area, rows).
fn tall(rig: &mut Rig, parent: svl_handle, h: f64) -> (svl_handle, Vec<svl_handle>) {
    let sa = rig.leaf(parent, "scroll-area", &[("height", Value::F64(h))]);
    let rows = (0..30).map(|i| rig.leaf(sa, "label", &[("text", s(&format!("Row {i}")))])).collect();
    (sa, rows)
}

fn setup() -> Rig {
    let rig = Rig::new();
    // a wheel step lands in one frame
    rig.harness.ctx.all_styles_mut(|st| st.scroll_animation = egui::style::ScrollAnimation::none());
    svelters_egui::native::set(&rig.harness.ctx, svelters_egui::native::Native::Gtk);
    rig
}

#[test]
fn ow2_scroll_area_is_the_own_widget_and_wheel_scrolls_it() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let (sa, _rows) = tall(&mut rig, col, 100.0);
    rig.observe(sa, "scroll-y", true);
    rig.listen(sa, "changed");
    rig.run();
    assert!(rig.backend.with_scene(|sc| matches!(sc.n(sa).w, W::Ext(_))), "own widget by default");
    let c = rig.drawn(sa).center();
    wheel(&mut rig, c, vec2(0.0, -60.0), Modifiers::NONE);
    let y = f(&rig.get(sa, "scroll-y"));
    assert!(y > 30.0, "{y}");
    let log = rig.take();
    let changed = rig.signal_id(sa, "changed");
    let ev = log.events.iter().rev().find(|(h, sig, _)| *h == sa && *sig == changed).expect("changed");
    assert_eq!(ev.2.len(), 2, "changed(scroll-x, scroll-y): {:?}", ev.2);
    // the edge states (R15): scrolled down, more below
    let top = svelters_egui::interact::scroll::sv::OVERFLOW_TOP.bits();
    let bottom = svelters_egui::interact::scroll::sv::OVERFLOW_BOTTOM.bits();
    let st = rig.backend.with_scene(|sc| sc.n(sa).state);
    assert!(st.contains(top) && st.contains(bottom), "{st:?}");
    // AccessKit: a scroll view with its offset
    let node = rig.harness.get_by_role(Role::ScrollView);
    assert!(node.accesskit_node().data().scroll_y().unwrap_or(0.0) > 30.0);
}

#[test]
fn ow2_shift_wheel_scrolls_horizontally() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let sa = rig.leaf(col, "scroll-area", &[("height", Value::F64(100.0)), ("width", Value::F64(120.0)), ("direction", s("both"))]);
    rig.leaf(sa, "box", &[("width", Value::F64(600.0)), ("height", Value::F64(600.0))]);
    rig.run();
    let c = rig.drawn(sa).center();
    wheel(&mut rig, c, vec2(0.0, -50.0), Modifiers::SHIFT);
    assert!(f(&rig.get(sa, "scroll-x")) > 20.0 && f(&rig.get(sa, "scroll-y")) == 0.0, "{:?} {:?}", rig.get(sa, "scroll-x"), rig.get(sa, "scroll-y"));
}

/// Scroll chaining (WG3s pin 9): the inner area takes the wheel until its edge, then the outer.
/// `overscroll-behavior: contain` would keep it; the inner area at its end passes it on.
#[test]
fn ow2_wheel_chains_from_the_inner_area_at_its_edge() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let outer = rig.leaf(col, "scroll-area", &[("height", Value::F64(200.0))]);
    let inner = rig.leaf(outer, "scroll-area", &[("height", Value::F64(100.0)), ("flex-shrink", Value::F64(0.0))]);
    for i in 0..20 {
        rig.leaf(inner, "label", &[("text", s(&format!("In {i}")))]);
    }
    for i in 0..30 {
        rig.leaf(outer, "label", &[("text", s(&format!("Out {i}")))]);
    }
    rig.run();
    let c = rig.drawn(inner).center();
    wheel(&mut rig, c, vec2(0.0, -30.0), Modifiers::NONE);
    let (iy, oy) = (f(&rig.get(inner, "scroll-y")), f(&rig.get(outer, "scroll-y")));
    assert!(iy > 0.0 && oy == 0.0, "inner first: {iy} {oy}");
    // to the inner end, then once more: the outer moves
    for _ in 0..8 {
        let c = rig.drawn(inner).center();
        wheel(&mut rig, c, vec2(0.0, -80.0), Modifiers::NONE);
    }
    assert!(f(&rig.get(outer, "scroll-y")) > 0.0, "chained to the outer area");
}

#[test]
fn ow2_page_keys_scroll_the_area_holding_focus() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let sa = rig.leaf(col, "scroll-area", &[("height", Value::F64(100.0))]);
    let b = rig.leaf(sa, "button", &[("text", s("First"))]);
    for i in 0..30 {
        rig.leaf(sa, "label", &[("text", s(&format!("Row {i}")))]);
    }
    rig.run();
    rig.click(b);
    rig.harness.key_press(egui::Key::PageDown);
    rig.harness.run_steps(4);
    let y = f(&rig.get(sa, "scroll-y"));
    assert!(y > 40.0, "a page: {y}");
    rig.harness.key_press(egui::Key::End);
    rig.harness.run_steps(4);
    let max = f(&rig.get(sa, "scroll-y"));
    assert!(max > y, "{max} {y}");
    rig.harness.key_press(egui::Key::Home);
    rig.harness.run_steps(4);
    assert_eq!(f(&rig.get(sa, "scroll-y")), 0.0);
}

/// WG6 QA13: a focus moved by Tab to a widget below the fold scrolls it into view; and
/// `scroll-into-view` reveals a node.
#[test]
fn ow2_focus_and_scroll_into_view_reveal_a_node() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let sa = rig.leaf(col, "scroll-area", &[("height", Value::F64(100.0))]);
    let first = rig.leaf(sa, "button", &[("text", s("Top"))]);
    for i in 0..20 {
        rig.leaf(sa, "label", &[("text", s(&format!("Row {i}")))]);
    }
    let low = rig.leaf(sa, "button", &[("text", s("Bottom"))]);
    let last = rig.leaf(sa, "label", &[("text", s("Last row"))]);
    rig.run();
    rig.click(first);
    rig.harness.key_press(egui::Key::Tab);
    rig.harness.run_steps(6);
    let view = rig.drawn(sa);
    assert!(view.expand(1.0).contains_rect(rig.drawn(low)), "the focused button is shown: {:?} in {view:?}", rig.drawn(low));
    rig.set(sa, "scroll-y", Value::F64(0.0));
    rig.harness.run_steps(3);
    rig.call(last, "scroll-into-view");
    rig.harness.run_steps(6);
    let view = rig.drawn(sa);
    assert!(view.expand(1.0).contains_rect(rig.drawn(last)), "{:?} in {view:?}", rig.drawn(last));
}

/// The thumb drags the offset; a press on the track (GTK: jump) moves the thumb there.
#[test]
fn ow2_thumb_drags_and_track_jumps() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let (sa, _rows) = tall(&mut rig, col, 100.0);
    rig.run();
    let r = rig.drawn(sa);
    // hover to show the overlay bar, then press its thumb (at the top) and drag down
    let thumb = pos2(r.right() - 4.0, r.top() + 5.0);
    rig.harness.hover_at(thumb);
    rig.harness.run_steps(2);
    rig.harness.event(Event::PointerButton { pos: thumb, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.step();
    for dy in [10.0, 20.0, 30.0] {
        rig.harness.hover_at(thumb + vec2(0.0, dy));
        rig.harness.step();
    }
    rig.harness.event(Event::PointerButton { pos: thumb + vec2(0.0, 30.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    rig.harness.run_steps(3);
    let y = f(&rig.get(sa, "scroll-y"));
    assert!(y > 50.0, "dragged: {y}");
    // jump to the bottom of the track
    let bottom = pos2(r.right() - 4.0, r.bottom() - 2.0);
    rig.harness.hover_at(bottom);
    rig.harness.step();
    rig.harness.event(Event::PointerButton { pos: bottom, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.step();
    rig.harness.event(Event::PointerButton { pos: bottom, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    rig.harness.run_steps(3);
    assert!(f(&rig.get(sa, "scroll-y")) > y, "jumped to the end");
}

/// VIS1's scrollbar references (VS Code 1.133 Dark/Light Modern, `scrollbarSlider.*`): the thumb's
/// rest, hover and drag colours, and the R15 shadow colour, from the part UA tables.
#[test]
fn ow2_scrollbar_colours_match_vis1() {
    use svelters_egui::paint::StyleView;
    use svelters_egui::provider::StateBits;
    use svelters_egui::widgets::scroll_area::{SHADOW_UA, THUMB_UA};
    let c = |ua, st, dark| StyleView::ua_only(ua, st, dark).color("background-color").unwrap().to_srgba_unmultiplied();
    // within one step per channel (egui keeps colours premultiplied)
    let near = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 1);
    // dark: rgba(121,121,121,.4), rgba(100,100,100,.7), rgba(191,191,191,.4)
    assert!(near(c(THUMB_UA, StateBits::NONE, true), [121, 121, 121, 102]), "{:?}", c(THUMB_UA, StateBits::NONE, true));
    assert!(near(c(THUMB_UA, StateBits::HOVERED, true), [100, 100, 100, 179]), "{:?}", c(THUMB_UA, StateBits::HOVERED, true));
    assert!(near(c(THUMB_UA, StateBits::DRAGGING, true), [191, 191, 191, 102]), "{:?}", c(THUMB_UA, StateBits::DRAGGING, true));
    // light: rgba(100,100,100,.4) rest, rgba(0,0,0,.6) dragged
    assert!(near(c(THUMB_UA, StateBits::NONE, false), [100, 100, 100, 102]), "{:?}", c(THUMB_UA, StateBits::NONE, false));
    assert!(near(c(THUMB_UA, StateBits::DRAGGING, false), [0, 0, 0, 153]), "{:?}", c(THUMB_UA, StateBits::DRAGGING, false));
    // `scrollbar.shadow`
    assert!(near(c(SHADOW_UA, StateBits::NONE, true), [0, 0, 0, 255]));
    assert!(near(c(SHADOW_UA, StateBits::NONE, false), [0xDD, 0xDD, 0xDD, 255]));
}

/// N panes (WIDGET-PARITY §2.17): three panes share the room; `sizes` places them; a sash drag
/// moves only its two neighbours and writes `sizes` and `commit`.
#[test]
fn ow2_split_view_has_n_panes_and_sashes_move_their_neighbours() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let sv = rig.leaf(col, "split-view", &[("height", Value::F64(80.0)), ("width", Value::F64(301.0)), ("align-self", s("start")), ("accessible-name", s("Three"))]);
    let panes: Vec<_> = (0..3).map(|_| rig.leaf(sv, "box", &[])).collect();
    rig.observe(sv, "sizes", true);
    rig.listen(sv, "commit");
    rig.run();
    let w: Vec<f32> = panes.iter().map(|p| rig.backend.with_scene(|sc| sc.n(*p).rect.width())).collect();
    assert_eq!(w.iter().sum::<f32>(), 299.0, "two 1pt sash lines: {w:?}");
    assert!(w.iter().all(|x| (x - 100.0).abs() <= 1.0), "{w:?}");
    rig.set(sv, "sizes", Value::List(vec![Value::F64(50.0), Value::F64(100.0), Value::F64(149.0)]));
    rig.run();
    let x1 = rig.backend.with_scene(|sc| sc.n(panes[1]).rect.left() - sc.n(sv).rect.left());
    assert_eq!(x1, 51.0);
    // drag the second sash (between the 2nd and 3rd panes) 20pt left
    let r = rig.drawn(sv);
    let at = pos2(r.left() + 151.5, r.center().y);
    rig.harness.hover_at(at);
    rig.harness.step();
    rig.harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.step();
    for dx in [-10.0, -20.0] {
        rig.harness.hover_at(at + vec2(dx, 0.0));
        rig.harness.step();
    }
    rig.harness.event(Event::PointerButton { pos: at + vec2(-20.0, 0.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    rig.harness.run_steps(3);
    assert_eq!(rig.get(sv, "sizes"), Value::List(vec![Value::F64(50.0), Value::F64(80.0), Value::F64(169.0)]));
    let log = rig.take();
    let commit = rig.signal_id(sv, "commit");
    assert!(log.events.iter().any(|(h, sig, _)| *h == sv && *sig == commit), "commit sent");
    assert!(log.changed.iter().any(|(h, _)| *h == sv), "sizes announced");
    assert_eq!(rig.harness.query_all_by_role(Role::Splitter).count(), 2, "one splitter per sash");
}

/// Collapsible panes, restore on double click, and the keyboard on a focused sash.
#[test]
fn ow2_split_view_collapses_restores_and_takes_keys() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let sv = rig.leaf(col, "split-view", &[("height", Value::F64(80.0)), ("width", Value::F64(301.0)), ("align-self", s("start")), ("collapsible", Value::Bool(true)), ("divider-position", Value::F64(120.0)), ("accessible-name", s("Two"))]);
    let a = rig.leaf(sv, "box", &[]);
    rig.leaf(sv, "box", &[]);
    rig.run();
    // kittest steps 0.25 s and egui reports a click a frame after its release: a slow double click
    svelters_egui::native::override_policy(&rig.harness.ctx, |p| p.double_click_ms = 1000);
    let mut widths = Vec::new();
    for _ in 0..2 {
        // the sash moves with the pane: find it again
        let sash = rig.harness.get_by_role_and_label(Role::Splitter, "Two").rect().center();
        rig.harness.hover_at(sash);
        rig.harness.step();
        // two clicks two kittest steps (0.5 s) apart: within this test's double-click time
        for _ in 0..2 {
            for pressed in [true, false] {
                rig.harness.event(Event::PointerButton { pos: sash, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            }
            rig.harness.step();
        }
        rig.harness.run_steps(3);
        widths.push(rig.backend.with_scene(|sc| sc.n(a).rect.width()));
    }
    assert_eq!(widths, vec![0.0, 120.0], "collapsed, then restored");
    rig.harness.get_by_role_and_label(Role::Splitter, "Two").focus();
    rig.harness.step();
    rig.harness.key_press(egui::Key::ArrowRight);
    rig.harness.run_steps(3);
    assert_eq!(f(&rig.get(sv, "divider-position")), 130.0);
    rig.harness.key_press_modifiers(Modifiers::SHIFT, egui::Key::ArrowLeft);
    rig.harness.run_steps(3);
    assert_eq!(f(&rig.get(sv, "divider-position")), 80.0);
    rig.harness.key_press(egui::Key::Home);
    rig.harness.run_steps(3);
    assert_eq!(f(&rig.get(sv, "divider-position")), 32.0, "min-size");
}

/// The standalone `scrollbar` (WIDGET-PARITY §2.11): the wheel moves `value`.
#[test]
fn ow2_scrollbar_kind_moves_its_value() {
    let mut rig = setup();
    let (_w, col) = rig.window();
    let sb = rig.leaf(col, "scrollbar", &[("height", Value::F64(100.0)), ("align-self", s("start")), ("max", Value::F64(400.0)), ("page", Value::F64(100.0)), ("accessible-name", s("Rows"))]);
    rig.listen(sb, "changed");
    rig.run();
    assert!(rig.backend.with_scene(|sc| matches!(sc.n(sb).w, W::Ext(_))));
    assert_eq!(rig.drawn(sb).width(), 10.0, "scrollbar-w");
    let c = rig.drawn(sb).center();
    wheel(&mut rig, c, vec2(0.0, -40.0), Modifiers::NONE);
    let v = f(&rig.get(sb, "value"));
    assert!(v > 10.0, "{v}");
    let node = rig.harness.get_by_role_and_label(Role::ScrollBar, "Rows");
    assert_eq!(node.accesskit_node().data().max_numeric_value(), Some(400.0));
    let log = rig.take();
    let changed = rig.signal_id(sb, "changed");
    assert!(log.events.iter().any(|(h, sig, a)| *h == sb && *sig == changed && a.len() == 1));
}

/// The window's own chrome: its background is the theme's `bg`, no egui `CentralPanel`.
#[test]
fn ow2_window_paints_its_background() {
    let mut rig = setup();
    rig.window();
    rig.run();
    let dark = rig.harness.ctx.theme() == egui::Theme::Dark;
    let bg = svelters_egui::paint::StyleView::ua_only(svelters_egui::widgets::WINDOW_UA, svelters_egui::provider::StateBits::NONE, dark).color("background-color").unwrap();
    let screen = rig.harness.ctx.content_rect();
    let shapes = rig.harness.output().shapes.clone();
    // (the harness's own root frame keeps an 8pt margin)
    let found = shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Rect(r) if r.fill == bg && r.rect.width() >= screen.width() - 16.5 && r.rect.height() >= screen.height() - 16.5));
    let rects: Vec<_> = shapes.iter().filter_map(|c| if let egui::Shape::Rect(r) = &c.shape { Some((r.rect, r.fill)) } else { None }).take(5).collect();
    assert!(found, "a full-window rect in {bg:?} over {screen:?}: {rects:?}");
}

