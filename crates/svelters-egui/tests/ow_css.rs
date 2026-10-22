//! OW-CSS stage 2 (plans/milestones/OW-CSS.md): the CSS engine behind the own widgets' paint
//! layer, end to end on the own `button` under kittest's virtual clock (`step_dt` 0.25 s): part
//! styles, `paint()` painters selected in CSS, gradients and per-side borders, transitions, and
//! the DESIGN R-rules' CSS.

mod common;

use common::Rig;
use std::sync::atomic::{AtomicUsize, Ordering};
use svelters_abi::*;
use svelters_egui::paint::{self, PaintArgs, PaintCx, PartInfo, StyleView, TraceKind};
use svelters_egui::provider::StateBits;
use svelters_egui::widgets::KindSet;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// A rig with the own button, `css` installed and paint tracing on; one button `props`.
fn rig_with(css: &str, props: &[(&str, Value)]) -> (Rig, svl_handle) {
    let mut rig = Rig::new();
    rig.backend.set_own_widgets(KindSet::tags(["button"]));
    let d = rig.backend.add_sheet("ow-css", css);
    assert!(d.is_empty(), "{d:?}");
    paint::trace::enable(&rig.harness.ctx, true);
    let (_w, col) = rig.window();
    let b = rig.leaf(col, "button", props);
    rig.run();
    (rig, b)
}

fn node(rig: &Rig, h: svl_handle) -> u64 {
    rig.backend.with_scene(|s| s.n(h).handle.to_raw())
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

/// The fill of `h`'s box (or `part`'s text) in the last frame.
fn fill(rig: &Rig, h: svl_handle, part: Option<&str>, kind: TraceKind) -> Option<egui::Color32> {
    paint::trace::last(&rig.harness.ctx).find(node(rig, h), part, kind).map(|e| e.fill)
}

fn press(rig: &mut Rig, h: svl_handle, down: bool) {
    let pos = rig.drawn(h).center();
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: down, modifiers: egui::Modifiers::NONE });
}

#[test]
fn css_paint_runs_registered_painters_on_the_own_button() {
    // OW0b read `background-image`/`content` as keyword text; the engine's values are typed
    // (`ComputedValue::Painter`, `Content::Paint`): a painter named in CSS must run
    static BG: AtomicUsize = AtomicUsize::new(0);
    static GLYPH: AtomicUsize = AtomicUsize::new(0);
    paint::register_painter("owcss.bg", |cx: &mut PaintCx, _s: &StyleView, r: egui::Rect, part: &PartInfo<'_>, _a: &PaintArgs<'_>| {
        assert_eq!((part.tag, part.part), ("button", None));
        cx.painter.rect_filled(r, 0.0, egui::Color32::from_rgb(1, 2, 3));
        BG.fetch_add(1, Ordering::Relaxed);
    });
    paint::register_painter("owcss.glyph", |_cx: &mut PaintCx, _s: &StyleView, _r: egui::Rect, part: &PartInfo<'_>, _a: &PaintArgs<'_>| {
        assert_eq!((part.tag, part.part), ("button", Some("icon")));
        GLYPH.fetch_add(1, Ordering::Relaxed);
    });
    let (rig, b) = rig_with(
        "button { background-image: paint(owcss.bg) } button::part(icon) { content: paint(owcss.glyph) }",
        &[("text", s("Knurled")), ("icon", s("check"))],
    );
    assert!(BG.load(Ordering::Relaxed) > 0, "the background painter ran");
    assert!(GLYPH.load(Ordering::Relaxed) > 0, "the glyph painter ran");
    // the painter replaced the glyph: no icon was drawn
    assert_eq!(fill(&rig, b, Some("icon"), TraceKind::Icon), None);
}

