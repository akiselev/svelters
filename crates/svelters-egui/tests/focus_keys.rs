//! EV3+: focus scopes, initial focus and restore, R11 key claims, edit commands and default/cancel
//! buttons (`svelters_egui::focus`), through real input on the built-in leaves.

mod common;

use common::Rig;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;
use svelters_abi::*;
use svelters_egui::EguiBackend;
use svelters_egui::focus::{self, TextCommand};
use svelters_egui::native::{self, Native};

const CTRL: egui::Modifiers = egui::Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn focused(rig: &Rig, label: &str) -> bool {
    rig.harness.query_by_label(label).is_some_and(|n| n.is_focused())
}

fn key(rig: &mut Rig, m: egui::Modifiers, k: egui::Key) {
    rig.harness.key_press_modifiers(m, k);
    rig.run();
}

fn clicked(rig: &Rig, h: svl_handle) -> usize {
    let sig = rig.signal_id(h, "clicked");
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).count()
}

struct Dialog {
    col: svl_handle,
    opener: svl_handle,
    dialog: svl_handle,
    name: svl_handle,
    ok: svl_handle,
    cancel: svl_handle,
}

/// An opener, a hidden `dialog` scope (a field, OK default, Cancel cancel) and a button after it.
fn dialog_rig() -> (Rig, Dialog) {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk); // a press focuses the opener
    let (_w, col) = rig.window();
    let opener = rig.leaf(col, "button", &[("text", s("Open"))]);
    let dialog = rig.create("box");
    rig.set(dialog, "focus-scope", s("dialog"));
    rig.set(dialog, "visible", Value::Bool(false));
    rig.insert(col, dialog);
    let name = rig.leaf(dialog, "text-edit", &[("accessible-name", s("name"))]);
    let ok = rig.leaf(dialog, "button", &[("text", s("OK")), ("default", Value::Bool(true))]);
    let cancel = rig.leaf(dialog, "button", &[("text", s("Cancel")), ("cancel", Value::Bool(true))]);
    let _after = rig.leaf(col, "button", &[("text", s("After"))]);
    for h in [ok, cancel] {
        rig.listen(h, "clicked");
    }
    rig.run();
    (rig, Dialog { col, opener, dialog, name, ok, cancel })
}

#[test]
fn ev3_dialog_scope_focus_tab_default_cancel_restore() {
    let (mut rig, d) = dialog_rig();
    assert_eq!(rig.get(d.dialog, "focus-scope"), s("dialog"));
    rig.click(d.opener);
    assert!(focused(&rig, "Open"));

    // opening: the dialog's first field takes focus
    rig.set(d.dialog, "visible", Value::Bool(true));
    rig.run();
    assert!(focused(&rig, "name"), "initial focus in the dialog");

    // Tab cycles inside the dialog and wraps; Shift+Tab goes back
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "OK"));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "Cancel"));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "name"), "Tab wraps to the first node, never to `After`");
    key(&mut rig, egui::Modifiers::SHIFT, egui::Key::Tab);
    assert!(focused(&rig, "Cancel"));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "name"));

    // Enter in the one-line field activates the default button; the field keeps focus
    rig.take();
    rig.harness.event(egui::Event::Text("Ada".into()));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!((clicked(&rig, d.ok), clicked(&rig, d.cancel)), (1, 0));
    assert!(focused(&rig, "name"));
    assert_eq!(rig.get(d.name, "text"), s("Ada"));

    // Escape activates the cancel button
    rig.take();
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Escape);
    assert_eq!((clicked(&rig, d.ok), clicked(&rig, d.cancel)), (0, 1));

    // Enter on a focused button is the button's own (no second activation of OK)
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "Cancel"));
    rig.take();
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!((clicked(&rig, d.ok), clicked(&rig, d.cancel)), (0, 1));

    // closing: focus goes back to the opener
    rig.set(d.dialog, "visible", Value::Bool(false));
    rig.run();
    assert!(focused(&rig, "Open"), "focus restored to the opener");
}

