//! `EguiHarness`: egui closures in a skelter egui island, driven with kittest's verbs through skelter's
//! input path. No GPU: passes, input and AccessKit work without a device.

use skelter_test::{EguiHarness, Role};
use skelter_base::accesskit::TreeId;

#[derive(Default)]
struct S {
    clicks: u32,
    on: bool,
    text: String,
}

fn ui(ui: &mut egui::Ui, s: &mut S) {
    if ui.button("Click me").clicked() {
        s.clicks += 1;
    }
    ui.label(format!("Clicks: {}", s.clicks));
    ui.checkbox(&mut s.on, "Enabled");
    ui.add(egui::TextEdit::singleline(&mut s.text).hint_text("Name"));
}

fn harness() -> EguiHarness<S> {
    EguiHarness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(ui, S::default())
}

#[test]
fn queries_descend_into_the_island() {
    let h = harness();
    let b = h.get_by_role_and_label(Role::Button, "Click me");
    assert_ne!(b.tree(), TreeId::ROOT, "the button is in the island's subtree");
    let r = b.rect();
    assert!(r.min.x >= 0.0 && r.max.x <= 400.0 && r.max.y <= 300.0, "{r:?}");
    assert!(r.width() > 0.0 && r.height() > 0.0);
    assert!(h.query_by_label("Clicks: 0").is_some());
    // The host's graft node holds the island's root.
    let graft = h.root().children().next().expect("the graft node");
    assert_ne!(graft.children().next().expect("the island root").tree(), TreeId::ROOT);
}

#[test]
fn a_click_reaches_egui_through_skelter_input() {
    let mut h = harness();
    h.get_by_label("Click me").click();
    h.run();
    assert_eq!(h.state().clicks, 1);
    assert!(h.query_by_label("Clicks: 1").is_some());
}

#[test]
fn accesskit_actions_reach_the_island() {
    let mut h = harness();
    h.get_by_label("Enabled").click_accesskit();
    h.run();
    assert!(h.state().on);
    h.get_by_label("Click me").click_accesskit();
    h.run();
    assert_eq!(h.state().clicks, 1);
}

#[test]
fn typing_and_keys_reach_a_focused_text_edit() {
    let mut h = harness();
    h.get_by_role(Role::TextInput).click();
    h.run();
    h.event(egui::Event::Text("abc".into()));
    h.run();
    assert_eq!(h.state().text, "abc");
    h.key_press(egui::Key::Backspace);
    h.run();
    assert_eq!(h.state().text, "ab");
    h.event(egui::Event::Paste("xy".into()));
    h.run();
    assert_eq!(h.state().text, "abxy");
}

#[test]
fn modifier_clicks_and_hover_take_egui_types() {
    let mut h = harness();
    let center = h.get_by_label("Click me").rect().center();
    h.hover_at(center);
    h.run();
    h.get_by_label("Click me").click_modifiers(egui::Modifiers::SHIFT);
    h.run();
    assert_eq!(h.state().clicks, 1);
    h.drag_at(center);
    h.drop_at(center);
    h.run();
    assert_eq!(h.state().clicks, 2, "a press and release on the button is a click");
}

#[test]
fn an_idle_island_runs_no_passes() {
    let mut h = harness();
    assert_eq!(h.busy(), None);
    let before = h.doc().passes();
    h.run_steps(5);
    assert_eq!(h.doc().passes(), before);
}

#[test]
fn the_context_and_state_are_kittests() {
    let mut h = EguiHarness::builder()
        .with_context_setup(|ctx| ctx.set_zoom_factor(1.0))
        .build_ui_state(
            |ui, n: &mut u32| {
                if ui.button("Add").clicked() {
                    *n += 1;
                }
            },
            0u32,
        );
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    h.get_by_label("Add").click();
    h.run();
    *h.state_mut() += 10;
    assert_eq!(*h.state(), 11);
    h.check_accessibility();
}

#[test]
fn state_changed_between_steps_shows_after_run() {
    let mut h = harness();
    h.state_mut().clicks = 5;
    h.run();
    assert!(h.query_by_label("Clicks: 5").is_some(), "{}", h.a11y_dump());
}
