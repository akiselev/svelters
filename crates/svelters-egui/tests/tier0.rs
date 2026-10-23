//! WG2b (plans/milestones/WG2b-tier0-controls.md): one kittest per kind, through the backend's
//! vtable like the JS runtime: it renders, the interaction that changes its bound value announces
//! it (`prop_changed` when observed, then `changed`), and it is found by role and label. Plus the
//! containers' layout and W5's primary selection (Linux). Headless.

mod common;

use common::Rig;
use egui::accesskit::{Role, Toggled};
use egui::{Modifiers, PointerButton, pos2, vec2};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// The events of signal `name` on `h` in a log.
fn events(rig: &Rig, log: &common::Log, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|(eh, es, _)| *eh == h && *es == sig).map(|(_, _, a)| a.clone()).collect()
}

fn announced(rig: &Rig, log: &common::Log, h: svl_handle, prop: &str) -> bool {
    let id = rig.prop_id(h, prop);
    log.changed.contains(&(h, id))
}

fn press(rig: &mut Rig, pos: egui::Pos2, button: PointerButton, pressed: bool) {
    rig.harness.event(egui::Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
}

#[test]
fn hyperlink_opens_its_url_and_is_a_link() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "hyperlink", &[("text", s("Docs")), ("url", s("https://example.org/docs")), ("align-self", s("start"))]);
    rig.listen(a, "clicked");
    rig.run();
    let node = rig.harness.get_by_role_and_label(Role::Link, "Docs");
    let center = node.rect().center();
    // one step, so the frame's output still holds the command
    rig.harness.hover_at(center);
    press(&mut rig, center, PointerButton::Primary, true);
    press(&mut rig, center, PointerButton::Primary, false);
    rig.harness.step();
    let opened: Vec<String> = rig
        .harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::OpenUrl(u) => Some(u.url.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(opened, vec!["https://example.org/docs".to_string()]);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, a, "clicked").len(), 1);
}

#[test]
fn radio_buttons_bind_group_and_checked() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    // bind:group: every button has its value and the group's value
    let mk = |rig: &Rig, text: &str, v: &str| rig.leaf(col, "radio-button", &[("text", s(text)), ("value", s(v)), ("group", s("a"))]);
    let (a, b, c) = (mk(&rig, "Alpha", "a"), mk(&rig, "Beta", "b"), mk(&rig, "Gamma", "c"));
    for h in [a, b, c] {
        rig.listen(h, "changed");
        rig.observe(h, "group", true);
        rig.observe(h, "checked", true);
    }
    rig.run();
    assert_eq!(rig.get(a, "checked"), Value::Bool(true), "checked follows group == value");
    assert_eq!(rig.get(b, "checked"), Value::Bool(false));
    let beta = rig.harness.get_by_role_and_label(Role::RadioButton, "Beta");
    assert_eq!(beta.accesskit_node().toggled(), Some(Toggled::False));
    beta.click();
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, b, "changed"), vec![vec![Value::Bool(true), s("b")]]);
    assert_eq!(events(&rig, &log, a, "changed"), vec![vec![Value::Bool(false), s("b")]], "the sibling is unchecked");
    assert_eq!(events(&rig, &log, c, "changed"), vec![vec![Value::Bool(false), s("b")]], "group follows on every sibling");
    assert!(announced(&rig, &log, b, "group") && announced(&rig, &log, b, "checked") && announced(&rig, &log, a, "checked"));
    assert!(!announced(&rig, &log, c, "checked"), "c was not checked before");
    for h in [a, b, c] {
        assert_eq!(rig.get(h, "group"), s("b"));
    }
    assert_eq!(rig.harness.get_by_role_and_label(Role::RadioButton, "Beta").accesskit_node().toggled(), Some(Toggled::True));
    // a host write of the group (the bound variable changed) re-derives `checked`, unannounced
    for h in [a, b, c] {
        rig.set(h, "group", s("c"));
    }
    rig.run();
    assert_eq!((rig.get(b, "checked"), rig.get(c, "checked")), (Value::Bool(false), Value::Bool(true)));
    assert!(rig.take().changed.is_empty());

    // bind:checked only (no values): siblings are exclusive
    let col2 = rig.leaf(col, "box", &[]);
    let x = rig.leaf(col2, "radio-button", &[("text", s("X")), ("checked", Value::Bool(true))]);
    let y = rig.leaf(col2, "radio-button", &[("text", s("Y"))]);
    rig.listen(x, "changed");
    rig.listen(y, "changed");
    rig.run();
    rig.click(y);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, y, "changed"), vec![vec![Value::Bool(true), s("")]]);
    assert_eq!(events(&rig, &log, x, "changed"), vec![vec![Value::Bool(false), s("")]]);
    // clicking a checked radio button changes nothing
    rig.click(y);
    rig.run();
    assert!(rig.take().events.iter().all(|(_, sig, _)| *sig != rig.signal_id(y, "changed")));
}