#[test]
fn ev3_autofocus_and_disabled_nodes_leave_the_chain() {
    let (mut rig, d) = dialog_rig();
    rig.set(d.ok, "autofocus", Value::Bool(true));
    rig.set(d.cancel, "enabled", Value::Bool(false));
    rig.set(d.dialog, "visible", Value::Bool(true));
    rig.run();
    assert!(focused(&rig, "OK"), "autofocus wins over the first node");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "name"), "the disabled Cancel is skipped");
    // a disabled cancel button is not activated
    rig.take();
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Escape);
    assert_eq!(clicked(&rig, d.cancel), 0);
    // removing the dialog restores focus too (nothing was focused before it: nothing to restore)
    rig.br().remove(d.col, d.dialog).unwrap();
    rig.run();
    assert!(rig.harness.ctx.memory(|m| m.focused()).is_none());
}

#[derive(Default)]
struct Keymap {
    /// What the host keymap stub saw: `shortcut_override` before the content, the key after it.
    overridden: Vec<bool>,
    seen_after: Vec<bool>,
    consumed: Vec<egui::KeyboardShortcut>,
}

/// A rig whose frame is: the host keymap asks `shortcut_override` for Ctrl+Z, the svelters content
/// runs, then the host keymap looks for Ctrl+Z in what is left of the input (cad's keymap stub).
fn keymap_rig() -> (Rig, Rc<RefCell<Keymap>>) {
    let mut rig = Rig::new();
    let km: Rc<RefCell<Keymap>> = Rc::default();
    let k = km.clone();
    let z = egui::KeyboardShortcut::new(CTRL, egui::Key::Z);
    rig.harness = Harness::builder().with_size(egui::vec2(480.0, 360.0)).build_ui_state(
        move |ui, b: &mut EguiBackend| {
            let pressed = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Z, pressed: true, .. })));
            if pressed {
                k.borrow_mut().overridden.push(b.shortcut_override(ui.ctx(), &z));
            }
            b.ui(ui);
            if pressed {
                let seen = ui.input_mut(|i| i.consume_shortcut(&z));
                let mut k = k.borrow_mut();
                k.seen_after.push(seen);
                k.consumed.extend(focus::consumed_keys(ui.ctx()));
            }
        },
        rig.backend.clone(),
    );
    (rig, km)
}

#[test]
fn ev3_text_field_claims_keys_from_the_host_keymap() {
    let (mut rig, km) = keymap_rig();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let field = rig.leaf(col, "text-edit", &[("accessible-name", s("field")), ("text", s("abc"))]);
    let _btn = rig.leaf(col, "button", &[("text", s("Plain"))]);
    rig.run();

    // a focused field takes Ctrl+Z: the keymap is told before and does not see it after
    rig.type_into(field, "abcd");
    assert_eq!(rig.get(field, "text"), s("abcd"));
    let z = egui::KeyboardShortcut::new(CTRL, egui::Key::Z);
    let save = egui::KeyboardShortcut::new(CTRL, egui::Key::S);
    let del = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Delete);
    let left = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::ArrowLeft);
    let ctx = rig.harness.ctx.clone();
    assert!(rig.backend.shortcut_override(&ctx, &z) && rig.backend.shortcut_override(&ctx, &del) && rig.backend.shortcut_override(&ctx, &left));
    assert!(!rig.backend.shortcut_override(&ctx, &save), "Ctrl+S stays the app's");
    let info = rig.backend.focus_info(&ctx);
    assert!(info.text_input && !info.multiline && !info.composing && info.kind == Some("text-edit"), "{info:?}");
    km.borrow_mut().overridden.clear();
    key(&mut rig, CTRL, egui::Key::Z);
    {
        let k = km.borrow();
        assert_eq!(k.overridden, vec![true]);
        assert_eq!(k.seen_after, vec![false], "the keymap never sees a claimed key");
        assert!(k.consumed.contains(&z), "{:?}", k.consumed);
    }
    assert_eq!(rig.get(field, "text"), s("abc"), "the field undid the typing");

    // a focused button claims nothing of the kind: the keymap gets Ctrl+Z
    let pos = rig.harness.query_by_label("Plain").unwrap().rect().center();
    rig.harness.hover_at(pos);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    rig.run();
    assert!(focused(&rig, "Plain"));
    *km.borrow_mut() = Keymap::default();
    key(&mut rig, CTRL, egui::Key::Z);
    let k = km.borrow();
    assert_eq!((k.overridden.clone(), k.seen_after.clone()), (vec![false], vec![true]));
    assert!(!rig.backend.focus_info(&ctx).text_input);
}

