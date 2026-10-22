//! X6 styling on GTK at the backend level (ABI v2.3 `style` op). Each test reads the result back
//! natively: the computed colour of a widget (`gtk_widget_get_color`) or a rendered pixel.
//! Needs a display (xvfb-run).

use gtk::gdk;
use gtk::prelude::*;
use svelters_abi::{
    SVL_STYLE_CLASSES, SVL_STYLE_CLIP, SVL_STYLE_INERT, SVL_STYLE_INLINE, SVL_STYLE_NAME, SVL_STYLE_OPACITY, SVL_STYLE_SCHEME, SVL_STYLE_SHEET, SVL_STYLE_TARGET,
    SVL_STYLE_TRANSFORM,
};
use svelters_gtk::testing::{on_gtk, pump, Harness};

const SEP: &str = "\u{1}";

fn sheet(h: &Harness, id: &str, css: &str) {
    h.style(SVL_STYLE_SHEET, 0, Some(&format!("{id}{SEP}{css}"))).unwrap();
}

fn rgb(c: gdk::RGBA) -> (u8, u8, u8) {
    ((c.red() * 255.0).round() as u8, (c.green() * 255.0).round() as u8, (c.blue() * 255.0).round() as u8)
}

/// A shown window with a label and a button; returns (harness, label, button).
fn app() -> (Harness, u64, u64) {
    let h = Harness::new();
    h.style(SVL_STYLE_SHEET, 0, None).unwrap();
    let win = h.create("window");
    let boxx = h.create("box");
    let label = h.create("label");
    let button = h.create("button");
    h.set_str(label, "label", "text").unwrap();
    h.set_str(button, "label", "go").unwrap();
    h.insert(boxx, label, None, None).unwrap();
    h.insert(boxx, button, None, None).unwrap();
    h.insert(win, boxx, None, None).unwrap();
    h.call(win, "present").unwrap();
    pump(100);
    (h, label, button)
}

fn color_of(h: &Harness, handle: u64) -> (u8, u8, u8) {
    pump(60);
    #[allow(deprecated)]
    rgb(h.widget(handle).style_context().color())
}

#[test]
fn the_backend_speaks_css() {
    on_gtk(|| {
        let h = Harness::new();
        assert_eq!(h.style(SVL_STYLE_TARGET, 0, None).unwrap(), 1);
    });
}

#[test]
fn a_class_selects_a_rule_and_toggling_it_restyles_only_that_widget() {
    on_gtk(|| {
        let (h, label, button) = app();
        sheet(&h, "App.svelte", ".warn.svelte-x { color: #c01c28; }\nlabel.sv-k-label.big.svelte-x { color: #26a269; }\n");
        assert!(h.style_errors().is_empty());
        h.style(SVL_STYLE_CLASSES, label, Some("warn svelte-x")).unwrap();
        assert_eq!(color_of(&h, label), (0xc0, 0x1c, 0x28));
        h.style(SVL_STYLE_CLASSES, label, Some("sv-k-label big svelte-x")).unwrap();
        assert_eq!(color_of(&h, label), (0x26, 0xa2, 0x69));
        h.style(SVL_STYLE_CLASSES, label, None).unwrap();
        let plain = color_of(&h, label);
        assert_ne!(plain, (0x26, 0xa2, 0x69));
        // the other widget never carried the class
        assert_eq!(color_of(&h, button), color_of(&h, button));
        assert!(!h.widget(button).has_css_class("warn"));
    });
}

#[test]
fn the_widgets_own_classes_survive_class_updates() {
    on_gtk(|| {
        let (h, _label, button) = app();
        assert!(h.widget(button).has_css_class("text-button"));
        h.style(SVL_STYLE_CLASSES, button, Some("primary a")).unwrap();
        h.style(SVL_STYLE_CLASSES, button, Some("primary b")).unwrap();
        let w = h.widget(button);
        assert!(w.has_css_class("text-button"), "builtin class kept");
        assert!(w.has_css_class("primary") && w.has_css_class("b") && !w.has_css_class("a"));
        h.style(SVL_STYLE_CLASSES, button, None).unwrap();
        assert!(w.has_css_class("text-button") && !w.has_css_class("primary"));
    });
}

#[test]
fn state_pseudo_classes_apply() {
    on_gtk(|| {
        let (h, _label, button) = app();
        sheet(
            &h,
            "S",
            "button.p { color: #111111; }\nbutton.p:hover { color: #222222; }\nbutton.p:disabled { color: #333333; }\nbutton.p:active { color: #444444; }\n",
        );
        h.style(SVL_STYLE_CLASSES, button, Some("p")).unwrap();
        assert_eq!(color_of(&h, button), (0x11, 0x11, 0x11));
        let w = h.widget(button);
        w.set_state_flags(gtk::StateFlags::PRELIGHT, false);
        assert_eq!(color_of(&h, button), (0x22, 0x22, 0x22));
        w.unset_state_flags(gtk::StateFlags::PRELIGHT);
        w.set_state_flags(gtk::StateFlags::ACTIVE, false);
        assert_eq!(color_of(&h, button), (0x44, 0x44, 0x44));
        w.unset_state_flags(gtk::StateFlags::ACTIVE);
        w.set_sensitive(false);
        assert_eq!(color_of(&h, button), (0x33, 0x33, 0x33));
    });
}

