//! WG2a menus (plans/milestones/WG2a-menus-popups.md): the menu family's payload ([`MenuW`]), its
//! props, measure and painting. Tables are `kinds/menus.rs`.
//!
//! Menus are immediate: `menu-bar` and `menu-button` are taffy leaves; their menu entries (`menu`,
//! `action`, `menu-separator`) are scene children that taffy never lays out (they are out of flow,
//! `display: none`) and that are drawn each frame with egui's own menus ([`egui::MenuBar`],
//! [`MenuButton`], [`SubMenuButton`], [`Popup`]), so egui keeps hover-to-switch in a bar, submenus
//! and arrow-key navigation. A `context-menu` attaches to its parent (`crate::overlays` finds it).
//!
//! Focus: egui's menus move focus into the menu (arrow keys) and do not hand it back to the opener
//! when the menu closes; restoring focus to the opener is EVENTS EV3's (not here yet).
//!
//! Writer lane: WIDGETS.

use crate::kinds::{self, P};
use crate::layout::Metrics;
use crate::scene::{Arena, Dirty, W};
use crate::walk::Out;
#[allow(clippy::disallowed_types)] // OW: wrapper until OW3
use egui::containers::menu::{MenuButton, SubMenuButton, menu_style};
#[allow(clippy::disallowed_types)] // OW: wrapper until OW3
use egui::{
    Align, Id, KeyboardShortcut, Layout, Modifiers, Popup, PopupAnchor, PopupKind, Rect, Role,
    SetOpenCommand, Ui, Vec2, WidgetInfo, vec2,
};
use svelters_abi::Value;

/// A menu-family node's typed props.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuW {
    /// The abstract base (never created).
    Entry,
    /// `own`: OW3 draws it (the own-widget switch when it was created).
    Bar {
        own: bool,
    },
    Menu {
        label: String,
    },
    Action {
        label: String,
        shortcut: String,
        checkable: bool,
        checked: bool,
        group: String,
    },
    Separator,
    /// `cmd`: a `popup`/`popdown` call applied at the next pass.
    Context {
        cmd: Option<bool>,
        own: bool,
    },
    /// `cmd`: a host write of `active` or a `popup`/`popdown` call, applied at the next pass.
    Button {
        label: String,
        active: bool,
        cmd: Option<bool>,
        own: bool,
    },
}

impl MenuW {
    pub fn label(&self) -> &str {
        match self {
            MenuW::Menu { label } | MenuW::Action { label, .. } | MenuW::Button { label, .. } => {
                label
            }
            _ => "",
        }
    }

    /// Drawn in its parent's flow (a taffy leaf): `menu-bar`, `menu-button`.
    pub fn in_flow(&self) -> bool {
        matches!(self, MenuW::Bar { .. } | MenuW::Button { .. })
    }

    /// OW3: a menu root that svelters draws itself (`menu-bar`, `menu-button`, `context-menu`
    /// created while the own-widget switch held its tag).
    pub fn is_own(&self) -> bool {
        matches!(self, MenuW::Bar { own: true } | MenuW::Button { own: true, .. } | MenuW::Context { own: true, .. })
    }

    /// OW3: marks a new menu root as drawn by svelters (`crate::widgets::mark_own`).
    pub(crate) fn set_own(&mut self, on: bool) {
        if let MenuW::Bar { own } | MenuW::Button { own, .. } | MenuW::Context { own, .. } = self {
            *own = on;
        }
    }
}

/// The payload of a new node of kind `id` (the defaults of `kinds/menus.rs`).
pub(crate) fn payload(id: &str) -> MenuW {
    match id {
        "MenuBar" => MenuW::Bar { own: false },
        "Menu" => MenuW::Menu {
            label: String::new(),
        },
        "Action" => MenuW::Action {
            label: String::new(),
            shortcut: String::new(),
            checkable: false,
            checked: false,
            group: String::new(),
        },
        "MenuSeparator" => MenuW::Separator,
        "ContextMenu" => MenuW::Context { cmd: None, own: false },
        "MenuButton" => MenuW::Button {
            label: String::new(),
            active: false,
            cmd: None,
            own: false,
        },
        _ => MenuW::Entry,
    }
}

// ---- values ----

pub(crate) fn as_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::I64(i) => Ok(*i != 0),
        other => Err(format!("expected a boolean, got {other:?}")),
    }
}

pub(crate) fn as_str(v: &Value) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        Value::Null => Ok(String::new()),
        Value::I64(i) => Ok(i.to_string()),
        Value::F64(f) => Ok(f.to_string()),
        other => Err(format!("expected a string, got {other:?}")),
    }
}

