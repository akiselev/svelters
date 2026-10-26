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
    // skelter's own rule (`with_pass_every_step(false)`): an idle island is not due
    let mut h = EguiHarness::builder().with_size(egui::vec2(400.0, 300.0)).with_pass_every_step(false).build_ui_state(ui, S::default());
    assert_eq!(h.busy(), None);
    let before = h.doc().passes();
    h.run_steps(5);
    assert_eq!(h.doc().passes(), before);
    // kittest parity (the default): one pass per step
    let mut h = harness();
    let before = h.doc().passes();
    h.run_steps(5);
    assert_eq!(h.doc().passes(), before + 5);
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

#[test]
fn a_repaint_requested_from_another_thread_shows_after_one_run() {
    use std::sync::{Arc, Mutex};
    let inbox: Arc<Mutex<Option<String>>> = Arc::default();
    let shown = inbox.clone();
    // skelter's own rule, so only the repaint callback can make the island due
    let mut h = EguiHarness::builder().with_size(egui::vec2(200.0, 100.0)).with_pass_every_step(false).build_ui(move |ui| {
        let text = shown.lock().unwrap().clone().unwrap_or_else(|| "waiting".into());
        ui.label(text);
    });
    assert!(h.query_by_label("waiting").is_some());
    let ctx = h.ctx.clone();
    let sender = inbox.clone();
    std::thread::spawn(move || {
        *sender.lock().unwrap() = Some("job done".into());
        ctx.request_repaint();
    })
    .join()
    .unwrap();
    let before = h.doc().passes();
    h.run();
    assert!(h.doc().passes() > before, "the out-of-pass repaint ran a pass");
    assert!(h.query_by_label("job done").is_some());
}

#[test]
fn step_and_run_always_run_a_pass() {
    let mut h = harness();
    let p = h.doc().passes();
    h.step();
    assert_eq!(h.doc().passes(), p + 1, "a step with nothing due still runs one pass");
    h.run();
    assert!(h.doc().passes() >= p + 2);
}

#[test]
#[should_panic(expected = "lack accessible name")]
fn the_build_runs_the_accessibility_check() {
    EguiHarness::builder().build_ui(|ui| {
        ui.add(egui::TextEdit::singleline(&mut String::new()));
    });
}

#[test]
fn the_build_skips_the_check_when_told() {
    EguiHarness::builder().with_accessibility_check(false).build_ui(|ui| {
        ui.add(egui::TextEdit::singleline(&mut String::new()));
    });
}

#[test]
fn state_changed_through_a_shared_handle_shows_after_run() {
    use std::cell::Cell;
    use std::rc::Rc;
    let count = Rc::new(Cell::new(0));
    let read = count.clone();
    let mut h = EguiHarness::builder().with_size(egui::vec2(200.0, 100.0)).build_ui(move |ui| {
        ui.label(format!("Count: {}", read.get()));
    });
    assert!(h.query_by_label("Count: 0").is_some());
    // no input, no repaint request: kittest's parity pass still reads the new state
    count.set(7);
    h.run();
    assert!(h.query_by_label("Count: 7").is_some());
    // settled although the island requested nothing: run returns after its step
    assert_eq!(h.busy(), None);
}

#[test]
fn a_ctrl_click_is_seen_with_ctrl_held_and_f2_then_typing_edits() {
    #[derive(Default)]
    struct E {
        ctrl_clicks: u32,
        editing: bool,
        text: String,
    }
    let mut h = EguiHarness::builder().with_size(egui::vec2(300.0, 200.0)).build_ui_state(
        |ui, s: &mut E| {
            if ui.button("Pick").clicked() && ui.input(|i| i.modifiers.ctrl) {
                s.ctrl_clicks += 1;
            }
            if ui.input(|i| i.key_pressed(egui::Key::F2)) {
                s.editing = true;
            }
            if s.editing {
                let r = ui.add(egui::TextEdit::singleline(&mut s.text).hint_text("Name"));
                if !r.has_focus() && !r.lost_focus() {
                    r.request_focus();
                }
            }
        },
        E::default(),
    );
    h.get_by_label("Pick").click_modifiers(egui::Modifiers::CTRL);
    h.run();
    assert_eq!(h.state().ctrl_clicks, 1, "the press saw the modifier change before it");
    // F2 shows and focuses the field in its own pass; the typing that follows goes into it
    h.key_press(egui::Key::F2);
    h.type_text("ab");
    h.run();
    assert_eq!(h.state().text, "ab");
}

/// A5c: an effect drawn above the button that causes it shows only in the pass after the click;
/// egui asks for that pass (repaint delay 0, at the next frame's time), and `run` waits for it.
/// `run` counts the steps the passes took.
#[test]
fn run_waits_for_the_pass_a_click_asks_for() {
    fn panel_above(ui: &mut egui::Ui, s: &mut S) {
        if s.on {
            ui.label("Problems: 3");
        }
        if ui.button("Show problems").clicked() {
            s.on = true;
        }
    }
    let mut h = EguiHarness::builder().with_size(egui::vec2(400.0, 300.0)).build_ui_state(panel_above, S::default());
    let p = h.doc().passes();
    h.get_by_label("Show problems").click();
    let n = h.run();
    assert!(h.state().on);
    assert!(h.query_by_label("Problems: 3").is_some(), "the click's effect is drawn when run returns");
    let passes = h.doc().passes() - p;
    assert!(passes >= 2, "the click's pass and the one it asked for: {passes}");
    assert_eq!(n, passes, "run counts a step per pass");
}