#[test]
fn id_selects_through_the_widget_name() {
    on_gtk(|| {
        let (h, label, _b) = app();
        sheet(&h, "S", "#lbl { color: #123456; }\n");
        h.style(SVL_STYLE_NAME, label, Some("lbl")).unwrap();
        assert_eq!(color_of(&h, label), (0x12, 0x34, 0x56));
    });
}

#[test]
fn inline_declarations_dedupe_into_shared_classes_and_swap_cleanly() {
    on_gtk(|| {
        let (h, label, button) = app();
        h.style(SVL_STYLE_INLINE, label, Some("color: #ff0000;")).unwrap();
        h.style(SVL_STYLE_INLINE, button, Some("color: #ff0000;")).unwrap();
        pump(60);
        assert_eq!(color_of(&h, label), (255, 0, 0));
        assert_eq!(color_of(&h, button), (255, 0, 0));
        let inline = |x: u64| h.widget(x).css_classes().iter().find(|c| c.starts_with("sv-s")).map(|c| c.to_string());
        assert!(inline(label).is_some());
        assert_eq!(inline(label), inline(button), "same declarations share one class");
        h.style(SVL_STYLE_INLINE, label, Some("color: #00ff00;")).unwrap();
        assert_eq!(color_of(&h, label), (0, 255, 0));
        assert_eq!(color_of(&h, button), (255, 0, 0));
        h.style(SVL_STYLE_INLINE, label, None).unwrap();
        assert!(inline(label).is_none());
        assert!(h.widget(label).has_css_class("sv-s1") == false);
        assert!(h.style_errors().is_empty());
    });
}

#[test]
fn inline_beats_component_rules() {
    on_gtk(|| {
        let (h, label, _b) = app();
        sheet(&h, "S", "label.c { color: #111111; }\n");
        h.style(SVL_STYLE_CLASSES, label, Some("c")).unwrap();
        assert_eq!(color_of(&h, label), (0x11, 0x11, 0x11));
        h.style(SVL_STYLE_INLINE, label, Some("color: #abcdef;")).unwrap();
        assert_eq!(color_of(&h, label), (0xab, 0xcd, 0xef));
    });
}

#[test]
fn opacity_goes_through_the_widget_api() {
    on_gtk(|| {
        let (h, label, _b) = app();
        h.style(SVL_STYLE_OPACITY, label, Some("0.25")).unwrap();
        pump(60);
        assert!((h.widget(label).opacity() - 0.25).abs() < 0.01);
        h.style(SVL_STYLE_OPACITY, label, None).unwrap();
        pump(60);
        assert!((h.widget(label).opacity() - 1.0).abs() < 0.01);
    });
}

/// TR1 (ABI v2.8): the transform and clip channels are CSS on the widget's own provider (the
/// clip squashes the widget into the clipped height, D3), inert turns off targeting and focus, and
/// `bounds` reports the allocation in the window.
#[test]
fn visual_channels_and_bounds() {
    on_gtk(|| {
        let (h, label, button) = app();
        let before = h.bounds(button).unwrap();
        assert!(before[2] > 0.0 && before[3] > 0.0, "a laid-out button has a size: {before:?}");
        let l = h.bounds(label).unwrap();
        assert!(before[1] >= l[1] + l[3] - 0.5, "the button is below the label: {before:?} vs {l:?}");

        h.style(SVL_STYLE_TRANSFORM, label, Some("10 -4 0.5 0.5 0.5 0.5")).unwrap();
        h.style(SVL_STYLE_INERT, label, Some("1")).unwrap();
        let w = h.widget(label);
        assert!(!w.can_target() && !w.can_focus(), "inert");
        h.style(SVL_STYLE_CLIP, label, Some(&format!("-1 {}", (l[3] / 2.0).round()))).unwrap();
        pump(100);
        let after = h.bounds(button).unwrap();
        assert!(after[1] < before[1] - 1.0, "the clipped label gives its space back: {after:?} vs {before:?}");
        assert!(h.style_errors().is_empty(), "{:?}", h.style_errors());

        for op in [SVL_STYLE_TRANSFORM, SVL_STYLE_CLIP, SVL_STYLE_INERT] {
            h.style(op, label, None).unwrap();
        }
        pump(100);
        assert!(w.can_target(), "inert lifted");
        let back = h.bounds(button).unwrap();
        assert!((back[1] - before[1]).abs() < 0.5, "layout restored: {back:?} vs {before:?}");
    });
}