/// Parses a `shortcut` (`Ctrl+Shift+S`, `F5`; empty: none).
pub(crate) fn parse_shortcut(s: &str) -> Result<Option<KeyboardShortcut>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    // `Ctrl++` names the plus key
    let (mods, key) = match s.strip_suffix("++") {
        Some(m) => (m, "Plus"),
        None => s.rsplit_once('+').unwrap_or(("", s)),
    };
    let mut m = Modifiers::NONE;
    for part in mods.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => m.ctrl = true,
            "shift" => m.shift = true,
            "alt" | "option" => m.alt = true,
            "cmd" | "command" | "meta" | "super" => m.mac_cmd = true,
            "primary" | "mod" | "cmdorctrl" => m.command = true,
            other => {
                return Err(format!(
                    "`{s}`: unknown modifier `{other}` (Ctrl, Shift, Alt, Cmd, Primary)"
                ));
            }
        }
    }
    let key = key.trim();
    let k = egui::Key::from_name(key)
        .or_else(|| egui::Key::from_name(&key.to_ascii_uppercase()))
        .ok_or_else(|| format!("`{s}`: unknown key `{key}` (an egui `Key` name such as `O`, `F5`, `Enter`, `Delete`)"))?;
    Ok(Some(KeyboardShortcut::new(m, k)))
}

// ---- props ----

pub(crate) fn set_prop(m: &mut MenuW, key: P, v: &Value) -> Result<Dirty, String> {
    let mut d = Dirty::default();
    match (key, m) {
        (P::Text, MenuW::Menu { label } | MenuW::Action { label, .. }) => *label = as_str(v)?,
        (P::Text, MenuW::Button { label, .. }) => {
            *label = as_str(v)?;
            d.insert(Dirty::MEASURE);
        }
        (P::Shortcut, MenuW::Action { shortcut, .. }) => {
            let s = as_str(v)?;
            parse_shortcut(&s)?;
            *shortcut = s;
        }
        (P::Checkable, MenuW::Action { checkable, .. }) => *checkable = as_bool(v)?,
        (P::Checked, MenuW::Action { checked, .. }) => *checked = as_bool(v)?,
        (P::Group, MenuW::Action { group, .. }) => *group = as_str(v)?,
        (P::Active, MenuW::Button { active, cmd, .. }) => {
            *active = as_bool(v)?;
            *cmd = Some(*active);
        }
        (key, m) => return Err(format!("{key:?} is not a prop of {m:?}")),
    }
    Ok(d)
}

pub(crate) fn get_prop(m: &MenuW, key: P) -> Result<Value, String> {
    let s = |x: &str| Value::Str(x.to_string());
    Ok(match (key, m) {
        (P::Text, m) => s(m.label()),
        (P::Shortcut, MenuW::Action { shortcut, .. }) => s(shortcut),
        (P::Checkable, MenuW::Action { checkable, .. }) => Value::Bool(*checkable),
        (P::Checked, MenuW::Action { checked, .. }) => Value::Bool(*checked),
        (P::Group, MenuW::Action { group, .. }) => s(group),
        (P::Active, MenuW::Button { active, .. }) => Value::Bool(*active),
        (key, m) => return Err(format!("{key:?} is not a prop of {m:?}")),
    })
}

/// `popup`/`popdown` (`ops.rs`): queued on the node, applied at its next pass.
pub(crate) fn call(arena: &mut Arena, i: usize, open: bool) {
    match arena.w_mut(i) {
        W::Menu(MenuW::Context { cmd, .. } | MenuW::Button { cmd, .. }) => *cmd = Some(open),
        W::Overlay(o) => o.cmd = Some(open),
        _ => {}
    }
}

// ---- measure ----

/// The size of an in-flow menu kind (`layout.rs`): a menu-bar is one interact row high and takes
/// no width of its own; a menu-button is a button. OW3's own roots measure from CSS
/// (`crate::widgets::menu::measure`).
pub(crate) fn measure(m: &Metrics, w: &MenuW, css: Option<&svelters_style::ComputedStyle>) -> Vec2 {
    if w.is_own() {
        return crate::widgets::menu::measure(m, w, css);
    }
    let sp = &m.style.spacing;
    match w {
        MenuW::Bar { .. } => vec2(0.0, sp.interact_size.y),
        MenuW::Button { label, .. } => {
            let font = egui::TextStyle::Button.resolve(&m.style);
            let t = m.ctx.fonts_mut(|f| {
                f.layout_no_wrap(label.clone(), font, egui::Color32::WHITE)
                    .size()
            });
            vec2(
                t.x + 2.0 * sp.button_padding.x,
                (t.y + 2.0 * sp.button_padding.y).max(sp.interact_size.y),
            )
        }
        _ => Vec2::ZERO,
    }
}

