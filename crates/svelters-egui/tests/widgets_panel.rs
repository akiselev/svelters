//! WG1: the SC1 widget kinds together, compiled from Svelte (`fixtures/embed-egui/src/TaskPanel.svelte`,
//! a task-panel-shaped form) and mounted in an embedded slot: every field found by role and label,
//! and each binding (`bind:text`, `bind:selected`, `bind:open`, `bind:checked`, a radio group,
//! an icon button) driving `$state` that a summary label echoes. Bundle:
//! `fixtures/embed-egui/dist/embed.js` (`pnpm build`). Headless kittest.

use egui::accesskit::{Invalid, Role};
use egui_kittest::kittest::{NodeT, Queryable};
use std::path::PathBuf;
use svelters_egui::embed::{EmbedConfig, MountSpec, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;

fn bundle() -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui/dist/embed.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

fn summary(eh: &EmbedHarness) -> String {
    let n = eh.harness.get_by_label_contains("Summary:");
    let a = n.accesskit_node();
    a.value().or_else(|| a.label()).unwrap_or_default().to_string()
}

#[test]
fn task_panel_form_drives_state_through_every_widget() {
    svelters_egui::fonts::register_icons("icons", [("gear", "*")]);
    let cfg = EmbedConfig {
        id_salt: "wg1",
        host: HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(420.0, 560.0), cfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let slot = e.create_slot("panel.task", SlotOptions::fill().label("Task"));
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    e.mount(MountSpec::new(entry, "TaskPanel", slot, "task")).unwrap();
    eh.run();
    let s = summary(&eh);
    assert!(s.contains("10 mm x  valid false"), "{s}");

    // every input by role and label
    eh.harness.get_by_role_and_label(Role::TextInput, "Width");
    let height = eh.harness.get_by_role_and_label(Role::TextInput, "Height");
    assert_eq!(height.accesskit_node().data().invalid(), Some(Invalid::True), "the empty height is invalid");
    eh.harness.get_by_role_and_label(Role::ComboBox, "Plane");
    for d in ["+X", "+Y", "+Z"] {
        eh.harness.get_by_role_and_label(Role::RadioButton, d);
    }
    eh.harness.get_by_role_and_label(Role::DisclosureTriangle, "Placement");
    eh.harness.get_by_role_and_label(Role::Button, "Settings");
    eh.harness.get_by_role_and_label(Role::Button, "Snap");
    assert!(eh.harness.query_by_label("Offset").is_none(), "Placement starts collapsed");

    // bind:text + a derived `invalid`
    eh.harness.get_by_label("Height").type_text("20 mm");
    eh.run();
    assert!(summary(&eh).contains("x 20 mm valid true"), "{}", summary(&eh));
    assert_eq!(eh.harness.get_by_label("Height").accesskit_node().data().invalid(), None);

    // a segmented radio group
    eh.harness.get_by_role_and_label(Role::RadioButton, "+X").click();
    eh.run();
    assert!(summary(&eh).contains("dir +X"), "{}", summary(&eh));
    let checked: Vec<bool> = ["+X", "+Y", "+Z"]
        .iter()
        .map(|d| eh.harness.get_by_role_and_label(Role::RadioButton, d).accesskit_node().data().toggled() == Some(egui::accesskit::Toggled::True))
        .collect();
    assert_eq!(checked, vec![true, false, false]);

    // bind:selected
    eh.harness.get_by_role_and_label(Role::ComboBox, "Plane").click();
    eh.run();
    eh.harness.get_by_label("ZX").click();
    eh.run();
    assert!(summary(&eh).contains("plane 2"), "{}", summary(&eh));

    // bind:open, and a field inside the opened group
    eh.harness.get_by_role_and_label(Role::DisclosureTriangle, "Placement").click();
    eh.run();
    assert!(summary(&eh).contains("placement true"), "{}", summary(&eh));
    eh.harness.get_by_role_and_label(Role::TextInput, "Offset").type_text("5");
    eh.run();
    let s = summary(&eh);
    assert!(s.contains("offset 05") || s.contains("offset 50"), "{s}");

    // the icon button and bind:checked
    eh.harness.get_by_role_and_label(Role::Button, "Settings").click();
    eh.run();
    eh.harness.get_by_role_and_label(Role::Button, "Snap").click();
    eh.run();
    let s = summary(&eh);
    assert!(s.contains("snap true") && s.contains("settings 1"), "{s}");
    assert!(e.settled().is_settled());
}