#[test]
fn token_changes_rewrite_one_provider_and_var_follows() {
    // CSS custom properties (`:root`, `var()`) arrived in GTK 4.16.
    if svelters_gtk::testing::skip_below_gtk(4, 16) {
        return;
    }
    on_gtk(|| {
        let (h, label, _b) = app();
        sheet(&h, "@tokens", ":root {\n  --sv-fg: #102030;\n}\n");
        sheet(&h, "S", "label.t { color: var(--sv-fg); }\n");
        h.style(SVL_STYLE_CLASSES, label, Some("t")).unwrap();
        assert_eq!(color_of(&h, label), (0x10, 0x20, 0x30));
        sheet(&h, "@tokens", ":root {\n  --sv-fg: #405060;\n}\n");
        assert_eq!(color_of(&h, label), (0x40, 0x50, 0x60));
        assert!(h.style_errors().is_empty());
    });
}

#[test]
fn the_scheme_drives_media_queries_in_every_provider() {
    // `@media (prefers-color-scheme)` and GtkCssProvider:prefers-color-scheme arrived in GTK 4.20.
    if svelters_gtk::testing::skip_below_gtk(4, 20) {
        return;
    }
    on_gtk(|| {
        let (h, label, _b) = app();
        sheet(&h, "S", "label.m { color: #111111; }\n@media (prefers-color-scheme: dark) {\n  label.m { color: #eeeeee; }\n}\n");
        h.style(SVL_STYLE_CLASSES, label, Some("m")).unwrap();
        assert_eq!(color_of(&h, label), (0x11, 0x11, 0x11));
        h.style(SVL_STYLE_SCHEME, 0, Some("dark")).unwrap();
        assert_eq!(color_of(&h, label), (0xee, 0xee, 0xee));
        // a provider created after the switch starts in the active scheme
        sheet(&h, "T", "label.n { color: #222222; }\n@media (prefers-color-scheme: dark) {\n  label.n { color: #dddddd; }\n}\n");
        h.style(SVL_STYLE_CLASSES, label, Some("n")).unwrap();
        assert_eq!(color_of(&h, label), (0xdd, 0xdd, 0xdd));
        h.style(SVL_STYLE_SCHEME, 0, Some("light")).unwrap();
        assert_eq!(color_of(&h, label), (0x22, 0x22, 0x22));
    });
}

#[test]
fn replacing_and_removing_a_sheet() {
    on_gtk(|| {
        let (h, label, _b) = app();
        sheet(&h, "S", "label.r { color: #010101; }\n");
        h.style(SVL_STYLE_CLASSES, label, Some("r")).unwrap();
        assert_eq!(color_of(&h, label), (1, 1, 1));
        sheet(&h, "S", "label.r { color: #020202; }\n");
        assert_eq!(color_of(&h, label), (2, 2, 2));
        h.style(SVL_STYLE_SHEET, 0, Some("S")).unwrap();
        assert_ne!(color_of(&h, label), (2, 2, 2));
    });
}

#[test]
fn a_css_parse_error_is_reported_not_swallowed() {
    on_gtk(|| {
        let (h, _l, _b) = app();
        sheet(&h, "Bad", "label.x { colour: red; }\n");
        let errors = h.style_errors();
        assert!(!errors.is_empty(), "expected a parse error");
        assert!(errors.iter().any(|e| e.contains("colour")), "{errors:?}");
    });
}

/// A pixel of the widget's rendered output (needs a mapped widget and a renderer).
fn pixel(h: &Harness, handle: u64, x: i32, y: i32) -> (u8, u8, u8) {
    pump(80);
    let w = h.widget(handle);
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
    (buf[i + 2], buf[i + 1], buf[i]) // BGRA
}

#[test]
fn background_colour_is_painted() {
    on_gtk(|| {
        let (h, label, _b) = app();
        sheet(&h, "S", "label.bg { background-color: #ff0000; min-width: 40px; min-height: 20px; }\n");
        h.style(SVL_STYLE_CLASSES, label, Some("bg")).unwrap();
        assert_eq!(pixel(&h, label, 2, 2), (255, 0, 0));
    });
}

/// Performance guard (docs/notes/styling-gtk-css-qss.md 5.2): class changes are local and cheap, and
/// a state change never reloads a provider, so 2000 class ops stay far below a generous budget.
#[test]
fn a_thousand_class_toggles_stay_cheap() {
    on_gtk(|| {
        let h = Harness::new();
        h.style(SVL_STYLE_SHEET, 0, None).unwrap();
        let win = h.create("window");
        let boxx = h.create("box");
        let buttons: Vec<u64> = (0..1000)
            .map(|_| {
                let b = h.create("button");
                h.insert(boxx, b, None, None).unwrap();
                b
            })
            .collect();
        h.insert(win, boxx, None, None).unwrap();
        h.call(win, "present").unwrap();
        sheet(&h, "S", "button.on { color: #ff0000; }\n");
        pump(300);
        let t = std::time::Instant::now();
        for b in &buttons {
            h.style(SVL_STYLE_CLASSES, *b, Some("on")).unwrap();
        }
        for b in &buttons {
            h.style(SVL_STYLE_CLASSES, *b, None).unwrap();
        }
        let apply = t.elapsed();
        assert!(apply < std::time::Duration::from_millis(1500), "2000 class ops took {apply:?}");
        assert!(h.style_errors().is_empty());
    });
}
