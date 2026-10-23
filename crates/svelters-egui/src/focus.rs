//! EV3+: keys and focus around the walk (plans/milestones/EV3-focus-keys.md; QT-SURVEY §2.3, §2.4,
//! §3 row 1; ARCH-EMBED R11).
//!
//! - **Focus scopes** (`focus-scope` on a `box`): Tab and Shift+Tab cycle inside a scope while
//!   focus is in it (QML `FocusScope`, a dialog's tab chain). A `dialog` scope also takes focus
//!   when it appears (its `autofocus` node, else its first focusable one) and gives it back to the
//!   widget that had it (the opener) when it goes away: hidden, removed, its slot hidden or its
//!   subtree inert (an outro). `autofocus` focuses a node when it first appears, scope or not.
//!   WG2a's `modal`, `inner-window` and `popup` are dialog scopes. When an egui popup (a menu,
//!   svelters' or the host's) closes and nothing has focus, focus goes back to where it was.
//! - **Key claims** (R11 `consume_key`, `composing()`): the focused field claims the keys it
//!   handles (text and editing keys, Ctrl+Z, Del, arrows); after it ran they are taken out of the
//!   pass's input, so code later in the pass never sees them, and [`EguiBackend::shortcut_override`]
//!   answers for a host keymap that runs before (Qt's `ShortcutOverride`). While an input method
//!   composes, every key is the field's.
//! - **Edit commands** ([`TextCommand`], [`EguiBackend::text_command`]): Undo, Redo, Cut, Copy,
//!   Paste, Select All and Delete sent to the focused text field, or to the last one when a click
//!   on host chrome (a Rust menu) took focus to nowhere. Through [`crate::Embed::text_command`] they
//!   use the host's clipboard (DX2, `Host::platform()`): Paste reads its text, Cut and Copy write
//!   to it and to egui's (`OutputCommand::CopyText`); through the backend alone, egui's (Paste:
//!   `ViewportCommand::RequestPaste`). [`TextCommand::Insert`] takes the text from the host.
//!   EV3b: a provider kind with `Widget::text_input` gets them through `Widget::text_command`
//!   first (its own undo); [`EguiBackend::last_text_field`] is the field they go to (the Edit
//!   menu's enabled state), also while a menu item has focus; [`EguiBackend::shortcut_override_press`]
//!   asks for a claim with the logical and the physical key (cadatomic KL1's Latin fallback).
//! - **Default and cancel buttons** (`default`, `cancel` on `button`): Enter activates the default
//!   button of the focused node's scope (its root when it has none), Escape the cancel button,
//!   except while composing, while a popup is open, or when the focused node takes the key itself
//!   (a button, a multi-line field, a provider kind that claims it).
//!
//! The walk records what it painted ([`record`], [`record_scope`]) per viewport and pass in egui's
//! temp data, keyed by egui ids (several embeds can share a context); [`begin`] runs once per pass
//! and viewport before the first svelters node is painted and acts on the last pass's record.
//! Disabled and inert (TR2b) subtrees are not recorded: they are out of every tab chain.

use crate::backend::EguiBackend;
use crate::scene::{Arena, NodeId, W};
use egui::{Context, Event, FocusDirection, Id, Key, KeyboardShortcut, Modifiers, Ui};
use std::sync::Arc;
use svelters_host::platform::{ClipboardBackend, ClipboardKind, MimeData, TEXT_PLAIN};

/// An edit command for the focused text field (Qt's Edit menu reaching the focused `QLineEdit`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    /// Pastes the clipboard's text (the integration reads it: `ViewportCommand::RequestPaste`).
    Paste,
    /// Pastes `text` from the host (what a multi-format clipboard chose).
    Insert(String),
    SelectAll,
    /// Deletes the selection, or the character after the caret.
    Delete,
}

/// What has keyboard focus, for a host keymap (cadatomic's `TextFocus` layer).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FocusInfo {
    /// The focused egui widget, when it is a svelters node's.
    pub widget: Option<Id>,
    /// Its scene node (in the backend asked).
    pub node: Option<NodeId>,
    /// Its tag (`text-edit`, `button`, a provider kind's).
    pub kind: Option<&'static str>,
    /// A text field has focus: plain keys and the editing chords are its.
    pub text_input: bool,
    pub multiline: bool,
    /// An input method is composing in it: every key is its.
    pub composing: bool,
    /// The focus scope it is in (the scope node's egui id), if any.
    pub scope: Option<Id>,
    /// Its root ([`crate::Embed::focus_info`] only).
    pub root: Option<crate::embed::RootId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Text { multiline: bool },
    Button { default: bool, cancel: bool },
    Toggle,
    /// The keys a slider claims (arrows, Home/End, PageUp/PageDown): OW2's sashes
    /// (`WidgetCx::record_part_focus`). OW1b's own sliders claim theirs through `consume_key`.
    Slider,
    /// LS1: a selectable `virtual-list` row (the list's tab stop is its selected row).
    List,
    /// WG5b: a `tree-view` (one tab stop, the list; its claims are `kinds::treeview::claims`).
    Tree,
    /// A provider kind (its claims are asked through `Widget::consume_key`).
    Ext { enter: bool, escape: bool },
    Other,
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    /// The widget's egui id (focus is on it).
    id: Id,
    /// `None`: a widget drawn without a scene node (OW-RUST's `imm`, host chrome).
    node: Option<NodeId>,
    /// The node's own egui id (a scene `Node.id`; the widget's id for `imm`).
    node_eid: Id,
    /// The nearest focus scope's id, else the root node's.
    scope: Id,
    scoped: bool,
    role: Role,
    autofocus: bool,
    /// In the tab chain (a list is one stop however many rows it shows).
    tab_stop: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Scope {
    id: Id,
    dialog: bool,
}

