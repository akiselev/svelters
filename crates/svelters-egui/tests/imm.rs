//! OW-RUST (plans/milestones/OW-RUST-immediate.md): an `imm` widget drawn by Rust with no scene
//! node paints the same shapes (PaintTrace) and exposes the same AccessKit node as the Svelte
//! widget of the same kind, in every state (rest, hover, press, checked, focus-visible), with the
//! same CSS (UA only, a theme, author rules), and it takes part in EV3 focus.
//!
//! Each test draws a scene node and an `imm` widget in the same frame, the `imm` one at the scene
//! node's rect moved down by a whole number of points, and compares their traces relative to
//! their rects.

mod common;

use common::Rig;
use egui::accesskit::{Action, Role, Toggled};
use egui::{Pos2, Rect, vec2};
use egui_kittest::kittest::{NodeT, Queryable};
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;
use svelters_egui::imm::{self, El, Sv};
use svelters_egui::native::{self, Native};
use svelters_egui::paint::{TraceKind, trace};
use svelters_egui::provider::StateBits;
use svelters_egui::theme::Theme;
use svelters_egui::widgets::KindSet;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// Draws one `imm` widget at a rect (filled in after the first frame).
type Draw = Box<dyn FnMut(&mut egui::Ui, Rect, &Sv) -> imm::Response>;

