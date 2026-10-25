//! The automation bridge end to end: the counter fixture (`fixtures/counter-egui`, compiled live from
//! source) runs in an egui island on presto, and a svelters-automation `Session` drives it through
//! `PrestoDriver` with the same selectors and acts as on the egui backend. Synthetic clicks take
//! presto's input path into the island; semantic ones go through the grafted AccessKit tree.
//!
//! The fixture needs the workspace's JS packages (`pnpm install`): without them the tests say so and
//! pass (the presto workflow installs none; linux-gtk's workspace run has them).

use presto_test::EguiHarness;
use presto_test::automation::PrestoDriver;
use presto_egui_island::island_tree_id;
use svelters_automation::{ActOptions, Expect, Session, TextMatch, Tier, WaitState};
use svelters_egui::embed::{Embed, EmbedConfig};
use svelters_host::HostConfig;

fn session() -> Option<Session<PrestoDriver<presto_test::IslandDoc<Embed>>>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui");
    let mut src = svelters_modules::SourceConfig::for_path(&dir).expect("the fixture");
    src.backend = "egui".into();
    let provider = match svelters_modules::SourceProvider::new(src) {
        Ok(p) => std::rc::Rc::new(p),
        Err(e) if e.contains("pnpm install") => {
            eprintln!("skipped: the fixture's JS packages are not installed ({e})");
            return None;
        }
        Err(e) => panic!("the fixture's sources: {e}"),
    };
    let cfg = HostConfig {
        echo_console: false,
        echo_errors: false,
        ..HostConfig::default()
    };
    let embed = Embed::new(&egui::Context::default(), EmbedConfig::app(cfg)).expect("an embed");
    embed.load_app_from(provider).expect("the app loads");
    let scene = embed.backend().scene();
    let attach = embed.clone();
    let h = EguiHarness::builder()
        .with_size(egui::vec2(360.0, 260.0))
        .with_step_dt(1.0 / 60.0)
        .with_context_setup(move |ctx| attach.attach(ctx))
        .build_ui_state(
            |ui, e: &mut Embed| {
                e.logic(ui.ctx());
                e.show_windows(ui);
            },
            embed,
        );
    let driver = PrestoDriver::new(h.into_harness(), scene, island_tree_id(1))
        .with_busy(|d| {
            let s = d.state().settled();
            (!s.is_settled()).then(|| format!("{:?}", s.busy))
        });
    let mut s = Session::for_test(driver);
    s.wait_settled(10_000.0).expect("the app settles");
    Some(s)
}

fn text(t: &str) -> Expect {
    Expect::Text(TextMatch::Exact(t.into()))
}

#[test]
fn selectors_and_acts_drive_a_presto_island() {
    let Some(mut s) = session() else { return };
    let by_role = s.locate(r#"role=button[name="Increment"]"#).unwrap();
    assert_eq!(s.locate("text=increment").unwrap(), by_role);

    let r = s
        .click("role=button[name=Increment]", ActOptions::default())
        .unwrap();
    assert_eq!(r.tier, Tier::Synthetic);
    assert_eq!(s.query(r#"text="Count: 1""#, None).unwrap().len(), 1);

    let r = s
        .click(
            "role=button[name=Increment]",
            ActOptions {
                tier: Tier::Semantic,
                ..ActOptions::default()
            },
        )
        .unwrap();
    assert_eq!(r.tier, Tier::Semantic);
    s.wait_for("role=label", WaitState::Visible, Some(&text("Count: 2")), 1_000.0)
        .unwrap();
}

#[test]
fn the_driver_reports_island_nodes_in_scene_coordinates() {
    let Some(s) = session() else { return };
    let id = s.locate(r#"role=button[name="Increment"]"#).unwrap();
    let scene_rect = s.node(id).unwrap().rect.expect("drawn");
    let nodes = s.a11y_tree().unwrap();
    let button = nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("Increment"))
        .expect("the button's AccessKit node");
    assert_ne!(button.tree, egui::accesskit::TreeId::ROOT);
    let i = button.rect.intersect(scene_rect);
    assert!(
        i.area() > 0.5 * scene_rect.area(),
        "the AccessKit node covers the scene node: {:?} vs {scene_rect:?}",
        button.rect
    );
}
