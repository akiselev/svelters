//! FX3: undo grouping, the CUA clipboard chords, line breaks in a paste, Ctrl+Shift+U, Enter and
//! the PRIMARY selection (`svelters_egui::native`, `svelters_egui::primary`) on the built-in
//! `text-edit`, through real input. The full comparison with the GTK and Qt references is
//! `svelters-conformance`'s `egui_measure`.

mod common;

use common::Rig;
use egui::text_edit::TextEditState;
use svelters_abi::*;
use svelters_egui::native::{self, Native};

const CTRL: egui::Modifiers = egui::Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const CTRL_SHIFT: egui::Modifiers = egui::Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };

fn s(t: &str) -> Value {
    Value::Str(t.into())
}

fn sel(rig: &Rig, h: svl_handle) -> Option<[usize; 2]> {
    let id = rig.backend.with_scene(|s| s.n(h).id);
    TextEditState::load(&rig.harness.ctx, id).and_then(|s| s.cursor.char_range()).map(|r| [r.secondary.index.0, r.primary.index.0])
}

fn focused(rig: &Rig, h: svl_handle) -> bool {
    let id = rig.backend.with_scene(|s| s.n(h).id);
    rig.harness.ctx.memory(|m| m.has_focus(id))
}

/// Presses `k`, one pass per event (a kittest `step` keeps only its last pass's output); the
/// clipboard text egui copied and whether it asked the integration for a paste.
fn chord(rig: &mut Rig, m: egui::Modifiers, k: egui::Key) -> (Option<String>, bool) {
    let key = |pressed| egui::Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: m };
    let (mut copied, mut paste) = (None, false);
    for e in [egui::Event::ModifiersChanged(m), key(true), key(false), egui::Event::ModifiersChanged(egui::Modifiers::NONE)] {
        rig.harness.event(e);
        rig.harness.step();
        let out = rig.harness.output();
        for c in &out.platform_output.commands {
            if let egui::OutputCommand::CopyText(t) = c {
                copied = Some(t.clone());
            }
        }
        paste |= out.viewport_output.values().any(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::RequestPaste)));
    }
    rig.run();
    (copied, paste)
}

fn key(rig: &mut Rig, m: egui::Modifiers, k: egui::Key) {
    rig.harness.key_press_modifiers(m, k);
    rig.run();
}

fn typ(rig: &mut Rig, t: &str) {
    for c in t.chars() {
        rig.harness.event(egui::Event::Text(c.to_string()));
        rig.run();
    }
}

fn field(native: Native, text: &str) -> (Rig, svl_handle) {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, native);
    let (_w, col) = rig.window();
    let t = rig.leaf(col, "text-edit", &[("text", s(text)), ("accessible-name", s("field"))]);
    rig.harness.ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
    rig.observe(t, "text", true);
    rig.listen(t, "changed");
    rig.run();
    rig.click(t);
    (rig, t)
}

#[test]
fn fx3_cua_chords() {
    let (mut rig, t) = field(Native::Gtk, "copy this text");
    key(&mut rig, CTRL, egui::Key::A);
    // Ctrl+Insert copies
    assert_eq!(chord(&mut rig, CTRL, egui::Key::Insert), (Some("copy this text".into()), false));
    assert_eq!(rig.get(t, "text"), s("copy this text"));
    // Shift+Insert asks the integration to paste (eframe sends `Event::Paste` next)
    key(&mut rig, egui::Modifiers::NONE, egui::Key::End);
    assert_eq!(chord(&mut rig, egui::Modifiers::SHIFT, egui::Key::Insert), (None, true));
    rig.harness.event(egui::Event::Paste("!".into()));
    rig.run();
    assert_eq!(rig.get(t, "text"), s("copy this text!"));
    // Shift+Delete cuts
    for _ in 0..5 {
        key(&mut rig, egui::Modifiers::SHIFT, egui::Key::ArrowLeft);
    }
    rig.take();
    assert_eq!(chord(&mut rig, egui::Modifiers::SHIFT, egui::Key::Delete), (Some("text!".into()), false));
    assert_eq!(rig.get(t, "text"), s("copy this "));
    assert_eq!(rig.take().changed.len(), 1);

    // egui's policy: Ctrl+Insert does nothing, Shift+Delete deletes without copying
    let (mut rig, t) = field(Native::Egui, "abc");
    key(&mut rig, CTRL, egui::Key::A);
    assert_eq!(chord(&mut rig, CTRL, egui::Key::Insert), (None, false));
    assert_eq!(chord(&mut rig, egui::Modifiers::SHIFT, egui::Key::Delete), (None, false));
    assert_eq!(rig.get(t, "text"), s(""));
}

#[test]
fn fx3_undo_grouping() {
    // GTK: one step per word
    let (mut rig, t) = field(Native::Gtk, "");
    typ(&mut rig, "foo bar");
    rig.take();
    key(&mut rig, CTRL, egui::Key::Z);
    assert_eq!(rig.get(t, "text"), s("foo"));
    assert_eq!(sel(&rig, t), Some([3, 3]));
    assert_eq!(rig.take().changed.len(), 1, "undo announces the text");
    key(&mut rig, CTRL, egui::Key::Y);
    assert_eq!(rig.get(t, "text"), s("foo bar"));
    key(&mut rig, CTRL, egui::Key::Z);
    key(&mut rig, CTRL, egui::Key::Z);
    assert_eq!(rig.get(t, "text"), s(""));
    key(&mut rig, CTRL_SHIFT, egui::Key::Z);
    assert_eq!(rig.get(t, "text"), s("foo"));
    // a text set from outside starts a new history
    rig.set(t, "text", s("set"));
    rig.run();
    key(&mut rig, CTRL, egui::Key::Z);
    assert_eq!(rig.get(t, "text"), s("set"));

    // Qt: the typing run is one step; a BackSpace run is another
    let (mut rig, t) = field(Native::Qt, "");
    typ(&mut rig, "foo bar");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Backspace);
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Backspace);
    key(&mut rig, CTRL, egui::Key::Z);
    assert_eq!(rig.get(t, "text"), s("foo bar"));
    key(&mut rig, CTRL, egui::Key::Z);
    assert_eq!(rig.get(t, "text"), s(""));
}

