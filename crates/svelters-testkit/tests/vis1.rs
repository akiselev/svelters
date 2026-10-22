//! VIS1 (plans/milestones/VIS1-visual-conformance.md): the default look against VS Code 1.133.
//!
//! The gallery page `VisControls` (fixtures/gallery-egui, `pnpm build`) is mounted in an embed slot
//! with `svelters_egui::theme::Theme::vscode` installed (affordances off: VS Code's exact colours), once per theme (Dark Modern, Light Modern). Each widget
//! is driven into its states (pointer over, pointer down, keyboard focus; disabled and checked are
//! props) and measured from what egui paints (the frame's `RectShape`, the checkbox's box, the first
//! `TextShape`): height, corner radius, border width, fill, border and text colour, font size. The
//! measurements go to `target/vis1/<theme>/metrics.json` in the schema of the VS Code capture
//! (`conformance/visual/vscode/<theme>/metrics.json`), and each mapped check is compared:
//! sizes within 1 px, colours within ΔE2000 2 (composited over the theme's `--sv-bg`), font size exact.
//!
//! The gate is a ratchet over `conformance/visual/baseline.json` (the checks that passed when it was
//! recorded must still pass; `SVELTERS_UPDATE=1` records new passes) plus a floor on the pass rate.
//! Pixels are only for review: with `--features snapshots` each probe is also rendered (wgpu,
//! lavapipe in CI) and cropped to `target/vis1/<theme>/<element>-<state>.png`, which
//! `conformance/visual/tools/sheet.py` puts next to the VS Code crops.

use svelters_egui::egui;
use egui::epaint::{ClippedShape, RectShape, Shape, TextShape};
use egui::{Color32, Rect};
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use svelters_egui::embed::{EmbedConfig, EmbedEvent, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::theme::{Contrast, Theme, parse_color};
use svelters_host::HostConfig;
use svelters_style::ColorScheme;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle() -> PathBuf {
    let p = root().join("fixtures/gallery-egui/dist/embed.js");
    assert!(p.exists(), "{} is missing: run `pnpm --filter ./fixtures/gallery-egui build` (needs svelters-compile)", p.display());
    p
}

const SIZE: egui::Vec2 = egui::vec2(1100.0, 720.0);

/// `VIS1_STOCK=1`: measure egui's stock look instead (the "before" of the milestone record), into
/// `target/vis1-stock`, without the gate.
fn stock() -> bool {
    std::env::var_os("VIS1_STOCK").is_some()
}

/// The theme the gate checks: VS Code's set without the affordance overlay (DESIGN.md §4.3).
fn base_theme(scheme: ColorScheme) -> Theme {
    Theme::vscode(scheme, Contrast::Normal).with_affordances(false)
}

fn page(scheme: ColorScheme) -> EmbedHarness {
    let cfg = EmbedConfig { host: HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..HostConfig::default() }, ..EmbedConfig::default() };
    let mut eh = EmbedHarness::new(SIZE, cfg).unwrap();
    eh.harness.ctx.set_theme(match scheme {
        ColorScheme::Dark => egui::Theme::Dark,
        ColorScheme::Light => egui::Theme::Light,
    });
    if !stock() {
        eh.embed.backend().set_theme(&base_theme(scheme));
    }
    let e = eh.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let slot = e.create_slot("VisControls", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        // the page sits on the side bar colour, as cadatomic's panels do
        egui::Frame::new().fill(ui.visuals().panel_fill).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            e.show_slot(ui, slot);
        });
    });
    e.mount(MountSpec::new(entry, "VisControls", slot, "VisControls").props(Props::new())).unwrap();
    eh.run();
    eh.run();
    let ev = e.take_events();
    assert!(!ev.iter().any(|x| matches!(x, EmbedEvent::Faulted { .. })), "{ev:?}");
    eh
}

// ---- measuring ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    /// The widget's frame: the largest rect egui paints within its response rect.
    Frame,
    /// A check box's box (a square of `icon_width`), or a radio's circle.
    Box,
    /// The fill of a progress bar (the second rect).
    Fill,
}

fn flatten<'a>(s: &'a Shape, out: &mut Vec<&'a Shape>) {
    match s {
        Shape::Vec(v) => v.iter().for_each(|s| flatten(s, out)),
        s => out.push(s),
    }
}