#[derive(Clone, Debug, Default)]
struct Pass {
    entries: Vec<Entry>,
    scopes: Vec<Scope>,
    focused: Option<Id>,
    /// EV3b: an own menu or select list (OW3's `Grab`) held the keyboard in this pass.
    grab: bool,
}

impl Pass {
    fn entry(&self, id: Option<Id>) -> Option<&Entry> {
        let id = id?;
        self.entries.iter().find(|e| e.id == id)
    }
    fn has_scope(&self, id: Id) -> bool {
        self.scopes.iter().any(|s| s.id == id)
    }
    fn top_dialog(&self) -> Option<Id> {
        self.scopes.iter().rev().find(|s| s.dialog).map(|s| s.id)
    }
}

/// Per viewport, in egui's temp data.
#[derive(Clone, Debug, Default)]
struct State {
    /// The pass `cur` records.
    pass: u64,
    /// The pass [`begin`] last ran in.
    routed: u64,
    cur: Pass,
    prev: Pass,
    /// The scopes of the pass before `prev`.
    older: Vec<Scope>,
    /// Open dialog scopes and the widget focused before each took focus.
    openers: Vec<(Id, Option<Id>)>,
    /// `autofocus` nodes already focused once (while they stay painted).
    autofocused: Vec<Id>,
    /// A default or cancel button to activate at its next paint (its node id).
    activate: Option<Id>,
    /// CX3: a one-line text field (its widget id) whose Enter went to the default button: it still
    /// commits at its next paint.
    entered: Option<Id>,
    /// An edit command for a text field (its id) at its next paint, with the host's clipboard.
    cmd: Option<(Id, TextCommand, Option<Clip>)>,
    /// A Cut or Copy fed to field `id`: what it copies goes to this clipboard after it ran.
    capture: Option<(Id, Clip)>,
    /// The field an input method is composing in.
    composing: Option<Id>,
    /// The last text field that had focus.
    last_text: Option<Id>,
    /// The focus when the last pass began, and the focus to give back when the open egui popup
    /// (a menu) closes.
    last_focus: Option<Id>,
    popup: Option<Option<Id>>,
    /// A popup closed at the end of the last pass: the focus to give back.
    popup_closed: Option<Option<Id>>,
    /// The keys the focused node claimed in pass `consumed_pass`.
    consumed: Vec<KeyboardShortcut>,
    consumed_pass: u64,
}

impl State {
    fn rotate(&mut self, pass: u64) {
        if self.pass != pass {
            self.older = std::mem::take(&mut self.prev.scopes);
            self.prev = std::mem::take(&mut self.cur);
            self.pass = pass;
        }
    }
}

/// DX2's clipboard (`Host::platform().clipboard`), carried with a pending command.
#[derive(Clone)]
pub(crate) struct Clip(pub(crate) Arc<dyn ClipboardBackend>);

impl std::fmt::Debug for Clip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clip")
    }
}

fn state_id(ctx: &Context) -> Id {
    Id::unique("svelters-egui/focus").with(ctx.viewport_id())
}

fn with<R>(ctx: &Context, f: impl FnOnce(&mut State) -> R) -> R {
    let id = state_id(ctx);
    ctx.data_mut(|d| f(d.get_temp_mut_or_default::<State>(id)))
}

fn read<R>(ctx: &Context, f: impl FnOnce(&State) -> R) -> R {
    let id = state_id(ctx);
    ctx.data(|d| f(&d.get_temp::<State>(id).unwrap_or_default()))
}

fn shortcut(key: Key, modifiers: Modifiers) -> KeyboardShortcut {
    KeyboardShortcut::new(modifiers, key)
}

/// Whether a node in `role` that has focus takes `key` with `m` (R11 `consume_key`).
pub(crate) fn claims(role: Role, key: Key, m: Modifiers, mac: bool) -> bool {
    let command = m.command || m.ctrl || m.mac_cmd;
    let nav = matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown);
    match role {
        Role::Text { multiline } => {
            if !command && !m.alt {
                // typing and editing keys; Tab moves focus, Escape and F-keys stay the app's, and
                // Enter in a one-line field goes to the default button
                return match key {
                    Key::Tab | Key::Escape => false,
                    Key::Enter => multiline,
                    Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown => multiline,
                    k => !is_function_key(k),
                };
            }
            if command && !m.alt {
                // undo, redo, clipboard, select all, word keys (Ctrl+Insert, Shift+Insert)
                return matches!(
                    key,
                    Key::A | Key::C | Key::V | Key::X | Key::Y | Key::Z | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End | Key::Backspace | Key::Delete | Key::Insert
                ) || (multiline && matches!(key, Key::ArrowUp | Key::ArrowDown));
            }
            // macOS moves and deletes by word with Option
            mac && m.alt && !command && matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::Backspace | Key::Delete)
        }
        Role::Slider => !command && !m.alt && nav,
        Role::List => !command && !m.alt && (nav || (!m.shift && matches!(key, Key::Space | Key::Enter))),
        Role::Tree => crate::kinds::treeview::claims(key, m),
        Role::Button { .. } | Role::Toggle => !command && !m.alt && !m.shift && matches!(key, Key::Space | Key::Enter),
        Role::Ext { .. } | Role::Other => false,
    }
}

fn is_function_key(k: Key) -> bool {
    let n = k.name();
    n.len() >= 2 && n.starts_with('F') && n[1..].chars().all(|c| c.is_ascii_digit())
}

fn is(key: Key, m: Modifiers, want: Key) -> bool {
    key == want && !m.any()
}

/// What [`route`] decided for this pass.
#[derive(Default, Debug)]
struct Plan {
    focus: Option<Id>,
    cancel_tab: bool,
    eat: Vec<Key>,
    repaint: bool,
}

