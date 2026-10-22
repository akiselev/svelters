//! CX3 (plans/milestones/CX3-bind-targets.md): bind targets end to end on egui, compiled live from
//! `fixtures/motion-egui/cx3`: `bind:text|commit` updates `$state` only on Enter or blur, a draft
//! bind source routes values through its `get`/`set`, and `bind:` on a Rust store field is a native
//! link (no JS per change, both ways).

use egui_kittest::kittest::{NodeT, Queryable};
use svelters_egui::harness::TestApp;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/cx3/src").join(name)
}

fn app(name: &str) -> TestApp {
    let mut app = TestApp::from_source(fixture(name)).unwrap();
    app.run();
    assert!(app.errors().is_empty(), "{:?}", app.errors());
    app
}

/// The text of the label that starts with `prefix`.
fn label(app: &TestApp, prefix: &str) -> String {
    let n = app.harness.get_by_label_contains(prefix);
    let a = n.accesskit_node();
    a.value().or_else(|| a.label()).unwrap_or_default().to_string()
}

/// The text in field `name`.
fn field(app: &TestApp, name: &str) -> String {
    app.harness.get_by_role_and_label(egui::accesskit::Role::TextInput, name).accesskit_node().value().unwrap_or_default().to_string()
}

fn click(app: &mut TestApp, name: &str) {
    let pos = app.harness.get_by_label(name).rect().center();
    app.harness.hover_at(pos);
    for pressed in [true, false] {
        app.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    app.run();
}

/// Focuses field `name`, selects everything and types `text` (one frame of input).
fn type_into(app: &mut TestApp, name: &str, text: &str) {
    click(app, name);
    app.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    app.harness.event(egui::Event::Text(text.to_string()));
    app.run();
}

fn enter(app: &mut TestApp) {
    app.harness.key_press(egui::Key::Enter);
    app.run();
}

#[test]
fn commit_updates_state_on_enter_or_blur_only() {
    let mut app = app("Commit.svelte");
    assert_eq!(field(&app, "name"), "Ann");
    type_into(&mut app, "name", "Ada");
    assert_eq!(field(&app, "name"), "Ada");
    assert_eq!(label(&app, "Name:"), "Name: Ann (0)", "typing does not reach $state");
    enter(&mut app);
    assert_eq!(label(&app, "Name:"), "Name: Ada (1)", "Enter commits");
    // an edit, then focus moves away: blur commits
    type_into(&mut app, "name", "Bob");
    assert_eq!(label(&app, "Name:"), "Name: Ada (1)");
    click(&mut app, "title");
    assert_eq!(label(&app, "Name:"), "Name: Bob (2)", "blur after an edit commits");
    // blur without an edit commits nothing
    click(&mut app, "name");
    click(&mut app, "title");
    assert_eq!(label(&app, "Name:"), "Name: Bob (2)");
    // Enter taken by the scope's default button: the field commits first, the handler reads it
    type_into(&mut app, "title", "X");
    assert_eq!(label(&app, "Result:"), "Result: none");
    enter(&mut app);
    assert_eq!(label(&app, "Result:"), "Result: ok:X", "{:?}", app.errors());
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}

#[test]
fn draft_source_reads_get_and_writes_set() {
    let mut app = app("Draft.svelte");
    assert_eq!(field(&app, "depth"), "5", "the node shows get(path)");
    assert_eq!(field(&app, "width"), "1");
    type_into(&mut app, "depth", "7");
    assert_eq!(label(&app, "Log:"), "Log: ", "|commit: no draft event while typing");
    enter(&mut app);
    assert_eq!(label(&app, "Log:"), "Log: depth=7");
    type_into(&mut app, "width", "3");
    assert_eq!(label(&app, "Log:"), "Log: depth=7 width=3", "without |commit every change is a draft event");
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}

/// The Rust store `Link.svelte` binds (`svelters:app/cx3`, JS `form`).
#[svelters::store(module = "cx3")]
pub struct Form {
    #[store(writable)]
    pub title: String,
    #[store(writable)]
    pub speed: f64,
}

impl svelters::StoreHooks for Form {
    /// Normalizes: a linked write comes back to the node once.
    fn on_write(&mut self, field: FormField) -> Result<(), svelters::NativeError> {
        if field == FormField::Title {
            self.title = self.title.to_uppercase();
        }
        Ok(())
    }
}

#[svelters::export(module = "cx3")]
pub static FORM: svelters::StoreStatic<Form> = svelters::StoreStatic::with(|| Form { title: "Start".into(), speed: 2.0 });

#[test]
fn store_fields_link_natively_without_js() {
    let mut app = app("Link.svelte");
    assert_eq!(field(&app, "title"), "Start", "the store's value at link time");
    let before = app.host().reactive_stats();
    type_into(&mut app, "title", "abc");
    for _ in 0..3 {
        app.run();
    }
    assert_eq!(FORM.get(|f| f.title.clone()), "ABC", "the edit reached the store, through on_write");
    assert_eq!(field(&app, "title"), "ABC", "the normalized value came back to the node");
    // a Rust write reaches the node
    FORM.write(FormField::Speed, |f| f.speed = 7.5);
    FORM.write(FormField::Title, |f| f.title = "FROM RUST".into());
    for _ in 0..3 {
        app.run();
    }
    assert_eq!(field(&app, "title"), "FROM RUST");
    let after = app.host().reactive_stats();
    assert!(after.link_writes >= before.link_writes + 4, "{before:?} -> {after:?}");
    assert_eq!(after.store_batches, before.store_batches, "no change batch reached JS: {after:?}");
    assert_eq!(after.store_delivered, before.store_delivered, "{after:?}");
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}
