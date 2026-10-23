//! OW1a3 (plans/milestones/OW1a3-basic-widgets-3.md): the own label by default (RT2 runs,
//! selection), remeasure on restyle, `::part()` on the indicator kinds, the public focus-stop API.
//! Headless (kittest, no JS: `common::Rig`).

mod common;

use common::Rig;
use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use svelters_abi::*;
use svelters_egui::provider::StateBits;
use svelters_egui::scene::W;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn own(rig: &Rig, h: svl_handle) -> bool {
    rig.backend.with_scene(|s| matches!(s.n(h).w, W::Ext(_)))
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

fn changed(rig: &Rig, h: svl_handle, name: &str) -> usize {
    let p = rig.prop_id(h, name);
    rig.log.borrow().changed.iter().filter(|c| c.0 == h && c.1 == p).count()
}

fn drag(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    rig.harness.hover_at(from);
    rig.harness.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.step();
    rig.harness.hover_at(to);
    rig.harness.step();
    rig.harness.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    rig.run();
}

#[test]
fn ow1a3_label_is_own_by_default() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", s("Hello"))]);
    rig.run();
    assert!(own(&rig, l), "label is in DEFAULT_OWN");
    assert_eq!(rig.backend.with_scene(|s| s.n(l).text().to_string()), "Hello");
}

#[test]
fn ow1a3_labels_are_not_selectable_by_default() {
    // the theme's egui style: app chrome's egui labels are not selectable (cadatomic)
    let style = svelters_egui::theme::Theme::default().egui_style();
    assert!(!style.interaction.selectable_labels);
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", s("Chrome text that is not selectable"))]);
    rig.observe(l, "selection-end", true);
    rig.run();
    let r = rig.drawn(l);
    drag(&mut rig, r.left_center() + vec2(2.0, 0.0), r.right_center() - vec2(2.0, 0.0));
    assert_eq!(changed(&rig, l, "selection-end"), 0, "no selection");
    assert!(!state(&rig, l).contains(StateBits::READ_ONLY));
    assert_eq!(rig.get(l, "selection-end"), Value::I64(0));
}

#[test]
fn ow1a3_a_selectable_label_selects_copies_and_binds_its_selection() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let text = "Selectable read-only text";
    let l = rig.leaf(col, "label", &[("text", s(text)), ("selectable", Value::Bool(true))]);
    rig.observe(l, "selection-start", true);
    rig.observe(l, "selection-end", true);
    rig.run();
    assert!(state(&rig, l).contains(StateBits::READ_ONLY), ":read-only");
    // pointer: drag across the whole text selects it, and `bind:` hears it
    let r = rig.drawn(l);
    drag(&mut rig, pos2(r.left() + 0.5, r.center().y), pos2(r.right() + 20.0, r.center().y));
    assert_eq!(rig.get(l, "selection-start"), Value::I64(0));
    assert_eq!(rig.get(l, "selection-end"), Value::I64(text.chars().count() as i64));
    assert!(changed(&rig, l, "selection-end") >= 1, "the selection is announced");
    // Ctrl+C (egui's Copy event) copies the selection
    rig.harness.event(Event::Copy);
    rig.harness.step();
    let copied = rig.harness.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == text));
    assert!(copied, "{:?}", rig.harness.output().platform_output.commands);
    // JS sets the selection: chars 0..10 ("Selectable")
    rig.set(l, "selection-start", Value::I64(0));
    rig.set(l, "selection-end", Value::I64(10));
    rig.run();
    rig.harness.event(Event::Copy);
    rig.harness.step();
    let copied = rig.harness.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "Selectable"));
    assert!(copied, "a JS selection: {:?}", rig.harness.output().platform_output.commands);
}

#[test]
fn ow1a3_a_rich_own_label_runs_restyle_and_remeasure() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[]);
    rig.leaf(l, "span", &[("text", s("Hello "))]);
    let b = rig.leaf(l, "b", &[("text", s("bold"))]);
    rig.run();
    assert!(own(&rig, l));
    assert_eq!(rig.backend.with_scene(|s| s.n(l).text().to_string()), "Hello bold", "the text mirror is all runs");
    let before = rig.drawn(l).width();
    rig.set(b, "text", s("bolder and longer"));
    rig.run();
    assert!(rig.drawn(l).width() > before + 20.0, "a run's text remeasures the own label");
}

