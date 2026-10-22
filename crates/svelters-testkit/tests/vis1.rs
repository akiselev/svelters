//! VIS1c (plans/milestones/VIS1c-visual-gate.md): svelters' own widgets against VS Code 1.133.
//!
//! The gallery page `VisControls` (fixtures/gallery-egui, `pnpm build`) is mounted in an embed slot
//! with every own widget switched on (`KindSet::All`) and the built-in VS Code theme installed
//! without the affordance overlay (`Theme::vscode(..).with_affordances(false)`: VS Code's exact
//! colours), once per theme (Dark Modern, Light Modern). Each widget is found by its accessible name
//! (`<element>:<variant>`), driven into a state (pointer over, pointer down, AccessKit focus;
//! disabled, checked and invalid are props) and measured from svelters' **PaintTrace** (OW0b,
//! debug builds): the node's box or a named part's box (`::part(indicator)`, `thumb`, `sash`), its
//! first text, its outline (the focus ring). Nothing is guessed from epaint shapes: a kind that
//! still draws through its egui wrapper traces nothing and its checks fail as "egui wrapper".
//!
//! The checks compare CSS px and colours with the reference metrics (`metrics.json` of
//! `conformance/visual/vscode/<theme>/` on Linux and Windows, `vscode-macos/` on macOS, or
//! `VIS_REFS=windows|macos`):
//! - `h`, `w`, `t` (height, width, thickness = the smaller side), `r` (radius), `b` (border width),
//!   `p` (padding, all four sides), `ow`, `oo` (outline width and offset): within 1 px (`p`, `b`,
//!   `ow`, `oo`: 0.5 px);
//! - `wt` (a text-sized width) and `tw` (the text's width): within 15% of the reference text width
//!   plus 1 px. Fonts differ (Segoe UI on Windows, SF on macOS, svelters' bundled fonts here);
//! - `bg`, `bc`, `fg`, `oc` (outline colour): CIEDE2000 ΔE ≤ 2, both composited over the theme's
//!   `--sv-bg`, opacity left out (it is its own check);
//! - `op` (opacity), within 0.05; `fs` (font size) within 0.01; `ta` (text alignment) exact.
//!
//! The gate is a ratchet: every check id in `conformance/visual/baseline-<refs>.json` must still
//! pass (new passes are printed; `SVELTERS_UPDATE=1` records them). It does not fail on the
//! absolute score. The score per element and theme goes to stdout, `target/vis1c/<refs>/score.md`
//! and the CI job summary; every check to `report.md`; our measurements to
//! `<theme>/metrics.json` in the references' schema. With `--features snapshots` each probe is also
//! rendered (wgpu; lavapipe in CI) and cropped to `<theme>/<element>-<state>.png` for
//! `conformance/visual/tools/sheet.py`: pixels are for review only, never compared.