#[test]
fn fx3_unicode_entry_and_enter() {
    // GTK: Ctrl+Shift+U, hex digits, Space commits; Enter keeps focus
    let (mut rig, t) = field(Native::Gtk, "ab");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Home);
    key(&mut rig, egui::Modifiers::NONE, egui::Key::ArrowRight);
    key(&mut rig, CTRL_SHIFT, egui::Key::U);
    typ(&mut rig, "65e5");
    assert_eq!(rig.get(t, "text"), s("ab"), "the preedit is not text");
    rig.harness.key_press(egui::Key::Space);
    typ(&mut rig, " ");
    assert_eq!(rig.get(t, "text"), s("a日b"));
    key(&mut rig, CTRL_SHIFT, egui::Key::U);
    typ(&mut rig, "41");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Backspace);
    typ(&mut rig, "2");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!(rig.get(t, "text"), s("a日Bb"));
    assert!(focused(&rig, t));
    key(&mut rig, CTRL_SHIFT, egui::Key::U);
    typ(&mut rig, "4e");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Escape);
    typ(&mut rig, "z");
    assert_eq!(rig.get(t, "text"), s("a日Bzb"));
    assert!(focused(&rig, t));

    // Qt: Ctrl+Shift+U does nothing (egui would delete to the line start)
    let (mut rig, t) = field(Native::Qt, "ab");
    key(&mut rig, CTRL_SHIFT, egui::Key::U);
    typ(&mut rig, "4e");
    assert_eq!(rig.get(t, "text"), s("ab4e"));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert!(focused(&rig, t));

    // egui: Enter ends editing
    let (mut rig, t) = field(Native::Egui, "ab");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert!(!focused(&rig, t));
}

#[test]
fn fx3_paste_keeps_line_breaks() {
    for (native, want) in [(Native::Gtk, "aone\ntwo\tthreeb"), (Native::Egui, "aone two\tthreeb")] {
        let (mut rig, t) = field(native, "ab");
        key(&mut rig, egui::Modifiers::NONE, egui::Key::Home);
        key(&mut rig, egui::Modifiers::NONE, egui::Key::ArrowRight);
        rig.take();
        rig.harness.event(egui::Event::Paste("one\ntwo\tthree".into()));
        rig.run();
        assert_eq!(rig.get(t, "text"), s(want), "{native:?}");
        assert_eq!(rig.take().changed.len(), 1);
    }
}

/// W5 folded onto egui 0.37's own PRIMARY: when the integration sends `MiddleClickPaste`, egui's
/// `TextEdit` pastes and the provider path does not paste again; a GTK selection that collapses
/// empties PRIMARY.
#[cfg(target_os = "linux")]
#[test]
fn fx3_primary_pastes_once_and_clears() {
    use std::sync::{Arc, Mutex};
    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<String>>);
    impl svelters_egui::primary::PrimarySelection for Fake {
        fn set(&self, text: &str) {
            *self.0.lock().unwrap() = text.to_string();
        }
        fn get(&self) -> Option<String> {
            Some(self.0.lock().unwrap().clone())
        }
    }
    let fake = Fake::default();
    svelters_egui::primary::install(fake.clone());
    let (mut rig, t) = field(Native::Gtk, "hello");
    key(&mut rig, CTRL, egui::Key::A);
    assert_eq!(*fake.0.lock().unwrap(), "hello");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::End);
    assert_eq!(*fake.0.lock().unwrap(), "", "GTK drops PRIMARY when the selection collapses");
    *fake.0.lock().unwrap() = "P".into();
    let at = rig.drawn(t).left_center() + egui::vec2(5.0, 0.0);
    rig.harness.hover_at(at);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Middle, pressed, modifiers: egui::Modifiers::NONE });
        if pressed {
            rig.harness.event(egui::Event::MiddleClickPaste { pos: at, text: "P".into() });
        }
        rig.run();
    }
    assert_eq!(rig.get(t, "text"), s("Phello"));
    svelters_egui::primary::uninstall();
}


/// GTK keeps an unfocused field's selection; putting it back each pass (egui collapses it) must
/// not keep the UI repainting (`Rig::run` panics when it does not settle).
#[test]
fn fx3_unfocused_gtk_selection_settles() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "text-edit", &[("text", s("hello")), ("accessible-name", s("a"))]);
    let b = rig.leaf(col, "text-edit", &[("text", s("world")), ("accessible-name", s("b"))]);
    rig.run();
    rig.click(a);
    key(&mut rig, CTRL, egui::Key::A);
    rig.click(b);
    rig.run();
    assert!(focused(&rig, b));
    assert_eq!(sel(&rig, a), Some([0, 5]));
}