#[test]
fn switch_toggles_active() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let sw = rig.leaf(col, "switch", &[("text", s("Wi-Fi")), ("align-self", s("start"))]);
    rig.listen(sw, "changed");
    rig.observe(sw, "active", true);
    rig.run();
    let node = rig.harness.get_by_role_and_label(Role::Switch, "Wi-Fi");
    assert_eq!(node.accesskit_node().toggled(), Some(Toggled::False));
    node.click();
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, sw, "changed"), vec![vec![Value::Bool(true)]]);
    assert!(announced(&rig, &log, sw, "active"));
    assert_eq!(rig.get(sw, "active"), Value::Bool(true));
    assert_eq!(rig.harness.get_by_role_and_label(Role::Switch, "Wi-Fi").accesskit_node().toggled(), Some(Toggled::True));
    // the caption is part of the switch: it fits its measured size
    let d = rig.drawn(sw);
    assert!(d.width() > 2.0 * d.height(), "{d:?}");
}

#[test]
fn range_slider_moves_one_end() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let r = rig.leaf(col, "range-slider", &[("text", s("Range")), ("low", Value::F64(10.0)), ("high", Value::F64(90.0)), ("step", Value::F64(1.0))]);
    rig.listen(r, "changed");
    rig.observe(r, "low", true);
    rig.observe(r, "high", true);
    rig.run();
    rig.harness.get_by_role_and_label(Role::Slider, "Range low").set_value_accesskit(25.0);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, r, "changed"), vec![vec![Value::F64(25.0), Value::F64(90.0)]]);
    assert!(announced(&rig, &log, r, "low") && !announced(&rig, &log, r, "high"));
    // the high handle cannot pass the low one
    rig.harness.get_by_role_and_label(Role::Slider, "Range high").set_value_accesskit(5.0);
    rig.run();
    assert_eq!((rig.get(r, "low"), rig.get(r, "high")), (Value::F64(25.0), Value::F64(25.0)));
    // it fills its rect
    let d = rig.drawn(r);
    assert!((d.width() - rig.backend.with_scene(|sc| sc.n(r).rect.width())).abs() < 12.0, "{d:?}");
}

#[test]
fn uncaptioned_range_slider_names_its_value_boxes() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let r = rig.leaf(col, "range-slider", &[("accessible-name", s("Span")), ("low", Value::F64(2.0)), ("high", Value::F64(8.0)), ("max", Value::F64(10.0)), ("step", Value::F64(1.0))]);
    rig.listen(r, "changed");
    rig.run(); // kittest's accessibility check: every box has a name
    let boxes: Vec<_> = rig.harness.get_all_by_role(Role::SpinButton).collect();
    assert_eq!(boxes.len(), 2);
    boxes[1].set_value_accesskit(6.6);
    rig.run();
    assert_eq!(events(&rig, &rig.take(), r, "changed"), vec![vec![Value::F64(2.0), Value::F64(7.0)]], "snapped to the step");
    // a box cannot pass the other end
    rig.harness.get_all_by_role(Role::SpinButton).next().unwrap().set_value_accesskit(9.0);
    rig.run();
    assert_eq!((rig.get(r, "low"), rig.get(r, "high")), (Value::F64(7.0), Value::F64(7.0)));
}