use svelters_egui::egui;
use egui::{Color32, Rect};
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use svelters_egui::embed::{EmbedConfig, EmbedEvent, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::paint::{PaintTrace, TraceEntry, TraceKind, trace};
use svelters_egui::scene::W;
use svelters_egui::theme::{ColorScheme, Contrast, Theme, parse_color};
use svelters_egui::widgets::KindSet;
use svelters_host::HostConfig;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle() -> PathBuf {
    let p = root().join("fixtures/gallery-egui/dist/embed.js");
    assert!(p.exists(), "{} is missing: run `pnpm --filter ./fixtures/gallery-egui build` (needs svelters-compile)", p.display());
    p
}

const SIZE: egui::Vec2 = egui::vec2(1200.0, 720.0);

/// Which references: `windows` (Linux and Windows) or `macos`.
fn refs() -> String {
    std::env::var("VIS_REFS").unwrap_or_else(|_| if cfg!(target_os = "macos") { "macos".into() } else { "windows".into() })
}

fn refs_dir(refs: &str) -> &'static str {
    if refs == "macos" { "vscode-macos" } else { "vscode" }
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
        _ => egui::Theme::Light,
    });
    trace::enable(&eh.harness.ctx, true);
    let theme = base_theme(scheme);
    let bg = theme.colors().bg;
    eh.embed.backend().set_theme(&theme);
    eh.embed.backend().set_own_widgets(KindSet::All);
    let e = eh.embed.clone();
    let entry = e.load(bundle()).unwrap();
    let slot = e.create_slot("VisControls", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        // the page sits on the side bar colour, as cadatomic's panels do
        egui::Frame::new().fill(bg).show(ui, |ui| {
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

/// A reference value the capture lacks: a resolved `--vscode-*` variable or a constant from
/// VS Code's CSS.
#[derive(Clone, Copy)]
enum Over {
    Var(&'static str),
    Px(f64),
}

/// One measured widget state and the VS Code item it is compared with.
struct Probe {
    /// The node's accessible name on the page (`<element>:<variant>`).
    node: &'static str,
    state: State,
    /// The measured box: the node's own (`None`) or a part's (`::part(name)`).
    part: Option<&'static str>,
    /// The part whose shape gives `fg` instead of the first text (a check box's `mark`).
    fg_part: Option<&'static str>,
    /// The reference item: (element, state) in the capture.
    reference: (&'static str, &'static str),
    checks: &'static [&'static str],
    over: &'static [(&'static str, Over)],
    /// The lane that owns the kind (where its gaps go).
    lane: &'static str,
}

const BTN_REST: &[&str] = &["h", "wt", "tw", "r", "b", "p", "bg", "bc", "fg", "fs", "op"];
const BTN_HOVER: &[&str] = &["h", "r", "b", "bg", "bc", "fg"];
const COLORS: &[&str] = &["bg", "bc", "fg"];
const BTN_FOCUS: &[&str] = &["bg", "bc", "fg", "oc", "ow", "oo"];
const BTN_DISABLED: &[&str] = &["bg", "bc", "fg", "op"];
const RING: &[&str] = &["oc", "ow", "oo"];

#[rustfmt::skip]
fn probes() -> Vec<Probe> {
    use State::*;
    let p = |node, state, part, reference, checks, lane| Probe { node, state, part, fg_part: None, reference, checks, over: &[], lane };
    let mut v = vec![
        // buttons (OW0): VS Code keeps the hover fill while the pointer is down
        p("button-primary:a", Rest, None, ("button-primary", "rest"), BTN_REST, "OW0"),
        p("button-primary:a", Hover, None, ("button-primary", "hover"), BTN_HOVER, "OW0"),
        p("button-primary:a", Pressed, None, ("button-primary", "active"), COLORS, "OW0"),
        p("button-primary:a", Focus, None, ("button-primary", "focus"), BTN_FOCUS, "OW0"),
        p("button-primary:disabled", Rest, None, ("button-primary", "disabled"), BTN_DISABLED, "OW0"),
        p("button-primary-dialog:a", Rest, None, ("button-primary-dialog", "rest"), &["h", "wt", "tw", "p", "bg", "bc", "fg", "fs"], "OW0"),
        p("button-primary-dialog:a", Hover, None, ("button-primary-dialog", "hover"), &["bg"], "OW0"),
        p("button-primary-dialog:a", Focus, None, ("button-primary-dialog", "focus"), &["bg", "oc", "ow", "oo"], "OW0"),
        p("button-secondary:a", Rest, None, ("button-secondary", "rest"), BTN_REST, "OW0"),
        p("button-secondary:a", Hover, None, ("button-secondary", "hover"), BTN_HOVER, "OW0"),
        p("button-secondary:a", Pressed, None, ("button-secondary", "active"), COLORS, "OW0"),
        p("button-secondary:a", Focus, None, ("button-secondary", "focus"), BTN_FOCUS, "OW0"),
        p("button-secondary:disabled", Rest, None, ("button-secondary", "disabled"), BTN_DISABLED, "OW0"),
        // check box: the box is `::part(indicator)` (18x18, radius 3), the check `::part(mark)`
        p("checkbox:a", Rest, Some("indicator"), ("checkbox", "rest"), &["h", "w", "r", "b", "bg", "bc"], "OW1a"),
        p("checkbox:a", Hover, Some("indicator"), ("checkbox", "hover"), &["bg", "bc"], "OW1a"),
        p("checkbox:a", Focus, Some("indicator"), ("checkbox", "focus"), &["bg", "bc", "oc", "ow", "oo"], "OW1a"),
        Probe { fg_part: Some("mark"), ..p("checkbox:checked", Rest, Some("indicator"), ("checkbox", "checked"), &["bg", "bc", "fg"], "OW1a") },
        p("checkbox:disabled", Rest, Some("indicator"), ("checkbox", "disabled"), &["bg", "bc", "op"], "OW1a"),
        // radio buttons: VS Code has none; they take the check box's box and colours
        p("radio:a", Rest, Some("indicator"), ("checkbox", "rest"), &["h", "w", "b", "bg", "bc"], "OW1a2"),
        Probe { fg_part: Some("dot"), ..p("radio:checked", Rest, Some("indicator"), ("checkbox", "checked"), &["bg", "bc", "fg"], "OW1a2") },
        // the input option toggle (Match Case)
        p("toggle:a", Rest, None, ("toggle", "rest"), &["h", "r", "b", "bg", "bc", "fg"], "OW1a"),
        p("toggle:a", Hover, None, ("toggle", "hover"), &["bg", "bc", "fg"], "OW1a"),
        p("toggle:a", Focus, None, ("toggle", "focus"), &["bg", "oc", "ow", "oo"], "OW1a"),
        p("toggle:checked", Rest, None, ("toggle", "checked"), &["r", "b", "bg", "bc", "fg"], "OW1a"),
        // input box (find widget; the disabled one is the settings number box)
        p("input:a", Rest, None, ("input", "rest"), &["h", "r", "b", "bg", "bc"], "OW4"),
        p("input:a", Focus, None, ("input", "focus"), &["bg", "oc", "ow", "oo"], "OW4"),
        p("input:filled", Rest, None, ("input", "rest"), &["fg", "fs"], "OW4"),
        p("input:invalid", Rest, None, ("input", "invalid"), &["bg", "bc"], "OW4"),
        p("input:disabled", Rest, None, ("input", "disabled"), &["h", "bg", "bc", "op"], "OW4"),
        // select box
        p("select:a", Rest, None, ("select", "rest"), &["h", "r", "b", "p", "bg", "bc", "fg", "fs"], "OW3"),
        p("select:a", Hover, None, ("select", "hover"), &["bg", "bc"], "OW3"),
        p("select:a", Focus, None, ("select", "focus"), RING, "OW3"),
        p("select:disabled", Rest, None, ("select", "disabled"), &["bg", "bc", "op"], "OW3"),
        // progress: the capture has no running bar; `.monaco-progress-container` is 2px tall
        Probe { over: &[("bg", Over::Var("--vscode-progressBar-background")), ("h", Over::Px(2.0))], ..p("progress:a", Rest, Some("fill"), ("progress", "rest"), &["h", "bg"], "OW1a") },
        // a label: the setting description's text
        p("label:a", Rest, None, ("setting-row", "rest"), &["fg", "fs"], "OW1a"),
        // pane header (collapsed section header): the collapsing header's `::part(header)` row
        p("pane-header:a", Rest, Some("header"), ("pane-header", "rest"), &["h", "bg", "fg", "fs"], "OW1a2"),
        // editor tabs
        p("tab:active", Rest, None, ("tab", "active"), &["h", "bg", "fg", "fs", "ta", "tw"], "OW5"),
        p("tab:inactive", Rest, None, ("tab", "inactive"), &["h", "bg", "fg", "ta"], "OW5"),
        p("tab:inactive", Hover, None, ("tab", "hover"), &["bg", "fg"], "OW5"),
        // the vertical scroll bar's thumb (VS Code's editor bar is 14px; lists use 10)
        p("scrollbar:a", Rest, Some("thumb-y"), ("scrollbar", "rest"), &["t", "bg"], "OW2"),
        p("scrollbar:a", Hover, Some("thumb-y"), ("scrollbar", "hover"), &["bg"], "OW2"),
        p("scrollbar:a", Pressed, Some("thumb-y"), ("scrollbar", "active"), &["bg"], "OW2"),
        // sash: the 4px hover bar in `sash.hoverBorder` (the capture caught the sash before its delay)
        Probe { over: &[("bg", Over::Var("--vscode-sash-hoverBorder"))], ..p("sash:a", Hover, Some("sash"), ("sash", "hover"), &["t", "bg"], "OW2") },
    ];
    v.sort_by_key(|p| p.state);
    v
}

// ---- measuring ----

/// What one probe measured.
#[derive(Default)]
struct Ours {
    /// Why nothing was measured: the node is missing, still an egui wrapper, or traced no box.
    missing: Option<String>,
    bx: Option<TraceEntry>,
    text: Option<TraceEntry>,
    fg: Option<TraceEntry>,
    outline: Option<TraceEntry>,
}

/// The node named `name`: (handle, drawn rect, own widget?).
fn find_node(eh: &EmbedHarness, name: &str) -> Option<(u64, Option<Rect>, bool)> {
    let be = eh.embed.backend();
    let handles = be.with_scene(|s| s.handles());
    let h = handles.into_iter().find(|&h| matches!(be.prop(h, "accessible-name"), Ok(svelters_abi::Value::Str(s)) if s == name))?;
    let (drawn, own) = be.with_scene(|s| {
        let n = s.n(h);
        (n.drawn, matches!(n.w, W::Ext(_)))
    });
    Some((h, (drawn != Rect::NOTHING && drawn.is_positive()).then_some(drawn), own))
}

fn area(r: &Rect) -> f32 {
    r.width() * r.height()
}

/// The entries of `node`, and of `part` anywhere inside `within` when the node itself traced none
/// (a part drawn by a child node, as a scroll area's bar).
fn measure(t: &PaintTrace, node: u64, within: Option<Rect>, pr: &Probe) -> Ours {
    let mine: Vec<&TraceEntry> = t.entries.iter().filter(|e| e.node == node).collect();
    let in_part = |e: &&TraceEntry, part: &str| e.part == Some(part);
    let boxes: Vec<&TraceEntry> = match pr.part {
        None => mine.iter().copied().filter(|e| e.part.is_none() && e.kind == TraceKind::Box).collect(),
        // a part's box, or its path (a progress bar's `fill`)
        Some(part) => {
            let shape = |e: &&TraceEntry| matches!(e.kind, TraceKind::Box | TraceKind::Path);
            let own: Vec<&TraceEntry> = mine.iter().copied().filter(|e| in_part(e, part) && shape(e)).collect();
            if own.is_empty() {
                let w = within.unwrap_or(Rect::EVERYTHING).expand(1.0);
                t.entries.iter().filter(|e| in_part(e, part) && shape(e) && w.contains_rect(e.rect)).collect()
            } else {
                own
            }
        }
    };
    // the largest box of the part (a sash's hover bar over its line)
    let bx = boxes.iter().copied().max_by(|a, b| area(&a.rect).total_cmp(&area(&b.rect))).copied();
    let text = mine.iter().copied().find(|e| e.kind == TraceKind::Text && e.part == Some("label")).or_else(|| mine.iter().copied().find(|e| e.kind == TraceKind::Text)).copied();
    let fg = match pr.fg_part {
        Some(part) => mine.iter().copied().find(|e| in_part(e, part) && e.kind != TraceKind::Box).or_else(|| mine.iter().copied().find(|e| in_part(e, part))).copied(),
        None => text,
    };
    let outline = mine.iter().copied().find(|e| e.kind == TraceKind::Outline).copied();
    let missing = if bx.is_none() && text.is_none() { Some(format!("nothing traced{}", pr.part.map(|p| format!(" for ::part({p})")).unwrap_or_default())) } else { None };
    Ours { missing, bx, text, fg, outline }
}

fn css(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    format!("rgba({r}, {g}, {b}, {})", (a as f32 / 255.0 * 1000.0).round() / 1000.0)
}

fn rect_json(r: Rect) -> Value {
    json!({ "x": r.min.x, "y": r.min.y, "w": r.width(), "h": r.height() })
}

/// Ours in the capture's schema (`metrics` of an item).
fn ours_json(o: &Ours) -> Value {
    if let Some(m) = &o.missing {
        return json!({ "missing": m });
    }
    let b = o.bx;
    json!({
        "rect": b.map(|b| rect_json(b.rect)),
        "padding": b.map(|b| b.padding.to_vec()),
        "border_width": b.map(|b| b.stroke.width),
        "border_color": b.and_then(|b| (b.stroke.width > 0.0).then(|| css(b.stroke.color))),
        "radius": b.map(|b| b.radius),
        "bg": b.map(|b| css(b.fill)),
        "fg": o.fg.map(|t| css(t.fill)),
        "font_size": o.text.map(|t| t.font_size),
        "opacity": b.or(o.text).map(|e| e.opacity),
        "outline": o.outline.map(|l| {
            let off = b.map(|b| (l.rect.width() - b.rect.width()) / 2.0).unwrap_or(0.0);
            format!("{}px {} offset {}px", l.stroke.width, css(l.stroke.color), off)
        }),
        "text_rect": o.text.map(|t| rect_json(t.rect)),
    })
}

struct Measured {
    ours: Ours,
    json: Value,
    file: String,
}

fn settle(eh: &mut EmbedHarness) {
    // hover eases (OW-CSS transitions) and R11's sash delay (300 ms) are over after half a second
    for _ in 0..30 {
        eh.harness.step();
    }
}

fn run_theme(scheme: ColorScheme, out: &Path) -> Vec<Measured> {
    let mut eh = page(scheme);
    std::fs::create_dir_all(out).unwrap();
    let mut items = Vec::new();
    for pr in probes() {
        let file = format!("{}-{}", pr.node.replace(':', "-"), pr.state.name());
        let Some((h, drawn, own)) = find_node(&eh, pr.node) else {
            items.push(Measured { ours: Ours { missing: Some("no node on the page".into()), ..Ours::default() }, json: json!({ "missing": "no node" }), file });
            continue;
        };
        if !own {
            let why = format!("egui wrapper ({})", pr.lane);
            items.push(Measured { json: json!({ "missing": why }), ours: Ours { missing: Some(why), ..Ours::default() }, file });
            continue;
        }
        let Some(drawn) = drawn else {
            items.push(Measured { ours: Ours { missing: Some("not drawn".into()), ..Ours::default() }, json: json!({ "missing": "not drawn" }), file });
            continue;
        };
        // a part's own centre where the part is the target (a scroll bar's thumb, which an overlay
        // bar shows only while the pointer is over its area)
        eh.harness.event(egui::Event::PointerGone);
        settle(&mut eh);
        let part_at = |eh: &EmbedHarness| pr.part.and_then(|_| measure(&trace::last(&eh.harness.ctx), h, Some(drawn), &pr).bx).map(|b| b.rect.center());
        let mut at = part_at(&eh);
        if at.is_none() && pr.part.is_some() && pr.state != State::Rest {
            eh.harness.hover_at(drawn.center());
            settle(&mut eh);
            at = part_at(&eh);
        }
        let at = at.unwrap_or(drawn.center());
        match pr.state {
            State::Rest => {}
            State::Hover => eh.harness.hover_at(at),
            State::Pressed => {
                eh.harness.hover_at(at);
                eh.harness.drag_at(at);
            }
            State::Focus => match eh.harness.query_by_label(pr.node) {
                Some(n) => n.focus(),
                None => eprintln!("VIS1c: no AccessKit node labelled {}", pr.node),
            },
        }
        settle(&mut eh);
        let ours = measure(&trace::last(&eh.harness.ctx), h, Some(drawn), &pr);
        #[cfg(feature = "snapshots")]
        {
            let img = eh.harness.render().expect("render");
            let ppp = eh.harness.ctx.pixels_per_point();
            let r = drawn.expand(4.0).intersect(Rect::from_min_size(egui::Pos2::ZERO, SIZE));
            let crop = image::imageops::crop_imm(&img, (r.min.x * ppp) as u32, (r.min.y * ppp) as u32, (r.width() * ppp) as u32, (r.height() * ppp) as u32).to_image();
            crop.save(out.join(format!("{file}.png"))).unwrap();
        }
        if pr.state == State::Pressed {
            // release outside the widget: no click
            let far = egui::pos2(SIZE.x - 2.0, SIZE.y - 2.0);
            eh.harness.event(egui::Event::PointerMoved(far));
            eh.harness.drop_at(far);
            eh.harness.step();
        }
        let json = ours_json(&ours);
        items.push(Measured { ours, json, file });
    }
    let doc = json!({
        "meta": { "theme": format!("{scheme:?}"), "page": "fixtures/gallery-egui/src/pages/VisControls.svelte", "scale": 1 },
        "items": probes().iter().zip(&items).map(|(pr, m)| json!({
            "element": pr.node, "state": pr.state.name(), "reference": format!("{}-{}", pr.reference.0, pr.reference.1),
            "png": format!("{}.png", m.file), "metrics": m.json,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(out.join("metrics.json"), serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    items
}

// ---- comparing ----

fn parse_css_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim();
    if s.starts_with('#') {
        let [r, g, b, a] = parse_color(s)?.to_srgba_unmultiplied();
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

/// `c` composited over the opaque `under`.
fn composite(c: [f32; 4], under: [f32; 4]) -> [f32; 3] {
    [0, 1, 2].map(|k| c[k] * c[3] + under[k] * (1.0 - c[3]))
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
    /// `<theme>/<node>/<state>/<check>`.
    id: String,
    /// The reference element (the score's row).
    element: &'static str,
    theme: &'static str,
    lane: &'static str,
    pass: bool,
    ours: String,
    theirs: String,
}

/// VS Code's `outline` string (`1px rgb(0, 120, 212) offset -1px`): width, colour, offset.
fn parse_outline(s: &str) -> Option<(f64, String, f64)> {
    let w = s.split_whitespace().next()?.strip_suffix("px")?.parse().ok()?;
    let i = s.find("rgb")?;
    let j = i + s[i..].find(')')? + 1;
    let off = s[j..].split_whitespace().skip_while(|t| *t != "offset").nth(1).and_then(|t| t.strip_suffix("px")).and_then(|t| t.parse().ok()).unwrap_or(0.0);
    Some((w, s[i..j].to_string(), off))
}

/// Where a text sits in its box: `left`, `centre` or `right` (side gaps within 2px are centred).
fn align(bx: Rect, text: Rect) -> &'static str {
    let (l, r) = (text.min.x - bx.min.x, bx.max.x - text.max.x);
    if (l - r).abs() <= 2.0 {
        "centre"
    } else if l < r {
        "left"
    } else {
        "right"
    }
}

fn num(v: &Value, path: &[&str]) -> Option<f64> {
    path.iter().fold(v, |v, p| &v[*p]).as_f64()
}

/// The reference's text width: its text rect, or the box less padding and border (buttons).
fn ref_text_w(r: &Value) -> Option<f64> {
    num(r, &["text_rect", "w"]).or_else(|| {
        let p = r["padding"].as_array()?;
        let pad = p.get(1)?.as_f64()? + p.get(3)?.as_f64()?;
        Some(num(r, &["rect", "w"])? - pad - 2.0 * r["border_width"].as_f64().unwrap_or(0.0))
    })
}

fn compare(theme: &'static str, measured: &[Measured], reference: &Value, backdrop: [f32; 4]) -> Vec<Check> {
    let refs: BTreeMap<(String, String), &Value> = reference["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| ((i["element"].as_str().unwrap().to_string(), i["state"].as_str().unwrap().to_string()), &i["metrics"]))
        .collect();
    let vars = &reference["vars"];
    let empty = json!({});
    let mut out = Vec::new();
    for (pr, m) in probes().iter().zip(measured) {
        let r = refs.get(&(pr.reference.0.to_string(), pr.reference.1.to_string())).copied().filter(|r| !r.is_null()).unwrap_or(&empty);
        let o = &m.ours;
        for &k in pr.checks {
            let id = format!("{theme}/{}/{}/{k}", pr.node, pr.state.name());
            let mut push = |pass: bool, ours: String, theirs: String| out.push(Check { id: id.clone(), element: pr.reference.0, theme, lane: pr.lane, pass, ours, theirs });
            let over = pr.over.iter().find(|(c, _)| *c == k).map(|(_, v)| *v);
            if let Some(why) = &o.missing {
                push(false, why.clone(), "-".into());
                continue;
            }
            let px = |path: &[&str]| match over {
                Some(Over::Px(v)) => Some(v),
                _ => num(r, path),
            };
            let near = |ours: Option<f64>, theirs: Option<f64>, tol: f64| match (ours, theirs) {
                (Some(a), Some(b)) => ((a - b).abs() <= tol, format!("{a:.2}"), format!("{b:.2}")),
                (a, b) => (false, format!("{a:?}"), format!("{b:?}")),
            };
            let bx = o.bx;
            let (pass, ours, theirs) = match k {
                "h" => near(bx.map(|b| b.rect.height() as f64), px(&["rect", "h"]), 1.0),
                "w" => near(bx.map(|b| b.rect.width() as f64), px(&["rect", "w"]), 1.0),
                "t" => {
                    let theirs = match over {
                        Some(Over::Px(v)) => Some(v),
                        _ => num(r, &["rect", "w"]).zip(num(r, &["rect", "h"])).map(|(w, h)| w.min(h)),
                    };
                    near(bx.map(|b| b.rect.width().min(b.rect.height()) as f64), theirs, 1.0)
                }
                "wt" => {
                    let tol = 1.0 + 0.15 * ref_text_w(r).unwrap_or(0.0);
                    near(bx.map(|b| b.rect.width() as f64), px(&["rect", "w"]), tol)
                }
                "tw" => {
                    let theirs = ref_text_w(r);
                    near(o.text.map(|t| t.rect.width() as f64), theirs, 1.0 + 0.15 * theirs.unwrap_or(0.0))
                }
                "r" => near(bx.map(|b| b.radius as f64), px(&["radius"]), 0.5),
                "b" => near(bx.map(|b| b.stroke.width as f64), px(&["border_width"]), 0.5),
                "fs" => near(o.text.map(|t| t.font_size as f64), px(&["font_size"]), 0.01),
                "op" => near(bx.or(o.text).map(|e| e.opacity as f64), px(&["opacity"]), 0.05),
                "p" => {
                    let ours = bx.map(|b| b.padding.map(|v| v as f64));
                    let theirs: Option<Vec<f64>> = r["padding"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect());
                    match (ours, theirs) {
                        (Some(a), Some(b)) if b.len() == 4 => (a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= 0.5), format!("{a:?}"), format!("{b:?}")),
                        (a, b) => (false, format!("{a:?}"), format!("{b:?}")),
                    }
                }
                "ta" => match (bx, o.text, r["text_rect"].is_object()) {
                    (Some(b), Some(t), true) => {
                        let rr = |v: &Value| Rect::from_min_size(egui::pos2(num(v, &["x"]).unwrap() as f32, num(v, &["y"]).unwrap() as f32), egui::vec2(num(v, &["w"]).unwrap() as f32, num(v, &["h"]).unwrap() as f32));
                        let (a, b) = (align(b.rect, t.rect), align(rr(&r["rect"]), rr(&r["text_rect"])));
                        (a == b, a.into(), b.into())
                    }
                    _ => (false, "-".into(), "-".into()),
                },
                "ow" | "oo" => {
                    let theirs = r["outline"].as_str().and_then(parse_outline).map(|(w, _, off)| if k == "ow" { w } else { off });
                    let ours = o.outline.map(|l| if k == "ow" { l.stroke.width as f64 } else { bx.map_or(0.0, |b| ((l.rect.width() - b.rect.width()) / 2.0) as f64) });
                    near(ours, theirs, 0.5)
                }
                _ => {
                    // colours
                    let ours = match k {
                        "bg" => bx.map(|b| b.fill),
                        "bc" => bx.map(|b| if b.stroke.width > 0.0 { b.stroke.color } else { Color32::TRANSPARENT }),
                        "fg" => o.fg.map(|t| t.fill),
                        _ => Some(o.outline.map_or(Color32::TRANSPARENT, |l| l.stroke.color)),
                    };
                    let theirs = match over {
                        Some(Over::Var(v)) => vars[v].as_str().map(str::to_string),
                        _ => match k {
                            "bg" => r["bg"].as_str().map(str::to_string),
                            "bc" => Some(r["border_color"].as_str().unwrap_or("rgba(0, 0, 0, 0)").to_string()),
                            "fg" => r["fg"].as_str().map(str::to_string),
                            _ => Some(r["outline"].as_str().and_then(parse_outline).map_or("rgba(0, 0, 0, 0)".into(), |(_, c, _)| c)),
                        },
                    };
                    match (ours, theirs.as_deref().and_then(parse_css_color)) {
                        (Some(oc), Some(tc)) => {
                            let oc4 = parse_css_color(&css(oc)).unwrap();
                            let de = delta_e(lab(composite(oc4, backdrop)), lab(composite(tc, backdrop)));
                            (de <= 2.0, format!("{} (ΔE {de:.1})", css(oc)), theirs.unwrap())
                        }
                        (oc, _) => (false, oc.map_or("-".into(), css), theirs.unwrap_or_else(|| "no reference".into())),
                    }
                }
            };
            push(pass, ours, theirs);
        }
    }
    out
}

fn score_table(checks: &[Check]) -> String {
    // element -> theme -> (pass, total), lane
    let mut rows: BTreeMap<&str, (BTreeMap<&str, (usize, usize)>, &str)> = BTreeMap::new();
    for c in checks {
        let row = rows.entry(c.element).or_insert_with(|| (BTreeMap::new(), c.lane));
        let e = row.0.entry(c.theme).or_default();
        e.1 += 1;
        if c.pass {
            e.0 += 1;
        }
    }
    let mut md = String::from("| element | lane | dark-modern | light-modern |\n|---|---|---|---|\n");
    let mut tot = BTreeMap::<&str, (usize, usize)>::new();
    for (el, (themes, lane)) in &rows {
        let cell = |t: &str| themes.get(t).map_or("-".into(), |(p, n)| format!("{p}/{n}"));
        let _ = writeln!(md, "| {el} | {lane} | {} | {} |", cell("dark-modern"), cell("light-modern"));
        for (t, (p, n)) in themes {
            let e = tot.entry(t).or_default();
            e.0 += p;
            e.1 += n;
        }
    }
    let cell = |t: &str| tot.get(t).map_or("-".into(), |(p, n)| format!("**{p}/{n}**"));
    let _ = writeln!(md, "| **all** | | {} | {} |", cell("dark-modern"), cell("light-modern"));
    md
}

#[test]
fn own_widgets_match_vscode() {
    if !cfg!(debug_assertions) {
        eprintln!("VIS1c: PaintTrace records in debug builds only; skipped");
        return;
    }
    let refs = refs();
    let target = root().join("target/vis1c").join(&refs);
    let mut checks = Vec::new();
    for (scheme, theme) in [(ColorScheme::Dark, "dark-modern"), (ColorScheme::Light, "light-modern")] {
        let measured = run_theme(scheme, &target.join(theme));
        let path = root().join(format!("conformance/visual/{}/{theme}/metrics.json", refs_dir(&refs)));
        let reference: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let bg = parse_css_color(&css(base_theme(scheme).colors().bg)).unwrap();
        checks.extend(compare(theme, &measured, &reference, bg));
    }
    let passed = checks.iter().filter(|c| c.pass).count();
    let score = format!("VIS1c against VS Code 1.133 ({refs} references): {passed} of {} checks pass\n\n{}", checks.len(), score_table(&checks));
    let mut md = format!("{score}\n| check | lane | ours | VS Code |\n|---|---|---|---|\n");
    for c in checks.iter().filter(|c| !c.pass).chain(checks.iter().filter(|c| c.pass)) {
        let _ = writeln!(md, "| {} {} | {} | {} | {} |", if c.pass { "pass" } else { "FAIL" }, c.id, c.lane, c.ours, c.theirs);
    }
    std::fs::write(target.join("report.md"), &md).unwrap();
    std::fs::write(target.join("score.md"), &score).unwrap();
    println!("{score}");
    if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(summary) {
            let _ = writeln!(f, "### {score}");
        }
    }

    // the ratchet
    let base_path = root().join(format!("conformance/visual/baseline-{refs}.json"));
    let baseline: Vec<String> = std::fs::read_to_string(&base_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let now: Vec<String> = checks.iter().filter(|c| c.pass).map(|c| c.id.clone()).collect();
    let regressed: Vec<&String> = baseline.iter().filter(|id| !now.contains(id)).collect();
    let new: Vec<&String> = now.iter().filter(|id| !baseline.contains(id)).collect();
    if std::env::var_os("SVELTERS_UPDATE").is_some() {
        let mut sorted = now.clone();
        sorted.sort();
        std::fs::write(&base_path, serde_json::to_string_pretty(&sorted).unwrap() + "\n").unwrap();
        println!("recorded {} passes in {}", sorted.len(), base_path.display());
        return;
    }
    if !new.is_empty() {
        println!("new passes (record with SVELTERS_UPDATE=1): {new:#?}");
    }
    assert!(regressed.is_empty(), "VIS1c: checks that pass in {} fail now: {regressed:#?}\n(report: {})", base_path.display(), target.join("report.md").display());
}

#[test]
fn delta_e_matches_the_reference_pairs() {
    // Sharma, Wu, Dalal (2005), test pairs 1 and 17
    assert!((delta_e([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485]) - 2.0425).abs() < 1e-3);
    assert!((delta_e([50.0, 2.5, 0.0], [73.0, 25.0, -18.0]) - 27.1492).abs() < 1e-3);
}

#[test]
fn outline_strings_parse() {
    assert_eq!(parse_outline("1px rgb(0, 120, 212) offset -1px"), Some((1.0, "rgb(0, 120, 212)".into(), -1.0)));
    assert_eq!(parse_outline("1px rgba(0, 0, 0, 0) offset 2px"), Some((1.0, "rgba(0, 0, 0, 0)".into(), 2.0)));
}
