//! WG2a: the gallery page of the menu family (`fixtures/gallery-egui-menus`, compiled by
//! `pnpm build`) end to end on the QuickJS host under kittest: the compiler's `attaches_to`,
//! `bind:checked` through `toggled`, `bind:open`/`bind:active`, shortcuts, and the overlays.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use svelters_egui::harness::TestApp;

fn bundle() -> std::path::PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gallery-egui-menus/dist/app.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

/// The status label's text (`S:...`).
fn status(app: &TestApp) -> String {
    app.harness.query_by_label_contains("S:").and_then(|n| n.label()).unwrap_or_default()
}

fn click(app: &mut TestApp, label: &str, button: egui::PointerButton) {
    let pos = app.harness.get_by_label(label).rect().center();
    app.harness.hover_at(pos);
    for pressed in [true, false] {
        app.harness.event(egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::NONE });
    }
    app.run();
}

#[test]
fn gallery_page_menus_and_popups() {
    let mut app = TestApp::load(bundle()).unwrap();
    app.run();
    assert_eq!(status(&app), "S:|false|true|false|false||true");
    assert!(app.harness.query_by_label("Selection: none").is_some(), "the inner window is open");

    // menu bar: an action, a check item (bind:checked), a radio pick
    click(&mut app, "File", egui::PointerButton::Primary);
    click(&mut app, "Open", egui::PointerButton::Primary);
    click(&mut app, "File", egui::PointerButton::Primary);
    click(&mut app, "Autosave", egui::PointerButton::Primary);
    click(&mut app, "Size", egui::PointerButton::Primary);
    click(&mut app, "Large", egui::PointerButton::Primary);
    assert_eq!(status(&app), "S:open;|true|false|true|false||true");
    // the accelerator
    app.harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Q);
    app.run();
    assert!(status(&app).starts_with("S:open;quit;|"), "{}", status(&app));

    // menu-button: bind:active follows the user
    click(&mut app, "More", egui::PointerButton::Primary);
    assert!(status(&app).ends_with("|true||true"), "{}", status(&app));
    click(&mut app, "Paste", egui::PointerButton::Primary);
    assert!(status(&app).contains("paste;") && status(&app).ends_with("|false||true"), "{}", status(&app));

    // popup attached to a button, opened by the app, typed into
    click(&mut app, "Pick", egui::PointerButton::Primary);
    app.harness.get_by_role_and_label(Role::TextInput, "Filter text").click();
    app.run();
    app.harness.get_by_role_and_label(Role::TextInput, "Filter text").type_text("ab");
    app.run();
    assert!(status(&app).ends_with("|ab|true"), "{}", status(&app));
    app.harness.key_press(egui::Key::Escape);
    app.run();
    assert!(app.harness.query_by_label("Filter text").is_none(), "Escape closed it (bind:open)");

    // modal dialog
    click(&mut app, "Delete", egui::PointerButton::Primary);
    assert!(app.harness.query_by_label("It cannot be undone.").is_some());
    click(&mut app, "Confirm", egui::PointerButton::Primary);
    assert!(app.harness.query_by_label("It cannot be undone.").is_none());
    assert!(status(&app).contains("deleted;"), "{}", status(&app));

    // context menu on a button hides the inner window
    click(&mut app, "Show inspector", egui::PointerButton::Secondary);
    click(&mut app, "Hide inspector", egui::PointerButton::Primary);
    assert!(app.harness.query_by_label("Selection: none").is_none());
    assert!(status(&app).ends_with("|false"), "{}", status(&app));

    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}