/// One traced shape relative to its widget's rect: kind, part, rect, fill, stroke, font size.
type Shape = (TraceKind, Option<&'static str>, Rect, egui::Color32, egui::Stroke, f32);

#[derive(Default)]
struct Plan {
    rect: Option<Rect>,
    sv: Option<Sv>,
    draw: Option<Draw>,
    last: Option<imm::Response>,
    /// Activations (and toggles) the `imm` widget reported, over all frames.
    clicks: usize,
    changes: usize,
}

struct T {
    rig: Rig,
    plan: Rc<RefCell<Plan>>,
    /// The scene node.
    h: svl_handle,
}

const OFFSET: egui::Vec2 = vec2(0.0, 180.0);

impl T {
    /// A scene node `tag` with `props` (own widgets on) and the `imm` widget `draw` at its rect +
    /// [`OFFSET`]; `theme`: install the default theme (the cascade runs) first.
    fn new(tag: &str, props: &[(&str, Value)], theme: bool, draw: Draw) -> T {
        let plan: Rc<RefCell<Plan>> = Rc::default();
        let p = plan.clone();
        let rig = Rig::with_extra(move |ui| {
            let mut p = p.borrow_mut();
            let (Some(rect), Some(sv)) = (p.rect, p.sv.clone()) else { return };
            let mut draw = p.draw.take().expect("draw");
            let r = draw(ui, rect, &sv);
            p.clicks += usize::from(r.clicked());
            p.changes += usize::from(r.changed());
            p.last = Some(r);
            p.draw = Some(draw);
        });
        let mut rig = rig;
        native::set(&rig.harness.ctx, Native::Gtk); // a press focuses (FX1), as ow1a.rs
        rig.backend.set_own_widgets(KindSet::All);
        if theme {
            rig.backend.set_theme(&Theme::default());
        }
        trace::enable(&rig.harness.ctx, true);
        let (_w, col) = rig.window();
        let h = rig.leaf(col, tag, props);
        rig.run();
        {
            // a label's drawn rect is its text (its hit area); its layout rect is where the walk
            // painted its CSS box
            let mut r = rig.drawn(h);
            if tag == "label"
                && let Some(b) = trace::last(&rig.harness.ctx).entries.iter().find(|e| e.node == h && e.part.is_none() && e.kind == TraceKind::Box)
            {
                r = b.rect;
            }
            let mut p = plan.borrow_mut();
            p.rect = Some(r.translate(OFFSET));
            p.sv = Some(rig.backend.imm());
            p.draw = Some(draw);
        }
        rig.run();
        T { rig, plan, h }
    }

    fn imm_rect(&self) -> Rect {
        self.plan.borrow().rect.expect("rect")
    }
    fn imm_id(&self) -> u64 {
        self.plan.borrow().last.as_ref().expect("drawn").inner.id.value()
    }
    fn imm_state(&self) -> StateBits {
        self.plan.borrow().last.as_ref().expect("drawn").state
    }
    fn scene_state(&self) -> StateBits {
        self.rig.backend.with_scene(|s| s.n(self.h).state)
    }

    /// The traced shapes of `node` in the last pass, relative to `origin`.
    fn shapes(&self, node: u64, origin: Pos2) -> Vec<Shape> {
        // rounded to 1/64 pt: the two rects differ by a whole offset, which float rounding of the
        // derived part rects can show in the last bits
        let q = |v: f32| (v * 64.0).round() / 64.0;
        let r = |r: Rect| Rect::from_min_max(Pos2::new(q(r.min.x), q(r.min.y)), Pos2::new(q(r.max.x), q(r.max.y)));
        trace::last(&self.rig.harness.ctx)
            .entries
            .iter()
            .filter(|e| e.node == node)
            .map(|e| (e.kind, e.part, r(e.rect.translate(-origin.to_vec2())), e.fill, egui::Stroke::new(q(e.stroke.width), e.stroke.color), e.font_size))
            .collect()
    }

    /// The scene node's and the `imm` widget's shapes, as painted in the last pass.
    fn both(&self) -> (Vec<Shape>, Vec<Shape>) {
        let scene = self.shapes(self.h, self.rig.drawn(self.h).min);
        let imm = self.shapes(self.imm_id(), self.imm_rect().min);
        (scene, imm)
    }

    fn assert_same(&self, what: &str) {
        let (a, b) = self.both();
        assert!(!a.is_empty(), "{what}: the scene node painted nothing traced");
        assert_eq!(a, b, "{what}: scene vs imm paint");
    }

    fn hover(&mut self, p: Pos2) {
        self.rig.harness.hover_at(p);
        self.rig.run();
    }

    fn click_at(&mut self, pos: Pos2) {
        self.rig.harness.hover_at(pos);
        for pressed in [true, false] {
            self.rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        self.rig.run();
    }

    fn press_at(&mut self, pos: Pos2, pressed: bool) {
        self.rig.harness.hover_at(pos);
        self.rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        self.rig.run();
    }

    /// Paints the same in the rest, hover and press states (each widget in turn).
    fn assert_same_in_pointer_states(&mut self, what: &str) {
        self.hover(Pos2::new(470.0, 350.0));
        self.assert_same(&format!("{what} at rest"));
        let sc = self.rig.drawn(self.h).center();
        let ic = sc + OFFSET; // the same point of each widget
        // hover the scene node, record; hover the imm widget, record
        self.hover(sc);
        assert!(self.scene_state().contains(StateBits::HOVERED), "{what}: scene hovered");
        let scene = self.shapes(self.h, self.rig.drawn(self.h).min);
        self.hover(ic);
        assert!(self.imm_state().contains(StateBits::HOVERED), "{what}: imm hovered");
        let imm = self.shapes(self.imm_id(), self.imm_rect().min);
        assert_eq!(scene, imm, "{what} hovered: scene vs imm paint");
        // held down
        self.press_at(sc, true);
        let scene = self.shapes(self.h, self.rig.drawn(self.h).min);
        let scene_pressed = self.scene_state().contains(StateBits::PRESSED);
        self.press_at(sc, false);
        self.press_at(ic, true);
        let imm = self.shapes(self.imm_id(), self.imm_rect().min);
        assert_eq!(self.imm_state().contains(StateBits::PRESSED), scene_pressed, "{what}: pressed bit");
        self.press_at(ic, false);
        assert_eq!(scene, imm, "{what} pressed: scene vs imm paint");
    }
}

fn button() -> Draw {
    Box::new(|ui, r, sv| imm::Button::new("Apply").show_at(ui, r, sv))
}

#[test]
fn owrust_button_paints_as_the_scene_button_in_every_pointer_state() {
    for theme in [false, true] {
        let mut t = T::new("button", &[("text", s("Apply"))], theme, button());
        t.assert_same_in_pointer_states(&format!("button (theme {theme})"));
    }
}

#[test]
fn owrust_button_props_icon_selected_frame_small() {
    let mut t = T::new(
        "button",
        &[("text", s("Save")), ("icon", s("codicons:save")), ("selected", Value::Bool(true)), ("small", Value::Bool(true)), ("font-weight", Value::I64(600))],
        true,
        Box::new(|ui, r, sv| imm::Button::new("Save").icon("codicons:save").selected(true).small(true).font_weight(600).show_at(ui, r, sv)),
    );
    t.assert_same_in_pointer_states("selected small button with an icon");
    let mut t = T::new("button", &[("text", s("Flat")), ("frame", Value::Bool(false))], true, Box::new(|ui, r, sv| imm::Button::new("Flat").frame(false).show_at(ui, r, sv)));
    t.assert_same_in_pointer_states("frameless button");
}

#[test]
fn owrust_toggles_paint_and_toggle_as_the_scene_ones() {
    let state = Rc::new(RefCell::new(false));
    for (tag, prop) in [("checkbox", "checked"), ("switch", "active"), ("toggle-button", "checked")] {
        for checked in [false, true] {
            let v = state.clone();
            *v.borrow_mut() = checked;
            let draw: Draw = match tag {
                "checkbox" => Box::new(move |ui, r, sv| imm::Checkbox::new(&mut v.borrow_mut(), "Wrap").show_at(ui, r, sv)),
                "switch" => Box::new(move |ui, r, sv| imm::Switch::new(&mut v.borrow_mut(), "Wrap").show_at(ui, r, sv)),
                _ => Box::new(move |ui, r, sv| imm::ToggleButton::new(&mut v.borrow_mut(), "Wrap").show_at(ui, r, sv)),
            };
            let mut t = T::new(tag, &[("text", s("Wrap")), (prop, Value::Bool(checked))], true, draw);
            assert_eq!(t.imm_state().contains(StateBits::CHECKED), checked, "{tag}: checked bit");
            t.assert_same_in_pointer_states(&format!("{tag} {prop}={checked}"));
            // the clicks above toggled both twice (scene: press + release on each press_at pair)
            let scene_now = t.rig.get(t.h, prop) == Value::Bool(true);
            assert_eq!(*state.borrow(), scene_now, "{tag}: the imm value follows clicks as the scene's");
            // one more click on each: both flip, the imm one reports `changed`
            let before = t.plan.borrow().changes;
            let (sc, ic) = (t.rig.drawn(t.h).center(), t.imm_rect().center());
            t.click_at(sc);
            t.click_at(ic);
            assert_eq!(t.plan.borrow().changes, before + 1, "{tag}: changed once");
            assert_eq!(*state.borrow(), !scene_now, "{tag}: imm flipped");
            assert_eq!(t.rig.get(t.h, prop), Value::Bool(!scene_now), "{tag}: scene flipped");
            t.hover(Pos2::new(470.0, 350.0));
            t.assert_same(&format!("{tag} after a click"));
        }
    }
}

#[test]
fn owrust_label_and_separator_paint_as_the_scene_ones() {
    let mut t = T::new("label", &[("text", s("Hello, chrome"))], true, Box::new(|ui, r, sv| imm::Label::new("Hello, chrome").show_at(ui, r, sv)));
    t.assert_same_in_pointer_states("label");
    let mut t = T::new("label", &[("text", s("Bold")), ("font-weight", Value::I64(700))], false, Box::new(|ui, r, sv| imm::Label::new("Bold").font_weight(700).show_at(ui, r, sv)));
    t.assert_same_in_pointer_states("bold label (no theme)");
    for theme in [false, true] {
        let t = T::new("separator", &[("accessible-name", s("Rule"))], theme, Box::new(|ui, r, sv| imm::Separator::horizontal().accessible_name("Rule").show_at(ui, r, sv)));
        t.assert_same(&format!("separator (theme {theme})"));
    }
}

#[test]
fn owrust_icon_paints_as_the_button_icon_part() {
    // no own `icon` kind yet: the imm icon is the paint primitive, checked against the button's
    // `::part(icon)` (same UA table, same primitive)
    let t = T::new("button", &[("text", s("Save")), ("icon", s("codicons:save"))], true, Box::new(|ui, r, sv| imm::Icon::new("codicons:save").label("Save").show_at(ui, r, sv)));
    let trace = trace::last(&t.rig.harness.ctx);
    let part = trace.find(t.h, Some("icon"), TraceKind::Icon).copied();
    let icon = trace.entries.iter().find(|e| e.node == t.imm_id()).copied();
    match (part, icon) {
        (Some(p), Some(i)) => {
            assert_eq!((p.fill, p.font_size), (i.fill, i.font_size), "icon tint and size");
        }
        (None, None) => {} // no icon font loaded in this build: both paint nothing
        (p, i) => panic!("button icon part {p:?} vs imm icon {i:?}"),
    }
}

#[test]
fn owrust_author_css_reaches_virtual_elements_and_restyles() {
    let mut t = T::new("button", &[("text", s("Apply"))], true, button());
    t.rig.backend.add_sheet("app", "button { background-color: rgb(200, 0, 0); } .toolbar button { background-color: rgb(0, 0, 200); }");
    t.rig.run();
    t.rig.run();
    t.assert_same("an author rule on `button`");
    let fill = |t: &T| t.shapes(t.imm_id(), t.imm_rect().min).iter().find(|e| e.0 == TraceKind::Box).map(|e| e.3);
    assert_eq!(fill(&t), Some(egui::Color32::from_rgb(200, 0, 0)), "imm button: the author rule");
    // within a `.toolbar`: the descendant rule
    let sv = t.plan.borrow().sv.clone().unwrap();
    t.plan.borrow_mut().sv = Some(sv.within(El::new("div").class("toolbar")));
    t.rig.run();
    assert_eq!(fill(&t), Some(egui::Color32::from_rgb(0, 0, 200)), "imm button in a toolbar");
    // a sheet change restyles the cached virtual elements
    t.rig.backend.add_sheet("app", ".toolbar button { background-color: rgb(0, 150, 0); }");
    t.rig.run();
    t.rig.run();
    assert_eq!(fill(&t), Some(egui::Color32::from_rgb(0, 150, 0)), "after the sheet changed");
    // a class on the element itself
    t.plan.borrow_mut().sv = Some(sv);
    t.plan.borrow_mut().draw = Some(Box::new(|ui, r, sv| imm::Button::new("Apply").class("primary").show_at(ui, r, sv)));
    t.rig.backend.add_sheet("app2", "button.primary { background-color: rgb(1, 2, 3); }");
    t.rig.run();
    t.rig.run();
    assert_eq!(fill(&t), Some(egui::Color32::from_rgb(1, 2, 3)), "a class on the virtual element");
    // `style_as`: an app's own element styled by `button` rules (and the button's UA table)
    t.rig.backend.add_sheet("app3", "button { background-color: rgb(9, 9, 9); }");
    t.plan.borrow_mut().draw = Some(Box::new(|ui, r, sv| imm::Button::new("Apply").element(El::new("cad-button").style_as("button")).show_at(ui, r, sv)));
    t.rig.run();
    t.rig.run();
    assert_eq!(fill(&t), Some(egui::Color32::from_rgb(9, 9, 9)), "style_as matches `button` rules");
    t.plan.borrow_mut().draw = Some(Box::new(|ui, r, sv| imm::Button::new("Apply").element(El::new("cad-button")).show_at(ui, r, sv)));
    t.rig.run();
    assert_ne!(fill(&t), Some(egui::Color32::from_rgb(9, 9, 9)), "without style_as, `button` rules do not match");
}

#[test]
fn owrust_accesskit_matches_the_scene_widget() {
    let v = Rc::new(RefCell::new(true));
    let v2 = v.clone();
    type Case = (&'static str, Vec<(&'static str, Value)>, Draw, Role, &'static str);
    let cases: Vec<Case> = vec![
        ("button", vec![("text", s("Apply"))], button(), Role::Button, "Apply"),
        ("checkbox", vec![("text", s("Wrap")), ("checked", Value::Bool(true))], Box::new(move |ui, r, sv| imm::Checkbox::new(&mut v2.borrow_mut(), "Wrap").show_at(ui, r, sv)), Role::CheckBox, "Wrap"),
        ("label", vec![("text", s("Status"))], Box::new(|ui, r, sv| imm::Label::new("Status").show_at(ui, r, sv)), Role::Label, "Status"),
    ];
    for (tag, props, draw, role, label) in cases {
        let t = T::new(tag, &props, true, draw);
        let nodes: Vec<_> = t.rig.harness.query_all_by_role_and_label(role, label).collect();
        assert_eq!(nodes.len(), 2, "{tag}: a scene node and an imm one, same role and name");
        let a = nodes[0].accesskit_node();
        let b = nodes[1].accesskit_node();
        assert_eq!(a.data().toggled(), b.data().toggled(), "{tag}: toggled");
        assert_eq!(a.is_disabled(), b.is_disabled(), "{tag}: disabled");
        assert_eq!(a.value(), b.value(), "{tag}: value");
        for act in [Action::Click, Action::Focus] {
            assert_eq!(a.data().supports_action(act), b.data().supports_action(act), "{tag}: supports {act:?}");
        }
        if tag == "checkbox" {
            assert_eq!(b.data().toggled(), Some(Toggled::True));
        }
    }
}

#[test]
fn owrust_disabled_keeps_its_shape_and_its_tooltip() {
    let mut t = T::new("button", &[("text", s("Off")), ("enabled", Value::Bool(false))], true, Box::new(|ui, r, sv| imm::Button::new("Off").enabled(false).tooltip("Not now").show_at(ui, r, sv)));
    t.assert_same_in_pointer_states("disabled button");
    assert!(t.imm_state().contains(StateBits::DISABLED));
    assert_eq!(t.plan.borrow().clicks, 0, "a disabled imm button never activates");
    let nodes: Vec<_> = t.rig.harness.query_all_by_role_and_label(Role::Button, "Off").collect();
    assert!(nodes.iter().all(|n| n.accesskit_node().is_disabled()));
    // OW3's tooltip path: the imm host is described by its tooltip (R9: the reason)
    assert!(nodes.iter().any(|n| n.accesskit_node().description().as_deref() == Some("Not now")), "imm: described by its tooltip");
}

#[test]
fn owrust_focus_tab_order_focus_visible_and_key_claims() {
    let mut t = T::new("button", &[("text", s("Apply"))], true, button());
    // Tab from nothing: the scene button first (painted first), then the imm one
    t.rig.harness.key_press(egui::Key::Tab);
    t.rig.run();
    assert!(t.scene_state().contains(StateBits::FOCUS_VISIBLE), "Tab focuses the scene button, ring shown");
    let scene = t.shapes(t.h, t.rig.drawn(t.h).min);
    assert!(scene.iter().any(|e| e.0 == TraceKind::Outline), "the scene button's focus ring");
    t.rig.harness.key_press(egui::Key::Tab);
    t.rig.run();
    assert!(!t.scene_state().contains(StateBits::FOCUSED), "focus left the scene button");
    assert!(t.imm_state().contains(StateBits::FOCUS_VISIBLE), "Tab moved to the imm button: focus-visible");
    let imm = t.shapes(t.imm_id(), t.imm_rect().min);
    assert_eq!(scene, imm, "focus-visible: scene vs imm paint");
    // Space activates the focused imm button, and is claimed (R11): the host's keymap is told
    let clicks = t.plan.borrow().clicks;
    t.rig.harness.key_press(egui::Key::Space);
    t.rig.run();
    assert_eq!(t.plan.borrow().clicks, clicks + 1, "Space activates the focused imm button");
    let chord = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Space);
    assert!(t.rig.backend.shortcut_override(&t.rig.harness.ctx, &chord), "Space is the focused imm button's (shortcut_override)");
    // a pointer press focuses without the ring (FX1), on both
    let ic = t.imm_rect().center();
    t.click_at(Pos2::new(470.0, 350.0));
    t.click_at(ic);
    let st = t.imm_state();
    assert!(st.contains(StateBits::FOCUSED) && !st.contains(StateBits::FOCUS_VISIBLE), "click focus: no ring ({st:?})");
}


/// berth's side of BD7, for the chrome tests: every `Response` is the caller's, and the painter
/// only paints (`imm::chrome`).
struct Berth {
    rig: Rig,
    seen: Rc<RefCell<Seen>>,
}

#[derive(Default, Clone)]
struct Seen {
    /// The draggable tab, the close button, the disabled close button.
    tab: StateBits,
    close: StateBits,
    off: StateBits,
    tab_color: Option<egui::Color32>,
    tab_font: Option<egui::FontId>,
    focus_before: Option<egui::Id>,
    focus_after: Option<egui::Id>,
}

const TAB: Rect = Rect { min: Pos2::new(20.0, 300.0), max: Pos2::new(140.0, 330.0) };
const CLOSE: Rect = Rect { min: Pos2::new(150.0, 304.0), max: Pos2::new(172.0, 326.0) };
const OFF: Rect = Rect { min: Pos2::new(180.0, 304.0), max: Pos2::new(202.0, 326.0) };

fn ids() -> (egui::Id, egui::Id, egui::Id) {
    (egui::Id::unique("berth-tab"), egui::Id::unique("berth-close"), egui::Id::unique("berth-close-off"))
}

impl Berth {
    fn new(css: &str) -> Berth {
        let sv: Rc<RefCell<Option<Sv>>> = Rc::default();
        let seen: Rc<RefCell<Seen>> = Rc::default();
        let (s2, seen2) = (sv.clone(), seen.clone());
        let mut rig = Rig::with_extra(move |ui| {
            let Some(sv) = s2.borrow().clone() else { return };
            let (tab_id, close_id, off_id) = ids();
            let mut seen = seen2.borrow_mut();
            seen.focus_before = ui.memory(|m| m.focused());
            // berth: a tab that drags, its label laid out in berth's tab_font
            let tab = ui.interact(TAB, tab_id, egui::Sense::click_and_drag());
            let st = sv.chrome(ui, &tab, &El::new("tab"), svelters_egui::paint::Ua::NONE, StateBits::SELECTED);
            let geom = imm::chrome::surface(ui.painter(), tab_id, &st, TAB);
            let font = imm::chrome::font(ui.ctx(), &st);
            let g = ui.ctx().fonts_mut(|f| f.layout_no_wrap("main.rs".into(), font.clone(), egui::Color32::RED));
            imm::chrome::galley(ui.painter(), tab_id, &st, &g, geom.content.left_top());
            tab.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Tab, true, "main.rs"));
            seen.tab = st.state;
            seen.tab_color = Some(imm::chrome::text_color(&st));
            seen.tab_font = Some(font);
            // berth: its close button (focusable, clickable), and a non-closable one (hover only,
            // disabled on an enabled Ui)
            for (id, rect, sense, props) in [(close_id, CLOSE, egui::Sense::click(), StateBits::NONE), (off_id, OFF, egui::Sense::hover(), StateBits::DISABLED)] {
                let resp = ui.interact(rect, id, sense);
                let st = sv.chrome(ui, &resp, &El::new("chrome-button").class("tab-close"), imm::chrome::BUTTON_UA, props);
                imm::chrome::surface(ui.painter(), id, &st, rect);
                imm::chrome::icon(ui.painter(), id, &st, "codicons:close", rect);
                imm::chrome::ring(ui.painter(), id, &st, rect);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, id == close_id, "Close"));
                if id == close_id {
                    seen.close = st.state;
                } else {
                    seen.off = st.state;
                }
            }
            seen.focus_after = ui.memory(|m| m.focused());
        });
        trace::enable(&rig.harness.ctx, true);
        native::set(&rig.harness.ctx, Native::Gtk);
        rig.backend.set_theme(&Theme::default());
        rig.backend.add_sheet("app", css);
        *sv.borrow_mut() = Some(rig.backend.imm());
        rig.run();
        rig.run();
        Berth { rig, seen }
    }
    fn seen(&self) -> Seen {
        self.seen.borrow().clone()
    }
    fn traced(&self, id: egui::Id, kind: TraceKind) -> Option<svelters_egui::paint::TraceEntry> {
        trace::last(&self.rig.harness.ctx).entries.iter().find(|e| e.node == id.value() && e.kind == kind).copied()
    }
    fn press(&mut self, p: Pos2, pressed: bool) {
        self.rig.harness.hover_at(p);
        self.rig.harness.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        self.rig.run();
    }
}