#[test]
fn drag_value_edits_its_value_within_bounds() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let d = rig.leaf(
        col,
        "drag-value",
        &[("value", Value::F64(3.0)), ("min", Value::F64(0.0)), ("max", Value::F64(10.0)), ("suffix", s(" mm")), ("accessible-name", s("Width")), ("align-self", s("start"))],
    );
    rig.listen(d, "changed");
    rig.observe(d, "value", true);
    rig.run();
    let node = rig.harness.get_by_role_and_label(Role::SpinButton, "Width");
    node.set_value_accesskit(7.5);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, d, "changed"), vec![vec![Value::F64(7.5)]]);
    assert!(announced(&rig, &log, d, "value"));
    rig.harness.get_by_role_and_label(Role::SpinButton, "Width").set_value_accesskit(42.0);
    rig.run();
    assert_eq!(rig.get(d, "value"), Value::F64(10.0), "clamped to max");
    // null bounds: unbounded
    rig.set(d, "max", Value::Null);
    rig.run();
    rig.harness.get_by_role_and_label(Role::SpinButton, "Width").set_value_accesskit(42.0);
    rig.run();
    assert_eq!(rig.get(d, "value"), Value::F64(42.0));
}

/// A `w`×`h` PNG, every pixel `rgba`.
fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn image_shows_its_source_at_its_size() {
    let mut rig = Rig::new();
    rig.harness.ctx.include_bytes("bytes://dot.png", png(24, 12, [255, 0, 0, 255]));
    let dir = std::env::temp_dir().join(format!("svelters-wg2b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("box.png");
    std::fs::write(&file, png(8, 16, [0, 0, 255, 255])).unwrap();
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "image", &[("source", s("bytes://dot.png")), ("accessible-name", s("Dot")), ("align-self", s("start"))]);
    let b = rig.leaf(col, "image", &[("source", s(file.to_str().unwrap())), ("accessible-name", s("Box")), ("align-self", s("start"))]);
    rig.listen(a, "clicked");
    rig.run();
    assert_eq!(rig.drawn(a).size(), vec2(24.0, 12.0), "measured at the image's size");
    assert_eq!(rig.drawn(b).size(), vec2(8.0, 16.0), "a bare path loads as file://");
    rig.harness.get_by_role_and_label(Role::Image, "Dot").click();
    rig.run();
    assert_eq!(events(&rig, &rig.take(), a, "clicked").len(), 1);
    // a new source is measured again
    rig.set(a, "source", s(file.to_str().unwrap()));
    rig.run();
    assert_eq!(rig.drawn(a).size(), vec2(8.0, 16.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn image_fit_rects() {
    use svelters_egui::elements::fit_rect;
    let r = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
    let img = vec2(20.0, 20.0);
    assert_eq!(fit_rect("contain", r, img).0, egui::Rect::from_center_size(r.center(), vec2(50.0, 50.0)));
    assert_eq!(fit_rect("fill", r, img).0, r);
    assert_eq!(fit_rect("none", r, img).0.size(), img);
    assert_eq!(fit_rect("scale-down", r, img).0.size(), img);
    let (dest, uv) = fit_rect("cover", r, img);
    assert_eq!(dest, r);
    assert!((uv.width() - 1.0).abs() < 1e-6 && (uv.height() - 0.5).abs() < 1e-6, "{uv:?}");
}

#[test]
fn progress_bar_and_spinner_are_progress_indicators() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let p = rig.leaf(col, "progress-bar", &[("fraction", Value::F64(0.25)), ("show-percentage", Value::Bool(true)), ("accessible-name", s("Upload"))]);
    let sp = rig.leaf(col, "spinner", &[("accessible-name", s("Loading")), ("size", Value::F64(20.0)), ("align-self", s("start"))]);
    rig.harness.run_steps(4); // a spinner repaints every frame
    rig.harness.get_by_role_and_label(Role::ProgressIndicator, "Upload");
    let spin = rig.harness.get_by_role_and_label(Role::ProgressIndicator, "Loading");
    assert!(spin.accesskit_node().is_busy());
    // the bar fills the column's width, the spinner is its size
    let col_w = rig.backend.with_scene(|sc| sc.n(col).rect.width());
    assert!((rig.drawn(p).width() - col_w).abs() < 1.0, "{:?} in {col_w}", rig.drawn(p));
    assert_eq!(rig.drawn(sp).size(), vec2(20.0, 20.0));
    // not spinning: no longer busy, and the space stays
    rig.set(sp, "spinning", Value::Bool(false));
    rig.run(); // settles: nothing spins
    assert!(!rig.harness.get_by_role_and_label(Role::ProgressIndicator, "Loading").accesskit_node().is_busy());
    assert_eq!(rig.drawn(sp).size(), vec2(20.0, 20.0));
}

#[test]
fn canvas_replays_the_display_list_and_reports_its_size() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let c = rig.create("canvas");
    rig.set(c, "content-width", Value::F64(120.0));
    rig.set(c, "content-height", Value::F64(60.0));
    rig.set(c, "accessible-name", s("Plot"));
    rig.set(c, "align-self", s("start"));
    rig.listen(c, "resize");
    rig.listen(c, "clicked");
    rig.insert(col, c);
    rig.set(c, "ops", s(r##"[["fillStyle","#ff0000"],["fillRect",10,10,20,20],["beginPath"],["moveTo",0,0],["lineTo",50,50],["stroke"]]"##));
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, c, "resize"), vec![vec![Value::F64(120.0), Value::F64(60.0)]], "posted once");
    rig.harness.get_by_role_and_label(Role::Canvas, "Plot").click();
    rig.run();
    assert_eq!(events(&rig, &rig.take(), c, "clicked").len(), 1);
    // the replay: the rect at the canvas origin + (10, 10), red; the line after it
    let origin = rig.drawn(c).min;
    let ops: serde_json::Value = serde_json::from_str(
        match rig.get(c, "ops") {
            Value::Str(t) => t,
            v => panic!("{v:?}"),
        }
        .as_str(),
    )
    .unwrap();
    let clip = rig.drawn(c);
    let shapes = svelters_egui::elements::canvas::shapes(&rig.harness.ctx, origin, clip, ops.as_array().unwrap(), egui::Color32::GRAY);
    assert_eq!(shapes.len(), 2);
    match &shapes[0].1 {
        egui::Shape::Path(p) => {
            assert_eq!(p.fill, egui::Color32::RED);
            assert_eq!(p.points[0], origin + vec2(10.0, 10.0));
            assert_eq!(p.points[2], origin + vec2(30.0, 30.0));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(&shapes[1].1, egui::Shape::Path(p) if !p.closed && p.points.len() == 2));
    // a bad list is refused, null clears
    assert!(rig.br().set_prop(c, rig.prop_id(c, "ops"), &s("{")).is_err());
    rig.set(c, "ops", Value::Null);
    assert_eq!(rig.get(c, "ops"), Value::Null);
}

#[test]
fn canvas_paths_transforms_and_text() {
    let rig = Rig::new();
    let ops: serde_json::Value = serde_json::from_str(
        r#"[["translate",100,0],["scale",2,2],["beginPath"],["moveTo",0,0],["lineTo",10,0],["lineTo",10,10],["lineTo",5,5],["lineTo",0,10],["closePath"],["fill"],
            ["setTransform",1,0,0,1,0,0],["beginPath"],["arc",50,50,10,0,6.2832],["stroke"],
            ["font","20px monospace"],["textAlign","center"],["textBaseline","top"],["fillText","hi",50,80]]"#,
    )
    .unwrap();
    let big = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 500.0));
    let shapes = svelters_egui::elements::canvas::shapes(&rig.harness.ctx, pos2(0.0, 0.0), big, ops.as_array().unwrap(), egui::Color32::GRAY);
    // a concave polygon is ear-clipped into a mesh, scaled and translated
    match &shapes[0].1 {
        egui::Shape::Mesh(m) => {
            assert_eq!(m.vertices.len(), 5);
            assert_eq!(m.indices.len(), 9, "three triangles");
            assert_eq!(m.vertices[1].pos, pos2(120.0, 0.0));
        }
        other => panic!("{other:?}"),
    }
    // setTransform resets to the canvas origin: the arc is around (50, 50)
    match &shapes[1].1 {
        egui::Shape::Path(p) => assert!(p.points.iter().all(|q| (q.distance(pos2(50.0, 50.0)) - 10.0).abs() < 0.01)),
        other => panic!("{other:?}"),
    }
    match &shapes[2].1 {
        egui::Shape::Text(t) => {
            assert!((t.pos.x + t.galley.size().x / 2.0 - 50.0).abs() < 0.5, "centred");
            assert_eq!(t.pos.y, 80.0, "top baseline");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn panel_resizes_by_its_edge() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let row = rig.leaf(col, "box", &[("flex-direction", s("row")), ("flex-grow", Value::F64(1.0))]);
    let panel = rig.leaf(row, "panel", &[("size", Value::F64(120.0)), ("resizable", Value::Bool(true)), ("min-width", Value::F64(60.0))]);
    let inner = rig.leaf(panel, "label", &[("text", s("Inside"))]);
    let body = rig.leaf(row, "label", &[("text", s("Body")), ("flex-grow", Value::F64(1.0))]);
    rig.listen(panel, "changed");
    rig.observe(panel, "size", true);
    rig.run();
    let pr = rig.backend.with_scene(|sc| sc.n(panel).rect);
    assert_eq!(pr.width(), 120.0);
    rig.harness.get_by_label("Inside");
    let edge = rig.drawn(panel).right_center();
    // drag the inner (right) edge 40 points to the right
    rig.harness.hover_at(edge);
    rig.harness.run();
    press(&mut rig, edge, PointerButton::Primary, true);
    rig.harness.run();
    for dx in [10.0, 20.0, 40.0] {
        rig.harness.hover_at(edge + vec2(dx, 0.0));
        rig.harness.run();
    }
    press(&mut rig, edge + vec2(40.0, 0.0), PointerButton::Primary, false);
    rig.run();
    let log = rig.take();
    assert_eq!(rig.get(panel, "size"), Value::F64(160.0));
    assert_eq!(events(&rig, &log, panel, "changed").last(), Some(&vec![Value::F64(160.0)]));
    assert!(announced(&rig, &log, panel, "size"));
    rig.run();
    assert_eq!(rig.backend.with_scene(|sc| sc.n(panel).rect.width()), 160.0, "relaid out");
    assert_eq!(rig.backend.with_scene(|sc| sc.n(body).rect.left()), pr.left() + 160.0);
    // min-width bounds the drag
    let edge = rig.drawn(panel).right_center();
    press(&mut rig, edge, PointerButton::Primary, true);
    rig.harness.run();
    rig.harness.hover_at(edge - vec2(150.0, 0.0));
    rig.harness.run();
    press(&mut rig, edge - vec2(150.0, 0.0), PointerButton::Primary, false);
    rig.run();
    assert_eq!(rig.get(panel, "size"), Value::F64(60.0));
    let _ = inner;
}

#[test]
fn sides_put_their_children_at_both_ends() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let sides = rig.leaf(col, "sides", &[]);
    let l = rig.leaf(sides, "label", &[("text", s("Left"))]);
    let r = rig.leaf(sides, "button", &[("text", s("Right"))]);
    rig.run();
    rig.harness.get_by_label("Left");
    rig.harness.get_by_role_and_label(Role::Button, "Right");
    let (sr, lr, rr) = rig.backend.with_scene(|sc| (sc.n(sides).rect, sc.n(l).rect, sc.n(r).rect));
    assert_eq!(lr.left(), sr.left());
    assert_eq!(rr.right(), sr.right());
    assert!(lr.right() < rr.left());
}

#[test]
fn frame_pads_its_content_by_its_margins_and_caption() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let f = rig.leaf(col, "frame", &[]);
    let inner = rig.leaf(f, "label", &[("text", s("Framed"))]);
    rig.run();
    rig.harness.get_by_label("Framed");
    let m = egui::Frame::group(&egui::Style::default()).total_margin();
    let (fr, ir) = rig.backend.with_scene(|sc| (sc.n(f).rect, sc.n(inner).rect));
    assert_eq!(ir.min, fr.min + vec2(m.left, m.top));
    // a caption adds a row above the content
    rig.set(f, "label", s("Group"));
    rig.run();
    let (fr, ir) = rig.backend.with_scene(|sc| (sc.n(f).rect, sc.n(inner).rect));
    // (own frame, OW1a2: the caption's own row plus the UA `gap`; the egui wrapper: `interact_size.y`)
    assert_eq!(ir.min.x, fr.min.x + m.left);
    assert!(ir.min.y >= fr.min.y + m.top + 14.0 && ir.min.y <= fr.min.y + m.top + 26.0, "{fr:?} {ir:?}");
    // no chrome: the content is at the frame's corner
    rig.set(f, "frame-style", s("none"));
    rig.set(f, "label", s(""));
    rig.run();
    let (fr, ir) = rig.backend.with_scene(|sc| (sc.n(f).rect, sc.n(inner).rect));
    assert_eq!(ir.min, fr.min);
}

