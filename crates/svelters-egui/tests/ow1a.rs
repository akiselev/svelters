//! OW1a (plans/milestones/OW1a-basic-widgets.md): the simple controls as svelters' own widgets
//! behind the per-kind switch. Behaviour tests run on both implementations (the egui wrapper and
//! the own widget); the state-bit and paint checks run on the own widget.

mod common;

use common::Rig;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::kinds::KindRegistry;
use svelters_egui::native::{self, Native};
use svelters_egui::provider::StateBits;
use svelters_egui::scene::W;
use svelters_egui::widgets::KindSet;

const TAGS: &[&str] = &["checkbox", "switch", "toggle-button", "label", "hyperlink", "separator", "progress-bar", "spinner"];

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn impls() -> [(&'static str, KindSet); 2] {
    [("egui", KindSet::None), ("own", KindSet::tags(TAGS))]
}

fn rig(set: &KindSet) -> Rig {
    let rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    rig.backend.set_own_widgets(set.clone());
    rig
}

fn own(rig: &Rig, h: svl_handle) -> bool {
    rig.backend.with_scene(|s| matches!(s.n(h).w, W::Ext(_)))
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

#[test]
fn ow1a_every_kind_replaces_its_builtin_row() {
    let reg = KindRegistry::global();
    for tag in TAGS {
        let k = reg.tag_index(tag).expect(tag);
        let r = reg.replacement(k).unwrap_or_else(|| panic!("an own widget replaces <{tag}>"));
        assert_eq!(r.entry.replaces, Some(*tag));
        for (i, p) in r.entry.props.iter().enumerate() {
            assert_eq!(reg.ext_prop_id(k, i), reg.find_prop(k, p.name).unwrap().0, "{tag}: {}", p.name);
        }
    }
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        for tag in TAGS {
            let h = rig.leaf(col, tag, &[]);
            assert_eq!(own(&rig, h), name == "own", "{name}: {tag}");
        }
        rig.harness.run_steps(4); // a spinner repaints every frame
    }
}

#[test]
fn ow1a_checkbox_switch_and_toggle_toggle_on_both() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let cb = rig.leaf(col, "checkbox", &[("text", s("Snap"))]);
        let sw = rig.leaf(col, "switch", &[("text", s("Grid"))]);
        let tb = rig.leaf(col, "toggle-button", &[("text", s("Bold"))]);
        let rb = rig.leaf(col, "toggle-button", &[("text", s("Left")), ("radio", Value::Bool(true))]);
        let off = rig.leaf(col, "checkbox", &[("text", s("Off")), ("enabled", Value::Bool(false))]);
        for h in [cb, sw, tb, rb, off] {
            rig.listen(h, "changed");
            rig.listen(h, "clicked");
        }
        rig.run();
        for (h, p) in [(cb, "checked"), (sw, "active"), (tb, "checked"), (rb, "checked")] {
            rig.click(h);
            assert_eq!(rig.get(h, p), Value::Bool(true), "{name}: {p} after one click");
            assert_eq!(events(&rig, h, "clicked").len(), 1, "{name}: clicked");
        }
        assert_eq!(events(&rig, cb, "changed"), vec![vec![Value::Bool(true)]], "{name}: changed carries the state");
        // a second click: toggles off, but a radio toggle stays on
        rig.take();
        for h in [cb, sw, tb, rb] {
            rig.click(h);
        }
        assert_eq!(rig.get(cb, "checked"), Value::Bool(false), "{name}");
        assert_eq!(rig.get(sw, "active"), Value::Bool(false), "{name}");
        assert_eq!(rig.get(tb, "checked"), Value::Bool(false), "{name}");
        assert_eq!(rig.get(rb, "checked"), Value::Bool(true), "{name}: radio stays on");
        // disabled: nothing
        rig.click(off);
        assert_eq!(rig.get(off, "checked"), Value::Bool(false), "{name}: disabled");
        assert!(events(&rig, off, "changed").is_empty(), "{name}: disabled");
        // AccessKit: roles and toggled state, and a Click toggles
        rig.set(cb, "checked", Value::Bool(true));
        rig.run();
        let n = rig.harness.get_by_role_and_label(Role::CheckBox, "Snap");
        assert_eq!(n.accesskit_node().data().toggled(), Some(Toggled::True), "{name}");
        rig.harness.get_by_role_and_label(Role::Switch, "Grid").click_accesskit();
        rig.run();
        assert_eq!(rig.get(sw, "active"), Value::Bool(true), "{name}: AccessKit click");
    }
}

#[test]
fn ow1a_space_toggles_the_focused_checkbox() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let cb = rig.leaf(col, "checkbox", &[("text", s("Snap"))]);
        rig.run();
        rig.click(cb);
        assert_eq!(rig.get(cb, "checked"), Value::Bool(true), "{name}");
        rig.harness.key_press(egui::Key::Space);
        rig.run();
        assert_eq!(rig.get(cb, "checked"), Value::Bool(false), "{name}: Space toggles");
    }
}