const BERTH_CSS: &str = "tab { background-color: rgb(10, 20, 30); color: rgb(200, 201, 202); } tab:selected { background-color: rgb(1, 1, 1); } tab:selected:hover { background-color: rgb(40, 50, 60); }";

#[test]
fn owrust_chrome_hover_press_and_focus_from_the_callers_response() {
    let mut b = Berth::new(BERTH_CSS);
    let (tab_id, close_id, _) = ids();
    // the tab: `:selected` from the caller's props, `:hover` from its response
    let s = b.seen();
    assert!(s.tab.contains(StateBits::SELECTED) && !s.tab.contains(StateBits::HOVERED), "{:?}", s.tab);
    assert_eq!(b.traced(tab_id, TraceKind::Box).map(|e| e.fill), Some(egui::Color32::from_rgb(1, 1, 1)));
    b.rig.harness.hover_at(TAB.center());
    b.rig.run();
    assert!(b.seen().tab.contains(StateBits::HOVERED));
    assert_eq!(b.traced(tab_id, TraceKind::Box).map(|e| e.fill), Some(egui::Color32::from_rgb(40, 50, 60)), ":selected:hover");
    // the close button: hover, pressed (R2: the button's pressed fill), from berth's response
    b.rig.harness.hover_at(CLOSE.center());
    b.rig.run();
    let rest_hover = b.traced(close_id, TraceKind::Box).map(|e| e.fill);
    assert!(b.seen().close.contains(StateBits::HOVERED));
    b.press(CLOSE.center(), true);
    assert!(b.seen().close.contains(StateBits::PRESSED), "pressed from the caller's response");
    let pressed = b.traced(close_id, TraceKind::Box).map(|e| e.fill);
    assert_ne!(pressed, rest_hover, "R2: the pressed fill differs from the hover fill");
    b.press(CLOSE.center(), false);
    assert!(!b.seen().close.contains(StateBits::PRESSED));
    // a click leaves focus where berth put it (the painter never focuses), and shows no ring
    let s = b.seen();
    assert_eq!(s.focus_before, s.focus_after, "the painter changed no focus");
    assert!(!s.close.contains(StateBits::FOCUS_VISIBLE));
    // keyboard focus (berth's): `:focus-visible` and the R8 ring
    b.rig.harness.hover_at(Pos2::new(470.0, 350.0));
    for _ in 0..4 {
        if b.seen().focus_after == Some(close_id) {
            break;
        }
        b.rig.harness.key_press(egui::Key::Tab);
        b.rig.run();
    }
    assert_eq!(b.seen().focus_after, Some(close_id), "Tab (egui's, berth's ids) reaches the close button");
    let s = b.seen();
    assert!(s.close.contains(StateBits::FOCUSED) && s.close.contains(StateBits::FOCUS_VISIBLE), "keyboard focus: {:?}", s.close);
    let ring = b.traced(close_id, TraceKind::Outline).expect("R8: the focus ring");
    assert!(ring.stroke.width > 0.0);
}