#[test]
fn stack_shows_one_child() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let st = rig.leaf(col, "stack", &[("align-self", s("start"))]);
    let one = rig.leaf(st, "button", &[("text", s("One")), ("anchor", s("one"))]);
    let two = rig.leaf(st, "label", &[("text", s("Two, a longer page")), ("anchor", s("two"))]);
    rig.run();
    assert!(rig.harness.query_by_label("One").is_some());
    assert!(rig.harness.query_by_label("Two, a longer page").is_none(), "only the visible child is drawn");
    // homogeneous: sized to the largest child, the visible one fills it
    let (sr, or, tw) = rig.backend.with_scene(|sc| (sc.n(st).rect, sc.n(one).rect, sc.n(two).intrinsic.unwrap_or_default().x));
    assert!(sr.width() >= tw.floor(), "{sr:?} {tw}");
    assert_eq!(or, sr);
    // by name (the child's anchor)
    rig.set(st, "visible-child-name", s("two"));
    rig.run();
    assert!(rig.harness.query_by_label("One").is_none());
    rig.harness.get_by_label("Two, a longer page");
    // by index, when the name is empty; not homogeneous: sized to the visible child
    rig.set(st, "visible-child-name", s(""));
    rig.set(st, "visible-index", Value::I64(0));
    rig.set(st, "homogeneous", Value::Bool(false));
    rig.run();
    rig.harness.get_by_role_and_label(Role::Button, "One");
    let (sr, or) = rig.backend.with_scene(|sc| (sc.n(st).rect, sc.n(one).rect));
    assert_eq!(sr.size(), or.size());
    assert!(sr.width() < tw.floor());
}

