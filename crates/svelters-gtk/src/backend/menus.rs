//! Menus and actions (X4, gap G7): the kinds `menubar`, `toolbar`, `contextmenu`, `menu`, `action`
//! and `separator`.
//!
//! GTK describes menus with a `GMenuModel` and handles them with `GAction`s found through
//! `insert_action_group`; neither is a widget, so the generated widget tables have no kind for them.
//! This module keeps a small node tree per handle and derives the native objects from it:
//!
//! | node | native |
//! |---|---|
//! | `menubar` | `GtkPopoverMenuBar` over the root `GMenu` |
//! | `toolbar` | `GtkBox` of buttons rebuilt from the child nodes |
//! | `contextmenu` | `GtkPopoverMenu` parented to the target widget, popped up by a secondary-button gesture |
//! | `menu` | a `GMenu` referenced as a submenu item by its parent, or the `menu-model` of a `GtkMenuButton` |
//! | `action` | a `GSimpleAction` (stateless, boolean state for `checkable`, or a member of the string-state action of its radio `group`) |
//! | `separator` | a section boundary of the parent `GMenu` |
//!
//! Every structural or presentational change re-syncs the whole tree of its root: the root's
//! `GSimpleActionGroup` (prefix `svl`, inserted on the widget that shows the menu), the `GMenu`
//! models (an unchanged model is left alone), the toolbar buttons and a global-scope
//! `GtkShortcutController` with one `GtkShortcut` per `shortcut`. A removed or reordered child is
//! therefore one `GMenu` update, which GTK's popovers follow natively.

use super::*;
use gtk::gio;

pub const KIND_MENUBAR: u32 = 0x4401;
pub const KIND_TOOLBAR: u32 = 0x4402;
pub const KIND_CONTEXT: u32 = 0x4403;
pub const KIND_MENU: u32 = 0x4404;
pub const KIND_ACTION: u32 = 0x4405;
pub const KIND_SEPARATOR: u32 = 0x4406;

/// Slot value recorded for a node inserted below another node or attached to a widget.
pub const SLOT_MENU: u32 = u32::MAX - 1;

const P_LABEL: u32 = 0x4411;
const P_ICON: u32 = 0x4412;
const P_SHORTCUT: u32 = 0x4413;
const P_TOOLTIP: u32 = 0x4414;
const P_ENABLED: u32 = 0x4415;
const P_VISIBLE: u32 = 0x4416;
const P_CHECKABLE: u32 = 0x4417;
const P_CHECKED: u32 = 0x4418;
const P_GROUP: u32 = 0x4419;
const M_POPUP: u32 = 0x4421;
const M_POPDOWN: u32 = 0x4422;
const S_ACTIVATE: u32 = 0x4431;
const S_TOGGLED: u32 = 0x4432;

/// The action group prefix every menu root uses.
const PREFIX: &str = "svl";

pub fn is_menu_kind(k: u32) -> bool {
    (KIND_MENUBAR..=KIND_SEPARATOR).contains(&k)
}

fn is_container(k: u32) -> bool {
    matches!(k, KIND_MENUBAR | KIND_TOOLBAR | KIND_CONTEXT | KIND_MENU)
}

pub fn kind_by_name(name: &str) -> u32 {
    match name {
        "menubar" => KIND_MENUBAR,
        "toolbar" => KIND_TOOLBAR,
        "contextmenu" => KIND_CONTEXT,
        "menu" => KIND_MENU,
        "action" => KIND_ACTION,
        "separator" => KIND_SEPARATOR,
        _ => 0,
    }
}

/// The name of a menu signal id (test harness).
pub fn signal_name(id: u32) -> &'static str {
    match id {
        S_ACTIVATE => "activate",
        S_TOGGLED => "toggled",
        _ => "?",
    }
}

pub fn is_own_member(id: u32) -> bool {
    (P_LABEL..=S_TOGGLED).contains(&id)
}

/// The kind whose generated tables serve the widget members of a menu widget.
pub fn table_name(kind: u32) -> Option<&'static str> {
    match kind {
        KIND_MENUBAR => Some("Widget"),
        KIND_TOOLBAR => Some("Box"),
        _ => None,
    }
}

pub fn find_member(what: u32, kind: u32, name: &str) -> u32 {
    match (kind, what, name) {
        (KIND_ACTION, SVL_PROP, "label") | (KIND_MENU, SVL_PROP, "label") => P_LABEL,
        (KIND_ACTION, SVL_PROP, "icon") | (KIND_MENU, SVL_PROP, "icon") => P_ICON,
        (KIND_ACTION, SVL_PROP, "shortcut") => P_SHORTCUT,
        (KIND_ACTION, SVL_PROP, "tooltip") => P_TOOLTIP,
        (KIND_ACTION | KIND_MENU, SVL_PROP, "enabled") => P_ENABLED,
        (KIND_ACTION | KIND_MENU | KIND_SEPARATOR, SVL_PROP, "visible") => P_VISIBLE,
        (KIND_ACTION, SVL_PROP, "checkable") => P_CHECKABLE,
        (KIND_ACTION, SVL_PROP, "checked") => P_CHECKED,
        (KIND_ACTION, SVL_PROP, "group") => P_GROUP,
        (KIND_CONTEXT, SVL_METHOD, "popup") => M_POPUP,
        (KIND_CONTEXT, SVL_METHOD, "popdown") => M_POPDOWN,
        (KIND_ACTION, SVL_SIGNAL, "activate") => S_ACTIVATE,
        (KIND_ACTION, SVL_SIGNAL, "toggled") => S_TOGGLED,
        _ => 0,
    }
}