/// The decisions of one pass from the last pass's record: scopes appearing and going away, Tab in
/// a scope, Enter and Escape to the default and cancel buttons. `focus` is egui's focus now (after
/// its own handling of this pass's keys).
fn route(st: &mut State, focus: Option<Id>, keys: &[(Key, Modifiers)], ime: &[bool]) -> Plan {
    // egui knows a popup is open only once it is shown in the pass: the end-of-pass hook tracks it
    let popup_open = st.popup.is_some();
    let mut plan = Plan::default();
    // the input method: composing from the first non-empty preedit until a commit or an empty one
    for &on in ime {
        st.composing = if on { focus } else { None };
    }
    if st.composing.is_some() && st.composing != focus {
        st.composing = None;
    }
    let composing = st.composing.is_some();
    let escape = keys.iter().any(|&(k, m)| is(k, m, Key::Escape));
    // egui drops focus on a plain Escape before anything runs (FX1 gives it back later)
    let f = if focus.is_none() && escape { st.prev.focused } else { focus };
    // EV3b: the last text field (`last_text_field`, `text_command`'s fallback). Another widget
    // taking focus ends it, unless a menu is open (an egui popup, or an own menu or list holding
    // the keyboard): a menu item, or the menu itself, has keyboard focus while the Edit menu is
    // used. A field not recorded in the last pass is removed, hidden, disabled or inert: it ends
    // too.
    match st.prev.entry(f) {
        Some(e) if matches!(e.role, Role::Text { .. }) => st.last_text = Some(e.id),
        Some(_) => st.last_text = None,
        None if f.is_some() && f != st.last_text && st.popup.is_none() && !st.prev.grab => st.last_text = None,
        None => {}
    }
    if st.last_text.is_some() && st.prev.entry(st.last_text).is_none() {
        st.last_text = None;
    }

    // an egui popup (a menu, svelters' or the host's) gives focus back when it closes: menus take
    // focus for their arrow keys and drop it when they go
    let before = std::mem::replace(&mut st.last_focus, focus);
    if let Some(opener) = st.popup_closed.take()
        && focus.is_none()
    {
        plan.focus = opener;
    }

    // dialogs that went away give focus back to their opener
    let prev = &st.prev;
    let mut gone = Vec::new();
    st.openers.retain(|&(s, opener)| {
        let keep = prev.has_scope(s);
        if !keep {
            gone.push(opener);
        }
        keep
    });
    if focus.is_none()
        && let Some(opener) = gone.into_iter().flatten().next_back()
    {
        plan.focus = Some(opener);
    }
    // dialogs that appeared take focus
    for s in prev.scopes.iter().filter(|s| s.dialog && !st.older.iter().any(|o| o.id == s.id)) {
        if st.openers.iter().any(|o| o.0 == s.id) {
            continue;
        }
        let inside = prev.entry(focus).is_some_and(|e| e.scope == s.id);
        // egui's `Modal` drops the focus of the layers below it as it opens: then the opener is
        // what had focus when the pass that showed the dialog began
        let opener = if inside { None } else { focus.or(plan.focus).or(before) };
        st.openers.push((s.id, opener));
        if !inside {
            let mut members = prev.entries.iter().filter(|e| e.scoped && e.tab_stop && e.scope == s.id);
            let target = members.clone().find(|e| e.autofocus).or_else(|| members.next());
            if let Some(t) = target {
                plan.focus = Some(t.id);
                if t.autofocus {
                    st.autofocused.push(t.node_eid);
                }
            }
        }
    }
    // `autofocus` outside a dialog: once, when the node appears
    st.autofocused.retain(|n| prev.entries.iter().any(|e| e.node_eid == *n));
    if let Some(e) = prev.entries.iter().find(|e| e.autofocus && !st.autofocused.contains(&e.node_eid)) {
        st.autofocused.push(e.node_eid);
        if plan.focus.is_none() {
            plan.focus = Some(e.id);
        }
    }

    if composing {
        // the input method's: no default or cancel button, no traversal, and the field must not
        // take Enter as "done" (egui ends a one-line field's input on it)
        plan.eat.extend(keys.iter().filter(|&&(k, m)| !m.any() && matches!(k, Key::Enter | Key::Escape | Key::Tab)).map(|&(k, _)| k));
        if escape && focus.is_none() {
            plan.focus = f;
        }
        return plan;
    }
    let focused = prev.entry(f);
    // the scope keys act on: the focused node's, else (nothing focused) the topmost dialog's
    let scope_of = |explicit: bool| -> Option<Id> {
        match focused {
            Some(e) if e.scoped || !explicit => Some(e.scope),
            Some(_) => None,
            None if f.is_none() => prev.top_dialog(),
            None => None, // a host widget has focus
        }
    };
    for &(key, m) in keys {
        if key == Key::Tab && (!m.any() || m.shift_only()) && !plan.cancel_tab {
            let Some(s) = scope_of(true) else { continue };
            let chain: Vec<Id> = prev.entries.iter().filter(|e| e.scoped && e.tab_stop && e.scope == s).map(|e| e.id).collect();
            if chain.is_empty() {
                continue;
            }
            let n = chain.len();
            let at = focused.and_then(|e| chain.iter().position(|&c| c == e.id));
            let next = match (at, m.shift) {
                (Some(i), false) => (i + 1) % n,
                (Some(i), true) => (i + n - 1) % n,
                (None, false) => 0,
                (None, true) => n - 1,
            };
            plan.focus = Some(chain[next]);
            plan.cancel_tab = true;
        } else if is(key, m, Key::Enter) || is(key, m, Key::Escape) {
            let enter = key == Key::Enter;
            if popup_open || st.activate.is_some() {
                continue;
            }
            let takes = focused.is_some_and(|e| match e.role {
                Role::Ext { enter: en, escape: es } => if enter { en } else { es },
                r => claims(r, key, m, false),
            });
            if takes {
                continue;
            }
            let Some(s) = scope_of(false) else { continue };
            let button = prev.entries.iter().find(|e| {
                e.scope == s && matches!(e.role, Role::Button { default, cancel } if if enter { default } else { cancel })
            });
            if let Some(b) = button {
                st.activate = Some(b.node_eid);
                if let Some(e) = focused.filter(|e| enter && matches!(e.role, Role::Text { multiline: false })) {
                    st.entered = Some(e.id);
                }
                plan.eat.push(key);
                plan.repaint = true;
                if focus.is_none() && plan.focus.is_none() {
                    plan.focus = f; // egui dropped focus on Escape; the key was the button's
                }
            }
        }
    }
    plan
}

