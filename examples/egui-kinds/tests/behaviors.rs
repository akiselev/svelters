//! EV2b acceptance (ARCH-EMBED §8.5 and §10): a native behaviour defined by this crate
//! (`#[svelters::behavior(namespace = "cad", name = "pick")]`, `Pick`) used from Svelte as
//! `use:pick` (imported from the dumped `svelters:behaviors/cad`):
//! - it runs every frame in Rust with zero JS entries (counted with the host's `Timing`);
//! - a params update is one `set_prop`;
//! - its node is attached to the host (not a light child, not laid out), and its `detached` hook
//!   cleans the host's state bits.

use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use svelters_egui::harness::TestApp;
use svelters_egui::kinds::KindRegistry;
use svelters_egui::scene::{NodeId, ProviderId, SceneHandle};
use svelters_host::HostConfig;

fn bundle() -> PathBuf {
    let p = egui_kinds_demo::bundle();
    assert!(
        p.exists(),
        "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)",
        p.display()
    );
    p
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

fn state_names(scene: &SceneHandle, n: NodeId) -> Vec<&'static str> {
    scene.read(|s| s.get(n).expect("live").state().names())
}

#[test]
fn use_pick_runs_in_rust_with_no_js_per_frame_and_a_params_update_is_one_set_prop() {
    egui_kinds_demo::link();
    let cfg = HostConfig {
        echo_console: false,
        echo_errors: false,
        timing: true,
        ..HostConfig::default()
    };
    let mut app = TestApp::with_config(bundle(), cfg).unwrap();
    app.run();
    let scene = app.backend.scene();

    // the label hosts one attached node of the behaviour's kind, outside its light children
    let (host, b) = scene.read(|s| {
        let n = s
            .of_tag("label")
            .find(|n| n.text() == "Face")
            .expect("the Face label");
        let att: Vec<_> = n.attached().collect();
        assert_eq!(att.len(), 1);
        assert_eq!(att[0].tag(), "cad.pick");
        assert_eq!(att[0].parent().map(|p| p.id()), Some(n.id()));
        assert_eq!(n.children().count(), 0);
        let params = att[0].widget(|p: &egui_kinds_demo::Pick| (p.accepts().to_vec(), p.active()));
        assert_eq!(params, Some((vec!["mcad.face".to_string()], true)));
        s.validate().unwrap();
        (n.id(), att[0].id())
    });
    let frames = |scene: &SceneHandle| {
        scene.read(|s| {
            s.get(b)
                .unwrap()
                .widget(|p: &egui_kinds_demo::Pick| p.frames())
                .unwrap()
        })
    };
    assert!(frames(&scene) > 0, "after_ui ran");
    assert!(state_names(&scene, host).contains(&"cad.pick-target"));
    // a container host: its behaviour runs on a response of its own, its children still draw
    let body = scene.read(|s| {
        let bx = s
            .of_tag("box")
            .find(|n| n.attached().count() == 1)
            .expect("the box with use:pick");
        assert_eq!(bx.children().count(), 1);
        bx.id()
    });
    assert!(state_names(&scene, body).contains(&"cad.pick-target"));
    assert!(has(&app, "Body"));

    // hovering for 10 frames: the hook runs each frame, JS never does
    app.host().take_timing();
    let f0 = frames(&scene);
    app.harness.get_by_label("Face").hover();
    for _ in 0..10 {
        app.harness.step();
    }
    let t = app.host().take_timing();
    assert_eq!(
        (t.checkpoints, t.js_ns, t.ops),
        (0, 0, 0),
        "no JS entry and no host op per frame: {t:?}"
    );
    assert!(frames(&scene) >= f0 + 10);
    assert!(
        state_names(&scene, host).contains(&"cad.pick-hover"),
        "{:?}",
        state_names(&scene, host)
    );

    // a click on the host: the behaviour emits `picked`, the params' `onpicked` gets the record
    app.harness.get_by_label("Face").click();
    app.run();
    assert!(
        has(&app, "picked mcad.face Face"),
        "{:#?}",
        app.harness.root()
    );

    // the params update (`active` flips; `accepts` is the same array): exactly one set_prop
    app.host().take_timing();
    app.harness.get_by_label("Toggle pick").click();
    app.run();
    let t = app.host().take_timing();
    assert_eq!(t.ops, 1, "a params update is one set_prop: {t:?}");
    scene.read(|s| {
        assert_eq!(
            s.get(b)
                .unwrap()
                .widget(|p: &egui_kinds_demo::Pick| p.active()),
            Some(false)
        )
    });
    app.harness.step();
    let names = state_names(&scene, host);
    assert!(
        !names.contains(&"cad.pick-target") && !names.contains(&"cad.pick-hover"),
        "{names:?}"
    );
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}

#[test]
fn attached_nodes_are_not_children_and_detach_cleans_the_host() {
    egui_kinds_demo::link();
    let scene = SceneHandle::default();
    let app = ProviderId::APP;
    let k = KindRegistry::global()
        .tag_index("cad.pick")
        .expect("the behaviour's kind");
    assert!(KindRegistry::global().attaches(k));
    let (bx, l, b) = scene
        .write(app, |s| {
            let bx = s.create_tag("box").unwrap();
            let l = s.create_tag("label").unwrap();
            let b = s.create_tag("cad.pick").unwrap();
            s.insert(bx, l, None).unwrap();
            // a leaf takes no children, but any Widget takes behaviours
            s.insert(l, b, None).unwrap();
            s.node(l)
                .unwrap()
                .set_state(&egui_kinds_demo::cad::PICK_TARGET, true);
            (bx, l, b)
        })
        .unwrap();
    scene.read(|s| {
        s.validate().unwrap();
        let n = s.get(l).unwrap();
        assert_eq!(n.children().count(), 0);
        assert_eq!(n.attached().map(|a| a.id()).collect::<Vec<_>>(), [b]);
        assert_eq!(s.get(bx).unwrap().children().count(), 1);
    });
    assert_eq!(state_names(&scene, l), ["cad.pick-target"]);

    // removing it runs `Behavior::detached` with the host: its bits are cleared
    scene.write(app, |s| s.remove(b).unwrap()).unwrap();
    assert!(state_names(&scene, l).is_empty());
    scene.read(|s| {
        s.validate().unwrap();
        assert_eq!(s.get(l).unwrap().attached().count(), 0);
        assert!(s.get(b).unwrap().parent().is_none());
    });

    // a behaviour does not take behaviours (it is not a Widget)
    scene
        .write(app, |s| {
            let b2 = s.create_tag("cad.pick").unwrap();
            assert!(s.insert(b, b2, None).is_err());
            // destroying a host destroys its subtree, attached nodes included
            s.insert(l, b, None).unwrap();
            s.destroy(bx).unwrap();
            assert!(s.read().get(b).is_none() && s.read().get(l).is_none());
            s.destroy(b2).unwrap();
        })
        .unwrap();
    scene.read(|s| s.validate().unwrap());
}