pub(super) struct Node {
    kind: u32,
    label: String,
    icon: String,
    shortcut: String,
    tooltip: String,
    enabled: bool,
    visible: bool,
    checkable: bool,
    checked: bool,
    group: String,
    parent: u64,
    children: Vec<u64>,
    /// Items of a container (all container kinds keep one; the toolbar's stays empty).
    model: gio::Menu,
    model_sig: String,
    /// Own action of a plain or check `action`; none while it is a radio member.
    action: Option<gio::SimpleAction>,
    /// 0 = activate, 1 = toggled.
    listened: u32,
    /// Roots: the actions of the whole tree.
    actions: gio::SimpleActionGroup,
    /// Containers: the string-state action of each radio group, with its action name.
    radio: HashMap<String, (String, gio::SimpleAction)>,
    /// `menubar` and `toolbar`: the widget.
    widget: Option<gtk::Widget>,
    /// Where the root's action group is inserted and its shortcut controller added.
    group_on: Option<gtk::Widget>,
    ctrl_on: Option<gtk::Widget>,
    ctrl: Option<gtk::ShortcutController>,
    ctrl_sig: String,
    bar_sig: String,
    /// `contextmenu`: the popover, and the widget and gesture it is attached to.
    popover: Option<gtk::PopoverMenu>,
    gesture: Option<gtk::GestureClick>,
    /// `contextmenu` target widget / `menu` owner button.
    attached: Option<gtk::Widget>,
}

impl Node {
    fn new(kind: u32) -> Node {
        Node {
            kind,
            label: String::new(),
            icon: String::new(),
            shortcut: String::new(),
            tooltip: String::new(),
            enabled: true,
            visible: true,
            checkable: false,
            checked: false,
            group: String::new(),
            parent: 0,
            children: Vec::new(),
            model: gio::Menu::new(),
            model_sig: String::new(),
            action: None,
            listened: 0,
            actions: gio::SimpleActionGroup::new(),
            radio: HashMap::new(),
            widget: None,
            group_on: None,
            ctrl_on: None,
            ctrl: None,
            ctrl_sig: String::new(),
            bar_sig: String::new(),
            popover: None,
            gesture: None,
            attached: None,
        }
    }
}

/// Value descriptors of the menu properties, so the renderer coerces bare attributes (`<action checkable>`).
pub fn describe_member(what: u32, member: u32) -> Option<MemberDesc> {
    if what != SVL_PROP {
        return None;
    }
    let (name, tag) = match member {
        P_LABEL => ("label", SVL_STR),
        P_ICON => ("icon", SVL_STR),
        P_SHORTCUT => ("shortcut", SVL_STR),
        P_TOOLTIP => ("tooltip", SVL_STR),
        P_GROUP => ("group", SVL_STR),
        P_ENABLED => ("enabled", SVL_BOOL),
        P_VISIBLE => ("visible", SVL_BOOL),
        P_CHECKABLE => ("checkable", SVL_BOOL),
        P_CHECKED => ("checked", SVL_BOOL),
        _ => return None,
    };
    let slot = SlotDesc::new(tag).named(name);
    Some(MemberDesc { name: name.into(), params: vec![slot.clone()], ret: slot, flags: 0 })
}

/// `Ctrl+Shift+O` to GTK's `<Control><Shift>o`.
pub fn gtk_accel(s: &str) -> String {
    let parts: Vec<&str> = s.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    let Some((key, mods)) = parts.split_last() else { return String::new() };
    let mut out = String::new();
    for m in mods {
        out.push_str(match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "<Control>",
            "shift" => "<Shift>",
            "alt" => "<Alt>",
            "meta" | "super" | "win" => "<Super>",
            _ => continue,
        });
    }
    let key = match key.to_ascii_lowercase().as_str() {
        "del" => "Delete".to_string(),
        "esc" => "Escape".to_string(),
        "enter" => "Return".to_string(),
        "ins" => "Insert".to_string(),
        "pgup" => "Page_Up".to_string(),
        "pgdown" => "Page_Down".to_string(),
        k if k.len() == 1 => k.to_string(),
        _ => key.to_string(),
    };
    out.push_str(&key);
    out
}

fn str_arg(v: &svl_value) -> Res<String> {
    match v.tag {
        SVL_NULL => Ok(String::new()),
        _ => Ok(unsafe { convert::as_str(v) }.ok_or("expected a string")?.into_owned()),
    }
}