/// Once per pass and viewport, before the first svelters node is painted: acts on the last pass's
/// record (see the module docs).
pub(crate) fn begin(ctx: &Context) {
    let pass = ctx.cumulative_pass_nr();
    if read(ctx, |st| st.routed == pass) {
        return;
    }
    install(ctx);
    let (keys, ime) = ctx.input(|i| {
        let mut keys = Vec::new();
        let mut ime = Vec::new();
        for e in &i.events {
            match e {
                Event::Key { key, pressed: true, modifiers, .. } => keys.push((*key, *modifiers)),
                Event::Ime(egui::ImeEvent::Preedit { text, .. }) => ime.push(!text.is_empty()),
                Event::Ime(egui::ImeEvent::Commit(_)) => ime.push(false),
                _ => {}
            }
        }
        (keys, ime)
    });
    let focus = ctx.memory(|m| m.focused());
    let plan = with(ctx, |st| {
        st.routed = pass;
        st.rotate(pass);
        route(st, focus, &keys, &ime)
    });
    if plan.cancel_tab {
        ctx.memory_mut(|m| m.move_focus(FocusDirection::None));
    }
    if let Some(id) = plan.focus {
        ctx.memory_mut(|m| m.request_focus(id));
        ctx.request_repaint();
    }
    if !plan.eat.is_empty() {
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { key, pressed: true, modifiers, .. } if !modifiers.any() && plan.eat.contains(key))));
    }
    if plan.repaint {
        ctx.request_repaint();
    }
}

/// Once per context: at the end of a pass that showed or hid a dialog scope (or showed an
/// `autofocus` node), asks for the pass that moves focus ([`begin`] acts on the last pass's record;
/// otherwise an idle app would wait for the next input).
fn install(ctx: &Context) {
    let key = Id::unique("svelters-egui/focus/installed");
    if ctx.data(|d| d.get_temp::<bool>(key).is_some()) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, true));
    ctx.on_end_pass(
        "svelters-focus",
        std::sync::Arc::new(|ui: &mut Ui| {
            let ctx = ui.ctx();
            let pass = ctx.cumulative_pass_nr();
            // a popup (a menu) opened in this pass: note what had focus before; closed: give it
            // back at the next pass
            let popup_open = ctx.any_popup_open();
            let focus = ctx.memory(|m| m.focused());
            let closed = with(ctx, |st| {
                if st.pass != pass {
                    return false;
                }
                match (popup_open, st.popup) {
                    (true, None) => st.popup = Some(focus.or(st.last_focus)),
                    (false, Some(opener)) => {
                        st.popup = None;
                        st.popup_closed = Some(opener);
                        return true;
                    }
                    _ => {}
                }
                false
            });
            if closed {
                ctx.request_repaint();
            }
            let due = read(ctx, |st| {
                if st.pass != pass {
                    return false;
                }
                let dialogs = |p: &Pass| p.scopes.iter().filter(|s| s.dialog).map(|s| s.id).collect::<Vec<_>>();
                dialogs(&st.cur) != dialogs(&st.prev)
                    || st.openers.iter().any(|o| !st.cur.has_scope(o.0))
                    || st.cur.entries.iter().any(|e| e.autofocus && !st.autofocused.contains(&e.node_eid))
            });
            if due {
                ctx.request_repaint();
            }
        }),
    );
}

/// Whether node `i` opens a focus scope, and whether it is a dialog: a `box` by its `focus-scope`;
/// WG2a's overlays (`modal`, `inner-window`, `popup`) are dialogs unless they say `contain`.
fn scope_kind(arena: &Arena, i: usize) -> Option<bool> {
    let s = arena.common(i).focus_scope;
    if matches!(arena.w(i), W::Overlay(_)) {
        return Some(s != "contain");
    }
    (s != "none").then_some(s == "dialog")
}

/// The nearest focus scope of node `i`, else its root's id (and false).
fn scope_of(arena: &Arena, i: usize) -> (Id, bool) {
    let mut at = arena.core[i].parent;
    let mut top = arena.core[i].egui_id;
    while let Some(p) = at {
        let Some(pi) = arena.idx(p) else { break };
        if scope_kind(arena, pi).is_some() {
            return (arena.core[pi].egui_id, true);
        }
        top = arena.core[pi].egui_id;
        at = arena.core[pi].parent;
    }
    (top, false)
}

/// Before a container's (or an open overlay's) children are painted: notes the focus scope it
/// opens (unless disabled or inert).
pub(crate) fn record_scope(arena: &Arena, i: usize, ui: &Ui) {
    let Some(dialog) = scope_kind(arena, i) else { return };
    if !ui.is_enabled() {
        return;
    }
    let scope = Scope { id: arena.core[i].egui_id, dialog };
    let pass = ui.ctx().cumulative_pass_nr();
    with(ui.ctx(), |st| {
        st.rotate(pass);
        st.cur.scopes.push(scope);
    });
}

