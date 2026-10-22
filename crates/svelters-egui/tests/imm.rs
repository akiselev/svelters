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
            let mut p = plan.borrow_mut();
            p.rect = Some(rig.drawn(h).translate(OFFSET));
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
        let (sc, ic) = (self.rig.drawn(self.h).center(), self.imm_rect().center());
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
    let cases: Vec<(&str, Vec<(&str, Value)>, Draw, Role, &str)> = vec![
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


#[test]
fn owrust_chrome_paints_a_foreign_element_on_the_callers_response() {
    // berth's case: a tab it interacts with itself (drag sense), styled by the app's CSS through
    // a virtual `tab` element; the active tab mirrors `:selected`
    type Seen = Rc<RefCell<Vec<(StateBits, Option<egui::Color32>)>>>;
    let seen: Seen = Rc::default();
    let s2 = seen.clone();
    let sv_slot: Rc<RefCell<Option<Sv>>> = Rc::default();
    let sv2 = sv_slot.clone();
    let mut rig = Rig::with_extra(move |ui| {
        let Some(sv) = sv2.borrow().clone() else { return };
        let rect = Rect::from_min_size(Pos2::new(20.0, 300.0), vec2(120.0, 30.0));
        let id = egui::Id::unique("berth-tab-1");
        let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
        let st = sv.chrome(ui, &resp, &El::new("tab"), svelters_egui::paint::Ua::NONE, StateBits::SELECTED);
        let geom = imm::chrome::surface(ui.painter(), id, &st, rect);
        imm::chrome::label(ui.painter(), id, &st, "main.rs", geom.content, svelters_egui::paint::Align::Start);
        imm::chrome::ring(ui.painter(), id, &st, rect);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "main.rs"));
        let fill = trace::last(ui.ctx()).entries.iter().rev().find(|e| e.node == id.value() && e.kind == TraceKind::Box).map(|e| e.fill);
        s2.borrow_mut().push((st.state, fill));
    });
    trace::enable(&rig.harness.ctx, true);
    rig.backend.set_theme(&Theme::default());
    rig.backend.add_sheet("app", "tab { background-color: rgb(10, 20, 30); } tab:selected { background-color: rgb(1, 1, 1); } tab:selected:hover { background-color: rgb(40, 50, 60); }");
    *sv_slot.borrow_mut() = Some(rig.backend.imm());
    rig.run();
    let last = |seen: &Seen| *seen.borrow().last().unwrap();
    rig.run();
    let (st, fill) = last(&seen);
    assert!(st.contains(StateBits::SELECTED) && !st.contains(StateBits::HOVERED), "{st:?}");
    assert_eq!(fill, Some(egui::Color32::from_rgb(1, 1, 1)), ":selected from the caller's props");
    rig.harness.hover_at(Pos2::new(60.0, 315.0));
    rig.run();
    let (st, fill) = last(&seen);
    assert!(st.contains(StateBits::HOVERED), "hover from the caller's response");
    assert_eq!(fill, Some(egui::Color32::from_rgb(40, 50, 60)), ":selected:hover");
}