fn bool_arg(v: &svl_value) -> Res<bool> {
    if v.tag == SVL_BOOL {
        return Ok(unsafe { v.u.b });
    }
    // A bare attribute (`<action checkable>`) reaches a host without descriptors as text.
    match unsafe { convert::as_str(v) }.as_deref() {
        Some("" | "true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err("expected a boolean".into()),
    }
}

fn radio_id(h: u64) -> String {
    format!("a{h}")
}

impl Inner {
    pub(super) fn menu_kind(&self, h: u64) -> Option<u32> {
        self.menus.borrow().get(&h).map(|n| n.kind)
    }

    fn menu_root(&self, mut h: u64) -> u64 {
        let nodes = self.menus.borrow();
        while let Some(n) = nodes.get(&h) {
            if n.parent == 0 {
                break;
            }
            h = n.parent;
        }
        h
    }

    pub(super) fn create_menu_node(self: &Rc<Self>, kind: u32) -> Res<u64> {
        let mut node = Node::new(kind);
        let (obj, hold): (glib::Object, Option<glib::Object>) = match kind {
            KIND_MENUBAR => {
                let w = gtk::PopoverMenuBar::from_model(Some(&node.model));
                node.widget = Some(w.clone().upcast());
                node.group_on = Some(w.clone().upcast());
                node.ctrl_on = Some(w.clone().upcast());
                (w.clone().upcast(), Some(w.upcast()))
            }
            KIND_TOOLBAR => {
                let w = gtk::Box::new(gtk::Orientation::Horizontal, 2);
                w.add_css_class("toolbar");
                node.widget = Some(w.clone().upcast());
                node.group_on = Some(w.clone().upcast());
                node.ctrl_on = Some(w.clone().upcast());
                (w.clone().upcast(), Some(w.upcast()))
            }
            KIND_MENU | KIND_CONTEXT => {
                let o: glib::Object = node.model.clone().upcast();
                (o.clone(), Some(o))
            }
            _ => {
                let o = glib::Object::new::<glib::Object>();
                (o.clone(), Some(o))
            }
        };
        let h = {
            let mut st = self.state.borrow_mut();
            st.next += 1;
            let h = st.next;
            st.entries.insert(
                h,
                Entry { obj: obj.downgrade(), hold, class: SVL_LIFE_OWNED, owner: 0, note: std::ptr::null_mut(), kind, signals: HashMap::new(), slot: None, capture_prev: None },
            );
            h
        };
        self.tag_object(&obj, h);
        self.menus.borrow_mut().insert(h, node);
        if matches!(kind, KIND_MENUBAR | KIND_TOOLBAR) {
            self.menus_sync(h);
        }
        Ok(h)
    }

    /// Forget the node of a handle that died natively (its toplevel was closed).
    pub(super) fn menus_forget(&self, h: u64) {
        self.menus.borrow_mut().remove(&h);
    }

    pub(super) fn destroy_menu_node(self: &Rc<Self>, h: u64) {
        self.menu_detach_target(h);
        let (parent, kids) = {
            let nodes = self.menus.borrow();
            match nodes.get(&h) {
                Some(n) => (n.parent, n.children.clone()),
                None => return,
            }
        };
        if parent != 0 {
            if let Some(p) = self.menus.borrow_mut().get_mut(&parent) {
                p.children.retain(|c| *c != h);
            }
        }
        for k in &kids {
            if let Some(n) = self.menus.borrow_mut().get_mut(k) {
                n.parent = 0;
            }
        }
        // Gone from its group before the node is dropped.
        if parent != 0 {
            if let Some(n) = self.menus.borrow_mut().get_mut(&h) {
                n.parent = 0;
            }
            self.menus_sync(parent);
        }
        for k in kids {
            self.menus_sync(k);
        }
        self.menus.borrow_mut().remove(&h);
    }

    // ---- tree ops --------------------------------------------------------------------------

    pub(super) fn menus_insert(self: &Rc<Self>, parent: u64, child: u64, before: u64) -> Res<()> {
        let ck = self.menu_kind(child).ok_or("not a menu node")?;
        let pk = self.menu_kind(parent);
        match (pk, ck) {
            (Some(pk), KIND_ACTION | KIND_SEPARATOR | KIND_MENU) if is_container(pk) => {}
            (Some(_), _) => return Err("a menu holds only actions, separators and menus".into()),
            (None, KIND_CONTEXT) => return self.menu_attach_context(parent, child),
            (None, KIND_MENU) => return self.menu_attach_button(parent, child),
            (None, _) => return Err("only a contextmenu or a menu (of a menubutton) can be inserted into a widget".into()),
        }
        {
            let mut nodes = self.menus.borrow_mut();
            let cur = nodes.get(&child).ok_or("stale handle")?.parent;
            if cur != 0 && cur != parent {
                return Err(format!("handle {child} is already inserted in {cur}"));
            }
            if before == child {
                return Ok(());
            }
            if cur == parent {
                nodes.get_mut(&parent).unwrap().children.retain(|c| *c != child);
            }
            let p = nodes.get_mut(&parent).ok_or("stale parent")?;
            let at = if before == 0 {
                p.children.len()
            } else {
                match p.children.iter().position(|c| *c == before) {
                    Some(i) => i,
                    None => {
                        if cur == parent {
                            p.children.push(child);
                        }
                        return Err("`before` is not a child of this menu".into());
                    }
                }
            };
            p.children.insert(at, child);
            nodes.get_mut(&child).unwrap().parent = parent;
        }
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = Some((parent, SLOT_MENU));
        }
        self.menus_sync(parent);
        Ok(())
    }

    pub(super) fn menus_remove(self: &Rc<Self>, parent: u64, child: u64) -> Res<()> {
        let at_widget = self.menus.borrow().get(&child).and_then(|n| n.attached.clone());
        if self.menu_kind(parent).is_none() {
            if at_widget.is_none() {
                return Err(format!("handle {child} is not inserted in {parent}"));
            }
            self.menu_detach_target(child);
            if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
                e.slot = None;
            }
            return Ok(());
        }
        {
            let mut nodes = self.menus.borrow_mut();
            if nodes.get(&child).map(|n| n.parent) != Some(parent) {
                return Err(format!("handle {child} is not inserted in {parent}"));
            }
            nodes.get_mut(&parent).unwrap().children.retain(|c| *c != child);
            nodes.get_mut(&child).unwrap().parent = 0;
        }
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = None;
        }
        self.menus_sync(parent);
        self.menus_sync(child);
        Ok(())
    }

    /// A `contextmenu` attaches to the widget it is inserted into.
    fn menu_attach_context(self: &Rc<Self>, parent: u64, child: u64) -> Res<()> {
        let (w, _) = self.widget_of(parent)?;
        if self.menus.borrow().get(&child).is_some_and(|n| n.attached.is_some()) {
            return Err("the contextmenu is already attached".into());
        }
        let (model, group) = {
            let nodes = self.menus.borrow();
            let n = nodes.get(&child).unwrap();
            (n.model.clone(), n.actions.clone())
        };
        let pop = gtk::PopoverMenu::from_model(Some(&model));
        pop.set_has_arrow(false);
        pop.set_parent(&w);
        pop.insert_action_group(PREFIX, Some(&group));
        let click = gtk::GestureClick::new();
        click.set_button(gtk::gdk::BUTTON_SECONDARY);
        let pw = pop.downgrade();
        click.connect_pressed(move |_, _, x, y| {
            if let Some(p) = pw.upgrade() {
                p.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                p.popup();
            }
        });
        w.add_controller(click.clone());
        let pw = pop.downgrade();
        w.connect_destroy(move |_| {
            if let Some(p) = pw.upgrade() {
                if p.parent().is_some() {
                    p.unparent();
                }
            }
        });
        {
            let mut nodes = self.menus.borrow_mut();
            let n = nodes.get_mut(&child).unwrap();
            n.popover = Some(pop.clone());
            n.gesture = Some(click);
            n.attached = Some(w.clone());
            n.group_on = Some(pop.upcast());
            n.ctrl_on = Some(w);
        }
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = Some((parent, SLOT_MENU));
        }
        self.menus_sync(child);
        Ok(())
    }

    /// A `menu` inserted into a `menubutton` is its `menu-model`.
    fn menu_attach_button(self: &Rc<Self>, parent: u64, child: u64) -> Res<()> {
        let (w, _) = self.widget_of(parent)?;
        let Some(btn) = w.downcast_ref::<gtk::MenuButton>() else {
            return Err("a menu can only be inserted into a menu, a menubar or a menubutton".into());
        };
        if self.menus.borrow().get(&child).is_some_and(|n| n.attached.is_some() || n.parent != 0) {
            return Err("the menu is already inserted".into());
        }
        let model = self.menus.borrow().get(&child).unwrap().model.clone();
        btn.set_menu_model(Some(&model));
        {
            let mut nodes = self.menus.borrow_mut();
            let n = nodes.get_mut(&child).unwrap();
            n.attached = Some(w.clone());
            n.group_on = Some(w.clone());
            n.ctrl_on = Some(w);
        }
        if let Some(e) = self.state.borrow_mut().entries.get_mut(&child) {
            e.slot = Some((parent, SLOT_MENU));
        }
        self.menus_sync(child);
        Ok(())
    }

    fn menu_detach_target(&self, h: u64) {
        let taken = {
            let mut nodes = self.menus.borrow_mut();
            let Some(n) = nodes.get_mut(&h) else { return };
            let Some(w) = n.attached.take() else { return };
            let ctrl = n.ctrl.take();
            let pop = n.popover.take();
            let gesture = n.gesture.take();
            n.group_on = None;
            n.ctrl_on = None;
            n.ctrl_sig.clear();
            (w, ctrl, pop, gesture)
        };
        let (w, ctrl, pop, gesture) = taken;
        if let Some(c) = ctrl {
            w.remove_controller(&c);
        }
        if let Some(g) = gesture {
            w.remove_controller(&g);
        }
        if let Some(p) = pop {
            if p.parent().is_some() {
                p.unparent();
            }
        } else if let Some(b) = w.downcast_ref::<gtk::MenuButton>() {
            b.set_menu_model(None::<&gio::MenuModel>);
            b.insert_action_group(PREFIX, None::<&gio::ActionGroup>);
        }
    }

    // ---- sync ------------------------------------------------------------------------------

    /// Re-derives the native objects of the whole tree `from` belongs to.
    pub(super) fn menus_sync(self: &Rc<Self>, from: u64) {
        let root = self.menu_root(from);
        let mut nodes = self.menus.borrow_mut();
        if !nodes.contains_key(&root) {
            return;
        }
        let mut order = vec![root];
        let mut i = 0;
        while i < order.len() {
            let kids = nodes[&order[i]].children.clone();
            i += 1;
            order.extend(kids);
        }
        self.sync_actions(&mut nodes, root, &order);
        // Children before parents, so a parent's signature sees the child's model as final.
        for &h in order.iter().rev() {
            if matches!(nodes[&h].kind, KIND_MENUBAR | KIND_MENU | KIND_CONTEXT) {
                Self::sync_model(&mut nodes, h);
            }
        }
        if nodes[&root].kind == KIND_TOOLBAR {
            self.sync_toolbar(&mut nodes, root);
        }
        Self::sync_shortcuts(&mut nodes, root, &order);
    }

    fn sync_actions(self: &Rc<Self>, nodes: &mut HashMap<u64, Node>, root: u64, order: &[u64]) {
        let group = nodes[&root].actions.clone();
        let mut want: Vec<(String, gio::Action)> = Vec::new();
        want.push(("off".into(), {
            match group.lookup_action("off") {
                Some(a) => a,
                None => {
                    let a = gio::SimpleAction::new("off", None);
                    a.set_enabled(false);
                    a.upcast()
                }
            }
        }));
        // Radio groups first: an item's checked flag decides the initial state.
        let containers: Vec<u64> = order.iter().copied().filter(|h| is_container(nodes[h].kind)).collect();
        for c in &containers {
            let kids = nodes[c].children.clone();
            let mut used: Vec<String> = Vec::new();
            for k in &kids {
                let n = &nodes[k];
                if n.kind != KIND_ACTION || n.group.is_empty() {
                    continue;
                }
                let g = n.group.clone();
                if !used.contains(&g) {
                    used.push(g.clone());
                }
                if !nodes[c].radio.contains_key(&g) {
                    let name = format!("g{c}x{}", g.bytes().map(|b| format!("{b:02x}")).collect::<String>());
                    let a = gio::SimpleAction::new_stateful(&name, Some(glib::VariantTy::STRING), &"".to_variant());
                    let cc = *c;
                    a.connect_activate(move |a, p| {
                        if let Some(p) = p {
                            a.change_state(p);
                        }
                    });
                    let weak = self.me.clone();
                    a.connect_change_state(move |a, v| {
                        if let (Some(i), Some(v)) = (weak.upgrade(), v) {
                            i.menu_radio_changed(cc, a, v);
                        }
                    });
                    nodes.get_mut(c).unwrap().radio.insert(g.clone(), (name, a));
                }
            }
            // Drop groups no item uses any more.
            nodes.get_mut(c).unwrap().radio.retain(|g, _| used.contains(g));
            let items: Vec<(String, gio::SimpleAction)> = nodes[c].radio.iter().map(|(g, (_, a))| (g.clone(), a.clone())).collect();
            for (g, a) in items {
                let members: Vec<u64> = kids.iter().copied().filter(|k| nodes[k].kind == KIND_ACTION && nodes[k].group == g).collect();
                let cur = a.state().and_then(|s| s.get::<String>()).unwrap_or_default();
                let valid = members.iter().any(|m| radio_id(*m) == cur);
                if !valid {
                    let pick = members.iter().find(|m| nodes[m].checked).map(|m| radio_id(*m)).unwrap_or_default();
                    a.set_state(&pick.to_variant());
                }
                let cur = a.state().and_then(|s| s.get::<String>()).unwrap_or_default();
                for m in &members {
                    nodes.get_mut(m).unwrap().checked = radio_id(*m) == cur;
                }
                want.push((nodes[c].radio[&g].0.clone(), a.upcast()));
            }
        }
        for &h in order {
            let n = &nodes[&h];
            if n.kind != KIND_ACTION || !n.group.is_empty() {
                if n.kind == KIND_ACTION {
                    nodes.get_mut(&h).unwrap().action = None;
                }
                continue;
            }
            if n.action.is_none() {
                let name = format!("a{h}");
                let a = if n.checkable {
                    gio::SimpleAction::new_stateful(&name, None, &n.checked.to_variant())
                } else {
                    gio::SimpleAction::new(&name, None)
                };
                a.set_enabled(n.enabled);
                let weak = self.me.clone();
                let checkable = n.checkable;
                a.connect_activate(move |a, _| {
                    // A handler on `activate` replaces GIO's default toggle, so a check action toggles here.
                    if checkable {
                        let on = a.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
                        a.change_state(&(!on).to_variant());
                    }
                    if let Some(i) = weak.upgrade() {
                        i.menu_activated(h);
                    }
                });
                if n.checkable {
                    let weak = self.me.clone();
                    a.connect_change_state(move |a, v| {
                        if let (Some(i), Some(v)) = (weak.upgrade(), v.and_then(|v| v.get::<bool>())) {
                            a.set_state(&v.to_variant());
                            i.menu_toggled(h, v);
                        }
                    });
                }
                nodes.get_mut(&h).unwrap().action = Some(a);
            }
            let a = nodes[&h].action.clone().unwrap();
            want.push((format!("a{h}"), a.upcast()));
        }
        // Replace what changed, drop what left the tree.
        for name in group.list_actions() {
            if !want.iter().any(|(n, _)| n.as_str() == name.as_str()) {
                group.remove_action(&name);
            }
        }
        for (name, a) in want {
            match group.lookup_action(&name) {
                Some(cur) if cur == a => {}
                Some(_) => {
                    group.remove_action(&name);
                    group.add_action(&a);
                }
                None => group.add_action(&a),
            }
        }
        if let Some(w) = &nodes[&root].group_on {
            w.insert_action_group(PREFIX, Some(&group));
        }
    }

    fn item_for(nodes: &HashMap<u64, Node>, parent: u64, h: u64) -> Option<gio::MenuItem> {
        let n = &nodes[&h];
        if !n.visible {
            return None;
        }
        let item = match n.kind {
            KIND_MENU => {
                let item = gio::MenuItem::new_submenu(Some(&n.label), &n.model);
                if !n.enabled {
                    item.set_action_and_target_value(Some(&format!("{PREFIX}.off")), None);
                }
                item
            }
            KIND_ACTION => {
                let item = gio::MenuItem::new(Some(&n.label), None);
                if n.group.is_empty() {
                    item.set_action_and_target_value(Some(&format!("{PREFIX}.a{h}")), None);
                } else {
                    let name = nodes[&parent].radio.get(&n.group).map(|(nm, _)| nm.clone())?;
                    if n.enabled {
                        item.set_action_and_target_value(Some(&format!("{PREFIX}.{name}")), Some(&radio_id(h).to_variant()));
                    } else {
                        item.set_action_and_target_value(Some(&format!("{PREFIX}.off")), None);
                    }
                }
                if !n.shortcut.is_empty() {
                    item.set_attribute_value("accel", Some(&gtk_accel(&n.shortcut).to_variant()));
                }
                item
            }
            _ => return None,
        };
        if !n.icon.is_empty() {
            item.set_icon(&gio::ThemedIcon::new(&n.icon));
        }
        Some(item)
    }

    fn sync_model(nodes: &mut HashMap<u64, Node>, h: u64) {
        let kids = nodes[&h].children.clone();
        let mut sections: Vec<Vec<gio::MenuItem>> = vec![Vec::new()];
        let mut sig = String::new();
        for k in &kids {
            let n = &nodes[k];
            match n.kind {
                KIND_SEPARATOR => {
                    if n.visible {
                        sig.push_str("|;");
                        sections.push(Vec::new());
                    }
                }
                KIND_ACTION | KIND_MENU => {
                    if let Some(item) = Self::item_for(nodes, h, *k) {
                        sig.push_str(&format!("|{}:{}:{}:{}:{}:{}:{}", n.kind, k, n.label, n.icon, n.shortcut, n.group, n.enabled));
                        sections.last_mut().unwrap().push(item);
                    }
                }
                _ => {}
            }
        }
        if nodes[&h].model_sig == sig && !sig.is_empty() {
            return;
        }
        let model = nodes[&h].model.clone();
        model.remove_all();
        let multi = sections.iter().filter(|s| !s.is_empty()).count() > 1;
        for s in sections.iter().filter(|s| !s.is_empty()) {
            if multi {
                let sec = gio::Menu::new();
                for it in s {
                    sec.append_item(it);
                }
                model.append_section(None, &sec);
            } else {
                for it in s {
                    model.append_item(it);
                }
            }
        }
        nodes.get_mut(&h).unwrap().model_sig = sig;
    }

    fn sync_toolbar(&self, nodes: &mut HashMap<u64, Node>, tb: u64) {
        let Some(bx) = nodes[&tb].widget.clone().and_then(|w| w.downcast::<gtk::Box>().ok()) else { return };
        let kids = nodes[&tb].children.clone();
        let mut sig = String::new();
        for k in &kids {
            let n = &nodes[k];
            sig.push_str(&format!("|{}:{}:{}:{}:{}:{}:{}:{}", n.kind, k, n.label, n.icon, n.tooltip, n.visible, n.checkable, n.group));
        }
        if nodes[&tb].bar_sig == sig {
            return;
        }
        while let Some(c) = bx.first_child() {
            bx.remove(&c);
        }
        for k in &kids {
            let n = &nodes[k];
            if !n.visible {
                continue;
            }
            match n.kind {
                KIND_ACTION => {
                    let radio = if n.group.is_empty() { None } else { nodes[&tb].radio.get(&n.group).map(|(nm, _)| nm.clone()) };
                    let toggle = n.checkable || radio.is_some();
                    let b: gtk::Button = if toggle { gtk::ToggleButton::new().upcast() } else { gtk::Button::new() };
                    if !n.icon.is_empty() {
                        b.set_icon_name(&n.icon);
                    } else {
                        b.set_label(&n.label);
                    }
                    let tip = if !n.tooltip.is_empty() { n.tooltip.clone() } else { n.label.clone() };
                    if !tip.is_empty() {
                        b.set_tooltip_text(Some(&tip));
                    }
                    match radio {
                        Some(name) => {
                            b.set_action_name(Some(&format!("{PREFIX}.{name}")));
                            b.set_action_target_value(Some(&radio_id(*k).to_variant()));
                        }
                        None => b.set_action_name(Some(&format!("{PREFIX}.a{k}"))),
                    }
                    bx.append(&b);
                }
                KIND_MENU => {
                    let b = gtk::MenuButton::new();
                    if !n.icon.is_empty() {
                        b.set_icon_name(&n.icon);
                    } else {
                        b.set_label(&n.label);
                    }
                    b.set_menu_model(Some(&n.model));
                    bx.append(&b);
                }
                KIND_SEPARATOR => bx.append(&gtk::Separator::new(gtk::Orientation::Vertical)),
                _ => {}
            }
        }
        nodes.get_mut(&tb).unwrap().bar_sig = sig;
    }

    fn sync_shortcuts(nodes: &mut HashMap<u64, Node>, root: u64, order: &[u64]) {
        let Some(on) = nodes[&root].ctrl_on.clone() else { return };
        let mut list: Vec<(String, String, Option<String>)> = Vec::new();
        for &h in order {
            let n = &nodes[&h];
            if n.kind != KIND_ACTION || n.shortcut.is_empty() || !n.visible {
                continue;
            }
            let parent = n.parent;
            if n.group.is_empty() {
                list.push((gtk_accel(&n.shortcut), format!("a{h}"), None));
            } else if let Some((name, _)) = nodes.get(&parent).and_then(|p| p.radio.get(&n.group)) {
                list.push((gtk_accel(&n.shortcut), name.clone(), Some(radio_id(h))));
            }
        }
        let sig = format!("{list:?}");
        let r = nodes.get_mut(&root).unwrap();
        if r.ctrl.is_some() && r.ctrl_sig == sig {
            return;
        }
        if let Some(c) = r.ctrl.take() {
            on.remove_controller(&c);
        }
        r.ctrl_sig = sig;
        if list.is_empty() {
            return;
        }
        let ctrl = gtk::ShortcutController::new();
        ctrl.set_scope(gtk::ShortcutScope::Global);
        let group = r.actions.clone();
        for (accel, name, target) in list {
            let Some(trigger) = gtk::ShortcutTrigger::parse_string(&accel) else { continue };
            let g = group.clone();
            let action = gtk::CallbackAction::new(move |_, _| {
                let t = target.as_ref().map(|t| t.to_variant());
                g.activate_action(&name, t.as_ref());
                glib::Propagation::Stop
            });
            ctrl.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
        }
        on.add_controller(ctrl.clone());
        r.ctrl = Some(ctrl);
    }

    // ---- events ----------------------------------------------------------------------------

    fn menu_emit(&self, h: u64, signal: u32, checked: bool) {
        let bit = if signal == S_ACTIVATE { 1 } else { 2 };
        if self.menus.borrow().get(&h).map(|n| n.listened).unwrap_or(0) & bit == 0 {
            return;
        }
        let host = self.host.load(Ordering::Acquire);
        if host.is_null() {
            return;
        }
        let v = svl_value::new(SVL_BOOL, svl_value_u { b: checked });
        unsafe { ((*host).post_event)((*host).self_, h, signal, &v, 1) };
    }

    fn menu_activated(&self, h: u64) {
        let checked = self.menus.borrow().get(&h).map(|n| n.checked).unwrap_or(false);
        self.menu_emit(h, S_ACTIVATE, checked);
    }

    fn menu_toggled(&self, h: u64, v: bool) {
        if let Some(n) = self.menus.borrow_mut().get_mut(&h) {
            n.checked = v;
        }
        self.menu_emit(h, S_TOGGLED, v);
    }

    /// The user picked another member of a radio group.
    fn menu_radio_changed(&self, container: u64, a: &gio::SimpleAction, v: &glib::Variant) {
        let Some(new) = v.get::<String>() else { return };
        let old = a.state().and_then(|s| s.get::<String>()).unwrap_or_default();
        a.set_state(v);
        let parse = |s: &str| s.strip_prefix('a').and_then(|n| n.parse::<u64>().ok());
        let _ = container;
        if let (Some(o), true) = (parse(&old), old != new) {
            self.menu_toggled(o, false);
        }
        if let Some(n) = parse(&new) {
            self.menu_toggled(n, true);
            self.menu_activated(n);
        }
    }

    // ---- members ---------------------------------------------------------------------------

    pub(super) fn menus_set_prop(self: &Rc<Self>, h: u64, prop: u32, v: &svl_value) -> Res<()> {
        let mut resync = false;
        {
            let mut nodes = self.menus.borrow_mut();
            let n = nodes.get_mut(&h).ok_or("stale handle")?;
            match prop {
                P_LABEL => {
                    n.label = str_arg(v)?;
                    resync = true;
                }
                P_ICON => {
                    n.icon = str_arg(v)?;
                    resync = true;
                }
                P_SHORTCUT => {
                    n.shortcut = str_arg(v)?;
                    resync = true;
                }
                P_TOOLTIP => {
                    n.tooltip = str_arg(v)?;
                    resync = true;
                }
                P_VISIBLE => {
                    n.visible = bool_arg(v)?;
                    resync = true;
                }
                P_ENABLED => {
                    n.enabled = bool_arg(v)?;
                    if let Some(a) = &n.action {
                        a.set_enabled(n.enabled);
                    }
                    resync = true;
                }
                P_CHECKABLE => {
                    n.checkable = bool_arg(v)?;
                    n.action = None;
                    resync = true;
                }
                P_GROUP => {
                    n.group = str_arg(v)?;
                    n.action = None;
                    resync = true;
                }
                P_CHECKED => {
                    let b = bool_arg(v)?;
                    n.checked = b;
                    if let Some(a) = &n.action {
                        a.set_state(&b.to_variant());
                    } else if !n.group.is_empty() {
                        let (g, parent) = (n.group.clone(), n.parent);
                        let id = radio_id(h);
                        if let Some((_, a)) = nodes.get(&parent).and_then(|p| p.radio.get(&g)) {
                            let cur = a.state().and_then(|s| s.get::<String>()).unwrap_or_default();
                            if b {
                                a.set_state(&id.to_variant());
                                // The previous member is no longer checked.
                                if let Some(prev) = cur.strip_prefix('a').and_then(|x| x.parse::<u64>().ok()) {
                                    if prev != h {
                                        if let Some(pn) = nodes.get_mut(&prev) {
                                            pn.checked = false;
                                        }
                                    }
                                }
                            } else if cur == id {
                                a.set_state(&"".to_variant());
                            }
                        }
                    }
                }
                _ => return Err("unknown property".into()),
            }
        }
        if resync {
            self.menus_sync(h);
        }
        Ok(())
    }

    pub(super) fn menus_get_prop(&self, h: u64, prop: u32, out: &mut svl_value) -> Res<()> {
        let nodes = self.menus.borrow();
        let n = nodes.get(&h).ok_or("stale handle")?;
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        let b = |x: bool| svl_value::new(SVL_BOOL, svl_value_u { b: x });
        *out = match prop {
            P_LABEL => buf.str_value(&n.label),
            P_ICON => buf.str_value(&n.icon),
            P_SHORTCUT => buf.str_value(&n.shortcut),
            P_TOOLTIP => buf.str_value(&n.tooltip),
            P_VISIBLE => b(n.visible),
            P_ENABLED => b(n.enabled),
            P_CHECKABLE => b(n.checkable),
            P_GROUP => buf.str_value(&n.group),
            P_CHECKED => {
                let live = match &n.action {
                    Some(a) if n.checkable => a.state().and_then(|s| s.get::<bool>()),
                    _ => nodes
                        .get(&n.parent)
                        .and_then(|p| p.radio.get(&n.group))
                        .and_then(|(_, a)| a.state())
                        .and_then(|s| s.get::<String>())
                        .map(|s| s == radio_id(h)),
                };
                b(live.unwrap_or(n.checked))
            }
            _ => return Err("unknown property".into()),
        };
        Ok(())
    }

    pub(super) fn menus_call(&self, h: u64, method: u32, args: &[svl_value]) -> Res<()> {
        let pop = self.menus.borrow().get(&h).and_then(|n| n.popover.clone());
        let Some(pop) = pop else { return Err("the contextmenu is not attached to a widget".into()) };
        match method {
            M_POPUP => {
                let num = |i: usize| -> i32 {
                    args.get(i)
                        .map(|v| match v.tag {
                            SVL_I64 => unsafe { v.u.i as i32 },
                            SVL_F64 => unsafe { v.u.f as i32 },
                            _ => 0,
                        })
                        .unwrap_or(0)
                };
                pop.set_pointing_to(Some(&gtk::gdk::Rectangle::new(num(0), num(1), 1, 1)));
                pop.popup();
            }
            M_POPDOWN => pop.popdown(),
            _ => return Err("unknown method".into()),
        }
        Ok(())
    }

    pub(super) fn menus_listen(&self, h: u64, signal: u32, on: bool, flags: u32) -> Res<()> {
        if flags != 0 {
            return Err("menu actions have no listener phases".into());
        }
        let bit = match signal {
            S_ACTIVATE => 1,
            S_TOGGLED => 2,
            _ => return Err("unknown signal".into()),
        };
        let mut nodes = self.menus.borrow_mut();
        let n = nodes.get_mut(&h).ok_or("stale handle")?;
        n.listened = if on { n.listened | bit } else { n.listened & !bit };
        Ok(())
    }

    /// Test hook: the `GMenu` of a container, as nested item lists of `label` / `section` / `submenu`.
    pub fn menu_dump(&self, h: u64) -> String {
        fn dump(m: &gio::MenuModel, out: &mut String) {
            for i in 0..m.n_items() {
                let label = m.item_attribute_value(i, "label", None).and_then(|v| v.get::<String>()).unwrap_or_default();
                let action = m.item_attribute_value(i, "action", None).and_then(|v| v.get::<String>()).unwrap_or_default();
                if let Some(sec) = m.item_link(i, "section") {
                    out.push('{');
                    dump(&sec, out);
                    out.push('}');
                } else if let Some(sub) = m.item_link(i, "submenu") {
                    out.push_str(&format!("{label}["));
                    dump(&sub, out);
                    out.push_str("],");
                } else {
                    let _ = action;
                    out.push_str(&label);
                    out.push(',');
                }
            }
        }
        let model = self.menus.borrow().get(&h).map(|n| n.model.clone());
        let mut s = String::new();
        if let Some(m) = model {
            dump(m.upcast_ref(), &mut s);
        }
        s
    }

    /// Test hook: the name of the action behind an action node, in its root group.
    pub fn menu_action_name(&self, h: u64) -> Option<String> {
        let nodes = self.menus.borrow();
        let n = nodes.get(&h)?;
        if n.group.is_empty() {
            Some(format!("{PREFIX}.a{h}"))
        } else {
            nodes.get(&n.parent)?.radio.get(&n.group).map(|(nm, _)| format!("{PREFIX}.{nm}"))
        }
    }

    /// Test hook: the first node of kind `kind` (`action`, `menu`, `menubar`, ...) with this label.
    pub fn menu_find(&self, kind: &str, label: &str) -> Option<u64> {
        let k = kind_by_name(kind);
        let nodes = self.menus.borrow();
        let mut hs: Vec<u64> = nodes.iter().filter(|(_, n)| n.kind == k && (n.label == label || !matches!(k, KIND_ACTION | KIND_MENU))).map(|(h, _)| *h).collect();
        hs.sort_unstable();
        hs.first().copied()
    }

    /// Test hook: the root's action group.
    pub fn menu_group(&self, h: u64) -> Option<gio::SimpleActionGroup> {
        let r = self.menu_root(h);
        self.menus.borrow().get(&r).map(|n| n.actions.clone())
    }
}