/// After leaf `i` ran in `ui` with response `resp`: records it as focusable (in tab order) and,
/// when it has focus, takes the keys it claims out of the pass's input (R11 `consume_key`).
pub(crate) fn record(arena: &Arena, i: usize, ui: &Ui, resp: &egui::Response) {
    if !ui.is_enabled() || !resp.sense.is_focusable() {
        return;
    }
    let c = arena.common(i);
    let role = match arena.w(i) {
        W::Button { .. } => Role::Button { default: c.default_button, cancel: c.cancel_button },
        // OW0: an own widget replacing `button` keeps its EV3 role (default and cancel keys)
        W::Ext(x) if x.entry().replaces == Some("button") => Role::Button { default: c.default_button, cancel: c.cancel_button },
        W::Ext(x) => {
            let w = x.0.try_borrow();
            // RA3: a provider text input (`Widget::text_input`) is a text field for EV3
            match w.as_ref().ok().and_then(|w| w.text_input()) {
                Some(multiline) => Role::Text { multiline },
                None => {
                    let claim = |k| w.as_ref().is_ok_and(|w| w.consume_key(k, Modifiers::NONE));
                    Role::Ext { enter: claim(Key::Enter), escape: claim(Key::Escape) }
                }
            }
        }
        // check boxes, toggles, switches, radio and menu buttons, combo boxes: Space and Enter
        _ if resp.sense.senses_click() => Role::Toggle,
        _ => Role::Other,
    };
    record_with(arena, i, ui, resp, role, true);
}

/// [`record`] with the role given: LS1's `virtual-list` records each selectable row it draws
/// (`node` is the list), one of them as the tab stop.
pub(crate) fn record_with(arena: &Arena, i: usize, ui: &Ui, resp: &egui::Response, role: Role, tab_stop: bool) {
    if !ui.is_enabled() || !resp.sense.is_focusable() {
        return;
    }
    let c = arena.common(i);
    let (scope, scoped) = scope_of(arena, i);
    let entry = Entry {
        id: resp.id,
        node: Some(arena.slots.id_at(i).expect("live leaf")),
        node_eid: arena.core[i].egui_id,
        scope,
        scoped,
        role,
        autofocus: c.autofocus && tab_stop,
        tab_stop,
    };
    let ext = match arena.w(i) {
        W::Ext(x) => Some(x),
        _ => None,
    };
    push_entry(ui, resp, entry, ext);
}

/// The focus scope of widgets drawn without a scene node (host chrome): never a dialog, and no
/// default or cancel button is in it.
fn foreign_scope() -> Id {
    Id::unique("svelters-egui/imm/chrome")
}

/// OW-RUST: [`record`] for a widget drawn without a scene node (`crate::imm`): in the tab order
/// in paint order with the scene's widgets (egui's focus traversal), outside every focus scope,
/// and, while focused, its `role`'s keys (Space and Enter for buttons and toggles) taken out of
/// the pass's input (R11), as a scene widget's.
pub(crate) fn record_foreign(ui: &Ui, resp: &egui::Response, role: Role) {
    if !ui.is_enabled() || !resp.sense.is_focusable() {
        return;
    }
    let entry = Entry { id: resp.id, node: None, node_eid: resp.id, scope: foreign_scope(), scoped: false, role, autofocus: false, tab_stop: true };
    push_entry(ui, resp, entry, None);
}

/// Records `entry` for this pass and, when it has focus, takes the keys it claims (`ext`: a
/// provider kind's `consume_key`, else its role's) out of the pass's input.
fn push_entry(ui: &Ui, resp: &egui::Response, entry: Entry, ext: Option<&crate::provider::ExtWidget>) {
    let ctx = ui.ctx();
    let role = entry.role;
    let focused = resp.has_focus();
    let pass = ctx.cumulative_pass_nr();
    let composing = with(ctx, |st| {
        st.rotate(pass);
        st.cur.entries.push(entry);
        if focused {
            st.cur.focused = Some(resp.id);
        }
        st.composing == Some(resp.id)
    });
    if !focused {
        return;
    }
    // R11: the focused node's keys are its; nothing later in the pass sees them
    let mac = ctx.os() == egui::os::OperatingSystem::Mac;
    let composing = composing || ext.is_some_and(|x| x.0.try_borrow().is_ok_and(|w| w.composing()));
    let taken: Vec<KeyboardShortcut> = ctx.input_mut(|inp| {
        let mut taken = Vec::new();
        inp.events.retain(|e| match e {
            Event::Key { key, pressed, modifiers, .. } => {
                let mine = composing
                    || match ext {
                        Some(x) => x.0.try_borrow().is_ok_and(|w| w.consume_key(*key, *modifiers)),
                        None => claims(role, *key, *modifiers, mac),
                    };
                if mine && *pressed {
                    taken.push(shortcut(*key, *modifiers));
                }
                !mine
            }
            _ => true,
        });
        taken
    });
    if !taken.is_empty() {
        with(ctx, |st| {
            if st.consumed_pass != pass {
                st.consumed.clear();
                st.consumed_pass = pass;
            }
            st.consumed.extend(taken);
        });
    }
}

/// Before text field `id` runs: feeds it a pending [`TextCommand`] as the input it stands for.
pub(crate) fn before_text(ui: &Ui, id: Id) {
    if let Some(cmd) = take_command(ui.ctx(), id) {
        feed(ui.ctx(), cmd);
    }
}

