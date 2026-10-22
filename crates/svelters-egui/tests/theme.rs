//! DS1: the theme module on a live backend, headless under kittest (the recording host of
//! `common`, no JS). One frame draws a stock `egui::Button` from Rust and a `button` node (what a
//! Svelte `<button>` compiles to) from the scene; switching the theme at run time moves both to the
//! same token value.

mod common;

use common::{Log, Rec};
use egui_kittest::Harness;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use svelters_abi::*;
use svelters_egui::EguiBackend;
use svelters_egui::theme::{ColorScheme, Contrast, Theme};
use svelters_style::ComputedValue;

struct Rig {
    backend: EguiBackend,
    vt: &'static BackendVtable,
    harness: Harness<'static, EguiBackend>,
    /// Where the stock button was drawn last frame.
    stock: Rc<Cell<egui::Rect>>,
}

impl Rig {
    fn new() -> Rig {
        let backend = EguiBackend::new();
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let log: Rc<RefCell<Log>> = Rc::default();
        let hv: &'static HostVtable = Box::leak(Box::new(HostVtable::new(Rec(log))));
        vt.backend_ref().start(hv).expect("start");
        let stock = Rc::new(Cell::new(egui::Rect::NOTHING));
        let s = stock.clone();
        let harness = Harness::builder().with_size(egui::vec2(480.0, 360.0)).build_ui_state(
            move |ui, b: &mut EguiBackend| {
                s.set(ui.add(egui::Button::new("Stock egui button")).rect);
                b.ui(ui);
            },
            backend.clone(),
        );
        Rig { backend, vt, harness, stock }
    }

    fn create(&self, tag: &str) -> svl_handle {
        let br = self.vt.backend_ref();
        let k = br.resolve(NameKind::Kind, 0, tag);
        br.create(k).unwrap()
    }

    fn set(&self, h: svl_handle, name: &str, v: Value) {
        let br = self.vt.backend_ref();
        let kind = br.handle_info(h).unwrap().kind;
        br.set_prop(h, br.resolve(NameKind::Prop, kind, name), &v).unwrap();
    }

    /// A presented window with a column holding one `button` node; returns the button.
    fn svelte_button(&self) -> svl_handle {
        let br = self.vt.backend_ref();
        let w = self.create("window");
        let col = self.create("box");
        self.set(col, "padding", Value::F64(40.0));
        br.insert(w, col, None, 0).unwrap();
        let b = self.create("button");
        self.set(b, "text", Value::Str("Svelte button".into()));
        br.insert(col, b, None, 0).unwrap();
        let kind = br.handle_info(w).unwrap().kind;
        br.call(w, br.resolve(NameKind::Method, kind, "present"), &[]).unwrap();
        b
    }

    fn computed_color(&self, h: svl_handle, prop: &str) -> Option<egui::Color32> {
        match self.backend.computed_style(h)?.get(prop)? {
            ComputedValue::Color(c) => {
                let [r, g, b, a] = c.rgba8();
                Some(egui::Color32::from_rgba_unmultiplied(r, g, b, a))
            }
            _ => None,
        }
    }

    fn drawn(&self, h: svl_handle) -> egui::Rect {
        self.backend.with_scene(|s| s.n(h).drawn)
    }

    /// The fill of the frame painted for a widget at `r` last frame.
    fn painted_fill(&self, r: egui::Rect) -> Option<egui::Color32> {
        fn walk(shape: &egui::Shape, r: egui::Rect, out: &mut Option<egui::Color32>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, r, out)),
                egui::Shape::Rect(rs) if (rs.rect.center() - r.center()).length() < 1.0 && (rs.rect.size() - r.size()).length() < 3.0 => {
                    out.get_or_insert(rs.fill);
                }
                _ => {}
            }
        }
        let mut out = None;
        for c in &self.harness.output().shapes {
            walk(&c.shape, r, &mut out);
        }
        out
    }
}

