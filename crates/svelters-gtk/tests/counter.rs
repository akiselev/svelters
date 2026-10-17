//! P acceptance: the counter tree (window > box > label + button) from a hand-written op sequence,
//! shown under Xvfb, clicked natively, with the event checked at a stub `svl_host` and a screenshot
//! written to `target/p3-counter.png` when ImageMagick's `import` is available.

use gtk::prelude::*;
use svelters_gtk::testing::{on_gtk, pump, Harness, V};

#[test]
fn counter_on_screen() {
    on_gtk(|| {
        let h = Harness::new();
        // The op sequence mirrors fixtures/counter/expected.oplog.
        let win = h.create("window");
        h.set_str(win, "title", "Counter").unwrap();
        let bx = h.create("box");
        h.set_str(bx, "orientation", "vertical").unwrap();
        h.set_i64(bx, "spacing", 6).unwrap();
        let label = h.create("label");
        h.set_str(label, "label", "Count: 0").unwrap();
        let button = h.create("button");
        h.listen(button, "clicked", true).unwrap();
        h.set_str(button, "label", "Increment").unwrap();
        h.insert(bx, label, None, None).unwrap();
        h.insert(bx, button, None, None).unwrap();
        h.insert(win, bx, None, None).unwrap();
        h.call(win, "present").unwrap();
        pump(300);

        h.widget(button).downcast::<gtk::Button>().unwrap().emit_clicked();
        assert_eq!(h.take_events(), vec![(button, "clicked".to_string())], "click reaches the stub host");
        // The app reacts the way the Svelte component would.
        h.set_str(label, "label", "Count: 1").unwrap();
        assert_eq!(h.get(label, "label"), V::Str("Count: 1".into()));
        pump(300);

        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/p3-counter.png");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        match std::process::Command::new("import").args(["-window", "root"]).arg(&out).status() {
            Ok(s) if s.success() => eprintln!("screenshot: {}", out.display()),
            other => eprintln!("no screenshot ({other:?}); install ImageMagick for `import`"),
        }

        // Native close: the host is told, and the handle goes stale.
        h.widget(win).downcast::<gtk::Window>().unwrap().destroy();
        pump(100);
        assert_eq!(h.take_destroyed(), vec![win]);
        assert!(h.set_str(win, "title", "x").is_err(), "stale handle is an error");
    });
}
