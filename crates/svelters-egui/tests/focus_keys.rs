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
    // OW3: the egui menus and svelters' own (the own menu holds the keyboard while open)
    for set in [svelters_egui::widgets::KindSet::None, svelters_egui::widgets::KindSet::tags(["menu-button"])] {
        ev3_focus_returns_after_a_menu_closes_on(set);
    }
}

fn ev3_focus_returns_after_a_menu_closes_on(set: svelters_egui::widgets::KindSet) {
    let mut rig = Rig::new();
    rig.backend.set_own_widgets(set);
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

// ---- EV3b: Embed hooks for cadatomic (last text field, Latin fallback, provider edit commands) ----

use std::cell::Cell;
use svelters_egui::provider::{AvailableSpace, MeasureCx, Widget, WidgetCx};

/// A provider text input whose egui `TextEdit` has the node's id (`<quantity-field>`'s shape): EV3's
/// edit commands reach it as the input they stand for.
#[svelters::widget(namespace = "t", tag = "t-field", caps = FOCUSABLE | EDITABLE)]
pub struct TField {
    #[prop]
    label: String,
    #[state]
    text: String,
}

impl Widget for TField {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(160.0, 24.0)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        ui.add(egui::TextEdit::singleline(&mut self.text).id(cx.id()).desired_width(160.0))
    }
    fn a11y(&self, _cx: &svelters_egui::provider::A11yCx<'_>, node: &mut egui::accesskit::Node) {
        node.set_label(if self.label.is_empty() { "qty" } else { self.label.as_str() });
    }
    fn text_input(&self) -> Option<bool> {
        Some(false)
    }
}

/// A provider text input with its own undo (`<code-editor>`'s shape, a host-owned document): it
/// takes the edit commands through `Widget::text_command`, except Delete, which it leaves to be
/// fed as the key. `editable = false` makes it a plain focusable kind (`text_input` is `None`).
#[svelters::widget(namespace = "t", tag = "t-editor", caps = FOCUSABLE | EDITABLE)]
pub struct TEditor {
    #[prop(default = true)]
    editable: bool,
    #[state]
    text: String,
    #[state]
    undo: Vec<String>,
    /// The commands `text_command` was given.
    #[state]
    handled: Vec<TextCommand>,
    /// The key events its `ui` saw while focused: (Ctrl/Cmd+Z, Delete).
    #[state]
    keys: (usize, usize),
}

impl Widget for TEditor {
    fn measure(&mut self, _cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        egui::vec2(160.0, 40.0)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let resp = ui.interact(cx.rect(), cx.id(), egui::Sense::click());
        if resp.clicked() {
            resp.request_focus();
        }
        if resp.has_focus() {
            ui.input(|i| {
                for e in &i.events {
                    match e {
                        egui::Event::Text(t) => {
                            self.undo.push(self.text.clone());
                            self.text.push_str(t);
                        }
                        egui::Event::Key { key: egui::Key::Z, pressed: true, modifiers, .. } if modifiers.command => self.keys.0 += 1,
                        egui::Event::Key { key: egui::Key::Delete, pressed: true, .. } => self.keys.1 += 1,
                        _ => {}
                    }
                }
            });
        }
        resp
    }
    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.editable && m.command && key == egui::Key::Z
    }
    fn text_input(&self) -> Option<bool> {
        self.editable.then_some(true)
    }
    fn text_command(&mut self, cmd: &TextCommand, cx: &mut WidgetCx<'_>) -> bool {
        self.handled.push(cmd.clone());
        match cmd {
            TextCommand::Undo => {
                if let Some(t) = self.undo.pop() {
                    self.text = t;
                }
            }
            TextCommand::Insert(t) => {
                self.undo.push(std::mem::replace(&mut self.text, t.clone()));
            }
            TextCommand::Copy => cx.ctx().copy_text(self.text.clone()),
            TextCommand::Delete => return false,
            _ => {}
        }
        true
    }
}

fn editor<R>(rig: &Rig, h: svl_handle, f: impl FnOnce(&TEditor) -> R) -> R {
    rig.backend.with_scene(|s| s.n(h).widget(f)).expect("a t-editor")
}

/// The egui id of the widget of node `h` (its node id for these leaves and kinds).
fn widget_id(rig: &Rig, h: svl_handle) -> egui::Id {
    rig.backend.with_scene(|s| s.n(h).id)
}