// ---- events ----

/// A widget-side change of node `i`: `key` (if any) stamped, journaled and announced when observed,
/// then each listened event of `events`.
pub(crate) fn announce(
    arena: &mut Arena,
    i: usize,
    key: Option<P>,
    events: &[(&str, Vec<Value>)],
    out: &mut Vec<Out>,
) {
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let kind = arena.core[i].kind as usize;
    if let Some(key) = key {
        let t = arena.stamp();
        arena.core[i].changed = t;
        let prop = kinds::prop_id(kind, key);
        arena.record(
            crate::scene::Change::Prop {
                node,
                prop: crate::scene::PropId(prop),
                origin: crate::scene::Origin::Widget,
            },
            crate::scene::Origin::Widget,
        );
        if arena.observes(i, prop) {
            out.push(Out::Changed(h, prop));
        }
    }
    for (name, args) in events {
        let s = kinds::signal_id(kind, name);
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, args.clone()));
        }
    }
}

/// Sets `checked` of action `i` from the widget side and announces it with `toggled`.
fn set_checked(arena: &mut Arena, i: usize, on: bool, out: &mut Vec<Out>) {
    if let W::Menu(MenuW::Action { checked, .. }) = arena.w_mut(i) {
        *checked = on;
    }
    announce(
        arena,
        i,
        Some(P::Checked),
        &[("toggled", vec![Value::Bool(on)])],
        out,
    );
}

/// The user picked action `i` (click, Enter, its shortcut): a check item toggles, a radio item turns
/// on and turns off the rest of its group (same parent, same group); then `activate(checked)`.
///
/// EV1 (OW3): `activate` goes through the scene's Rust filters first; a filter that prevents it
/// keeps the check state (the default action), one that stops it also keeps it from JS.
pub(crate) fn activate(arena: &mut Arena, i: usize, ctx: &egui::Context, out: &mut Vec<Out>) {
    let Some(node) = arena.slots.id_at(i) else { return };
    let (o, runs) = crate::events::dispatch_signal(arena, node, "activate");
    for r in runs {
        crate::provider::behavior::apply(arena, r, Some(ctx), out);
    }
    if o.stopped && o.prevented() {
        return;
    }
    if o.prevented() {
        let checked = matches!(arena.w(i), W::Menu(MenuW::Action { checked: true, .. }));
        announce(arena, i, None, &[("activate", vec![Value::Bool(checked)])], out);
        return;
    }
    let W::Menu(MenuW::Action {
        checkable,
        checked,
        group,
        ..
    }) = arena.w(i)
    else {
        return;
    };
    let (checkable, was, group) = (*checkable, *checked, group.clone());
    let now = if !group.is_empty() {
        if let Some(p) = arena.core[i].parent.map(|p| arena.ix(p)) {
            for k in 0..arena.children[p].len() {
                let c = arena.children[p][k].index() as usize;
                let other = matches!(arena.w(c), W::Menu(MenuW::Action { checked: true, group: g, .. }) if *g == group);
                if c != i && other {
                    set_checked(arena, c, false, out);
                }
            }
        }
        true
    } else {
        if checkable { !was } else { was }
    };
    if now != was {
        set_checked(arena, i, now, out);
    }
    if !o.stopped {
        announce(arena, i, None, &[("activate", vec![Value::Bool(now)])], out);
    }
}

/// Fires the actions under menu container `root` whose shortcut was pressed this frame (consumed).
pub(crate) fn shortcuts(arena: &mut Arena, root: usize, ui: &mut Ui, out: &mut Vec<Out>) {
    if !ui.is_enabled() || !arena.common(root).enabled {
        return;
    }
    let mut hits = Vec::new();
    let mut stack = vec![root];
    while let Some(p) = stack.pop() {
        for k in 0..arena.children[p].len() {
            let c = arena.children[p][k].index() as usize;
            let cm = arena.common(c);
            if !cm.visible || !cm.enabled {
                continue;
            }
            match arena.w(c) {
                W::Menu(MenuW::Menu { .. }) => stack.push(c),
                W::Menu(MenuW::Action { shortcut, .. }) if !shortcut.is_empty() => {
                    if let Ok(Some(sc)) = parse_shortcut(shortcut)
                        && ui.input_mut(|i| i.consume_shortcut(&sc))
                    {
                        hits.push(c);
                    }
                }
                _ => {}
            }
        }
    }
    for c in hits {
        activate(arena, c, &ui.ctx().clone(), out);
    }
}