#[test]
fn ev3_text_commands_reach_the_focused_field() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let field = rig.leaf(col, "text-edit", &[("accessible-name", s("field"))]);
    let btn = rig.leaf(col, "button", &[("text", s("Plain"))]);
    rig.run();
    let ctx = rig.harness.ctx.clone();
    assert!(!rig.backend.text_command(&ctx, TextCommand::SelectAll), "nothing focused, no field yet");

    rig.type_into(field, "hello");
    assert!(rig.backend.text_command(&ctx, TextCommand::SelectAll));
    rig.run();
    assert!(rig.backend.text_command(&ctx, TextCommand::Insert("bye".into())));
    rig.run();
    assert_eq!(rig.get(field, "text"), s("bye"), "select all, then the host's text replaced it");
    assert!(rig.backend.text_command(&ctx, TextCommand::Undo));
    rig.run();
    assert_ne!(rig.get(field, "text"), s("bye"), "undone (egui's undoer decides how far)");
    assert!(rig.backend.text_command(&ctx, TextCommand::Redo));
    rig.run();
    assert_eq!(rig.get(field, "text"), s("bye"));

    // a click on host chrome took focus to nowhere: the command goes to the last field again
    ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
    rig.run();
    assert!(ctx.memory(|m| m.focused()).is_none());
    assert!(rig.backend.text_command(&ctx, TextCommand::SelectAll));
    rig.run();
    assert!(rig.backend.text_command(&ctx, TextCommand::Delete));
    rig.run();
    assert_eq!(rig.get(field, "text"), s(""));
    assert!(focused(&rig, "field"));

    // another widget has focus: no command
    rig.click(btn);
    assert!(focused(&rig, "Plain"));
    assert!(!rig.backend.text_command(&ctx, TextCommand::Undo));
}

#[test]
fn ev3_composing_holds_enter_and_every_key() {
    let (mut rig, d) = dialog_rig();
    rig.set(d.dialog, "visible", Value::Bool(true));
    rig.run();
    assert!(focused(&rig, "name"));
    rig.take();
    let ctx = rig.harness.ctx.clone();
    rig.harness.event(egui::Event::Ime(egui::ImeEvent::Preedit { text: "ka".into(), active_range_chars: None }));
    rig.run();
    assert!(focus::composing(&ctx) && rig.backend.focus_info(&ctx).composing);
    let save = egui::KeyboardShortcut::new(CTRL, egui::Key::S);
    assert!(rig.backend.shortcut_override(&ctx, &save), "while composing every key is the field's");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!(clicked(&rig, d.ok), 0, "no Enter to the default button while composing");
    assert!(focused(&rig, "name") && focus::composing(&ctx), "Enter was the input method's");
    rig.harness.event(egui::Event::Ime(egui::ImeEvent::Commit("ka".into())));
    rig.run();
    assert!(!focus::composing(&ctx));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!(clicked(&rig, d.ok), 1);
}

/// The same from Svelte: the props compile, the dialog opens with focus in its `autofocus` field,
/// Enter runs the default button's handler, and focus returns to the opener when `{#if}` removes
/// the dialog; Escape runs the cancel button's.
#[test]
fn ev3_svelte_dialog_from_source() {
    use svelters_egui::harness::TestApp;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/src/FocusDialog.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    native::set(&app.harness.ctx, Native::Gtk);
    app.run();
    let is_focused = |app: &TestApp, l: &str| app.harness.query_by_label(l).is_some_and(|n| n.is_focused());
    let press = |app: &mut TestApp, l: &str| {
        let pos = app.harness.get_by_label(l).rect().center();
        app.harness.hover_at(pos);
        for pressed in [true, false] {
            app.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        app.run();
    };
    press(&mut app, "Open");
    assert!(is_focused(&app, "name"), "autofocus in the dialog");
    app.harness.event(egui::Event::Text("Ada".into()));
    app.run();
    app.harness.key_press(egui::Key::Enter);
    app.run();
    assert!(app.harness.query_by_label("Result: ok:Ada").is_some(), "{:?}", app.errors());
    assert!(app.harness.query_by_label("OK").is_none());
    assert!(is_focused(&app, "Open"), "focus back on the opener");
    // Enter on the focused opener is its own: the dialog opens again; Escape cancels it
    app.harness.key_press(egui::Key::Enter);
    app.run();
    assert!(is_focused(&app, "name"));
    app.harness.key_press(egui::Key::Escape);
    app.run();
    assert!(app.harness.query_by_label("Result: cancel").is_some());
    assert!(is_focused(&app, "Open"));
}

/// WG2a's `modal` is a dialog scope: focus moves in when it opens, Tab stays in it, Enter reaches its
/// default button, and focus goes back to the opener when it closes.
#[test]
fn ev3_modal_overlay_is_a_dialog_scope() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let opener = rig.leaf(col, "button", &[("text", s("Open"))]);
    let modal = rig.leaf(col, "modal", &[("open", Value::Bool(false))]);
    let _f = rig.leaf(modal, "text-edit", &[("accessible-name", s("field"))]);
    let ok = rig.leaf(modal, "button", &[("text", s("OK")), ("default", Value::Bool(true))]);
    rig.listen(ok, "clicked");
    rig.run();
    rig.click(opener);
    assert!(focused(&rig, "Open"));
    rig.set(modal, "open", Value::Bool(true));
    rig.run();
    assert!(focused(&rig, "field"), "the modal's first field takes focus");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "OK"));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, "field"), "Tab stays in the modal");
    rig.take();
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Enter);
    assert_eq!(clicked(&rig, ok), 1);
    rig.set(modal, "open", Value::Bool(false));
    rig.run();
    assert!(focused(&rig, "Open"), "focus back on the opener");
}

