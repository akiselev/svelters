//! OW4: svelters' own `text-edit` on TC0 (`quire-text`), through real input: the platform rules
//! (`native::TextPolicy`), the signals, the extras and the DESIGN look. The comparison with the
//! native references (GTK, Qt, AppKit, Win32/WinUI) is `svelters-conformance`'s `egui_measure`.

mod common;

use common::Rig;
use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::native::{self, Native};

const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const CMD: Modifiers = Modifiers { alt: false, ctrl: false, shift: false, mac_cmd: true, command: true };
const ALT: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn key(rig: &mut Rig, m: Modifiers, k: Key) {
    rig.harness.key_press_modifiers(m, k);
    rig.run();
}

fn text(rig: &mut Rig, t: &str) {
    rig.harness.event(egui::Event::Text(t.into()));
    rig.run();
}

fn events(rig: &Rig, log: &common::Log, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|(eh, es, _)| *eh == h && *es == sig).map(|(_, _, a)| a.clone()).collect()
}

fn field_in(rig: &mut Rig, native: Native, props: &[(&str, Value)]) -> (svl_handle, svl_handle) {
    native::set(&rig.harness.ctx, native);
    let (_w, col) = rig.window();
    let mut p = vec![("accessible-name", s("f"))];
    p.extend_from_slice(props);
    let f = rig.leaf(col, "text-edit", &p);
    rig.run();
    (f, col)
}

fn field(rig: &mut Rig, native: Native, props: &[(&str, Value)]) -> svl_handle {
    field_in(rig, native, props).0
}

/// Enter commits and submits a one-line field; blur after an edit commits; typing announces
/// `changed` every edit (CX3's `bindings.text.commit` contract, now the own widget's).
#[test]
fn ow4_commit_submit_and_changed() {
    let mut rig = Rig::new();
    let (f, col) = field_in(&mut rig, Native::Gtk, &[("text", s("ab"))]);
    let other = rig.leaf(col, "text-edit", &[("accessible-name", s("o"))]);
    for e in ["changed", "commit", "submit", "gained-focus", "lost-focus"] {
        rig.listen(f, e);
    }
    rig.run();
    rig.click(f);
    key(&mut rig, Modifiers::NONE, Key::End);
    text(&mut rig, "c");
    let l = rig.take();
    assert_eq!(events(&rig, &l, f, "changed"), vec![vec![s("abc")]]);
    assert_eq!(events(&rig, &l, f, "gained-focus").len(), 1);
    assert!(events(&rig, &l, f, "commit").is_empty(), "typing does not commit");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    let l = rig.take();
    assert_eq!(events(&rig, &l, f, "commit"), vec![vec![s("abc")]]);
    assert_eq!(events(&rig, &l, f, "submit"), vec![vec![s("abc")]]);
    // blur without an edit since the commit: no commit; after one: one
    rig.click(other);
    assert!(events(&rig, &rig.take(), f, "commit").is_empty());
    rig.click(f);
    text(&mut rig, "d");
    rig.click(other);
    let l = rig.take();
    assert_eq!(events(&rig, &l, f, "commit").len(), 1, "{:?}", l.events);
    assert_eq!(events(&rig, &l, f, "lost-focus").len(), 1);
    assert_eq!(rig.get(f, "modified"), Value::Bool(false), "a commit clears modified");
}

/// FX1's word rules through TC0, per platform: Pango splits `snake_case`, Qt stops at word
/// starts, AppKit keeps `.` and `_` inside a word and moves with Option.
#[test]
fn ow4_word_rules_per_platform() {
    let mut rig = Rig::new();
    let f = field(&mut rig, Native::Gtk, &[("text", s("snake_case baz.qux"))]);
    rig.click(f);
    key(&mut rig, Modifiers::NONE, Key::Home);
    key(&mut rig, CTRL, Key::ArrowRight);
    assert_eq!(rig.backend.text_selection(f), Some([5, 5]), "GTK: `_` ends a word");

    let mut rig = Rig::new();
    let f = field(&mut rig, Native::Qt, &[("text", s("foo bar"))]);
    rig.click(f);
    key(&mut rig, Modifiers::NONE, Key::Home);
    key(&mut rig, CTRL, Key::ArrowRight);
    assert_eq!(rig.backend.text_selection(f), Some([4, 4]), "Qt: to the next word's start");

    let mut rig = Rig::new();
    let f = field(&mut rig, Native::Mac, &[("text", s("snake_case baz.qux"))]);
    rig.click(f);
    key(&mut rig, CMD, Key::ArrowLeft);
    assert_eq!(rig.backend.text_selection(f), Some([0, 0]), "Cmd+Left: the line start");
    key(&mut rig, ALT, Key::ArrowRight);
    assert_eq!(rig.backend.text_selection(f), Some([10, 10]), "AppKit: snake_case is one word");
    key(&mut rig, ALT, Key::ArrowRight);
    assert_eq!(rig.backend.text_selection(f), Some([18, 18]), "AppKit: baz.qux is one word");
    key(&mut rig, ALT, Key::Backspace);
    assert_eq!(rig.get(f, "text"), s("snake_case "));
}