// ---- painting ----

/// Decorates an entry's response: AccessKit name and author id, `invalid`, tooltip.
fn decorate(arena: &Arena, i: usize, resp: egui::Response) -> egui::Response {
    let h = arena.slots.id_at(i).expect("live node").to_raw();
    let resp = crate::a11y::decorate(resp, &arena.n(h));
    let tip = &arena.common(i).tooltip;
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip.clone())
    }
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW1a, OW3
/// Draws the menu entries under `parent` in `ui` (a menu bar, a menu popup). `flatten`: a `menu`
/// child shows its items inline (a menu-button's menu).
fn items(
    arena: &mut Arena,
    parent: usize,
    ui: &mut Ui,
    out: &mut Vec<Out>,
    in_bar: bool,
    flatten: bool,
) {
    for k in 0..arena.children[parent].len() {
        let c = arena.children[parent][k].index() as usize;
        if !arena.common(c).visible {
            continue;
        }
        let enabled = arena.common(c).enabled;
        match arena.w(c) {
            W::Menu(MenuW::Menu { .. }) if flatten => items(arena, c, ui, out, false, false),
            W::Menu(MenuW::Menu { label }) => {
                let label = label.clone();
                let mut resp = None;
                ui.add_enabled_ui(enabled, |ui| {
                    let (r, _) = if in_bar {
                        MenuButton::new(label.as_str())
                            .ui(ui, |ui| items(arena, c, ui, out, false, false))
                    } else {
                        SubMenuButton::new(label.as_str())
                            .ui(ui, |ui| items(arena, c, ui, out, false, false))
                    };
                    resp = Some(r);
                });
                if let Some(r) = resp {
                    decorate(arena, c, r);
                }
            }
            W::Menu(MenuW::Action {
                label,
                shortcut,
                checkable,
                checked,
                group,
            }) => {
                let (label, checked, radio) = (label.clone(), *checked, !group.is_empty());
                let check = *checkable || radio;
                let sc = parse_shortcut(shortcut)
                    .ok()
                    .flatten()
                    .map(|s| ui.ctx().format_shortcut(&s));
                let r = match (check, sc) {
                    (true, None) if radio => {
                        ui.add_enabled(enabled, egui::RadioButton::new(checked, label.as_str()))
                    }
                    (true, None) => {
                        let mut on = checked;
                        ui.add_enabled(enabled, egui::Checkbox::new(&mut on, label.as_str()))
                    }
                    (check, sc) => {
                        let text = if check {
                            format!("{} {label}", if checked { "✔" } else { "  " })
                        } else {
                            label.clone()
                        };
                        let mut b = egui::Button::new(text);
                        if let Some(sc) = sc.clone() {
                            b = b.shortcut_text(sc);
                        }
                        let r = ui.add_enabled(enabled, b);
                        // named by the label alone; the accelerator is AccessKit's keyboard shortcut
                        if check {
                            let role = if radio {
                                Role::RadioButton
                            } else {
                                Role::CheckBox
                            };
                            r.widget_info(|| WidgetInfo::selected(role, enabled, checked, &label));
                        } else {
                            r.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &label));
                        }
                        if let Some(sc) = sc {
                            ui.ctx().accesskit_node_builder(r.id, |n| n.set_keyboard_shortcut(sc));
                        }
                        r
                    }
                };
                let r = decorate(arena, c, r);
                if r.clicked() {
                    activate(arena, c, &ui.ctx().clone(), out);
                }
            }
            W::Menu(MenuW::Separator) => {
                ui.separator();
            }
            _ => {}
        }
    }
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW3
/// Paints an in-flow menu kind at its rect (`leaf.rs` dispatches here): `menu-bar`, `menu-button`.
pub(crate) fn leaf(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> egui::Response {
    if let W::Menu(m) = arena.w(i)
        && m.is_own()
    {
        return crate::widgets::menu::leaf(arena, i, ui, out); // OW3
    }
    let size = arena.core[i].rect.size();
    let resp = match arena.w(i) {
        W::Menu(MenuW::Bar { .. }) => {
            shortcuts(arena, i, ui, out);
            egui::MenuBar::new()
                .ui(ui, |ui| items(arena, i, ui, out, true, false))
                .response
        }
        W::Menu(MenuW::Button { label, active, cmd, .. }) => {
            let (label, active, cmd) = (label.clone(), *active, *cmd);
            if let W::Menu(MenuW::Button { cmd, .. }) = arena.w_mut(i) {
                *cmd = None;
            }
            shortcuts(arena, i, ui, out);
            let button = egui::Button::new(label.as_str()).min_size(size);
            let (resp, _) =
                MenuButton::from_button(button).ui(ui, |ui| items(arena, i, ui, out, false, true));
            let ctx = ui.ctx().clone();
            let pid = Popup::default_response_id(&resp);
            match cmd {
                Some(true) if !Popup::is_id_open(&ctx, pid) => {
                    Popup::open_id(&ctx, pid);
                    ctx.request_repaint();
                }
                Some(false) => Popup::close_id(&ctx, pid),
                _ => {}
            }
            let now = Popup::is_id_open(&ctx, pid);
            if now != active {
                if let W::Menu(MenuW::Button { active, .. }) = arena.w_mut(i) {
                    *active = now;
                }
                announce(
                    arena,
                    i,
                    Some(P::Active),
                    &[("changed", vec![Value::Bool(now)])],
                    out,
                );
            }
            resp
        }
        _ => return ui.response(),
    };
    let resp = decorate(arena, i, resp);
    if arena.common(i).invalid {
        crate::leaf::invalid_outline(ui, resp.rect);
    }
    arena.core[i].drawn = resp.rect;
    resp
}

/// The id under which the context-menu claims of one pass are kept (the innermost owner wins).
pub(crate) fn claim(ctx: &egui::Context, area: f32) -> bool {
    let key = Id::NULL.with("svelters-egui/context-menu-claim");
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        if let Some((p, a)) = d.get_temp::<(u64, f32)>(key)
            && p == pass
            && a <= area
        {
            return false;
        }
        d.insert_temp(key, (pass, area));
        true
    })
}