/// A rig whose frame draws the svelters content, then a host button ("Host item", a menu item or
/// host chrome) that takes focus when `focus_host` is set.
fn host_rig() -> (Rig, Rc<Cell<bool>>) {
    let mut rig = Rig::new();
    let focus_host: Rc<Cell<bool>> = Rc::default();
    let f = focus_host.clone();
    rig.harness = Harness::builder().with_size(egui::vec2(480.0, 360.0)).build_ui_state(
        move |ui, b: &mut EguiBackend| {
            b.ui(ui);
            let r = ui.add(egui::Button::new("Host item"));
            if f.take() {
                r.request_focus();
            }
        },
        rig.backend.clone(),
    );
    (rig, focus_host)
}

#[test]
fn ev3b_last_text_field_survives_menus_and_ends_with_the_field() {
    let (mut rig, focus_host) = host_rig();
    native::set(&rig.harness.ctx, Native::Egui); // a click does not focus the menu button
    let (_w, col) = rig.window();
    let field = rig.leaf(col, "text-edit", &[("accessible-name", s("field"))]);
    let _btn = rig.leaf(col, "button", &[("text", s("Plain"))]);
    let mb = rig.leaf(col, "menu-button", &[("label", s("More")), ("align-self", s("start"))]);
    let _copy = rig.leaf(mb, "action", &[("label", s("Copy"))]);
    let qty = rig.create("t-field");
    rig.insert(col, qty);
    rig.run();
    let ctx = rig.harness.ctx.clone();
    assert_eq!(rig.backend.last_text_field(&ctx), None, "no text field had focus yet");

    // the focused field is the last one
    rig.type_into(field, "hi");
    let info = rig.backend.last_text_field(&ctx).expect("the focused field");
    assert_eq!((info.kind, info.text_input, info.widget), (Some("text-edit"), true, Some(widget_id(&rig, field))));

    // a click on host chrome took focus to nowhere: still the field
    ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
    rig.run();
    assert_eq!(rig.backend.last_text_field(&ctx).and_then(|i| i.widget), Some(widget_id(&rig, field)));

    // a menu opens and one of its items (a host widget) takes focus: still the field, and the
    // Edit menu's command reaches it
    let pos = rig.harness.get_by_label("More").rect().center();
    rig.harness.hover_at(pos);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    rig.run();
    assert!(rig.harness.query_by_label("Copy").is_some(), "the menu is open");
    focus_host.set(true);
    rig.run();
    assert!(focused(&rig, "Host item"));
    assert_eq!(rig.backend.last_text_field(&ctx).and_then(|i| i.widget), Some(widget_id(&rig, field)), "a menu item has focus");
    assert!(rig.backend.text_command(&ctx, TextCommand::SelectAll));
    rig.run();
    assert!(focused(&rig, "field"), "the command focused the field again");
    rig.harness.key_press(egui::Key::Escape); // closes the menu
    rig.run();
    rig.run();
    assert!(rig.harness.query_by_label("Copy").is_none(), "the menu closed");

    // another widget takes focus (a host widget with no menu open, a svelters button): no target
    focus_host.set(true);
    rig.run();
    rig.run();
    assert!(focused(&rig, "Host item"));
    assert_eq!(rig.backend.last_text_field(&ctx), None, "a host widget took focus");
    assert!(!rig.backend.text_command(&ctx, TextCommand::Undo));
    rig.type_into(field, "hi");
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab); // (a click does not focus a button here)
    assert!(focused(&rig, "Plain"));
    ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
    rig.run();
    assert_eq!(rig.backend.last_text_field(&ctx), None, "the button had focus after the field");

    // a provider kind with `text_input` is a text field too
    rig.click(qty);
    rig.run();
    let info = rig.backend.last_text_field(&ctx).expect("the provider field");
    assert_eq!((info.kind, info.text_input, info.multiline), (Some("t-field"), true, false));

    // disabled: gone, and it does not come back when enabled again
    ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
    rig.run();
    assert!(rig.backend.last_text_field(&ctx).is_some());
    rig.set(qty, "enabled", Value::Bool(false));
    rig.run();
    rig.run();
    assert_eq!(rig.backend.last_text_field(&ctx), None, "a disabled field is no target");
    assert!(!rig.backend.text_command(&ctx, TextCommand::Undo));
    rig.set(qty, "enabled", Value::Bool(true));
    rig.run();
    rig.run();
    assert_eq!(rig.backend.last_text_field(&ctx), None);

    // removed
    rig.type_into(field, "hi");
    ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
    rig.run();
    assert!(rig.backend.last_text_field(&ctx).is_some());
    rig.br().remove(col, field).unwrap();
    rig.run();
    rig.run();
    assert_eq!(rig.backend.last_text_field(&ctx), None, "a removed field is no target");
}