/// GTK: one undo step per word, each deletion its own; Qt: a typing run one step (FX3's models in
/// quire-text).
#[test]
fn ow4_undo_grouping_per_platform() {
    for (native, after_one_undo) in [(Native::Gtk, "foo bar"), (Native::Qt, "")] {
        let mut rig = Rig::new();
        let f = field(&mut rig, native, &[]);
        rig.click(f);
        for c in "foo bar baz".chars() {
            text(&mut rig, &c.to_string());
        }
        assert_eq!(rig.get(f, "text"), s("foo bar baz"));
        assert_eq!(rig.get(f, "can-undo"), Value::Bool(true));
        key(&mut rig, CTRL, Key::Z);
        assert_eq!(rig.get(f, "text"), s(after_one_undo), "{native:?}");
        key(&mut rig, CTRL | Modifiers::SHIFT, Key::Z);
        assert_eq!(rig.get(f, "text"), s("foo bar baz"), "{native:?} redo");
    }
}

/// GTK's Ctrl+Shift+U hex entry: digits in a preedit (the text unchanged), Space commits, Escape
/// cancels; Qt has none.
#[test]
fn ow4_gtk_hex_entry() {
    let mut rig = Rig::new();
    let f = field(&mut rig, Native::Gtk, &[("text", s("ab"))]);
    rig.click(f);
    key(&mut rig, Modifiers::NONE, Key::Home);
    key(&mut rig, Modifiers::NONE, Key::ArrowRight);
    key(&mut rig, CTRL | Modifiers::SHIFT, Key::U);
    text(&mut rig, "42");
    assert_eq!(rig.get(f, "text"), s("ab"), "composing: the text is unchanged");
    rig.harness.key_press(Key::Space);
    rig.harness.event(egui::Event::Text(" ".into()));
    rig.run();
    assert_eq!(rig.get(f, "text"), s("aBb"));
    key(&mut rig, CTRL | Modifiers::SHIFT, Key::U);
    text(&mut rig, "41");
    key(&mut rig, Modifiers::NONE, Key::Escape);
    assert_eq!(rig.get(f, "text"), s("aBb"), "Escape cancels");
    assert!(rig.harness.get_by_label("f").is_focused(), "and keeps focus");
}

/// Line breaks pasted into a one-line field are kept (GTK, Qt, AppKit); Enter in a multi-line field
/// inserts one; Ctrl+Enter submits it.
#[test]
fn ow4_line_breaks() {
    let mut rig = Rig::new();
    let f = field(&mut rig, Native::Gtk, &[("text", s("ab"))]);
    rig.click(f);
    key(&mut rig, Modifiers::NONE, Key::End);
    rig.harness.event(egui::Event::Paste("1\n2".into()));
    rig.run();
    assert_eq!(rig.get(f, "text"), s("ab1\n2"));

    let mut rig = Rig::new();
    let m = field(&mut rig, Native::Gtk, &[("multiline", Value::Bool(true))]);
    rig.listen(m, "submit");
    rig.click(m);
    text(&mut rig, "a");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    text(&mut rig, "b");
    assert_eq!(rig.get(m, "text"), s("a\nb"));
    key(&mut rig, Modifiers::NONE, Key::ArrowUp);
    assert_eq!(rig.backend.text_selection(m), Some([1, 1]), "Up keeps the column");
    key(&mut rig, CTRL, Key::Enter);
    assert_eq!(events(&rig, &rig.take(), m, "submit"), vec![vec![s("a\nb")]]);
}