/// Switching the theme at run time changes a stock egui button's fill and a Svelte `<button>`'s
/// computed (and painted) background to the same token, `--sv-button-bg`, in the same frame.
#[test]
fn theme_switch_moves_stock_and_svelte_buttons_to_the_same_token() {
    let mut rig = Rig::new();
    let b = rig.svelte_button();
    rig.harness.run();
    // before any theme: stock egui visuals, and the backend's default `--sv-*` tokens once the
    // cascade runs (the runtime no longer sends its own on egui), by egui's scheme
    assert!(rig.backend.theme().is_none());
    rig.backend.set_inline(b, Some("color: var(--sv-fg)")).unwrap();
    rig.harness.run();
    let dark = rig.harness.ctx.global_style().visuals.dark_mode;
    let default = if dark { Theme::dark_modern() } else { Theme::light_modern() };
    assert_eq!(rig.computed_color(b, "color"), Some(default.colors().fg), "default tokens");
    assert_eq!(rig.computed_color(b, "background-color"), None, "no base sheet without a theme");

    let mut seen = Vec::new();
    for (theme, name) in [(Theme::dark_modern(), "Dark Modern"), (Theme::light_modern(), "Light Modern"), (Theme::hc_dark(), "Dark High Contrast")] {
        let generation = rig.backend.theme_generation();
        rig.backend.set_theme(&theme);
        assert_eq!(rig.backend.theme_generation(), generation + 1);
        rig.harness.run();
        assert_eq!(theme.name(), name);
        let token = theme.components().button_bg;
        assert_eq!(theme.color("button-bg"), Some(token));
        let ctx = rig.harness.ctx.clone();
        assert_eq!(ctx.global_style().visuals.widgets.inactive.weak_bg_fill, token, "{name}: the context's Style");
        assert_eq!(ctx.global_style().visuals.dark_mode, theme.is_dark());
        assert_eq!(rig.painted_fill(rig.stock.get()), Some(token), "{name}: the stock egui button's painted fill");
        assert_eq!(rig.computed_color(b, "background-color"), Some(token), "{name}: the Svelte button's computed background");
        assert_eq!(rig.painted_fill(rig.drawn(b)), Some(token), "{name}: the Svelte button's painted fill");
        seen.push(token);
    }
    assert_ne!(seen[0], seen[1], "dark and light differ");

    // the runtime's `setColorScheme` (the SCHEME op) switches the installed theme's scheme
    rig.vt.backend_ref().style(SVL_STYLE_SCHEME, 0, Some("light")).unwrap();
    rig.harness.run();
    let t = rig.backend.theme().unwrap();
    assert_eq!((t.scheme(), t.contrast()), (ColorScheme::Light, Contrast::High));
    assert_eq!(rig.computed_color(b, "background-color"), Some(t.components().button_bg));

    // a token override restyles both through the same path
    let red = egui::Color32::from_rgb(200, 30, 30);
    rig.backend.set_theme(&Theme::dark_modern().with([("--sv-button-bg", "#C81E1E")]));
    rig.harness.run();
    assert_eq!(rig.painted_fill(rig.stock.get()), Some(red));
    assert_eq!(rig.computed_color(b, "background-color"), Some(red));

    // hover: both take `--sv-button-bg-hover` in the frame the pointer arrives
    let hover = Theme::dark_modern().components().button_bg_hover;
    rig.harness.hover_at(rig.drawn(b).center());
    rig.harness.run();
    assert_eq!(rig.painted_fill(rig.drawn(b)), Some(hover), "Svelte button hovered");
    rig.harness.hover_at(rig.stock.get().center());
    rig.harness.run();
    assert_eq!(rig.painted_fill(rig.stock.get()), Some(hover), "stock button hovered");
}

/// High Contrast casts no shadows (DESIGN.md R7): every elevation token is `none`, and the egui
/// Style it installs has no window or popup shadow; the normal themes do cast them.
#[test]
fn high_contrast_has_no_shadows() {
    let mut rig = Rig::new();
    rig.svelte_button();
    for theme in [Theme::hc_dark(), Theme::hc_light()] {
        for i in 0..5 {
            assert_eq!(theme.token(&format!("elev-{i}")), Some("none"), "{}", theme.name());
        }
        assert_eq!(theme.elevation().levels, [egui::Shadow::NONE; 5]);
        rig.backend.set_theme(&theme);
        rig.harness.run();
        let v = rig.harness.ctx.global_style().visuals.clone();
        assert_eq!((v.window_shadow, v.popup_shadow), (egui::Shadow::NONE, egui::Shadow::NONE), "{}", theme.name());
    }
    for theme in [Theme::dark_modern(), Theme::light_modern()] {
        let v = theme.egui_style().visuals;
        assert_ne!(v.window_shadow, egui::Shadow::NONE, "{}", theme.name());
        assert_ne!(v.popup_shadow, egui::Shadow::NONE, "{}", theme.name());
    }
}