/// The Latin-fallback matrix (cadatomic KL1): the typed key wins; Ctrl/Cmd chords also ask for the
/// physical key only when the logical one is a symbol or a non-Latin letter (which egui reports
/// as its physical key).
#[test]
fn ev3b_shortcut_override_press_latin_fallback() {
    use egui::Key as K;
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let field = rig.leaf(col, "text-edit", &[("accessible-name", s("field"))]);
    let btn = rig.leaf(col, "button", &[("text", s("Plain"))]);
    rig.run();
    rig.type_into(field, "abc");
    let ctx = rig.harness.ctx.clone();
    let none = egui::Modifiers::NONE;
    let cmd = egui::Modifiers { mac_cmd: true, command: true, ..Default::default() };
    // (layout, logical, physical, modifiers, claimed by the focused field)
    let matrix: &[(&str, K, Option<K>, egui::Modifiers, bool)] = &[
        // Latin (AZERTY, QWERTZ): the letter typed is the key, never its physical neighbour
        ("azerty Ctrl+Z at US W", K::Z, Some(K::W), CTRL, true),
        ("azerty Ctrl+W at US Z", K::W, Some(K::Z), CTRL, false),
        ("qwertz Ctrl+Z at US Y", K::Z, Some(K::Y), CTRL, true),
        ("qwerty Ctrl+S", K::S, Some(K::S), CTRL, false),
        ("azerty W at US Z", K::W, Some(K::Z), none, true),
        ("latin, no physical key", K::Z, None, CTRL, true),
        // symbol keys (Dvorak's `;` at US Z, AZERTY's `,` at US M): Ctrl falls back to the Latin key
        ("dvorak Ctrl+; at US Z", K::Semicolon, Some(K::Z), CTRL, true),
        ("dvorak Cmd+; at US Z", K::Semicolon, Some(K::Z), cmd, true),
        ("azerty Ctrl+, at US M", K::Comma, Some(K::M), CTRL, false),
        ("dvorak ; at US Z", K::Semicolon, Some(K::Z), none, true),
        ("symbol, no physical key", K::Semicolon, None, CTRL, false),
        // non-Latin (Russian): egui reports the physical key as the logical one
        ("ru Ctrl+я at US Z", K::Z, Some(K::Z), CTRL, true),
        ("ru Ctrl+ы at US S", K::S, Some(K::S), CTRL, false),
        ("ru а at US F", K::F, Some(K::F), none, true),
        ("ru Cmd+с at US C", K::C, Some(K::C), cmd, true),
    ];
    for &(name, logical, physical, m, want) in matrix {
        assert_eq!(rig.backend.shortcut_override_press(&ctx, logical, physical, m), want, "field: {name}");
    }
    // the same claim as `shortcut_override` for the typed key
    assert!(rig.backend.shortcut_override(&ctx, &egui::KeyboardShortcut::new(CTRL, K::Z)));
    // a focused button claims none of these chords, fallback or not
    rig.click(btn);
    assert!(focused(&rig, "Plain"));
    for &(name, logical, physical, m, _) in matrix.iter().filter(|r| r.3 != none) {
        assert!(!rig.backend.shortcut_override_press(&ctx, logical, physical, m), "button: {name}");
    }
}