#[test]
fn owrust_chrome_disabled_on_an_enabled_ui() {
    let mut b = Berth::new(BERTH_CSS);
    let (_, _, off_id) = ids();
    let s = b.seen();
    assert!(s.off.contains(StateBits::DISABLED), "{:?}", s.off);
    // R9: still hoverable; never pressed, never focused
    b.rig.harness.hover_at(OFF.center());
    b.rig.run();
    assert!(b.seen().off.contains(StateBits::HOVERED), "a disabled chrome button stays hoverable");
    b.press(OFF.center(), true);
    assert!(!b.seen().off.contains(StateBits::PRESSED), "a disabled chrome button never sinks");
    b.press(OFF.center(), false);
    // the button UA's disabled look: no lip (box-shadow none), drawn (faded at opacity .4)
    assert!(b.traced(off_id, TraceKind::Box).is_some(), "the disabled button keeps its shape");
}

#[test]
fn owrust_chrome_galley_in_the_style_colour_and_font() {
    let b = Berth::new(BERTH_CSS);
    let (tab_id, close_id, _) = ids();
    let s = b.seen();
    let color = egui::Color32::from_rgb(200, 201, 202);
    assert_eq!(s.tab_color, Some(color), "text_color is the CSS `color`");
    let text = trace::last(&b.rig.harness.ctx).entries.iter().find(|e| e.node == tab_id.value() && e.part == Some("label")).copied().expect("the tab label");
    assert_eq!(text.fill, color, "the caller's galley painted in the style's colour (not its own red)");
    let font = s.tab_font.expect("font");
    assert!(font.size > 0.0, "font() gives the style's font: {font:?}");
    // an icon by key in a rect, traced as part `icon`
    assert!(trace::last(&b.rig.harness.ctx).entries.iter().any(|e| e.node == close_id.value() && e.part == Some("icon")), "the close icon");
}