/// The extras: `read-only` (motions, no edits), `maxlength` with `rejected`, `password` (no copy),
/// `placeholder`; AccessKit roles and the hint as name.
#[test]
fn ow4_extras_and_accessibility() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let ro = rig.leaf(col, "text-edit", &[("text", s("fixed")), ("read-only", Value::Bool(true)), ("accessible-name", s("ro"))]);
    let max = rig.leaf(col, "text-edit", &[("maxlength", Value::I64(3)), ("placeholder", s("Code"))]);
    let pw = rig.leaf(col, "text-edit", &[("password", Value::Bool(true)), ("text", s("secret")), ("accessible-name", s("pw"))]);
    rig.listen(max, "rejected");
    rig.run();
    rig.click(ro);
    text(&mut rig, "x");
    assert_eq!(rig.get(ro, "text"), s("fixed"), "read-only refuses edits");
    key(&mut rig, Modifiers::NONE, Key::Home);
    assert_eq!(rig.backend.text_selection(ro), Some([0, 0]), "and moves");

    let n = rig.harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Code");
    assert_eq!(n.accesskit_node().data().placeholder(), Some("Code"));
    rig.click(max);
    text(&mut rig, "abcd");
    assert_eq!(rig.get(max, "text"), s("abc"));
    assert_eq!(events(&rig, &rig.take(), max, "rejected"), vec![vec![s("max-chars")]]);

    rig.click(pw);
    key(&mut rig, CTRL, Key::A);
    rig.harness.event(egui::Event::Copy);
    rig.run();
    let copied = rig.harness.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(_)));
    assert!(!copied, "a password field copies nothing");
    assert_eq!(rig.get(pw, "can-copy"), Value::Bool(false));
    let n = rig.harness.get_by_role_and_label(egui::accesskit::Role::PasswordInput, "pw");
    assert_eq!(n.accesskit_node().value().as_deref(), Some("\u{2022}".repeat(6).as_str()));
}

/// macOS (WG3m): Tab into a field selects all of it, Return selects all, an unfocused field has no
/// selection, Escape keeps focus.
#[test]
fn ow4_mac_focus_rules() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Mac);
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "text-edit", &[("text", s("first")), ("accessible-name", s("a"))]);
    let b = rig.leaf(col, "text-edit", &[("text", s("second")), ("accessible-name", s("b"))]);
    rig.run();
    rig.click(a);
    key(&mut rig, Modifiers::NONE, Key::Tab);
    assert_eq!(rig.backend.text_selection(b), Some([0, 6]), "Tab selects all");
    assert_eq!(rig.backend.text_selection(a), Some([0, 0]), "NSTextField: no selection unfocused");
    key(&mut rig, Modifiers::NONE, Key::End);
    key(&mut rig, Modifiers::NONE, Key::Enter);
    assert_eq!(rig.backend.text_selection(b), Some([0, 6]), "Return selects all");
    key(&mut rig, Modifiers::NONE, Key::Escape);
    assert!(rig.harness.get_by_label("b").is_focused());
}

/// DESIGN R4: the field is sunken (fill, boundary, inset) and 26px tall at the default density;
/// its height comes from CSS (padding 4/6, 16px lines, a 1px border).
#[test]
fn ow4_field_metrics() {
    let mut rig = Rig::new();
    let f = field(&mut rig, Native::Gtk, &[("text", s("x"))]);
    assert_eq!(rig.drawn(f).height(), 26.0);
}

/// X11 PRIMARY (WG3b pin 5): a middle click pastes it at the pointer into an unfocused field and
/// focuses it; the field that held the selection keeps it (GTK).
#[test]
fn ow4_middle_click_pastes_primary_into_another_field() {
    let mut rig = Rig::new();
    let (a, col) = field_in(&mut rig, Native::Gtk, &[("text", s("primary source words"))]);
    let b = rig.leaf(col, "text-edit", &[("text", s("dest")), ("accessible-name", s("b"))]);
    rig.run();
    rig.click(a);
    rig.backend.set_text_selection(b, 0, 0);
    rig.run();
    let at = rig.backend.text_char_point(b, 2, 0.3).unwrap();
    rig.harness.hover_at(at);
    rig.run();
    rig.harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Middle, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.event(egui::Event::MiddleClickPaste { pos: at, text: "source".into() });
    rig.run();
    rig.harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Middle, pressed: false, modifiers: Modifiers::NONE });
    rig.run();
    assert_eq!(rig.get(b, "text"), s("desourcest"));
    assert_eq!(rig.backend.text_selection(b), Some([8, 8]));
    assert!(rig.harness.get_by_label("b").is_focused());
}