#[test]
fn part_styles_reach_the_paint_and_follow_host_state() {
    let (mut rig, b) = rig_with(
        "button::part(label) { color: rgb(255, 0, 0) }
         button:hover::part(label) { color: rgb(0, 0, 255) }
         button::part(icon) { content: icon(close) }
         button.bare::part(icon) { content: none }",
        &[("text", s("Label")), ("icon", s("check"))],
    );
    assert_eq!(fill(&rig, b, Some("label"), TraceKind::Text), Some(egui::Color32::from_rgb(255, 0, 0)));
    assert!(fill(&rig, b, Some("icon"), TraceKind::Icon).is_some(), "content: icon(close) draws an icon");
    rig.harness.hover_at(rig.drawn(b).center());
    rig.run();
    assert!(state(&rig, b).contains(StateBits::HOVERED));
    assert_eq!(fill(&rig, b, Some("label"), TraceKind::Text), Some(egui::Color32::from_rgb(0, 0, 255)), "a host state reaches the part");
    // `content: none` hides the glyph part
    rig.br().style(SVL_STYLE_CLASSES, b, Some("bare")).unwrap();
    rig.run();
    assert_eq!(fill(&rig, b, Some("icon"), TraceKind::Icon), None);
}

#[test]
fn gradients_per_side_borders_and_corner_radii_paint() {
    let (rig, b) = rig_with(
        "button { background-image: linear-gradient(rgb(255, 0, 0), rgb(0, 0, 255)); border-width: 0; border-top: 3px solid rgb(0, 255, 0); border-radius: 0 }",
        &[("text", s("Grad"))],
    );
    let r = rig.drawn(b);
    // the gradient: a vertex-coloured mesh, red along the top edge and blue along the bottom
    let meshes: Vec<_> = rig
        .harness
        .output()
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Mesh(m) => Some(m.clone()),
            _ => None,
        })
        .collect();
    let mesh = meshes.iter().find(|m| m.vertices.iter().any(|v| v.pos == r.left_top())).expect("the gradient mesh");
    for v in &mesh.vertices {
        if (v.pos.y - r.top()).abs() < 0.01 {
            assert_eq!(v.color, egui::Color32::from_rgb(255, 0, 0), "{:?}", v.pos);
        }
        if (v.pos.y - r.bottom()).abs() < 0.01 {
            assert_eq!(v.color, egui::Color32::from_rgb(0, 0, 255), "{:?}", v.pos);
        }
    }
    // the top border alone, as a quad of its own
    let t = paint::trace::last(&rig.harness.ctx);
    let side = t.of(node(&rig, b), None).find(|e| e.kind == TraceKind::Box && e.stroke.width == 3.0).expect("the top side");
    assert_eq!(side.stroke.color, egui::Color32::from_rgb(0, 255, 0));
    assert_eq!((side.rect.top(), side.rect.height()), (r.top(), 3.0));
}

#[test]
fn transition_samples_over_virtual_time_and_active_is_instant() {
    // R3 (hover eases), R2 (pressed is instant: `transition: none` on `:active`)
    let (mut rig, b) = rig_with(
        "button { background-color: rgb(0, 0, 0); border-width: 0; transition: background-color 1s linear }
         button:hover { background-color: rgb(200, 0, 0) }
         button:active { background-color: rgb(0, 0, 200); transition: none }",
        &[("text", s("Ease"))],
    );
    let red = |rig: &Rig| fill(rig, b, None, TraceKind::Box).map(|c| c.r()).unwrap_or(0) as i32;
    assert_eq!(red(&rig), 0);
    rig.harness.hover_at(rig.drawn(b).center());
    rig.harness.step();
    assert!(state(&rig, b).contains(StateBits::HOVERED));
    let start = red(&rig);
    assert!(start < 10, "starts from the painted colour: {start}");
    // 0.25 s per step, linear over 1 s: +50 a step
    let mut seen = vec![start];
    for _ in 0..3 {
        rig.harness.step();
        seen.push(red(&rig));
    }
    for (k, w) in seen.windows(2).enumerate() {
        assert!((w[1] - w[0] - 50).abs() <= 3, "step {k}: {seen:?}");
    }
    rig.backend.with_scene(|s| assert!(s.style_transitions_running(), "running mid-way"));
    for _ in 0..3 {
        rig.harness.step();
    }
    assert_eq!(red(&rig), 200, "{seen:?}");
    rig.backend.with_scene(|s| assert!(!s.style_transitions_running(), "ended"));
    // pressed: no transition
    press(&mut rig, b, true);
    rig.harness.step();
    assert!(state(&rig, b).contains(StateBits::PRESSED));
    assert_eq!(fill(&rig, b, None, TraceKind::Box), Some(egui::Color32::from_rgb(0, 0, 200)));
    press(&mut rig, b, false);
    rig.harness.step();
    // released, still hovered: eases back from blue towards the hover red
    rig.harness.step();
    let c = fill(&rig, b, None, TraceKind::Box).unwrap();
    assert!(c.r() > 0 && c.r() < 200 && c.b() > 0, "{c:?}");
}

