//! X6 end to end on GTK: the real host runs the `fixtures/styles` Svelte bundle. Component styles,
//! `class:`, `style:`, tokens and the colour scheme come from JS; the results are read back
//! natively (rendered pixels, computed colour, CSS classes). Needs a display (xvfb-run) and the
//! built fixture (`pnpm -r build`); skipped without the bundle.
//!
//! Handles in the fixture: window #1, card box #2, label #3, toggle button #4, entry #5, theme
//! button #6, accent button #7.

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::BackendRef;
use svelters_gtk::testing::on_gtk;
use svelters_gtk::GtkBackend;
use svelters_host::{Host, HostConfig};

const WIN: u64 = 1;
const CARD: u64 = 2;
const LABEL: u64 = 3;
const TOGGLE: u64 = 4;
const THEME: u64 = 6;
const ACCENT: u64 = 7;

fn widget(b: &GtkBackend, h: u64) -> gtk::Widget {
    b.object(h).unwrap().downcast::<gtk::Widget>().unwrap()
}

fn px(b: &GtkBackend, h: u64, x: i32, y: i32) -> String {
    let w = widget(b, h);
    let (width, height) = (w.width().max(1), w.height().max(1));
    let paintable = gtk::WidgetPaintable::new(Some(&w));
    let snap = gtk::Snapshot::new();
    paintable.snapshot(&snap, width as f64, height as f64);
    let node = snap.to_node().expect("render node");
    let tex = w.native().unwrap().renderer().unwrap().render_texture(&node, None);
    let stride = (width * 4) as usize;
    let mut buf = vec![0u8; stride * height as usize];
    tex.download(&mut buf, stride);
    let i = (y as usize) * stride + (x as usize) * 4;
    format!("{:02x}{:02x}{:02x}", buf[i + 2], buf[i + 1], buf[i])
}

fn color(b: &GtkBackend, h: u64) -> String {
    #[allow(deprecated)]
    let c = widget(b, h).style_context().color();
    format!("{:02x}{:02x}{:02x}", (c.red() * 255.0).round() as u8, (c.green() * 255.0).round() as u8, (c.blue() * 255.0).round() as u8)
}

fn classes(b: &GtkBackend, h: u64) -> Vec<String> {
    widget(b, h).css_classes().iter().map(|c| c.to_string()).collect()
}

#[test]
fn styles_from_svelte_reach_gtk_widgets_and_follow_tokens_and_scheme() {
    let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/styles/dist/app.js");
    if !bundle.exists() {
        eprintln!("skipped: {} is not built", bundle.display());
        return;
    }
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
        let host = Host::new(raw, cfg).unwrap();
        host.load_bundle(bundle).unwrap();

        let seen: Rc<RefCell<Vec<(&'static str, String)>>> = Rc::default();
        let at = |ms: u64, f: Box<dyn FnOnce(&GtkBackend, &mut Vec<(&'static str, String)>)>| {
            let (b, s) = (backend.clone(), seen.clone());
            glib::timeout_add_local_once(Duration::from_millis(ms), move || f(&b, &mut s.borrow_mut()));
        };
        let click = |b: &GtkBackend, h: u64| widget(b, h).downcast::<gtk::Button>().unwrap().emit_clicked();
        at(500, Box::new(move |b, l| {
            l.push(("win bg (token --sv-bg)", px(b, WIN, 2, 2)));
            l.push(("card bg (token --sv-surface)", px(b, CARD, 8, 8)));
            l.push(("button bg (token --sv-accent)", px(b, TOGGLE, 20, 14)));
            l.push(("label classes", classes(b, LABEL).join(" ")));
            click(b, TOGGLE);
        }));
        at(1000, Box::new(move |b, l| {
            l.push(("label classes after toggle", classes(b, LABEL).join(" ")));
            l.push(("label color (style:color)", color(b, LABEL)));
            l.push(("button classes after toggle", classes(b, TOGGLE).join(" ")));
            l.push(("button border (class:active)", px(b, TOGGLE, 30, 0)));
            click(b, THEME);
        }));
        at(1600, Box::new(move |b, l| {
            l.push(("win bg dark", px(b, WIN, 2, 2)));
            l.push(("card bg dark", px(b, CARD, 8, 8)));
            click(b, ACCENT);
        }));
        at(2200, Box::new(move |b, l| {
            l.push(("button bg after setTokens", px(b, TOGGLE, 20, 14)));
            b.object(WIN).unwrap().downcast::<gtk::Window>().unwrap().close();
        }));
        let raw2 = raw;
        glib::timeout_add_local_once(Duration::from_millis(9000), move || raw2.quit());

        host.run().unwrap();
        let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
        assert!(errs.is_empty(), "{errs:?}");
        let parse_errors = backend.take_style_errors();
        assert!(parse_errors.is_empty(), "CSS parse errors: {parse_errors:?}");
        let seen = seen.borrow();
        let get = |k: &str| seen.iter().find(|(n, _)| *n == k).unwrap_or_else(|| panic!("no record {k}: {seen:?}")).1.clone();
        assert_eq!(get("win bg (token --sv-bg)"), "f6f5f4");
        assert_eq!(get("card bg (token --sv-surface)"), "ffffff");
        assert_eq!(get("button bg (token --sv-accent)"), "3584e4");
        assert!(get("label classes").contains("title") && !get("label classes").contains("big"), "{}", get("label classes"));
        assert!(get("label classes after toggle").contains("big"));
        assert_eq!(get("label color (style:color)"), "c01c28");
        assert!(get("button classes after toggle").contains("active") && get("button classes after toggle").contains("text-button"));
        assert_eq!(get("button border (class:active)"), "c01c28");
        assert_eq!(get("win bg dark"), "242424");
        assert_eq!(get("card bg dark"), "303030");
        assert_eq!(get("button bg after setTokens"), "26a269");
    });
}