/// The pending [`TextCommand`] for text field `id`, if any, taken. With the host's clipboard, a
/// Paste comes back as the `Insert` of its text (nothing when it holds none), and a Cut or Copy
/// arranges for what the field copies (egui's `CopyText`) to reach it ([`after_text`]).
pub(crate) fn take_command(ctx: &Context, id: Id) -> Option<TextCommand> {
    let (cmd, clip) = with(ctx, |st| if st.cmd.as_ref().is_some_and(|c| c.0 == id) { st.cmd.take().map(|c| (c.1, c.2)) } else { None })?;
    match (cmd, clip) {
        // DX2: the host's clipboard, the one `svelters:clipboard` reads
        (TextCommand::Paste, Some(c)) => match c.0.read(ClipboardKind::Clipboard, &[TEXT_PLAIN]) {
            Ok(data) => data.plain_text().filter(|t| !t.is_empty()).map(|t| TextCommand::Insert(t.to_string())),
            Err(_) => None,
        },
        (cmd @ (TextCommand::Cut | TextCommand::Copy), Some(c)) => {
            // egui's own copy (`OutputCommand::CopyText`, the integration's clipboard) stays; what
            // the field copies also goes to the host's clipboard after it ran ([`after_text`])
            with(ctx, |st| st.capture = Some((id, c)));
            Some(cmd)
        }
        (cmd, _) => Some(cmd),
    }
}

/// Feeds `cmd` to the text field about to run as the input it stands for, so egui's undoer and
/// FX1's policy apply unchanged. Paste without the host's clipboard asks the integration
/// (`ViewportCommand::RequestPaste`).
pub(crate) fn feed(ctx: &Context, cmd: TextCommand) {
    let mac = ctx.os() == egui::os::OperatingSystem::Mac;
    // the modifiers the integration reports for the platform's command key
    let cmd_mods = if mac { Modifiers::MAC_CMD | Modifiers::COMMAND } else { Modifiers::CTRL | Modifiers::COMMAND };
    let key = |key: Key, modifiers: Modifiers| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
    let event = match cmd {
        TextCommand::Undo => key(Key::Z, cmd_mods),
        TextCommand::Redo => key(Key::Z, cmd_mods | Modifiers::SHIFT),
        TextCommand::SelectAll => key(Key::A, cmd_mods),
        TextCommand::Delete => key(Key::Delete, Modifiers::NONE),
        TextCommand::Cut => Event::Cut,
        TextCommand::Copy => Event::Copy,
        TextCommand::Insert(text) => Event::Paste(text),
        TextCommand::Paste => {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            return;
        }
    };
    ctx.input_mut(|i| i.events.push(event));
}

/// After text field `id` ran: a Cut or Copy fed to it by a [`TextCommand`] writes what it copied
/// (egui's last `CopyText` of the pass) to the host's clipboard too.
pub(crate) fn after_text(ui: &Ui, id: Id) {
    let ctx = ui.ctx();
    let Some(clip) = with(ctx, |st| if st.capture.as_ref().is_some_and(|c| c.0 == id) { st.capture.take().map(|c| c.1) } else { None }) else {
        return;
    };
    let copied = ctx.output(|o| {
        o.commands.iter().rev().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
    });
    if let Some(t) = copied {
        let _ = clip.0.write(ClipboardKind::Clipboard, MimeData::text(t));
    }
}

/// For a default or cancel button with node id `node_eid`: whether Enter or Escape activated it.
pub(crate) fn take_activation(ui: &Ui, node_eid: Id) -> bool {
    with(ui.ctx(), |st| {
        let hit = st.activate == Some(node_eid);
        if hit {
            st.activate = None;
        }
        hit
    })
}

/// CX3/OW4: for a one-line text field with widget id `id`: whether its Enter went to the default
/// button this pass (the field still commits and submits: `widgets::text_edit`).
pub(crate) fn take_entered(ui: &Ui, id: Id) -> bool {
    with(ui.ctx(), |st| {
        let hit = st.entered == Some(id);
        if hit {
            st.entered = None;
        }
        hit
    })
}

/// The keys the focused node claimed in the current pass of `ctx`'s viewport so far (R11
/// `consume_key`): a host keymap that runs after the svelters content skips them.
pub fn consumed_keys(ctx: &Context) -> Vec<KeyboardShortcut> {
    let pass = ctx.cumulative_pass_nr();
    read(ctx, |st| if st.consumed_pass == pass { st.consumed.clone() } else { Vec::new() })
}

/// OW3: the keyboard focus when this pass began (before a press took it from a text field): what
/// an own menu or list gives focus back to when it closes.
pub(crate) fn pass_start_focus(ctx: &Context) -> Option<Id> {
    read(ctx, |st| st.last_focus)
}

/// Whether an input method is composing in a svelters text field of `ctx`'s viewport (R11
/// `composing()`): no Enter or Escape to default or cancel buttons, every key is the field's.
pub fn composing(ctx: &Context) -> bool {
    read(ctx, |st| st.composing.is_some())
}

/// EV3b: an own menu, menu bar (F10) or select list holds the keyboard in this pass (OW3's
/// `Grab`): a menu is open for [`EguiBackend::last_text_field`], as an egui popup is.
pub(crate) fn note_grab(ctx: &Context) {
    let pass = ctx.cumulative_pass_nr();
    with(ctx, |st| {
        st.rotate(pass);
        st.cur.grab = true;
    });
}

/// OW-RUST: the focused widget when it is one drawn without a scene node (`imm`).
fn foreign_focused(ctx: &Context) -> Option<Entry> {
    let focus = ctx.memory(|m| m.focused())?;
    read(ctx, |st| st.cur.entry(Some(focus)).or_else(|| st.prev.entry(Some(focus))).filter(|e| e.node.is_none()).cloned())
}

/// OW2: the scene node holding keyboard focus in `ctx`'s viewport (from the latest record).
pub(crate) fn focused_node(ctx: &Context) -> Option<crate::scene::NodeId> {
    let focus = ctx.memory(|m| m.focused())?;
    read(ctx, |st| st.cur.entry(Some(focus)).or_else(|| st.prev.entry(Some(focus))).and_then(|e| e.node))
}