fn css(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    format!("rgba({r}, {g}, {b}, {})", (a as f32 / 255.0 * 1000.0).round() / 1000.0)
}

/// What egui painted for the widget at `rect`, in the capture's schema.
fn measure(shapes: &[ClippedShape], rect: Rect, part: Part) -> Value {
    let mut all = Vec::new();
    for c in shapes {
        flatten(&c.shape, &mut all);
    }
    let inside = |r: Rect| rect.expand(2.0).contains_rect(r);
    let rects: Vec<&RectShape> = all
        .iter()
        .filter_map(|s| match s {
            Shape::Rect(r) if inside(r.rect) && (r.fill.a() > 0 || r.stroke.width > 0.0) => Some(r),
            _ => None,
        })
        .collect();
    let circles: Vec<(Rect, Color32, egui::Stroke)> = all
        .iter()
        .filter_map(|s| match s {
            Shape::Circle(c) if inside(Rect::from_center_size(c.center, egui::Vec2::splat(c.radius * 2.0))) => {
                Some((Rect::from_center_size(c.center, egui::Vec2::splat(c.radius * 2.0)), c.fill, c.stroke))
            }
            _ => None,
        })
        .collect();
    let texts: Vec<&TextShape> = all
        .iter()
        .filter_map(|s| match s {
            Shape::Text(t) if inside(t.galley.rect.translate(t.pos.to_vec2())) && !t.galley.text().trim().is_empty() => Some(t),
            _ => None,
        })
        .collect();
    let area = |r: &Rect| r.width() * r.height();
    let chosen: Option<(Rect, Color32, egui::Stroke, f32)> = match part {
        // the outermost filled rect gives the fill, the outermost stroked one the border (an
        // `invalid` outline is drawn over the frame, as VS Code's error border replaces the border)
        Part::Frame => {
            let big = rects.iter().map(|r| area(&r.rect)).fold(0.0, f32::max);
            let filled = rects.iter().filter(|r| r.fill.a() > 0 && area(&r.rect) >= 0.6 * big).max_by(|a, b| area(&a.rect).total_cmp(&area(&b.rect)));
            let stroked = rects.iter().filter(|r| r.stroke.width > 0.0 && r.stroke.color.a() > 0).max_by(|a, b| area(&a.rect).total_cmp(&area(&b.rect)));
            let outer = rects.iter().max_by(|a, b| area(&a.rect).total_cmp(&area(&b.rect)));
            outer.map(|o| {
                let f = filled.copied().unwrap_or(*o);
                (f.rect, filled.map_or(Color32::TRANSPARENT, |r| r.fill), stroked.map_or(egui::Stroke::NONE, |r| r.stroke), f.corner_radius.nw as f32)
            })
        }
        Part::Fill => rects.iter().filter(|r| r.fill.a() > 0).min_by(|a, b| area(&a.rect).total_cmp(&area(&b.rect))).map(|r| (r.rect, r.fill, r.stroke, r.corner_radius.nw as f32)),
        Part::Box => rects
            .iter()
            .filter(|r| (r.rect.width() - r.rect.height()).abs() < 1.0 && r.rect.width() >= 8.0)
            .max_by(|a, b| area(&a.rect).total_cmp(&area(&b.rect)))
            .map(|r| (r.rect, r.fill, r.stroke, r.corner_radius.nw as f32))
            .or_else(|| circles.iter().max_by(|a, b| area(&a.0).total_cmp(&area(&b.0))).map(|c| (c.0, c.1, c.2, c.0.width() / 2.0))),
    };
    let text = texts.first().map(|t| {
        let f = t.galley.job.sections.first().map(|s| s.format.clone()).unwrap_or_default();
        let col = t.override_text_color.unwrap_or(if f.color == Color32::PLACEHOLDER { t.fallback_color } else { f.color });
        (col.gamma_multiply(t.opacity_factor), f.font_id.size, t.galley.text().to_string())
    });
    let (r, fill, stroke, radius) = chosen.unwrap_or((rect, Color32::TRANSPARENT, egui::Stroke::NONE, 0.0));
    json!({
        "rect": { "x": r.min.x, "y": r.min.y, "w": r.width(), "h": r.height() },
        "widget_rect": { "x": rect.min.x, "y": rect.min.y, "w": rect.width(), "h": rect.height() },
        "radius": radius,
        "border_width": stroke.width,
        "border_color": if stroke.width > 0.0 { Value::from(css(stroke.color)) } else { Value::Null },
        "bg": css(fill),
        "fg": text.as_ref().map(|t| css(t.0)),
        "font_size": text.as_ref().map(|t| t.1),
        "text": text.as_ref().map(|t| t.2.clone()),
        "opacity": 1.0,
    })
}