/// A menu (an egui popup) takes focus for its keys; when it closes, focus goes back to the field
/// that had it, so an Edit-menu command reaches it.
#[test]
fn ev3_focus_returns_after_a_menu_closes() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Egui); // a click does not focus the menu button
    let (_w, col) = rig.window();
    let field = rig.leaf(col, "text-edit", &[("accessible-name", s("field"))]);
    let mb = rig.leaf(col, "menu-button", &[("label", s("More")), ("align-self", s("start"))]);
    let copy = rig.leaf(mb, "action", &[("label", s("Copy"))]);
    rig.listen(copy, "activate");
    rig.run();
    rig.type_into(field, "hi");
    assert!(focused(&rig, "field"));
    let click = |rig: &mut Rig, l: &str| {
        let pos = rig.harness.get_by_label(l).rect().center();
        rig.harness.hover_at(pos);
        for pressed in [true, false] {
            rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        rig.run();
    };
    click(&mut rig, "More");
    assert!(rig.harness.query_by_label("Copy").is_some(), "the menu is open");
    assert!(!focused(&rig, "field"));
    click(&mut rig, "Copy");
    assert!(rig.harness.query_by_label("Copy").is_none(), "the menu closed");
    assert!(focused(&rig, "field"), "focus back in the field");
}

/// Through the embed, Cut, Copy and Paste use the host's clipboard (DX2 `Host::platform()`, the one
/// `svelters:clipboard` uses; in-process under kittest), and Copy still reaches egui's.
#[test]
fn ev3_edit_commands_use_the_host_clipboard() {
    use svelters_egui::harness::TestApp;
    use svelters_host::platform::{ClipboardKind, MimeData};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/src/FocusDialog.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    app.harness.get_by_label("Open").click();
    app.run();
    let ctx = app.harness.ctx.clone();
    let clip = app.embed.host().platform().clipboard.clone();
    let text = |clip: &std::sync::Arc<dyn svelters_host::platform::ClipboardBackend>| clip.read(ClipboardKind::Clipboard, &[]).unwrap().plain_text().map(str::to_string);
    let run = |app: &mut TestApp, cmd: TextCommand| {
        assert!(app.embed.text_command(&ctx, cmd));
        app.run();
    };
    app.harness.event(egui::Event::Text("Ada".into()));
    app.run();
    run(&mut app, TextCommand::SelectAll);
    run(&mut app, TextCommand::Copy);
    assert_eq!(text(&clip).as_deref(), Some("Ada"), "Copy wrote the host's clipboard");
    // what JS (or a Rust panel) put on the clipboard is what Paste inserts
    clip.write(ClipboardKind::Clipboard, MimeData::text("Bob")).unwrap();
    run(&mut app, TextCommand::SelectAll);
    run(&mut app, TextCommand::Paste);
    run(&mut app, TextCommand::SelectAll);
    run(&mut app, TextCommand::Cut);
    assert_eq!(text(&clip).as_deref(), Some("Bob"), "Cut wrote the host's clipboard");
    run(&mut app, TextCommand::Paste);
    app.harness.key_press(egui::Key::Enter);
    app.run();
    assert!(app.harness.query_by_label("Result: ok:Bob").is_some(), "{:?}", app.errors());
}