#[test]
fn ow1a3_own_leaves_remeasure_when_css_changes_their_font_or_padding() {
    let mut rig = Rig::new();
    rig.backend.add_sheet("app", ".big { font-size: 30px; } .roomy { padding: 20px; }");
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", s("Grows with its font"))]);
    let b = rig.leaf(col, "button", &[("text", s("Pad me"))]);
    rig.run();
    assert!(own(&rig, l) && own(&rig, b));
    let (l0, b0) = (rig.drawn(l), rig.drawn(b));
    rig.backend.set_classes(l, "big").unwrap();
    rig.backend.set_classes(b, "roomy").unwrap();
    rig.run();
    let (l1, b1) = (rig.drawn(l), rig.drawn(b));
    assert!(l1.height() > l0.height() + 10.0 && l1.width() > l0.width() * 1.5, "font-size: {l0:?} -> {l1:?}");
    assert!(b1.height() >= b0.height() + 20.0, "padding: {b0:?} -> {b1:?}");
    // and back
    rig.backend.set_classes(l, "").unwrap();
    rig.run();
    assert!((rig.drawn(l).height() - l0.height()).abs() < 1.0, "font-size back: {:?}", rig.drawn(l));
}

/// The fill of `h`'s `part` (of `kind`) in the last frame's paint trace.
fn part_fill(rig: &Rig, h: svl_handle, part: &str, kind: svelters_egui::paint::TraceKind) -> Option<egui::Color32> {
    let n = rig.backend.with_scene(|s| s.n(h).handle.to_raw());
    svelters_egui::paint::trace::last(&rig.harness.ctx).find(n, Some(part), kind).map(|e| e.fill)
}

/// An author `::part()` rule reaches each indicator kind's part (OW-CSS stage 2 through
/// `parts::part_of`); the UA tables stay the UA layer under it.
#[test]
fn ow1a3_part_rules_paint_the_indicator_kinds() {
    use svelters_egui::paint::TraceKind::{Box as B, Path as P};
    let red = egui::Color32::from_rgb(255, 0, 0);
    type Case<'a> = (&'a str, &'a [(&'a str, Value)], &'a str, svelters_egui::paint::TraceKind, &'a str);
    let cases: &[Case] = &[
        ("checkbox", &[("text", Value::Str("Check".into())), ("checked", Value::Bool(true))], "mark", P, "color"),
        ("checkbox", &[("text", Value::Str("Check".into()))], "indicator", B, "background-color"),
        ("switch", &[("text", Value::Str("Switch".into())), ("active", Value::Bool(true))], "knob", P, "color"),
        ("radio-button", &[("text", Value::Str("Radio".into())), ("checked", Value::Bool(true))], "dot", P, "color"),
        ("progress-bar", &[("fraction", Value::F64(0.5))], "fill", P, "background-color"),
        ("progress-bar", &[("fraction", Value::F64(0.5))], "track", B, "background-color"),
        ("frame", &[("label", Value::Str("Group".into())), ("checkable", Value::Bool(true))], "indicator", B, "background-color"),
        ("collapsing-header", &[("text", Value::Str("Section".into()))], "header", B, "background-color"),
        ("collapsing-header", &[("text", Value::Str("Section".into()))], "twistie", P, "color"),
    ];
    for (tag, props, part, kind, prop) in cases {
        let mut rig = Rig::new();
        svelters_egui::paint::trace::enable(&rig.harness.ctx, true);
        let (_w, col) = rig.window();
        let h = rig.leaf(col, tag, props);
        rig.run();
        assert!(own(&rig, h), "{tag}");
        let before = part_fill(&rig, h, part, *kind);
        assert!(before.is_some() && before != Some(red), "{tag}::part({part}) painted by its UA table: {before:?}");
        let d = rig.backend.add_sheet("author", &format!("{tag}::part({part}) {{ {prop}: #ff0000; }}"));
        assert!(d.is_empty(), "{d:?}");
        rig.run();
        assert_eq!(part_fill(&rig, h, part, *kind), Some(red), "{tag}::part({part}) {{ {prop} }}");
    }
}

use svelters_egui::provider::{AvailableSpace, FocusStop, MeasureCx, Widget, WidgetCx};

/// A test-only provider kind with three focusable parts (it replaces `menu-separator` in this
/// test binary only): two tab stops with the role `Tab`, and one that Tab skips.
#[svelters::widget(namespace = "t", replaces = "menu-separator")]
pub struct ThreeStops {}

impl Widget for ThreeStops {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(90.0, 20.0)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let r = cx.rect();
        for (k, name) in ["one", "two", "three"].into_iter().enumerate() {
            let pr = egui::Rect::from_min_size(r.min + vec2(30.0 * k as f32, 0.0), vec2(30.0, r.height()));
            let resp = ui.interact(pr, cx.id().with(name), egui::Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Unknown, true, name));
            if resp.clicked() {
                resp.request_focus();
            }
            cx.focus_stop(ui, &resp, FocusStop::new(egui::accesskit::Role::Tab).tab_stop(k < 2));
        }
        ui.interact(r, cx.id(), egui::Sense::hover())
    }
}