#[test]
fn owrust_tooltip_comes_up_through_ow3s_attach() {
    let mut t = T::new("button", &[("text", s("Apply"))], true, Box::new(|ui, r, sv| imm::Button::new("Apply").tooltip("Applies it").show_at(ui, r, sv)));
    native::override_policy(&t.rig.harness.ctx, |p| p.tooltip_delay_ms = 60_000);
    let tip = |t: &T| t.rig.harness.query_all_by_label("Applies it").filter(|n| n.accesskit_node().role() == Role::Tooltip).count();
    t.rig.harness.hover_at(t.rig.drawn(t.h).center() + OFFSET);
    t.rig.harness.run();
    assert_eq!(tip(&t), 0, "not before the platform delay (Tooltipable)");
    native::override_policy(&t.rig.harness.ctx, |p| p.tooltip_delay_ms = 0);
    t.rig.harness.run_steps(2);
    t.rig.harness.run();
    assert_eq!(tip(&t), 1, "the imm button's tooltip shows through tooltip::attach");
}

/// ICR1 + ZM1: `imm::chrome::icon` (and `imm::Icon`) draw a registry key through `icons::paint`,
/// rasterised at the drawn size × `pixels_per_point`; an unknown key falls back to the glyph path.
#[test]
fn owrust_chrome_icon_from_the_registry_at_physical_size() {
    use svelters_egui::icons::{self, Icon as RegIcon, IconRegistry, Owner};
    const WARNING_16: &[u8] = include_bytes!("icons/build/16/chrome.warning.svg");
    let ctx = egui::Context::default();
    let reg = IconRegistry::new();
    reg.register(Owner::Host, "chrome.warning", RegIcon::svg(16, WARNING_16)).unwrap();
    icons::set_registry(&ctx, reg);
    ctx.set_pixels_per_point(2.0);
    trace::enable(&ctx, true);
    let sv = Sv::ua_only();
    let id = egui::Id::unique("chrome-warning");
    let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), vec2(22.0, 22.0));
    for _ in 0..2 {
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let resp = ui.interact(rect, id, egui::Sense::hover());
            let st = sv.chrome(ui, &resp, &El::new("chrome-button"), imm::chrome::BUTTON_UA, StateBits::NONE);
            imm::chrome::icon(ui.painter(), id, &st, "chrome.warning", rect);
            imm::Icon::new("chrome.warning").show(ui, &sv);
            imm::chrome::icon(ui.painter(), egui::Id::unique("unknown"), &st, "no.such-icon", rect);
        });
        out.textures_delta.clear(); // no renderer here
    }
    // the raster: 16 pt × ppp 2 = 32 physical pixels (ZM1), from the registry (ICR1)
    let names: Vec<(String, [usize; 2])> = ctx.tex_manager().read().allocated().map(|(_, m)| (m.name.clone(), m.size)).collect();
    let raster = names.iter().find(|(n, _)| n.starts_with("icon:chrome.warning@")).unwrap_or_else(|| panic!("no registry raster in {names:?}"));
    assert_eq!(raster.0, "icon:chrome.warning@32", "rasterised at 16 pt × 2");
    assert_eq!(raster.1, [32, 32]);
    let e = trace::last(&ctx).entries.iter().find(|e| e.node == id.value() && e.part == Some("icon")).copied().expect("the chrome icon is traced");
    assert_eq!(e.rect.size(), vec2(16.0, 16.0), "a 16 pt box, centred in the 22 pt button");
    assert_eq!(e.rect.center(), rect.center());
    let m = e.rect.min.to_vec2() * 2.0;
    assert_eq!(m, m.round(), "Z1: on the physical pixel grid");
}