// ---- probes ----

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
enum State {
    Rest,
    Hover,
    Pressed,
    Focus,
}

impl State {
    fn name(self) -> &'static str {
        match self {
            State::Rest => "rest",
            State::Hover => "hover",
            State::Pressed => "pressed",
            State::Focus => "focus",
        }
    }
}

/// One measured widget state and the VS Code item it is compared with.
struct Probe {
    /// The widget's accessible name on the page (`<element>:<variant>`), or a tab's title.
    node: &'static str,
    state: State,
    part: Part,
    /// The reference item (`<element>-<state>` in the capture).
    reference: &'static str,
    /// Which checks apply: h(eight), r(adius), b(order width), bg, bc (border colour), fg, fs (font size).
    checks: &'static [&'static str],
    /// A colour the reference crop missed, from the theme's resolved variables: (check, variable).
    var: Option<(&'static str, &'static str)>,
}

const FRAME: &[&str] = &["h", "r", "b", "bg", "bc", "fg", "fs"];
const COLORS: &[&str] = &["bg", "bc", "fg"];

#[rustfmt::skip]
fn probes() -> Vec<Probe> {
    use Part::*;
    use State::*;
    let p = |node, state, part, reference, checks| Probe { node, state, part, reference, checks, var: None };
    let mut v = vec![
        // buttons: VS Code keeps the hover fill while the pointer is down, and shows it on focus
        p("button-primary:a", Rest, Frame, "button-primary-rest", FRAME),
        p("button-primary:a", Hover, Frame, "button-primary-hover", FRAME),
        p("button-primary:a", Pressed, Frame, "button-primary-hover", COLORS),
        p("button-primary:a", Focus, Frame, "button-primary-focus", COLORS),
        p("button-primary:disabled", Rest, Frame, "button-primary-disabled", &["h", "r", "bg", "fg"]),
        p("button-secondary:a", Rest, Frame, "button-secondary-rest", FRAME),
        p("button-secondary:a", Hover, Frame, "button-secondary-hover", FRAME),
        p("button-secondary:a", Pressed, Frame, "button-secondary-hover", COLORS),
        p("button-secondary:a", Focus, Frame, "button-secondary-focus", COLORS),
        p("button-secondary:disabled", Rest, Frame, "button-secondary-disabled", &["h", "r", "bg", "fg"]),
        // checkbox box (18x18 radius 3); radio buttons take the checkbox's colours
        p("checkbox:a", Rest, Box, "checkbox-rest", &["h", "r", "b", "bg", "bc"]),
        p("checkbox:a", Hover, Box, "checkbox-hover", &["h", "bg", "bc"]),
        p("checkbox:a", Focus, Box, "checkbox-focus", &["bg", "bc"]),
        p("checkbox:checked", Rest, Box, "checkbox-checked", &["h", "bg", "bc"]),
        p("checkbox:disabled", Rest, Box, "checkbox-disabled", &["h", "bg", "bc"]),
        p("radio:a", Rest, Box, "checkbox-rest", &["h", "bg", "bc"]),
        p("radio:checked", Rest, Box, "checkbox-checked", &["bg", "bc"]),
        // the input option toggle (Match Case)
        p("toggle:a", Rest, Frame, "toggle-rest", &["r", "bg", "fg"]),
        p("toggle:a", Hover, Frame, "toggle-hover", &["r", "bg", "fg"]),
        p("toggle:checked", Rest, Frame, "toggle-checked", &["r", "b", "bg", "bc", "fg"]),
        // input box
        p("input:a", Rest, Frame, "input-rest", &["h", "r", "b", "bg", "bc"]),
        p("input:a", Focus, Frame, "input-focus", &["h", "r", "b", "bg", "oc"]),
        p("input:filled", Rest, Frame, "input-rest", &["h", "r", "b", "bg", "bc", "fg", "fs"]),
        p("input:invalid", Rest, Frame, "input-invalid", &["h", "r", "bg"]),
        p("input:disabled", Rest, Frame, "input-disabled", &["h", "r", "bg", "bc"]),
        // select box
        p("select:a", Rest, Frame, "select-rest", FRAME),
        p("select:a", Hover, Frame, "select-hover", FRAME),
        p("select:a", Focus, Frame, "select-focus", &["h", "r", "b", "bg"]),
        // list rows (selectable labels)
        p("list-row:a", Rest, Frame, "list-row-rest", &["fg", "fs"]),
        Probe { node: "list-row:a", state: Hover, part: Frame, reference: "list-row-hover", checks: &["bg", "fg", "fs"], var: Some(("bg", "--vscode-list-hoverBackground")) },
        p("list-row:selected", Rest, Frame, "list-row-selected-focused", &["bg", "fg", "fs"]),
        // pane header (collapsed section header)
        p("pane-header:a", Rest, Frame, "pane-header-rest", &["h", "fs"]),
        // tabs
        p("long.txt", Rest, Frame, "tab-active", &["h", "bg", "fg", "fs"]),
        p("main.ts", Rest, Frame, "tab-inactive", &["h", "bg", "fg", "fs"]),
        p("main.ts", Hover, Frame, "tab-hover", &["bg", "fg"]),
        // progress fill
        Probe { node: "progress:a", state: Rest, part: Fill, reference: "progress-rest", checks: &["bg"], var: Some(("bg", "--vscode-progressBar-background")) },
    ];
    v.sort_by_key(|p| p.state);
    v
}