/// Bad input never panics: images that do not load get a placeholder, NaN and infinities in the
/// numeric props fall back, a display list with odd ops and overflowing transforms replays.
#[test]
fn bad_input_falls_back_and_never_panics() {
    let mut rig = Rig::new();
    let nan = Value::F64(f64::NAN);
    let inf = Value::F64(f64::INFINITY);
    // wider than the max texture side: scaled down to fit
    let side = rig.harness.ctx.input(|i| i.max_texture_side) as u32;
    rig.harness.ctx.include_bytes("bytes://wide.png", png(side + 100, 2, [0, 255, 0, 255]));
    let (_w, col) = rig.window();
    let missing = rig.leaf(col, "image", &[("source", s("/no/such/dir/missing.png")), ("align-self", s("start"))]);
    let bad_url = rig.leaf(col, "image", &[("source", s("nope://what?")), ("align-self", s("start"))]);
    let not_png = rig.leaf(col, "image", &[("source", s("/no/such/file.jpg")), ("align-self", s("start"))]);
    let wide = rig.leaf(col, "image", &[("source", s("bytes://wide.png")), ("align-self", s("start"))]);
    let rs = rig.leaf(col, "range-slider", &[("text", s("R")), ("min", nan.clone()), ("max", inf.clone()), ("low", nan.clone()), ("high", Value::F64(f64::NEG_INFINITY)), ("step", nan.clone())]);
    let rs2 = rig.leaf(col, "range-slider", &[("text", s("Q")), ("min", Value::F64(10.0)), ("max", Value::F64(-10.0)), ("low", Value::F64(5.0)), ("high", Value::F64(1.0))]);
    let dv = rig.leaf(col, "drag-value", &[("value", nan.clone()), ("min", nan.clone()), ("max", inf.clone()), ("speed", nan.clone()), ("accessible-name", s("D"))]);
    rig.listen(dv, "changed");
    let pb = rig.leaf(col, "progress-bar", &[("fraction", nan.clone()), ("show-percentage", Value::Bool(true))]);
    let pb2 = rig.leaf(col, "progress-bar", &[("fraction", inf.clone()), ("animate", Value::Bool(true))]);
    let sp = rig.leaf(col, "spinner", &[("size", inf.clone()), ("align-self", s("start"))]);
    let cv = rig.leaf(col, "canvas", &[("content-width", inf.clone()), ("content-height", nan.clone()), ("ops", s("[]"))]);
    let panel = rig.leaf(col, "panel", &[("side", s("top")), ("size", inf.clone())]);
    rig.harness.run_steps(4);
    for h in [missing, bad_url, not_png] {
        assert_eq!(rig.drawn(h).size(), vec2(16.0, 16.0), "a placeholder");
    }
    let w = rig.drawn(wide).size();
    assert!(w.x <= side as f32 && w.x > 0.0, "{w:?}");
    for h in [rs, rs2, dv, pb, pb2, sp, cv, panel] {
        let r = rig.drawn(h);
        assert!(r.min.is_finite() && r.max.is_finite(), "{r:?}");
    }
    assert_eq!((rig.get(sp, "size"), rig.get(panel, "size"), rig.get(cv, "content-width")), (Value::Null, Value::Null, Value::F64(0.0)));
    // the widgets still work, and NaN never comes back from them
    rig.harness.get_by_role_and_label(Role::SpinButton, "D").set_value_accesskit(4.0);
    rig.harness.get_by_role_and_label(Role::Slider, "R low").set_value_accesskit(0.5);
    rig.harness.get_by_role_and_label(Role::Slider, "Q high").set_value_accesskit(7.0);
    rig.harness.run_steps(4);
    assert_eq!(events(&rig, &rig.take(), dv, "changed"), vec![vec![Value::F64(4.0)]]);
    for (h, p) in [(rs, "low"), (rs, "high"), (rs2, "low"), (rs2, "high")] {
        assert!(matches!(rig.get(h, p), Value::F64(x) if x.is_finite()), "{p}: {:?}", rig.get(h, p));
    }
    // odd display lists replay
    let ops = r#"[[], 1, "x", ["arc"], ["moveTo", "a", 1], ["scale", 1e300, 1e300], ["font", "NaNpx sans"], ["setLineDash", [1e-9, 1e-9]],
        ["lineWidth", 1e300], ["beginPath"], ["moveTo", 0, 0], ["lineTo", 1e300, 1e300], ["lineTo", -1e300, 1e300], ["closePath"], ["fill"], ["stroke"],
        ["fillText", "big", 1, 1], ["arc", 0, 0, 1e300, 0, 1e300], ["clip"], ["fillRect", 0, 0, 1, 1], ["restore"], ["restore"]]"#;
    rig.set(cv, "content-width", Value::F64(50.0));
    rig.set(cv, "content-height", Value::F64(50.0));
    rig.set(cv, "ops", s(ops));
    rig.harness.run_steps(2);
    let list: serde_json::Value = serde_json::from_str(ops).unwrap();
    let r = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(50.0, 50.0));
    for (_, shape) in svelters_egui::elements::canvas::shapes(&rig.harness.ctx, r.min, r, list.as_array().unwrap(), egui::Color32::GRAY) {
        assert!(shape.visual_bounding_rect().is_finite() || shape.visual_bounding_rect() == egui::Rect::NOTHING, "{shape:?}");
    }
}