/// Edit commands on a focused provider kind with `text_input`: through `Widget::text_command` when
/// the kind takes them (its own undo), else as the input they stand for; a kind without
/// `text_input` gets none and the host runs its own command (its document's undo).
#[test]
fn ev3b_text_commands_reach_provider_kinds() {
    let (mut rig, km) = keymap_rig();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let qty = rig.create("t-field");
    rig.insert(col, qty);
    let ed = rig.create("t-editor");
    rig.insert(col, ed);
    rig.run();
    let ctx = rig.harness.ctx.clone();
    let z = egui::KeyboardShortcut::new(CTRL, egui::Key::Z);
    let qty_text = |rig: &Rig| rig.backend.with_scene(|s| s.n(qty).widget(|w: &TField| w.text.clone())).unwrap();

    // a `TextEdit`-backed kind (quantity-field): Undo is fed as Ctrl+Z to its field, not the host's
    rig.type_into(qty, "12");
    assert_eq!(qty_text(&rig), "12");
    assert!(rig.backend.shortcut_override(&ctx, &z), "the keymap leaves Ctrl+Z to the field");
    assert!(rig.backend.text_command(&ctx, TextCommand::Undo), "the field takes Undo: no document undo");
    rig.run();
    assert_ne!(qty_text(&rig), "12", "undone by egui's undoer");
    assert!(km.borrow().seen_after.is_empty(), "no Ctrl+Z reached the host keymap");

    // a kind with its own undo: the commands go to `Widget::text_command`, not as keys
    rig.click(ed);
    assert!(rig.backend.focus_info(&ctx).kind == Some("t-editor"));
    rig.harness.event(egui::Event::Text("ab".into()));
    rig.run();
    assert_eq!(editor(&rig, ed, |e| e.text.clone()), "ab");
    assert!(rig.backend.text_command(&ctx, TextCommand::Undo));
    rig.run();
    assert_eq!(editor(&rig, ed, |e| (e.text.clone(), e.handled.clone(), e.keys)), (String::new(), vec![TextCommand::Undo], (0, 0)));
    assert!(rig.backend.text_command(&ctx, TextCommand::Insert("xy".into())));
    rig.run();
    assert!(rig.backend.text_command(&ctx, TextCommand::Copy));
    rig.run();
    // Delete is not the kind's: fed as the key
    assert!(rig.backend.text_command(&ctx, TextCommand::Delete));
    rig.run();
    // Paste without the host's clipboard (`EguiBackend` alone) comes as Paste
    assert!(rig.backend.text_command(&ctx, TextCommand::Paste));
    rig.run();
    let (text, handled, keys) = editor(&rig, ed, |e| (e.text.clone(), e.handled.clone(), e.keys));
    assert_eq!(text, "xy");
    assert_eq!(handled, vec![TextCommand::Undo, TextCommand::Insert("xy".into()), TextCommand::Copy, TextCommand::Delete, TextCommand::Paste]);
    assert_eq!(keys, (0, 1), "no Ctrl+Z fed; the declined Delete fed as its key");

    // nothing focused (a click on a Rust menu): the last text field is the kind; it gets focus back
    ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
    rig.run();
    assert_eq!(rig.backend.last_text_field(&ctx).and_then(|i| i.kind), Some("t-editor"));
    assert!(rig.backend.text_command(&ctx, TextCommand::Undo));
    rig.run();
    assert_eq!(editor(&rig, ed, |e| e.text.clone()), "");
    assert!(rig.backend.focus_info(&ctx).kind == Some("t-editor"), "focused again");

    // the same kind without `text_input`: no edit commands, the host keeps Ctrl+Z
    rig.set(ed, "editable", Value::Bool(false));
    rig.run();
    rig.run();
    let before = editor(&rig, ed, |e| e.handled.len());
    assert!(!rig.backend.text_command(&ctx, TextCommand::Undo), "falls back to the host's command");
    assert!(!rig.backend.shortcut_override(&ctx, &z));
    rig.run();
    assert_eq!(editor(&rig, ed, |e| e.handled.len()), before);
}

/// Through the embed: `last_text_field` carries the root, and ends when the dialog holding the
/// field goes away; `shortcut_override_press` asks the focused field.
#[test]
fn ev3b_embed_hooks() {
    use svelters_egui::harness::TestApp;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/src/FocusDialog.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    let ctx = app.harness.ctx.clone();
    assert_eq!(app.embed.last_text_field(&ctx), None);
    app.harness.get_by_label("Open").click();
    app.run();
    app.run();
    app.harness.event(egui::Event::Text("Ada".into()));
    app.run();
    let info = app.embed.last_text_field(&ctx).expect("the dialog's field");
    assert!(info.kind == Some("text-edit") && info.text_input, "{info:?}"); // a window: no embed root
    assert_eq!(info, app.embed.focus_info(&ctx));
    assert!(app.embed.shortcut_override_press(&ctx, egui::Key::Semicolon, Some(egui::Key::Z), CTRL));
    assert!(!app.embed.shortcut_override_press(&ctx, egui::Key::W, Some(egui::Key::Z), CTRL));

    // host chrome took focus: still the field (the Edit menu stays enabled)
    ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap()));
    app.run();
    assert_eq!(app.embed.last_text_field(&ctx).map(|i| i.root), Some(info.root));
    // Escape (nothing focused: the topmost dialog's cancel button) closes the dialog: no field
    app.harness.key_press(egui::Key::Escape);
    app.run();
    app.run();
    assert!(app.harness.query_by_label("Result: cancel").is_some(), "{:?}", app.errors());
    assert_eq!(app.embed.last_text_field(&ctx), None, "the field went with the dialog");
    assert!(!app.embed.text_command(&ctx, TextCommand::Paste));
}