fn find_rect(eh: &EmbedHarness, name: &str) -> Option<Rect> {
    eh.harness.query_by_label(name).map(|n| n.rect())
}

fn node_focus(eh: &EmbedHarness, name: &str) {
    if let Some(n) = eh.harness.query_by_label(name) {
        n.focus();
    }
}

fn run_theme(scheme: ColorScheme, theme: &str, out: &Path) -> Vec<Value> {
    let mut eh = page(scheme);
    std::fs::create_dir_all(out).unwrap();
    let mut items = Vec::new();
    for pr in probes() {
        let Some(rect) = find_rect(&eh, pr.node) else {
            items.push(json!({ "element": pr.node, "state": pr.state.name(), "metrics": null }));
            continue;
        };
        let c = rect.center();
        match pr.state {
            State::Rest => eh.harness.event(egui::Event::PointerGone),
            State::Hover => eh.harness.hover_at(c),
            State::Pressed => {
                eh.harness.hover_at(c);
                eh.harness.drag_at(c);
            }
            State::Focus => {
                eh.harness.event(egui::Event::PointerGone);
                node_focus(&eh, pr.node);
            }
        }
        for _ in 0..4 {
            eh.harness.step();
        }
        let m = measure(&eh.harness.output().shapes, rect, pr.part);
        let file = format!("{}-{}", pr.node.replace(':', "-").replace('.', "_"), pr.state.name());
        #[cfg(feature = "snapshots")]
        {
            let img = eh.harness.render().expect("render");
            let r = rect.expand(4.0).intersect(Rect::from_min_size(egui::Pos2::ZERO, SIZE));
            let crop = image::imageops::crop_imm(&img, r.min.x as u32, r.min.y as u32, r.width() as u32, r.height() as u32).to_image();
            crop.save(out.join(format!("{file}.png"))).unwrap();
        }
        if pr.state == State::Pressed {
            // release outside the widget: no click
            eh.harness.event(egui::Event::PointerMoved(egui::pos2(SIZE.x - 2.0, SIZE.y - 2.0)));
            eh.harness.drop_at(egui::pos2(SIZE.x - 2.0, SIZE.y - 2.0));
            eh.harness.step();
        }
        items.push(json!({ "element": pr.node, "state": pr.state.name(), "reference": pr.reference, "png": format!("{file}.png"), "metrics": m }));
    }
    let doc = json!({ "meta": { "theme": theme, "egui": "0.37.0", "scale": 1 }, "items": items });
    std::fs::write(out.join("metrics.json"), serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    items
}

// ---- comparing ----

fn parse_css_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let c = parse_color(&format!("#{h}"))?;
        let [r, g, b, a] = c.to_srgba_unmultiplied();
        return Some([r as f32, g as f32, b as f32, a as f32 / 255.0]);
    }
    let inner = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?.strip_suffix(')')?;
    let v: Vec<f32> = inner.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    match v.as_slice() {
        [r, g, b] => Some([*r, *g, *b, 1.0]),
        [r, g, b, a] => Some([*r, *g, *b, *a]),
        _ => None,
    }
}