/// W5: select-to-copy and middle-click paste through an installed provider (Linux only).
#[cfg(target_os = "linux")]
#[test]
fn text_edit_uses_the_primary_selection() {
    use std::sync::{Arc, Mutex};
    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<Vec<String>>>);
    impl svelters_egui::primary::PrimarySelection for Fake {
        fn set(&self, text: &str) {
            self.0.lock().unwrap().push(text.to_string());
        }
        fn get(&self) -> Option<String> {
            Some("PASTE".into())
        }
    }
    let fake = Fake::default();
    svelters_egui::primary::install(fake.clone());
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let t = rig.leaf(col, "text-edit", &[("text", s("hello")), ("hint-text", s("Name"))]);
    rig.listen(t, "changed");
    rig.run();
    // select everything: the selection becomes the primary selection
    rig.click(t);
    rig.harness.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    rig.run();
    assert_eq!(fake.0.lock().unwrap().last().map(String::as_str), Some("hello"));
    // a middle click at the start pastes there
    let at = rig.drawn(t).left_center() + vec2(5.0, 0.0);
    rig.harness.hover_at(at);
    press(&mut rig, at, PointerButton::Middle, true);
    press(&mut rig, at, PointerButton::Middle, false);
    rig.run();
    let log = rig.take();
    assert_eq!(rig.get(t, "text"), s("PASTEhello"));
    assert_eq!(events(&rig, &log, t, "changed").last(), Some(&vec![s("PASTEhello")]));
    svelters_egui::primary::uninstall();
}

/// FX3s (from FX3): under eframe, egui-winit sends `MiddleClickPaste` with the middle press and
/// egui 0.37's `TextEdit` pastes it; W5 must not paste PRIMARY a second time on the release.
#[cfg(target_os = "linux")]
#[test]
fn text_edit_middle_click_pastes_once() {
    struct Fake;
    impl svelters_egui::primary::PrimarySelection for Fake {
        fn set(&self, _: &str) {}
        fn get(&self) -> Option<String> {
            Some("P".into())
        }
    }
    svelters_egui::primary::install(Fake);
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let t = rig.leaf(col, "text-edit", &[("text", s("hello")), ("accessible-name", s("field"))]);
    rig.run();
    rig.click(t);
    rig.run();
    let at = rig.drawn(t).left_center() + vec2(5.0, 0.0);
    rig.harness.hover_at(at);
    // egui-winit sends MiddleClickPaste with the press
    press(&mut rig, at, PointerButton::Middle, true);
    rig.harness.event(egui::Event::MiddleClickPaste { pos: at, text: "P".into() });
    press(&mut rig, at, PointerButton::Middle, false);
    rig.run();
    assert_eq!(rig.get(t, "text"), s("Phello"));
    svelters_egui::primary::uninstall();
}