#[test]
fn ow1a3_a_provider_kind_registers_focus_stops_with_a_role() {
    use egui_kittest::kittest::Queryable;
    let mut rig = Rig::new();
    rig.backend.set_own_widgets(svelters_egui::widgets::KindSet::All);
    let (_w, col) = rig.window();
    let h = rig.leaf(col, "menu-separator", &[]);
    rig.run();
    assert!(own(&rig, h));
    let focused = |rig: &Rig| ["one", "two", "three"].into_iter().find(|n| rig.harness.get_by_role_and_label(egui::accesskit::Role::Tab, n).is_focused());
    rig.harness.key_press(egui::Key::Tab);
    rig.run();
    assert_eq!(focused(&rig), Some("one"), "Tab reaches the first stop, an AccessKit Tab");
    rig.harness.key_press(egui::Key::Tab);
    rig.run();
    assert_eq!(focused(&rig), Some("two"));
    // a click focuses a stop Tab skips (Tab skips it in EV3's scoped chains: dialogs, scopes)
    let three = rig.harness.get_by_role_and_label(egui::accesskit::Role::Tab, "three").rect();
    let at = three.center();
    rig.harness.hover_at(at);
    for pressed in [true, false] {
        rig.harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    rig.run();
    assert_eq!(focused(&rig), Some("three"));
}

/// Pressable `repeat` runs for as long as the button is held, past egui's 0.8 s click cutoff
/// (kittest's virtual clock: 0.25 s a step; 0.3 s delay, then every 0.1 s, at most one a pass).
#[test]
fn ow1a3_a_held_repeat_button_repeats_past_the_click_cutoff() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let b = rig.leaf(col, "button", &[("text", s("+")), ("repeat", Value::Bool(true))]);
    rig.listen(b, "clicked");
    rig.run();
    assert!(own(&rig, b));
    let pos = rig.drawn(b).center();
    rig.harness.hover_at(pos);
    rig.harness.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    for _ in 0..12 {
        rig.harness.step();
    }
    let clicked = rig.signal_id(b, "clicked");
    let held = rig.log.borrow().events.iter().filter(|e| e.0 == b && e.1 == clicked).count();
    assert!(held >= 9, "3 s held: {held} repeats");
    assert!(state(&rig, b).contains(StateBits::PRESSED), ":active while held");
    rig.harness.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    rig.run();
    let after = rig.log.borrow().events.iter().filter(|e| e.0 == b && e.1 == clicked).count();
    assert!(after <= held + 1, "the release adds at most its click: {held} -> {after}");
    assert!(!state(&rig, b).contains(StateBits::PRESSED));
}

/// DESIGN R3 in the theme's base sheet: a standalone control eases its colours over 100 ms when
/// motion is allowed (`@media (prefers-reduced-motion: no-preference)`), R2: not on `:active`.
#[test]
fn ow1a3_the_base_sheet_eases_hover_over_100ms_unless_motion_is_reduced() {
    for reduced in [false, true] {
        let mut rig = Rig::new();
        rig.backend.set_theme(&svelters_egui::theme::Theme::dark_modern());
        rig.backend.set_motion(svelters_egui::anim::MotionConfig { slow: 1.0, reduced });
        let (_w, col) = rig.window();
        let b = rig.leaf(col, "button", &[("text", s("Hover me"))]);
        rig.run();
        let dur = |rig: &Rig| rig.backend.computed_style(b).and_then(|c| c.get("transition").map(|v| format!("{v:?}")));
        let d = dur(&rig);
        if reduced {
            assert!(d.as_deref().is_none_or(|d| !d.contains("0.1")), "reduced motion: no ease: {d:?}");
        } else {
            assert!(d.as_deref().is_some_and(|d| d.contains("0.1")), "100 ms: {d:?}");
        }
        // painted: the ease's start in the hover frame, its end a step (0.25 s) later
        svelters_egui::paint::trace::enable(&rig.harness.ctx, true);
        rig.run();
        let n = rig.backend.with_scene(|s| s.n(b).handle.to_raw());
        let fill = |rig: &Rig| svelters_egui::paint::trace::last(&rig.harness.ctx).find(n, None, svelters_egui::paint::TraceKind::Box).map(|e| e.fill);
        let rest = fill(&rig);
        rig.harness.hover_at(rig.drawn(b).center());
        rig.harness.step();
        let first = fill(&rig);
        rig.run();
        let end = fill(&rig);
        assert_ne!(rest, end, "hover changes the fill");
        assert_eq!(first == end, reduced, "reduced {reduced}: rest {rest:?}, hover frame {first:?}, end {end:?}");
    }
}