#[test]
fn reduced_motion_ends_and_skips_transitions() {
    let (mut rig, b) = rig_with(
        "button { background-color: rgb(0, 0, 0); border-width: 0; transition: background-color 1s linear }
         button:hover { background-color: rgb(200, 0, 0) }",
        &[("text", s("Calm"))],
    );
    rig.harness.hover_at(rig.drawn(b).center());
    rig.harness.step();
    rig.harness.step();
    assert!(rig.backend.with_scene(|s| s.style_transitions_running()));
    rig.backend.set_motion(svelters_egui::anim::MotionConfig { slow: 1.0, reduced: true });
    rig.harness.step();
    assert!(!rig.backend.with_scene(|s| s.style_transitions_running()), "switching reduced motion on ends them");
    rig.harness.hover_at(egui::pos2(470.0, 350.0));
    rig.harness.step();
    rig.harness.step();
    assert_eq!(fill(&rig, b, None, TraceKind::Box), Some(egui::Color32::from_rgb(0, 0, 0)), "un-hover is instant");
}

#[test]
fn r_rules_css_on_the_own_button() {
    // R8 focus ring from `:focus-visible`, R10 cursor, R11 a zero-duration transition with a delay
    let (mut rig, b) = rig_with(
        "button { background-color: rgb(0, 0, 0); border-width: 0; cursor: crosshair; transition: background-color 0s 300ms }
         button:hover { background-color: rgb(0, 200, 0) }
         button:focus-visible { outline: 2px solid rgb(255, 128, 0); outline-offset: 1px }",
        &[("text", s("Rules"))],
    );
    // R11: still the old colour before the delay, the new one after it
    rig.harness.hover_at(rig.drawn(b).center());
    rig.harness.step();
    assert_eq!(fill(&rig, b, None, TraceKind::Box), Some(egui::Color32::from_rgb(0, 0, 0)), "t = 0");
    rig.harness.step();
    assert_eq!(fill(&rig, b, None, TraceKind::Box), Some(egui::Color32::from_rgb(0, 0, 0)), "t = 250 ms");
    rig.harness.step();
    assert_eq!(fill(&rig, b, None, TraceKind::Box), Some(egui::Color32::from_rgb(0, 200, 0)), "t = 500 ms");
    // R10
    assert_eq!(rig.harness.output().platform_output.cursor_icon, egui::CursorIcon::Crosshair);
    // R8: no ring from a pointer press, the sheet's ring after Tab
    rig.click(b);
    assert_eq!(paint::trace::last(&rig.harness.ctx).find(node(&rig, b), None, TraceKind::Outline).map(|e| e.stroke.width), None);
    rig.harness.hover_at(egui::pos2(470.0, 350.0));
    for _ in 0..3 {
        rig.harness.key_press(egui::Key::Tab);
        rig.run();
        if state(&rig, b).contains(StateBits::FOCUS_VISIBLE) {
            break;
        }
    }
    assert!(state(&rig, b).contains(StateBits::FOCUS_VISIBLE));
    let ring = paint::trace::last(&rig.harness.ctx).find(node(&rig, b), None, TraceKind::Outline).cloned().expect("the ring");
    assert_eq!((ring.stroke.width, ring.stroke.color), (2.0, egui::Color32::from_rgb(255, 128, 0)));
    assert!(ring.rect.contains_rect(rig.drawn(b)) && ring.rect != rig.drawn(b), "offset +1");
}
