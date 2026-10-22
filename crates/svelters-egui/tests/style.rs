//! CS5e: the Rust CSS engine on the egui scene, driven through the `style` host op and the Rust
//! API, headless under kittest (the recording host of `common`, no JS).

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_egui::scene::{NodeId, ProviderId, StateBits};
use svelters_style::{Color, ComputedValue, Length};

svelters_egui::state_bits! { pub mod cadt { PENDING = "pending" } }

fn sheet(rig: &Rig, id: &str, css: &str) {
    rig.br().style(SVL_STYLE_SHEET, 0, Some(&format!("{id}{SVL_STYLE_SEP}{css}"))).unwrap();
}

fn classes(rig: &Rig, h: svl_handle, c: &str) {
    rig.br().style(SVL_STYLE_CLASSES, h, Some(c)).unwrap();
}

fn get(rig: &Rig, h: svl_handle, prop: &str) -> Option<ComputedValue> {
    rig.backend.computed_style(h)?.get(prop).cloned()
}

fn rgb(r: u8, g: u8, b: u8) -> Option<ComputedValue> {
    Some(ComputedValue::Color(Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }))
}

fn rect(rig: &Rig, h: svl_handle) -> egui::Rect {
    rig.backend.with_scene(|s| s.n(h).rect)
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.scene().read(|s| s.get(NodeId::from_raw(h).unwrap()).unwrap().state())
}

/// The engine target, then a cascade over a real scene: type, class, id, combinators,
/// `:first-child`, inline style over rules, inherited `color`, `var()` from Rust tokens (SV5),
/// a class change, a provider state (`:state(cadt.pending)`), `%`/`calc()` widths, per-side
/// margins, and an explicit prop winning over every rule.
#[test]
fn cascade_over_a_real_scene() {
    let mut rig = Rig::new();
    assert_eq!(rig.br().style(SVL_STYLE_TARGET, 0, None).unwrap(), svelters_egui::style::SVL_STYLE_TARGET_ENGINE);
    let (_w, col) = rig.window();
    let panel = rig.leaf(col, "box", &[]);
    let a = rig.leaf(panel, "label", &[("text", Value::Str("a".into()))]);
    let b = rig.leaf(panel, "label", &[("text", Value::Str("b".into()))]);
    let btn = rig.leaf(panel, "button", &[("text", Value::Str("go".into()))]);
    classes(&rig, panel, "panel");
    rig.br().style(SVL_STYLE_NAME, b, Some("second")).unwrap();
    rig.backend.set_tokens(None, &[("--cad-accent", "#00ff00")]).unwrap();
    sheet(
        &rig,
        "app",
        ".panel { color: #ff0000; width: calc(50% - 20px); padding-left: 12px; }
         .panel > label:first-child { color: var(--cad-accent); }
         #second { margin-top: 7px; }
         .wide { width: 100%; }
         button:state(cadt.pending) { border-width: 3px; }
         label { min-width: 10px; }",
    );
    rig.br().style(SVL_STYLE_INLINE, b, Some("color: #0000ff")).unwrap();
    rig.run();

    assert_eq!(get(&rig, a, "color"), rgb(0, 255, 0), "first child, token through var()");
    assert_eq!(get(&rig, b, "color"), rgb(0, 0, 255), "inline beats rules");
    assert_eq!(get(&rig, btn, "color"), rgb(255, 0, 0), "inherited from .panel");
    assert_eq!(get(&rig, b, "margin-top"), Some(ComputedValue::Length(Length::px(7.0))));
    let col_w = rect(&rig, col).width();
    assert!((rect(&rig, panel).width() - (col_w / 2.0 - 20.0)).abs() < 1.0, "calc(50% - 20px) of {col_w}: {:?}", rect(&rig, panel));
    assert!((rect(&rig, a).min.x - rect(&rig, panel).min.x - 12.0).abs() < 0.5, "padding-left");
    let gap = rect(&rig, b).min.y - rect(&rig, a).max.y;
    assert!((gap - 7.0).abs() < 0.5, "margin-top 7px, no gap: {gap}");

    // a class change restyles at the next frame
    classes(&rig, panel, "panel wide");
    rig.run();
    assert!((rect(&rig, panel).width() - col_w).abs() < 1.0, "width: 100%");

    // an explicit prop wins over every rule
    rig.set(panel, "width", Value::F64(150.0));
    rig.run();
    assert_eq!(rect(&rig, panel).width(), 150.0);

    // a provider state written from Rust matches `:state(ns.name)` (journal → logic restyle)
    assert_eq!(get(&rig, btn, "border-width"), None);
    rig.backend.scene().write(ProviderId::APP, |s| s.node(NodeId::from_raw(btn).unwrap()).unwrap().set_state(&cadt::PENDING, true)).unwrap();
    rig.run();
    assert_eq!(get(&rig, btn, "border-width"), Some(ComputedValue::Length(Length::px(3.0))));

    // the tokens follow the scheme they were set for
    rig.backend.set_tokens(Some(svelters_style::ColorScheme::Dark), &[("--cad-accent", "#ffffff")]).unwrap();
    rig.backend.set_color_scheme(Some(svelters_style::ColorScheme::Dark));
    rig.run();
    assert_eq!(get(&rig, a, "color"), rgb(255, 255, 255), "dark tokens win in dark mode");
    rig.backend.set_color_scheme(Some(svelters_style::ColorScheme::Light));
    rig.run();
    assert_eq!(get(&rig, a, "color"), rgb(0, 255, 0));
}