/// `c` with `opacity` composited over the opaque `under`.
fn over(c: [f32; 4], opacity: f32, under: [f32; 4]) -> [f32; 3] {
    let a = c[3] * opacity;
    [0, 1, 2].map(|k| c[k] * a + under[k] * (1.0 - a))
}

fn lab(c: [f32; 3]) -> [f32; 3] {
    let lin = |v: f32| {
        let v = v / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(c[0]), lin(c[1]), lin(c[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    [116.0 * f(y) - 16.0, 500.0 * (f(x) - f(y)), 200.0 * (f(y) - f(z))]
}

/// CIEDE2000.
fn delta_e(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (l1, a1, b1) = (a[0] as f64, a[1] as f64, a[2] as f64);
    let (l2, a2, b2) = (b[0] as f64, b[1] as f64, b[2] as f64);
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = (a1 * (1.0 + g), a2 * (1.0 + g));
    let (c1p, c2p) = ((a1p * a1p + b1 * b1).sqrt(), (a2p * a2p + b2 * b2).sqrt());
    let hp = |b: f64, a: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            let h = b.atan2(a).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let (h1p, h2p) = (hp(b1, a1p), hp(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dhp = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dh = 2.0 * (c1p * c2p).sqrt() * (dhp.to_radians() / 2.0).sin();
    let lm = (l1 + l2) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hm - 30.0).to_radians().cos() + 0.24 * (2.0 * hm).to_radians().cos() + 0.32 * (3.0 * hm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hm - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cmp.powi(7) / (cmp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let rt = -(2.0 * dtheta.to_radians()).sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rt * (dc / sc) * (dh / sh)).sqrt() as f32
}

struct Check {
    id: String,
    pass: bool,
    ours: String,
    theirs: String,
}

fn compare(theme: &str, ours: &[Value], reference: &Value, backdrop: [f32; 4]) -> Vec<Check> {
    let refs: BTreeMap<String, &Value> = reference["items"].as_array().unwrap().iter().map(|i| (format!("{}-{}", i["element"].as_str().unwrap(), i["state"].as_str().unwrap()), &i["metrics"])).collect();
    let vars = &reference["vars"];
    let mut out = Vec::new();
    for (pr, item) in probes().iter().zip(ours) {
        let m = &item["metrics"];
        let empty = json!({});
        // an item the capture could not reach still has its colour in the theme's variables
        let r = refs.get(pr.reference).copied().filter(|r| !r.is_null()).unwrap_or(if pr.var.is_some() { &empty } else { &Value::Null });
        for &k in pr.checks {
            let id = format!("{theme}/{}/{}/{k}", pr.node, pr.state.name());
            if m.is_null() || r.is_null() {
                out.push(Check { id, pass: false, ours: if m.is_null() { "missing".into() } else { "-".into() }, theirs: if r.is_null() { "no reference".into() } else { "-".into() } });
                continue;
            }
            let num = |v: &Value, path: &[&str]| path.iter().fold(v, |v, p| &v[*p]).as_f64();
            let (pass, o, t) = match k {
                "h" | "r" | "b" | "fs" => {
                    let path: &[&str] = match k {
                        "h" => &["rect", "h"],
                        "r" => &["radius"],
                        "b" => &["border_width"],
                        _ => &["font_size"],
                    };
                    let (o, t) = (num(m, path), num(r, path));
                    let tol = if k == "fs" { 0.01 } else { 1.0 };
                    let pass = matches!((o, t), (Some(o), Some(t)) if (o - t).abs() <= tol);
                    (pass, format!("{o:?}"), format!("{t:?}"))
                }
                _ => {
                    // oc: the focus ring, VS Code's outline colour against our (focused) border
                    let field = match k {
                        "bg" => "bg",
                        "bc" | "oc" => "border_color",
                        _ => "fg",
                    };
                    let theirs = match pr.var {
                        Some((c, var)) if c == k => vars[var].as_str().map(str::to_string),
                        _ if k == "oc" => r["outline"].as_str().and_then(|s| s.find("rgb").map(|i| s[i..s[i..].find(')').map_or(s.len(), |j| i + j + 1)].to_string())),
                        _ => r[field].as_str().map(str::to_string),
                    }
                    .unwrap_or_else(|| "rgba(0, 0, 0, 0)".into());
                    let o = m[field].as_str().unwrap_or("rgba(0, 0, 0, 0)").to_string();
                    let op = r["opacity"].as_f64().unwrap_or(1.0) as f32;
                    let (Some(oc), Some(tc)) = (parse_css_color(&o), parse_css_color(&theirs)) else {
                        out.push(Check { id, pass: false, ours: o, theirs });
                        continue;
                    };
                    let de = delta_e(lab(over(oc, 1.0, backdrop)), lab(over(tc, op, backdrop)));
                    (de <= 2.0, format!("{o} (ΔE {de:.1})"), if op < 1.0 { format!("{theirs} at opacity {op}") } else { theirs })
                }
            };
            out.push(Check { id, pass, ours: o, theirs: t });
        }
    }
    out
}

#[test]
fn default_look_matches_vscode() {
    let target = root().join(if stock() { "target/vis1-stock" } else { "target/vis1" });
    let mut checks = Vec::new();
    for (scheme, theme) in [(ColorScheme::Dark, "dark-modern"), (ColorScheme::Light, "light-modern")] {
        let ours = run_theme(scheme, theme, &target.join(theme));
        let reference: Value = serde_json::from_str(&std::fs::read_to_string(root().join(format!("conformance/visual/vscode/{theme}/metrics.json"))).unwrap()).unwrap();
        let bg = parse_css_color(&css(base_theme(scheme).colors().bg)).unwrap();
        checks.extend(compare(theme, &ours, &reference, bg));
    }
    // the report: every check, failures first
    let mut md = String::from("| check | ours | VS Code |\n|---|---|---|\n");
    for c in checks.iter().filter(|c| !c.pass).chain(checks.iter().filter(|c| c.pass)) {
        md += &format!("| {} {} | {} | {} |\n", if c.pass { "pass" } else { "FAIL" }, c.id, c.ours, c.theirs);
    }
    let passed = checks.iter().filter(|c| c.pass).count();
    md = format!("{passed} of {} checks pass\n\n{md}", checks.len());
    std::fs::write(target.join("report.md"), &md).unwrap();
    println!("{md}");
    if stock() {
        return;
    }

    // the ratchet
    let base_path = root().join("conformance/visual/baseline.json");
    let baseline: Vec<String> = std::fs::read_to_string(&base_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let now: Vec<String> = checks.iter().filter(|c| c.pass).map(|c| c.id.clone()).collect();
    let regressed: Vec<&String> = baseline.iter().filter(|id| !now.contains(id)).collect();
    if std::env::var_os("SVELTERS_UPDATE").is_some() {
        std::fs::write(&base_path, serde_json::to_string_pretty(&now).unwrap() + "\n").unwrap();
    } else {
        assert!(regressed.is_empty(), "checks that passed in conformance/visual/baseline.json fail now: {regressed:#?}\n(report: target/vis1/report.md)");
        let new: Vec<&String> = now.iter().filter(|id| !baseline.contains(id)).collect();
        if !new.is_empty() {
            println!("new passes (record with SVELTERS_UPDATE=1): {new:#?}");
        }
    }
    let rate = passed as f32 / checks.len() as f32;
    assert!(rate >= MIN_PASS_RATE, "{passed} of {} checks pass ({:.0}%), below the floor of {:.0}%", checks.len(), rate * 100.0, MIN_PASS_RATE * 100.0);
}

/// DESIGN.md §6.5: at least 90% of the mapped numeric checks pass on the exact base.
const MIN_PASS_RATE: f32 = 0.9;

#[test]
fn delta_e_matches_the_reference_pairs() {
    // Sharma, Wu, Dalal (2005), test pairs 1 and 17
    assert!((delta_e([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485]) - 2.0425).abs() < 1e-3);
    assert!((delta_e([50.0, 2.5, 0.0], [73.0, 25.0, -18.0]) - 27.1492).abs() < 1e-3);
}