#[test]
fn ow1a_own_toggles_mirror_checked_and_show_focus_only_by_keyboard() {
    let mut rig = rig(&KindSet::tags(TAGS));
    let (_w, col) = rig.window();
    let cb = rig.leaf(col, "checkbox", &[("text", s("A")), ("checked", Value::Bool(true))]);
    let sw = rig.leaf(col, "switch", &[("text", s("B")), ("active", Value::Bool(true))]);
    let tb = rig.leaf(col, "toggle-button", &[("text", s("C")), ("checked", Value::Bool(true))]);
    rig.run();
    rig.run();
    // DOC1's gap: `:checked` beyond checkbox (toggle, switch)
    for h in [cb, sw, tb] {
        assert!(state(&rig, h).contains(StateBits::CHECKED), "{:?}", state(&rig, h));
    }
    rig.set(sw, "active", Value::Bool(false));
    rig.run();
    rig.run();
    assert!(!state(&rig, sw).contains(StateBits::CHECKED), "a prop write clears the mirror");
    // a click focuses (GTK) without a ring; Tab moves focus with the ring (R8, FX1)
    rig.click(cb);
    rig.run();
    let st = state(&rig, cb);
    assert!(st.contains(StateBits::FOCUSED) && !st.contains(StateBits::FOCUS_VISIBLE), "{st:?}");
    rig.harness.key_press(egui::Key::Tab);
    rig.run();
    rig.run();
    let st = state(&rig, sw);
    assert!(st.contains(StateBits::FOCUSED) && st.contains(StateBits::FOCUS_VISIBLE), "{st:?}");
    // hover (R3: same frame)
    rig.harness.hover_at(rig.drawn(tb).center());
    rig.run();
    assert!(state(&rig, tb).contains(StateBits::HOVERED));
}

#[test]
fn ow1a_label_and_hyperlink_click_only_when_listened() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let plain = rig.leaf(col, "label", &[("text", s("Plain"))]);
        let clickable = rig.leaf(col, "label", &[("text", s("Click me"))]);
        let link = rig.leaf(col, "hyperlink", &[("text", s("Docs"))]);
        rig.listen(clickable, "clicked");
        rig.listen(link, "clicked");
        rig.run();
        assert!(rig.drawn(plain).width() > 0.0, "{name}: drawn");
        assert!(rig.harness.query_by_role_and_label(Role::Link, "Docs").is_some(), "{name}: a Link named by its text");
        rig.click(clickable);
        rig.click(link);
        assert_eq!(events(&rig, clickable, "clicked").len(), 1, "{name}: label");
        assert_eq!(events(&rig, link, "clicked").len(), 1, "{name}: link");
    }
}

#[test]
fn ow1a_wrapping_label_grows_in_height_for_width() {
    let mut rig = rig(&KindSet::tags(TAGS));
    let (_w, col) = rig.window();
    let long = "a long label that wraps onto several lines when its column is narrow ".repeat(4);
    let l = rig.leaf(col, "label", &[("text", s(&long)), ("width", Value::F64(120.0))]);
    let t = rig.leaf(col, "label", &[("text", s(&long)), ("width", Value::F64(120.0)), ("wrap-mode", s("truncate"))]);
    rig.run();
    rig.run();
    let (rl, rt) = (rig.drawn(l), rig.drawn(t));
    assert!(rl.height() > 2.5 * rt.height(), "wrap: {rl:?}, truncate: {rt:?}");
}

#[test]
fn ow1a_separators_progress_and_spinner() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let h = rig.leaf(col, "separator", &[]);
        let row = rig.leaf(col, "box", &[("flex-direction", s("row")), ("height", Value::F64(40.0))]);
        let v = rig.leaf(row, "separator", &[("orientation", s("vertical"))]);
        let bar = rig.leaf(col, "progress-bar", &[("fraction", Value::F64(0.5)), ("show-percentage", Value::Bool(true))]);
        let busy = rig.leaf(col, "progress-bar", &[("animate", Value::Bool(true))]);
        let sp = rig.leaf(col, "spinner", &[]);
        rig.harness.run_steps(4); // a spinner and a busy bar repaint every frame
        let (rh, rv) = (rig.drawn(h), rig.drawn(v));
        assert!(rh.width() > 10.0 * rh.height(), "{name}: horizontal fills across: {rh:?}");
        assert!(rv.height() > 3.0 * rv.width(), "{name}: vertical fills down: {rv:?}");
        assert!(rig.drawn(bar).width() > 0.0 && rig.drawn(sp).width() > 0.0, "{name}");
        assert!(rig.harness.query_all_by_role(Role::ProgressIndicator).count() >= 3, "{name}: progress indicators");
        if name == "own" {
            // DOC1's gap: `:busy`
            assert!(state(&rig, busy).contains(StateBits::BUSY));
            assert!(state(&rig, sp).contains(StateBits::BUSY));
            assert!(!state(&rig, bar).contains(StateBits::BUSY));
            rig.set(sp, "spinning", Value::Bool(false));
            rig.harness.run_steps(4);
            assert!(!state(&rig, sp).contains(StateBits::BUSY));
        }
    }
}