/// KL1's Latin fallback (cadatomic `keylayout`) for a press: with Ctrl or Cmd, the physical key
/// (US position) when the layout's logical key is a symbol (egui reports the physical key as the
/// logical one for a non-Latin letter, so they are equal then). A Latin letter or digit is what
/// the user typed and never turns into its physical neighbour.
pub(crate) fn latin_fallback(logical: Key, physical: Option<Key>, m: Modifiers) -> Option<Key> {
    let physical = physical?;
    let command = m.command || m.ctrl || m.mac_cmd;
    (command && physical != logical && !is_latin(logical)).then_some(physical)
}

/// A Latin letter or a digit (the keys KL1's typed-key rule keeps as typed).
fn is_latin(k: Key) -> bool {
    let n = k.name();
    n.len() == 1 && (n.as_bytes()[0].is_ascii_uppercase() || n.as_bytes()[0].is_ascii_digit())
}

impl EguiBackend {
    /// Whether `e` is (still) a node of this backend.
    fn ours(&self, e: &Entry) -> bool {
        let Ok(scene) = self.0.scene.try_borrow() else { return false };
        e.node.and_then(|n| scene.idx(n)).is_some_and(|i| scene.core[i].egui_id == e.node_eid)
    }

    /// The focused entry of this backend, from the latest record of `ctx`'s viewport.
    fn focused_entry(&self, ctx: &Context) -> Option<Entry> {
        let focus = ctx.memory(|m| m.focused())?;
        let e = read(ctx, |st| st.cur.entry(Some(focus)).or_else(|| st.prev.entry(Some(focus))).cloned())?;
        self.ours(&e).then_some(e)
    }

    /// The text field an edit command goes to: the focused one, else (nothing focused, or a menu
    /// item while a menu is open: a click or the keys on host chrome; an egui popup, or an own
    /// menu or list holding the keyboard) the last one that had focus. The flag says it is the
    /// last one (not focused).
    fn text_target(&self, ctx: &Context) -> Option<(Entry, bool)> {
        let focus = ctx.memory(|m| m.focused());
        if let Some(e) = self.focused_entry(ctx) {
            return matches!(e.role, Role::Text { .. }).then_some((e, false));
        }
        let (recorded, last, popup) = read(ctx, |st| {
            let recorded = st.cur.entry(focus).or_else(|| st.prev.entry(focus)).is_some();
            let last = st.last_text.and_then(|t| st.prev.entry(Some(t)).or_else(|| st.cur.entry(Some(t))).cloned());
            (recorded, last, st.popup.is_some() || st.cur.grab || st.prev.grab)
        });
        let menu = !recorded && (popup || ctx.any_popup_open());
        if focus.is_some() && !menu {
            return None; // another widget has focus
        }
        last.filter(|e| self.ours(e)).map(|e| (e, true))
    }

    fn info(&self, ctx: &Context, e: &Entry) -> FocusInfo {
        let kind = self.0.scene.try_borrow().ok().and_then(|s| {
            let i = s.idx(e.node?)?;
            crate::kinds::kind(s.core[i].kind as usize).tag
        });
        let (text_input, multiline) = match e.role {
            Role::Text { multiline } => (true, multiline),
            _ => (false, false),
        };
        FocusInfo {
            widget: Some(e.id),
            node: e.node,
            kind,
            text_input,
            multiline,
            composing: read(ctx, |st| st.composing == Some(e.id)),
            scope: e.scoped.then_some(e.scope),
            root: None,
        }
    }

    /// What has keyboard focus in `ctx`'s viewport, if it is one of this backend's nodes
    /// (QT-SURVEY §2.3 `focus_info`).
    pub fn focus_info(&self, ctx: &Context) -> FocusInfo {
        match self.focused_entry(ctx) {
            Some(e) => self.info(ctx, &e),
            None => FocusInfo::default(),
        }
    }

    /// EV3b: the text field of this backend that [`EguiBackend::text_command`] acts on: the
    /// focused one, else the last one that had focus while nothing or a menu has it (the Edit menu
    /// enables Cut, Copy and Paste on it). `None` once another widget took focus, or the field was
    /// removed, hidden or disabled. A built-in `text-edit`, an own text widget or a provider kind
    /// whose `Widget::text_input` is `Some`.
    pub fn last_text_field(&self, ctx: &Context) -> Option<FocusInfo> {
        self.text_target(ctx).map(|(e, _)| self.info(ctx, &e))
    }

    /// Qt's `ShortcutOverride`: true when the focused node of this backend takes `chord` itself,
    /// so a host keymap must leave it (Ctrl+Z, Del and the arrows in a text field; every key
    /// while composing).
    ///
    /// OW-RUST: also for a focused `imm` widget of host chrome (a Rust-drawn button keeps Space
    /// and Enter).
    pub fn shortcut_override(&self, ctx: &Context, chord: &KeyboardShortcut) -> bool {
        let Some(e) = self.focused_entry(ctx).or_else(|| foreign_focused(ctx)) else { return false };
        if read(ctx, |st| st.composing == Some(e.id)) {
            return true;
        }
        match e.role {
            Role::Ext { .. } => {
                let Ok(scene) = self.0.scene.try_borrow() else { return false };
                let Some(i) = e.node.and_then(|n| scene.idx(n)) else { return false };
                match scene.w(i) {
                    W::Ext(x) => x.0.try_borrow().is_ok_and(|w| w.composing() || w.consume_key(chord.logical_key, chord.modifiers)),
                    _ => false,
                }
            }
            r => claims(r, chord.logical_key, chord.modifiers, ctx.os() == egui::os::OperatingSystem::Mac),
        }
    }

    /// EV3b: [`EguiBackend::shortcut_override`] for a press with its logical (typed) and physical
    /// key (cadatomic KL1). The typed key wins; a Ctrl or Cmd chord whose logical key is a symbol
    /// or a non-Latin letter also asks for the physical (Latin) key, the stroke the keymap falls
    /// back to.
    pub fn shortcut_override_press(&self, ctx: &Context, logical: Key, physical: Option<Key>, modifiers: Modifiers) -> bool {
        std::iter::once(logical)
            .chain(latin_fallback(logical, physical, modifiers))
            .any(|k| self.shortcut_override(ctx, &shortcut(k, modifiers)))
    }