/// A hover restyle in the same frame: the walk reads this pass's hover from the button's
/// response and restyles the button and its later sibling before they paint; one frame, no
/// logic restyle and no JS in between.
#[test]
fn hover_restyles_in_the_same_frame() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let btn = rig.leaf(col, "button", &[("text", Value::Str("Hover me".into()))]);
    let label = rig.leaf(col, "label", &[("text", Value::Str("status".into()))]);
    sheet(
        &rig,
        "app",
        "button { background-color: #202020; }
         button:hover { background-color: #ff0000; }
         button:hover + label { color: #00ff00; }
         box:hover > label { min-height: 30px; }",
    );
    rig.run();
    assert_eq!(get(&rig, label, "color"), None);
    assert!(!state(&rig, btn).contains(StateBits::HOVERED));
    let checkpoints = rig.log.borrow().checkpoints;

    rig.harness.hover_at(rig.drawn(btn).center());
    rig.harness.step(); // one frame
    assert!(state(&rig, btn).contains(StateBits::HOVERED), "hover from this frame's response");
    assert!(state(&rig, col).contains(StateBits::HOVERED), "the column box under the pointer too");
    assert_eq!(get(&rig, btn, "background-color"), rgb(255, 0, 0));
    assert_eq!(get(&rig, label, "color"), rgb(0, 255, 0), "the later sibling restyled before it painted");
    assert_eq!(rig.log.borrow().checkpoints, checkpoints, "no JS for a hover");

    // the layout change of the hover shows at the next frame
    rig.run();
    assert!(rect(&rig, label).height() >= 30.0, "{:?}", rect(&rig, label));

    rig.harness.hover_at(egui::pos2(470.0, 350.0));
    rig.harness.step();
    assert!(!state(&rig, btn).contains(StateBits::HOVERED));
    assert_eq!(get(&rig, label, "color"), None, "unhovered in the same frame");
    assert_eq!(get(&rig, btn, "background-color"), rgb(32, 32, 32));
}

/// Prop-derived states (`:disabled`, `:checked`) and `display: grid` with track lists and
/// placement.
#[test]
fn prop_states_and_grid() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let grid = rig.leaf(col, "box", &[]);
    classes(&rig, grid, "grid");
    let c1 = rig.leaf(grid, "checkbox", &[("text", Value::Str("one".into())), ("checked", Value::Bool(true))]);
    let c2 = rig.leaf(grid, "button", &[("text", Value::Str("two".into())), ("enabled", Value::Bool(false))]);
    let c3 = rig.leaf(grid, "label", &[("text", Value::Str("three".into()))]);
    sheet(
        &rig,
        "app",
        ".grid { display: grid; grid-template-columns: 100px 1fr; row-gap: 4px; }
         checkbox:checked { border-width: 2px; }
         button:disabled { border-width: 5px; }
         label { grid-column: 1 / span 2; }",
    );
    rig.run();
    assert!(state(&rig, c1).contains(StateBits::CHECKED));
    assert!(state(&rig, c2).contains(StateBits::DISABLED));
    assert_eq!(get(&rig, c1, "border-width"), Some(ComputedValue::Length(Length::px(2.0))));
    assert_eq!(get(&rig, c2, "border-width"), Some(ComputedValue::Length(Length::px(5.0))));
    let g = rect(&rig, grid);
    assert_eq!(rect(&rig, c1).width(), 100.0, "first track");
    assert_eq!(rect(&rig, c2).min.x - g.min.x, 100.0, "second track starts after 100px");
    assert_eq!(rect(&rig, c3).width(), g.width(), "spans both columns");
    assert!(rect(&rig, c3).min.y > rect(&rig, c1).max.y, "on the second row");

    // a prop write flips the state and the style at the next frame
    rig.set(c1, "checked", Value::Bool(false));
    rig.run();
    assert!(!state(&rig, c1).contains(StateBits::CHECKED));
    assert_eq!(get(&rig, c1, "border-width"), None);
}