#[allow(clippy::disallowed_types)] // OW: wrapper until OW3
/// Paints `context-menu` node `i` attached to an owner drawn at `owner` (screen): a secondary click
/// on the owner (in `ui`'s layer, not covered) opens it at the pointer; `popup()` below the owner.
pub(crate) fn context(arena: &mut Arena, i: usize, owner: Rect, ui: &mut Ui, out: &mut Vec<Out>) {
    let W::Menu(MenuW::Context { cmd, own }) = arena.w(i) else {
        return;
    };
    if *own {
        return crate::widgets::menu::context(arena, i, owner, ui, out); // OW3
    }
    let cmd = *cmd;
    if let W::Menu(MenuW::Context { cmd, .. }) = arena.w_mut(i) {
        *cmd = None;
    }
    let id = arena.core[i].egui_id.with("context-menu");
    let ctx = ui.ctx().clone();
    let enabled = arena.common(i).enabled && ui.is_enabled();
    let at_key = id.with("at");
    let mut set = None;
    if enabled
        && owner.is_positive()
        && ui.input(|inp| inp.pointer.secondary_clicked())
        && ui.rect_contains_pointer(owner)
        && claim(&ctx, owner.area())
        && let Some(pos) = ctx.pointer_interact_pos()
    {
        ctx.data_mut(|d| d.insert_temp(at_key, pos));
        set = Some(SetOpenCommand::Bool(true));
    }
    match cmd {
        Some(true) => {
            ctx.data_mut(|d| d.insert_temp(at_key, owner.left_bottom()));
            set = Some(SetOpenCommand::Bool(true));
            ctx.request_repaint();
        }
        Some(false) => set = Some(SetOpenCommand::Bool(false)),
        None => {}
    }
    if enabled {
        shortcuts(arena, i, ui, out);
    }
    let at = ctx
        .data(|d| d.get_temp::<egui::Pos2>(at_key))
        .unwrap_or(owner.left_bottom());
    let name = arena.common(i).accessible_name.clone();
    let mut p = Popup::new(id, ctx.clone(), PopupAnchor::Position(at), ui.layer_id())
        .kind(PopupKind::Menu)
        .layout(Layout::top_down_justified(Align::Min))
        .style(menu_style)
        .gap(0.0)
        .open_memory(set);
    if !name.is_empty() {
        p = p.accessible_name(name);
    }
    p.show(|ui| items(arena, i, ui, out, false, false));
}