    /// Sends `cmd` to the focused text field of this backend in `ctx`'s viewport, or, when nothing
    /// (or a menu item) has focus (a click on host chrome such as a Rust menu took it), to the last
    /// text field that had it, focusing it again. Applied when the field is next drawn: a provider
    /// kind's `Widget::text_command` gets it first. False when there is no such field.
    pub fn text_command(&self, ctx: &Context, cmd: TextCommand) -> bool {
        self.text_command_with(ctx, cmd, None)
    }

    /// [`EguiBackend::text_command`] with the host's clipboard (`Embed::text_command`): Paste reads
    /// it, Cut and Copy write to it as well as to egui's.
    pub(crate) fn text_command_with(&self, ctx: &Context, cmd: TextCommand, clip: Option<Clip>) -> bool {
        let Some((e, last)) = self.text_target(ctx) else { return false };
        if last {
            ctx.memory_mut(|m| m.request_focus(e.id));
        }
        with(ctx, |st| st.cmd = Some((e.id, cmd, clip)));
        ctx.request_repaint();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

    #[test]
    fn ev3_text_claims() {
        let t = Role::Text { multiline: false };
        let m = Role::Text { multiline: true };
        for k in [Key::Z, Key::Y, Key::A, Key::C, Key::V, Key::X, Key::ArrowLeft, Key::Delete, Key::Backspace] {
            assert!(claims(t, k, CTRL, false), "{k:?}");
        }
        for k in [Key::Delete, Key::Backspace, Key::ArrowLeft, Key::ArrowRight, Key::Home, Key::End, Key::L, Key::Space, Key::Num1] {
            assert!(claims(t, k, Modifiers::NONE, false), "{k:?}");
        }
        assert!(claims(t, Key::L, Modifiers::SHIFT, false));
        // the app's: focus keys, function keys, other chords
        for (k, mods) in [(Key::Tab, Modifiers::NONE), (Key::Escape, Modifiers::NONE), (Key::F5, Modifiers::NONE), (Key::S, CTRL), (Key::O, CTRL), (Key::F2, Modifiers::NONE)] {
            assert!(!claims(t, k, mods, false), "{k:?}");
        }
        // Enter and the vertical keys only in a multi-line field
        assert!(!claims(t, Key::Enter, Modifiers::NONE, false) && claims(m, Key::Enter, Modifiers::NONE, false));
        assert!(!claims(t, Key::ArrowUp, Modifiers::NONE, false) && claims(m, Key::ArrowUp, Modifiers::NONE, false));
        // Option+arrows on macOS only
        assert!(claims(t, Key::ArrowLeft, Modifiers::ALT, true) && !claims(t, Key::ArrowLeft, Modifiers::ALT, false));
        assert!(claims(Role::Slider, Key::ArrowLeft, Modifiers::NONE, false) && !claims(Role::Slider, Key::Z, CTRL, false));
        assert!(claims(Role::Toggle, Key::Space, Modifiers::NONE, false) && !claims(Role::Toggle, Key::Delete, Modifiers::NONE, false));
    }

    fn entry(n: u64, scope: Id, scoped: bool, role: Role) -> Entry {
        Entry {
            id: Id::unique("w").with(n),
            node: Some(NodeId::from_raw(n + 1).unwrap()),
            node_eid: Id::unique("n").with(n),
            scope,
            scoped,
            role,
            autofocus: false,
            tab_stop: true,
        }
    }

    #[test]
    fn ev3_route_scope_tab_and_dialog_restore() {
        let root = Id::unique("root");
        let dlg = Id::unique("dlg");
        let opener = entry(0, root, false, Role::Button { default: false, cancel: false });
        let a = entry(1, dlg, true, Role::Text { multiline: false });
        let ok = entry(2, dlg, true, Role::Button { default: true, cancel: false });
        let mut st = State::default();
        // pass 1: only the opener; pass 2: the dialog appears
        st.rotate(1);
        st.cur.entries = vec![opener.clone()];
        st.rotate(2);
        st.cur.entries = vec![opener.clone(), a.clone(), ok.clone()];
        st.cur.scopes = vec![Scope { id: dlg, dialog: true }];
        st.rotate(3);
        let p = route(&mut st, Some(opener.id), &[], &[]);
        assert_eq!(p.focus, Some(a.id), "the dialog's first field takes focus");
        // Tab wraps inside the dialog
        st.cur = st.prev.clone();
        st.rotate(4);
        let p = route(&mut st, Some(ok.id), &[(Key::Tab, Modifiers::NONE)], &[]);
        assert_eq!((p.focus, p.cancel_tab), (Some(a.id), true));
        // Enter in the field activates the default button
        st.cur = st.prev.clone();
        st.rotate(5);
        let p = route(&mut st, Some(a.id), &[(Key::Enter, Modifiers::NONE)], &[]);
        assert_eq!(p.eat, vec![Key::Enter]);
        assert_eq!(st.activate, Some(ok.node_eid));
        st.activate = None;
        // but not while composing
        st.cur = st.prev.clone();
        st.rotate(6);
        let p = route(&mut st, Some(a.id), &[(Key::Enter, Modifiers::NONE)], &[true]);
        assert!(p.eat == vec![Key::Enter] && st.activate.is_none(), "the input method's Enter");
        st.composing = None;
        // the dialog goes away: focus back to the opener
        st.cur.entries = vec![opener.clone()];
        st.rotate(7);
        let p = route(&mut st, None, &[], &[]);
        assert_eq!(p.focus, Some(opener.id));
        assert!(st.openers.is_empty());
    }
}
