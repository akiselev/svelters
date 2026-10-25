//! The menu family's data and navigation, without geometry: the entries of a menu bar or context
//! menu ([`MenuModel`]: submenus, items with check or radio state, separators, group labels,
//! shortcuts, `&` mnemonics), the pick logic (a check item toggles, a radio item checks itself and
//! unchecks its group, [`MenuModel::pick`]), and the open-menu state machine ([`MenuNav`]): which
//! levels are open, the current row of each, the menu bar's keyboard mode, and the keys (arrows,
//! Home/End, Enter/Space, Escape one level at a time, Right/Left into and out of submenus and across
//! the bar, type-ahead, mnemonics, F10 and a lone Alt). Ported from svelters-egui `menus.rs`
//! (model and pick), `widgets/menu.rs` (keys) and `interact/{roving_list,typeahead}.rs`.
//!
//! Disabled rows: reachable by the arrow keys but never picked (svelters' R9, ARIA APG menus) unless
//! [`MenuNav::disabled_focusable`] is off, which skips them as Qt and GTK do.

use skelter_base::event::{KeyState, Modifiers};
use skelter_base::key::Key;
use crate::shim::core::Value;
use skelter_base::time::{Span, Time};

/// An entry's index in its [`MenuModel`].
pub type EntryIx = usize;

/// What an entry is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// A container: the root, a bar title or a submenu row.
    Menu,
    /// A pickable row (`menu-item`).
    Item,
    /// A line between rows.
    Separator,
    /// A heading over a group of rows (not pickable, not reachable).
    GroupLabel,
}

/// A menu item's check behaviour.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Check {
    #[default]
    None,
    /// Picking it toggles `checked`.
    Check,
    /// One of the parent's items with this group: picking it checks it and unchecks the others.
    Radio(String),
}

/// One entry of a menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The author id signals carry (`""`: none).
    pub id: String,
    pub kind: EntryKind,
    /// The caption with the mnemonic marker removed.
    pub label: String,
    /// The letter after a single `&` in the source label, lower-cased.
    pub mnemonic: Option<char>,
    /// Where the mnemonic letter is in `label` (byte offset), for the underline.
    pub mnemonic_at: Option<usize>,
    /// The accelerator as authored (`Ctrl+Shift+S`, `Primary+O`); see [`shortcut_text`].
    pub shortcut: String,
    pub enabled: bool,
    pub visible: bool,
    pub check: Check,
    pub checked: bool,
    pub children: Vec<EntryIx>,
    pub parent: Option<EntryIx>,
}

impl Entry {
    fn new(kind: EntryKind, id: &str, label: &str) -> Entry {
        let (label, mnemonic, mnemonic_at) = parse_mnemonic(label);
        Entry {
            id: id.to_string(),
            kind,
            label,
            mnemonic,
            mnemonic_at,
            shortcut: String::new(),
            enabled: true,
            visible: true,
            check: Check::None,
            checked: false,
            children: Vec::new(),
            parent: None,
        }
    }
    /// A submenu (or a bar title).
    pub fn menu(id: &str, label: &str) -> Entry {
        Entry::new(EntryKind::Menu, id, label)
    }
    pub fn item(id: &str, label: &str) -> Entry {
        Entry::new(EntryKind::Item, id, label)
    }
    pub fn separator() -> Entry {
        Entry::new(EntryKind::Separator, "", "")
    }
    pub fn group_label(label: &str) -> Entry {
        Entry::new(EntryKind::GroupLabel, "", label)
    }
    pub fn shortcut(mut self, s: &str) -> Entry {
        self.shortcut = s.to_string();
        self
    }
    pub fn checkable(mut self, checked: bool) -> Entry {
        self.check = Check::Check;
        self.checked = checked;
        self
    }
    pub fn radio(mut self, group: &str, checked: bool) -> Entry {
        self.check = Check::Radio(group.to_string());
        self.checked = checked;
        self
    }
    pub fn disabled(mut self) -> Entry {
        self.enabled = false;
        self
    }
    pub fn hidden(mut self) -> Entry {
        self.visible = false;
        self
    }
    /// Sets the caption, parsing its mnemonic.
    pub fn set_label(&mut self, label: &str) {
        let (l, m, at) = parse_mnemonic(label);
        self.label = l;
        self.mnemonic = m;
        self.mnemonic_at = at;
    }
    /// A row the arrow keys can reach (items and submenus).
    pub fn is_row(&self) -> bool {
        matches!(self.kind, EntryKind::Item | EntryKind::Menu)
    }
}

/// `"&File"` → (`"File"`, `f`, 0); `"&&"` is a literal `&`; a trailing `&` is kept.
pub fn parse_mnemonic(src: &str) -> (String, Option<char>, Option<usize>) {
    let mut out = String::with_capacity(src.len());
    let mut mnemonic = None;
    let mut at = None;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.peek() {
                Some('&') => {
                    chars.next();
                    out.push('&');
                }
                Some(&n) if mnemonic.is_none() && !n.is_whitespace() => {
                    mnemonic = n.to_lowercase().next();
                    at = Some(out.len());
                }
                Some(_) => {}
                None => out.push('&'),
            }
        } else {
            out.push(c);
        }
    }
    (out, mnemonic, at)
}

/// The accelerator as the platform shows it: `Primary` is Cmd on macOS and Ctrl elsewhere; macOS
/// uses the modifier symbols without separators (`⇧⌘S`), others `Ctrl+Shift+S`.
pub fn shortcut_text(s: &str, macos: bool) -> String {
    if s.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = s.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    if macos {
        // the order macOS menus show: Control, Option, Shift, Command
        let mut mods = [false; 4];
        let mut key = String::new();
        for p in &parts {
            match *p {
                "Ctrl" => mods[0] = true,
                "Alt" => mods[1] = true,
                "Shift" => mods[2] = true,
                "Cmd" | "Primary" => mods[3] = true,
                k => key = k.to_string(),
            }
        }
        let syms = ["⌃", "⌥", "⇧", "⌘"];
        let mut out: String = (0..4).filter(|&i| mods[i]).map(|i| syms[i]).collect();
        out.push_str(&key);
        out
    } else {
        parts.iter().map(|p| if *p == "Primary" { "Ctrl" } else { p }).collect::<Vec<_>>().join("+")
    }
}

/// What a pick changed.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Picked {
    /// The picked item's id and its `checked` after the pick.
    pub activated: Option<(String, bool)>,
    /// Every item whose `checked` changed, with its new state (the picked one last).
    pub toggled: Vec<(String, bool)>,
}

/// The entries of one menu bar or context menu. Entry 0 is the root container.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuModel {
    pub entries: Vec<Entry>,
}

impl Default for MenuModel {
    fn default() -> Self {
        MenuModel::new()
    }
}

impl MenuModel {
    pub const ROOT: EntryIx = 0;

    pub fn new() -> MenuModel {
        MenuModel { entries: vec![Entry::menu("", "")] }
    }

    /// The model a `menu-bar` or `context-menu` holds as its element children (`menu`, `menu-item`,
    /// `menu-separator`), from the element records the host folds in ([`crate::shim::core::Kind::folds_elements`]).
    /// An entry's id is its `anchor`, else its position path (`"1.0.2"`), so ids stay put while the
    /// structure does. Other tags are skipped.
    pub fn from_elements(els: &[Value]) -> MenuModel {
        let mut m = MenuModel::new();
        m.add_elements(MenuModel::ROOT, els, "");
        m
    }

    fn add_elements(&mut self, parent: EntryIx, els: &[Value], path: &str) {
        fn field<'v>(v: &'v Value, name: &str) -> Option<&'v Value> {
            match v {
                Value::Record(fs) => fs.iter().find(|(k, _)| &**k == name).map(|(_, v)| v),
                _ => None,
            }
        }
        for (i, el) in els.iter().enumerate() {
            let tag = field(el, "tag").and_then(Value::as_str).unwrap_or("");
            let props = field(el, "props");
            let prop = |n: &str| props.and_then(|p| field(p, n));
            let text = |n: &str| prop(n).and_then(Value::as_str).unwrap_or("").to_string();
            let flag = |n: &str, d: bool| prop(n).and_then(Value::as_bool).unwrap_or(d);
            let here = if path.is_empty() { i.to_string() } else { format!("{path}.{i}") };
            let anchor = field(el, "anchor").and_then(Value::as_str).unwrap_or("");
            let id = if anchor.is_empty() { here.clone() } else { anchor.to_string() };
            let mut e = match tag {
                "menu" => Entry::menu(&id, &text("label")),
                "menu-item" => {
                    let mut e = Entry::item(&id, &text("label")).shortcut(&text("shortcut"));
                    let group = text("group");
                    if !group.is_empty() {
                        e = e.radio(&group, flag("checked", false));
                    } else if flag("checkable", false) {
                        e = e.checkable(flag("checked", false));
                    }
                    e
                }
                "menu-separator" => Entry::separator(),
                _ => continue,
            };
            e.enabled = flag("enabled", true);
            e.visible = flag("visible", true);
            let ix = self.add(parent, e);
            if tag == "menu"
                && let Some(Value::List(kids)) = field(el, "children")
            {
                self.add_elements(ix, kids, &here);
            }
        }
    }

    /// Appends `e` to `parent`'s children.
    pub fn add(&mut self, parent: EntryIx, mut e: Entry) -> EntryIx {
        let ix = self.entries.len();
        e.parent = Some(parent);
        self.entries.push(e);
        self.entries[parent].children.push(ix);
        ix
    }

    pub fn get(&self, ix: EntryIx) -> &Entry {
        &self.entries[ix]
    }
    pub fn get_mut(&mut self, ix: EntryIx) -> &mut Entry {
        &mut self.entries[ix]
    }

    /// The entry with author id `id`.
    pub fn find(&self, id: &str) -> Option<EntryIx> {
        if id.is_empty() {
            return None;
        }
        self.entries.iter().position(|e| e.id == id)
    }

    /// The visible children of `container`, in order (rows, separators and labels).
    pub fn rows(&self, container: EntryIx) -> Vec<EntryIx> {
        self.entries[container].children.iter().copied().filter(|&c| self.entries[c].visible).collect()
    }

    /// Whether a row with `ix` can be picked or opened.
    pub fn enabled(&self, ix: EntryIx) -> bool {
        let e = &self.entries[ix];
        e.enabled && e.is_row() && (e.kind != EntryKind::Menu || !self.rows(ix).is_empty())
    }

    /// The native menu bar's description of this model (skelter-platform's `MenuDesc`): the root's
    /// visible submenus as the bar's menus. Items keep their authored shortcut and report their id;
    /// check and radio items carry their state; a group label is a disabled item with no id; hidden
    /// entries are left out. Labels are without mnemonic markers.
    #[cfg(feature = "native-menu")]
    pub fn to_menu_desc(&self) -> skelter_platform::MenuDesc {
        skelter_platform::MenuDesc { menus: self.rows(MenuModel::ROOT).into_iter().map(|ix| self.menu_node(ix)).collect() }
    }

    #[cfg(feature = "native-menu")]
    fn menu_node(&self, ix: EntryIx) -> skelter_platform::MenuNode {
        use skelter_platform::MenuNode;
        let e = &self.entries[ix];
        match e.kind {
            EntryKind::Menu => MenuNode::Submenu { label: e.label.clone(), enabled: e.enabled, items: self.rows(ix).into_iter().map(|c| self.menu_node(c)).collect() },
            EntryKind::Item => MenuNode::Item {
                id: e.id.clone(),
                label: e.label.clone(),
                shortcut: e.shortcut.clone(),
                enabled: e.enabled,
                checked: (e.check != Check::None).then_some(e.checked),
            },
            EntryKind::Separator => MenuNode::Separator,
            EntryKind::GroupLabel => MenuNode::Item { id: String::new(), label: e.label.clone(), shortcut: String::new(), enabled: false, checked: None },
        }
    }

    /// Picks item `ix`: a check item toggles, a radio item checks itself and unchecks the visible
    /// and hidden items of its group under the same parent. A disabled item or a submenu is not
    /// picked (`None`).
    pub fn pick(&mut self, ix: EntryIx) -> Option<Picked> {
        if self.entries[ix].kind != EntryKind::Item || !self.entries[ix].enabled {
            return None;
        }
        let mut out = Picked::default();
        let was = self.entries[ix].checked;
        let now = match self.entries[ix].check.clone() {
            Check::None => was,
            Check::Check => !was,
            Check::Radio(group) => {
                if let Some(p) = self.entries[ix].parent {
                    for c in self.entries[p].children.clone() {
                        let other = &mut self.entries[c];
                        if c != ix && other.checked && other.check == Check::Radio(group.clone()) {
                            other.checked = false;
                            out.toggled.push((other.id.clone(), false));
                        }
                    }
                }
                true
            }
        };
        if now != was {
            self.entries[ix].checked = now;
            out.toggled.push((self.entries[ix].id.clone(), now));
        }
        out.activated = Some((self.entries[ix].id.clone(), now));
        Some(out)
    }
}

// ---- roving and type-ahead (interact::roving_list and interact::typeahead semantics) ----

/// The next index from `from` by `step` among `len` items for which `ok(i)`, wrapping when `wrap`
/// (without wrapping it stays at `from`); from nothing, the first (`step > 0`) or last such item.
pub(crate) fn next_by(len: usize, from: Option<usize>, step: i32, wrap: bool, ok: impl Fn(usize) -> bool) -> Option<usize> {
    let n = len as i64;
    let Some(from) = from.filter(|f| (*f as i64) < n) else {
        return if step > 0 { (0..len).find(|&i| ok(i)) } else { (0..len).rev().find(|&i| ok(i)) };
    };
    let mut i = from as i64;
    for _ in 0..n {
        i += step as i64;
        if i < 0 || i >= n {
            if !wrap {
                return Some(from);
            }
            i = i.rem_euclid(n);
        }
        if ok(i as usize) {
            return Some(i as usize);
        }
    }
    Some(from)
}

/// Seconds after the last typed key before a new type-ahead prefix starts.
pub(crate) const TYPEAHEAD_TIMEOUT: Span = Span::from_millis(1000);

/// Printable keys typed within [`TYPEAHEAD_TIMEOUT`] build a prefix.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Typeahead {
    buf: String,
    last: Time,
}

impl Typeahead {
    /// Adds `text` typed at `now`; returns the prefix to search.
    pub(crate) fn push(&mut self, text: &str, now: Time) -> Option<String> {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if text.is_empty() || (self.buf.is_empty() && text.trim().is_empty()) {
            return None;
        }
        if now - self.last > TYPEAHEAD_TIMEOUT {
            self.buf.clear();
        }
        self.last = now;
        self.buf.push_str(&text);
        Some(self.buf.clone())
    }
    pub(crate) fn reset(&mut self) {
        self.buf.clear();
    }
    /// A prefix is being typed (a Space belongs to it, not to activation).
    pub(crate) fn typing(&self, now: Time) -> bool {
        !self.buf.is_empty() && now - self.last <= TYPEAHEAD_TIMEOUT
    }
}

/// The item for prefix `query` among `labels` (case-insensitive) searching from `current`: the
/// current item while it still matches a longer prefix, else the next match, wrapping. One letter
/// (or one letter repeated) cycles through the items starting with it. `ok(i)` skips items.
pub(crate) fn typeahead_find(labels: &[&str], ok: impl Fn(usize) -> bool, query: &str, current: Option<usize>) -> Option<usize> {
    let labels: Vec<String> = labels.iter().map(|l| l.trim_start().to_lowercase()).collect();
    let q = query.to_lowercase();
    let mut chars = q.chars();
    let first = chars.next()?;
    let one_letter = chars.all(|c| c == first);
    let n = labels.len();
    if n == 0 {
        return None;
    }
    let (needle, start) = if one_letter { (first.to_string(), current.map_or(0, |c| c + 1)) } else { (q.clone(), current.unwrap_or(0)) };
    (0..n).map(|k| (start + k) % n).find(|&i| ok(i) && labels[i].starts_with(&needle))
}

// ---- the open menus ----

/// One open menu surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    /// The container whose rows it shows.
    pub container: EntryIx,
    /// The current row (keyboard or pointer).
    pub current: Option<EntryIx>,
}

/// What navigation reports, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuOut {
    /// A level showing `container` opened.
    Opened(EntryIx),
    Closed(EntryIx),
    /// A row became current (`hover` signal).
    Current(EntryIx),
    /// An item was picked.
    Picked(Picked),
    /// The bar entered or left keyboard mode (a title highlighted, no menu open).
    BarKeys(bool),
}

/// What a key event was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyIn<'a> {
    pub key: Option<Key>,
    pub text: Option<&'a str>,
    pub state: KeyState,
    pub modifiers: Modifiers,
    pub now: Time,
}

/// The open state of one menu bar or context menu.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuNav {
    /// Open levels, outermost first.
    pub levels: Vec<Level>,
    /// The menu bar's current title (a bar only).
    pub bar_current: Option<EntryIx>,
    /// The bar highlights `bar_current` for the keyboard with no menu open (F10, Alt, Escape).
    pub bar_keys: bool,
    /// The last change came from the keyboard (draws `:focus-visible` rows).
    pub keyboard: bool,
    /// Disabled rows are reachable by the arrow keys (R9).
    pub disabled_focusable: bool,
    /// A lone Alt is down (no other key since).
    alt_alone: bool,
    typeahead: Typeahead,
}

impl Default for MenuNav {
    fn default() -> Self {
        MenuNav {
            levels: Vec::new(),
            bar_current: None,
            bar_keys: false,
            keyboard: false,
            disabled_focusable: true,
            alt_alone: false,
            typeahead: Typeahead::default(),
        }
    }
}

impl MenuNav {
    pub fn is_open(&self) -> bool {
        !self.levels.is_empty()
    }

    /// The deepest level's current row.
    pub fn current(&self) -> Option<EntryIx> {
        self.levels.last().and_then(|l| l.current)
    }

    /// The container open at level `k + 1` (the submenu of a row at level `k`).
    pub fn open_below(&self, k: usize) -> Option<EntryIx> {
        self.levels.get(k + 1).map(|l| l.container)
    }

    fn reachable(&self, m: &MenuModel, ix: EntryIx) -> bool {
        let e = m.get(ix);
        e.is_row() && (self.disabled_focusable || m.enabled(ix))
    }

    /// The first reachable row of `container`.
    fn first(&self, m: &MenuModel, container: EntryIx) -> Option<EntryIx> {
        m.rows(container).into_iter().find(|&r| self.reachable(m, r))
    }

    /// Opens bar title `title` (a bar's level 0); `keyboard`: its first row current.
    pub fn open_title(&mut self, m: &MenuModel, title: EntryIx, keyboard: bool, out: &mut Vec<MenuOut>) {
        if self.levels.first().is_some_and(|l| l.container == title) {
            return;
        }
        self.truncate(0, out);
        self.bar_current = Some(title);
        self.keyboard = keyboard;
        let current = if keyboard { self.first(m, title) } else { None };
        self.levels.push(Level { container: title, current });
        out.push(MenuOut::Opened(title));
        if let Some(c) = current {
            out.push(MenuOut::Current(c));
        }
    }

    /// Opens the root as a context menu (level 0); `keyboard`: its first row current.
    pub fn open_root(&mut self, m: &MenuModel, keyboard: bool, out: &mut Vec<MenuOut>) {
        self.truncate(0, out);
        self.keyboard = keyboard;
        let current = if keyboard { self.first(m, MenuModel::ROOT) } else { None };
        self.levels.push(Level { container: MenuModel::ROOT, current });
        out.push(MenuOut::Opened(MenuModel::ROOT));
        if let Some(c) = current {
            out.push(MenuOut::Current(c));
        }
    }

    /// Opens submenu `row` of level `k` (closing whatever is open below `k`).
    pub fn open_sub(&mut self, m: &MenuModel, k: usize, row: EntryIx, keyboard: bool, out: &mut Vec<MenuOut>) {
        if !m.enabled(row) || m.get(row).kind != EntryKind::Menu || k >= self.levels.len() {
            return;
        }
        self.set_current(k, Some(row), out);
        if self.open_below(k) == Some(row) {
            if keyboard && self.levels[k + 1].current.is_none() {
                let c = self.first(m, row);
                self.set_current(k + 1, c, out);
            }
            return;
        }
        self.truncate(k + 1, out);
        let current = if keyboard { self.first(m, row) } else { None };
        self.levels.push(Level { container: row, current });
        out.push(MenuOut::Opened(row));
        if let Some(c) = current {
            out.push(MenuOut::Current(c));
        }
    }

    /// Closes the levels from `n` on (innermost first).
    pub fn truncate(&mut self, n: usize, out: &mut Vec<MenuOut>) {
        while self.levels.len() > n {
            let l = self.levels.pop().expect("non-empty");
            out.push(MenuOut::Closed(l.container));
        }
        if n == 0 {
            self.typeahead.reset();
        }
    }

    /// Closes every level and leaves the bar's keyboard mode.
    pub fn close_all(&mut self, out: &mut Vec<MenuOut>) {
        self.truncate(0, out);
        self.set_bar_keys(false, out);
    }

    pub fn set_current(&mut self, k: usize, row: Option<EntryIx>, out: &mut Vec<MenuOut>) {
        if let Some(l) = self.levels.get_mut(k)
            && l.current != row
        {
            l.current = row;
            if let Some(r) = row {
                out.push(MenuOut::Current(r));
            }
        }
    }

    fn set_bar_keys(&mut self, on: bool, out: &mut Vec<MenuOut>) {
        if self.bar_keys != on {
            self.bar_keys = on;
            out.push(MenuOut::BarKeys(on));
        }
        if !on && !self.is_open() {
            self.bar_current = None;
        }
    }

    /// Picks `row` (an enabled item): applies check and radio state and closes every level.
    pub fn pick(&mut self, m: &mut MenuModel, row: EntryIx, out: &mut Vec<MenuOut>) -> bool {
        let Some(p) = m.pick(row) else { return false };
        self.close_all(out);
        out.push(MenuOut::Picked(p));
        true
    }

    /// The bar's titles that open a menu.
    fn titles(m: &MenuModel) -> Vec<EntryIx> {
        m.rows(MenuModel::ROOT).into_iter().filter(|&t| m.get(t).kind == EntryKind::Menu).collect()
    }

    /// The bar's next (`step` 1) or previous title; opens it when a menu is open.
    pub fn bar_step(&mut self, m: &MenuModel, step: i32, out: &mut Vec<MenuOut>) {
        let titles = Self::titles(m);
        let from = self.bar_current.and_then(|c| titles.iter().position(|&t| t == c));
        let Some(to) = next_by(titles.len(), from, step, true, |i| m.enabled(titles[i])) else { return };
        let title = titles[to];
        if self.is_open() {
            self.open_title(m, title, true, out);
        } else {
            self.bar_current = Some(title);
        }
    }

    /// One key. `bar`: this is a menu bar (titles at the root). Returns whether it was used.
    pub fn key(&mut self, m: &mut MenuModel, bar: bool, policy_alt: bool, k: KeyIn<'_>, out: &mut Vec<MenuOut>) -> bool {
        let alt_key = matches!(k.key, Some(Key::AltLeft | Key::AltRight));
        if k.state == KeyState::Released {
            // a lone Alt press and release toggles the bar's keyboard mode
            if alt_key && std::mem::take(&mut self.alt_alone) && bar && policy_alt {
                if self.is_open() || self.bar_keys {
                    self.close_all(out);
                } else {
                    self.enter_bar_keys(m, out);
                }
                return true;
            }
            return false;
        }
        if alt_key {
            self.alt_alone = !k.modifiers.contains(Modifiers::CTRL) && !k.modifiers.contains(Modifiers::SHIFT);
            return false;
        }
        self.alt_alone = false;
        let mods = k.modifiers;
        let plain = !mods.contains(Modifiers::CTRL) && !mods.contains(Modifiers::ALT) && !mods.contains(Modifiers::SUPER);
        if self.is_open() {
            let used = self.key_open(m, bar, k, plain, out);
            if used {
                self.keyboard = true;
            }
            return used;
        }
        if !bar {
            return false;
        }
        if self.bar_keys {
            return self.key_bar(m, k, plain, out);
        }
        match k.key {
            Some(Key::F10) if !mods.contains(Modifiers::SHIFT) && plain => {
                self.enter_bar_keys(m, out);
                true
            }
            // Alt+letter opens the title with that mnemonic
            _ if mods.contains(Modifiers::ALT) && !mods.contains(Modifiers::CTRL) => {
                let Some(c) = mnemonic_char(&k) else { return false };
                let Some(t) = Self::titles(m).into_iter().find(|&t| m.get(t).mnemonic == Some(c) && m.enabled(t)) else {
                    return false;
                };
                self.open_title(m, t, true, out);
                true
            }
            _ => false,
        }
    }

    fn enter_bar_keys(&mut self, m: &MenuModel, out: &mut Vec<MenuOut>) {
        let titles = Self::titles(m);
        if let Some(t) = titles.iter().copied().find(|&t| m.enabled(t)) {
            self.bar_current = Some(t);
            self.set_bar_keys(true, out);
            self.keyboard = true;
        }
    }

    /// Keys in the bar's keyboard mode (a title highlighted, nothing open).
    fn key_bar(&mut self, m: &mut MenuModel, k: KeyIn<'_>, plain: bool, out: &mut Vec<MenuOut>) -> bool {
        match k.key {
            Some(Key::ArrowRight) if plain => self.bar_step(m, 1, out),
            Some(Key::ArrowLeft) if plain => self.bar_step(m, -1, out),
            Some(Key::ArrowDown | Key::ArrowUp | Key::Enter | Key::Space) if plain => {
                if let Some(t) = self.bar_current {
                    self.open_title(m, t, true, out);
                }
            }
            Some(Key::Escape | Key::F10) => self.close_all(out),
            Some(Key::Tab) => {}
            _ => {
                let Some(c) = mnemonic_char(&k) else { return false };
                if let Some(t) = Self::titles(m).into_iter().find(|&t| m.get(t).mnemonic == Some(c) && m.enabled(t)) {
                    self.open_title(m, t, true, out);
                }
            }
        }
        true
    }

    /// Keys while a menu is open: the deepest level has the keyboard.
    fn key_open(&mut self, m: &mut MenuModel, bar: bool, k: KeyIn<'_>, plain: bool, out: &mut Vec<MenuOut>) -> bool {
        let d = self.levels.len() - 1;
        let rows = m.rows(self.levels[d].container);
        let cur = self.levels[d].current;
        let pos = cur.and_then(|c| rows.iter().position(|&r| r == c));
        let typing = self.typeahead.typing(k.now);
        let moved = {
            let ok = |i: usize| self.reachable(m, rows[i]);
            match k.key {
                Some(Key::ArrowDown) if plain => Some(next_by(rows.len(), pos, 1, true, ok)),
                Some(Key::ArrowUp) if plain => Some(next_by(rows.len(), pos, -1, true, ok)),
                Some(Key::Home | Key::PageUp) if plain => Some(next_by(rows.len(), None, 1, false, ok)),
                Some(Key::End | Key::PageDown) if plain => Some(next_by(rows.len(), None, -1, false, ok)),
                _ => None,
            }
        };
        if let Some(to) = moved {
            if let Some(to) = to {
                self.set_current(d, Some(rows[to]), out);
                self.truncate(d + 1, out);
            }
            return true;
        }
        match k.key {
            Some(Key::ArrowRight) if plain => match cur {
                Some(r) if m.get(r).kind == EntryKind::Menu && m.enabled(r) => self.open_sub(m, d, r, true, out),
                _ if bar => self.bar_step(m, 1, out),
                _ => {}
            },
            Some(Key::ArrowLeft) if plain => {
                if d > 0 {
                    self.truncate(d, out);
                } else if bar {
                    self.bar_step(m, -1, out);
                }
            }
            Some(Key::Space) if typing => return self.text(m, d, " ", k.now, out),
            Some(Key::Enter | Key::Space) if plain => match cur {
                Some(r) if m.get(r).kind == EntryKind::Menu => self.open_sub(m, d, r, true, out),
                Some(r) => {
                    self.pick(m, r, out);
                }
                None => {}
            },
            Some(Key::Escape) => {
                if d > 0 {
                    self.truncate(d, out);
                } else {
                    self.truncate(0, out);
                    // a bar keeps its title highlighted for the keyboard (Windows, GTK, Qt)
                    if bar && self.bar_current.is_some() {
                        self.set_bar_keys(true, out);
                    } else {
                        self.close_all(out);
                    }
                }
            }
            Some(Key::F10) if bar => self.close_all(out),
            Some(Key::Tab) => {}
            _ => {
                // a modifier alone, or a chord the menu does not know: not used, the menu stays open
                let Some(t) = k.text.filter(|_| plain || !k.modifiers.contains(Modifiers::CTRL)) else { return false };
                return self.text(m, d, t, k.now, out);
            }
        }
        true
    }

    /// Typed text at level `d`: a mnemonic unique among the level's rows picks or opens its row,
    /// otherwise type-ahead moves the current row.
    fn text(&mut self, m: &mut MenuModel, d: usize, t: &str, now: Time, out: &mut Vec<MenuOut>) -> bool {
        let rows = m.rows(self.levels[d].container);
        if !self.typeahead.typing(now) {
            let mut chars = t.chars();
            if let (Some(c), None) = (chars.next().and_then(|c| c.to_lowercase().next()), chars.next()) {
                let hits: Vec<EntryIx> = rows.iter().copied().filter(|&r| m.get(r).mnemonic == Some(c) && m.enabled(r)).collect();
                if let [r] = hits[..] {
                    if m.get(r).kind == EntryKind::Menu {
                        self.open_sub(m, d, r, true, out);
                    } else {
                        self.pick(m, r, out);
                    }
                    return true;
                }
            }
        }
        let Some(q) = self.typeahead.push(t, now) else { return false };
        let labels: Vec<&str> = rows.iter().map(|&r| m.get(r).label.as_str()).collect();
        let pos = self.levels[d].current.and_then(|c| rows.iter().position(|&r| r == c));
        if let Some(to) = typeahead_find(&labels, |i| self.reachable(m, rows[i]), &q, pos) {
            self.set_current(d, Some(rows[to]), out);
            self.truncate(d + 1, out);
        }
        true
    }
}

/// The lower-case letter a key event types, for mnemonics (text first, else a letter key's name).
fn mnemonic_char(k: &KeyIn<'_>) -> Option<char> {
    if let Some(t) = k.text {
        let mut cs = t.chars();
        if let (Some(c), None) = (cs.next(), cs.next())
            && c.is_alphanumeric()
        {
            return c.to_lowercase().next();
        }
    }
    let name = k.key?.name();
    let mut cs = name.chars();
    match (cs.next(), cs.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(ms: u64) -> Time {
        Time(ms * 1_000_000)
    }

    fn key(k: Key) -> KeyIn<'static> {
        KeyIn { key: Some(k), text: None, state: KeyState::Pressed, modifiers: Modifiers::default(), now: t(0) }
    }

    fn typed(s: &'static str, ms: u64) -> KeyIn<'static> {
        KeyIn { key: None, text: Some(s), state: KeyState::Pressed, modifiers: Modifiers::default(), now: t(ms) }
    }

    /// File: New, (sep), Open (disabled), Recent > [a, b], Save; Edit: Undo, Redo; View: Wrap (check),
    /// Small/Large (radio).
    fn bar() -> (MenuModel, [EntryIx; 3]) {
        let mut m = MenuModel::new();
        let file = m.add(MenuModel::ROOT, Entry::menu("file", "&File"));
        m.add(file, Entry::item("new", "&New").shortcut("Primary+N"));
        m.add(file, Entry::separator());
        m.add(file, Entry::item("open", "&Open").disabled());
        let recent = m.add(file, Entry::menu("recent", "&Recent"));
        m.add(recent, Entry::item("a", "alpha"));
        m.add(recent, Entry::item("b", "beta"));
        m.add(file, Entry::item("save", "&Save"));
        let edit = m.add(MenuModel::ROOT, Entry::menu("edit", "&Edit"));
        m.add(edit, Entry::item("undo", "&Undo"));
        m.add(edit, Entry::item("redo", "&Redo"));
        let view = m.add(MenuModel::ROOT, Entry::menu("view", "&View"));
        m.add(view, Entry::item("wrap", "&Wrap").checkable(false));
        m.add(view, Entry::item("small", "S&mall").radio("size", true));
        m.add(view, Entry::item("large", "&Large").radio("size", false));
        (m, [file, edit, view])
    }

    fn id(m: &MenuModel, ix: Option<EntryIx>) -> &str {
        ix.map_or("", |i| m.get(i).id.as_str())
    }

    #[test]
    fn mnemonics_parse() {
        assert_eq!(parse_mnemonic("&File"), ("File".into(), Some('f'), Some(0)));
        assert_eq!(parse_mnemonic("Save &As…"), ("Save As…".into(), Some('a'), Some(5)));
        assert_eq!(parse_mnemonic("Fish && Chips"), ("Fish & Chips".into(), None, None));
        assert_eq!(parse_mnemonic("Tail&"), ("Tail&".into(), None, None));
    }

    #[test]
    fn shortcuts_show_per_platform() {
        assert_eq!(shortcut_text("Primary+Shift+S", false), "Ctrl+Shift+S");
        assert_eq!(shortcut_text("Primary+Shift+S", true), "⇧⌘S");
        assert_eq!(shortcut_text("Ctrl+Alt+F5", true), "⌃⌥F5");
        assert_eq!(shortcut_text("", true), "");
    }

    #[test]
    fn arrows_skip_separators_reach_disabled_and_wrap() {
        let (mut m, [file, ..]) = bar();
        let mut nav = MenuNav::default();
        let mut out = Vec::new();
        nav.open_title(&m, file, true, &mut out);
        assert_eq!(id(&m, nav.current()), "new");
        nav.key(&mut m, true, false, key(Key::ArrowDown), &mut out);
        assert_eq!(id(&m, nav.current()), "open", "the separator is skipped; disabled is reachable (R9)");
        // Enter on a disabled row does nothing
        nav.key(&mut m, true, false, key(Key::Enter), &mut out);
        assert!(nav.is_open());
        nav.key(&mut m, true, false, key(Key::ArrowDown), &mut out);
        nav.key(&mut m, true, false, key(Key::ArrowDown), &mut out);
        assert_eq!(id(&m, nav.current()), "save");
        nav.key(&mut m, true, false, key(Key::ArrowDown), &mut out);
        assert_eq!(id(&m, nav.current()), "new", "wraps");
        nav.key(&mut m, true, false, key(Key::ArrowUp), &mut out);
        assert_eq!(id(&m, nav.current()), "save", "wraps backwards");
        nav.key(&mut m, true, false, key(Key::Home), &mut out);
        assert_eq!(id(&m, nav.current()), "new");
        nav.key(&mut m, true, false, key(Key::End), &mut out);
        assert_eq!(id(&m, nav.current()), "save");
    }

    #[test]
    fn disabled_rows_are_skipped_when_not_focusable() {
        let (mut m, [file, ..]) = bar();
        let mut nav = MenuNav { disabled_focusable: false, ..MenuNav::default() };
        let mut out = Vec::new();
        nav.open_title(&m, file, true, &mut out);
        nav.key(&mut m, true, false, key(Key::ArrowDown), &mut out);
        assert_eq!(id(&m, nav.current()), "recent");
    }

    #[test]
    fn right_and_left_open_and_close_submenus_and_cross_the_bar() {
        let (mut m, [file, edit, view]) = bar();
        let recent = m.find("recent").unwrap();
        let mut nav = MenuNav::default();
        let mut out = Vec::new();
        nav.open_title(&m, file, true, &mut out);
        // on a plain row, Right goes to the next bar menu
        nav.key(&mut m, true, false, key(Key::ArrowRight), &mut out);
        assert_eq!(nav.levels[0].container, edit);
        assert_eq!(id(&m, nav.current()), "undo");
        nav.key(&mut m, true, false, key(Key::ArrowLeft), &mut out);
        nav.key(&mut m, true, false, key(Key::ArrowLeft), &mut out);
        assert_eq!(nav.levels[0].container, view, "wraps to the last title");
        nav.key(&mut m, true, false, key(Key::ArrowRight), &mut out);
        assert_eq!(nav.levels[0].container, file);
        // to Recent, Right opens it with its first row current
        nav.key(&mut m, true, false, key(Key::End), &mut out);
        nav.key(&mut m, true, false, key(Key::ArrowUp), &mut out);
        assert_eq!(nav.current(), Some(recent));
        out.clear();
        nav.key(&mut m, true, false, key(Key::ArrowRight), &mut out);
        assert_eq!(nav.levels.len(), 2);
        assert_eq!(id(&m, nav.current()), "a");
        assert!(out.contains(&MenuOut::Opened(recent)));
        // Left closes only the submenu
        nav.key(&mut m, true, false, key(Key::ArrowLeft), &mut out);
        assert_eq!(nav.levels.len(), 1);
        assert_eq!(nav.current(), Some(recent));
    }

    #[test]
    fn escape_closes_one_level_at_a_time() {
        let (mut m, [file, ..]) = bar();
        let recent = m.find("recent").unwrap();
        let mut nav = MenuNav::default();
        let mut out = Vec::new();
        nav.open_title(&m, file, true, &mut out);
        nav.open_sub(&m, 0, recent, true, &mut out);
        nav.key(&mut m, true, false, key(Key::Escape), &mut out);
        assert_eq!(nav.levels.len(), 1);
        nav.key(&mut m, true, false, key(Key::Escape), &mut out);
        assert!(!nav.is_open());
        assert!(nav.bar_keys, "the title stays highlighted");
        assert_eq!(nav.bar_current, Some(file));
        nav.key(&mut m, true, false, key(Key::Escape), &mut out);
        assert!(!nav.bar_keys && nav.bar_current.is_none());
    }

    #[test]
    fn typeahead_moves_and_mnemonics_pick() {
        let (mut m, [file, ..]) = bar();
        let recent = m.find("recent").unwrap();
        let mut nav = MenuNav::default();
        let mut out = Vec::new();
        nav.open_title(&m, file, true, &mut out);
        nav.open_sub(&m, 0, recent, true, &mut out);
        // "beta" has no mnemonic: type-ahead moves to it, nothing is picked
        nav.key(&mut m, true, false, typed("b", 0), &mut out);
        assert_eq!(id(&m, nav.current()), "b");
        assert!(nav.is_open());
        // a second letter within the timeout extends the prefix; "be" still matches
        nav.key(&mut m, true, false, typed("e", 300), &mut out);
        assert_eq!(id(&m, nav.current()), "b");
        // back in File, after the timeout: `s` is Save's mnemonic, unique: picks it
        nav.key(&mut m, true, false, key(Key::ArrowLeft), &mut out);
        out.clear();
        nav.key(&mut m, true, false, typed("s", 5000), &mut out);
        assert!(!nav.is_open());
        assert!(out.iter().any(|o| matches!(o, MenuOut::Picked(p) if p.activated == Some(("save".into(), false)))));
    }

    #[test]
    fn alt_letter_f10_and_lone_alt_reach_the_bar() {
        let (mut m, [file, edit, _]) = bar();
        let mut nav = MenuNav::default();
        let mut out = Vec::new();
        let alt_e = KeyIn { key: Some(Key::E), text: None, state: KeyState::Pressed, modifiers: Modifiers::ALT, now: t(0) };
        assert!(nav.key(&mut m, true, true, alt_e, &mut out));
        assert_eq!(nav.levels[0].container, edit);
        nav.close_all(&mut out);
        assert!(nav.key(&mut m, true, false, key(Key::F10), &mut out));
        assert!(nav.bar_keys && nav.bar_current == Some(file) && !nav.is_open());
        nav.key(&mut m, true, false, key(Key::ArrowRight), &mut out);
        assert_eq!(nav.bar_current, Some(edit));
        nav.key(&mut m, true, false, key(Key::ArrowDown), &mut out);
        assert_eq!(id(&m, nav.current()), "undo");
        nav.close_all(&mut out);
        // a lone Alt, where the platform does it
        let alt = |state| KeyIn { key: Some(Key::AltLeft), text: None, state, modifiers: Modifiers::ALT, now: t(0) };
        nav.key(&mut m, true, true, alt(KeyState::Pressed), &mut out);
        assert!(nav.key(&mut m, true, true, alt(KeyState::Released), &mut out));
        assert!(nav.bar_keys);
        nav.close_all(&mut out);
        // Alt then another key: not lone
        nav.key(&mut m, true, true, alt(KeyState::Pressed), &mut out);
        nav.key(&mut m, true, true, KeyIn { modifiers: Modifiers::ALT, ..key(Key::Q) }, &mut out);
        assert!(!nav.key(&mut m, true, true, alt(KeyState::Released), &mut out));
        assert!(!nav.bar_keys);
        // where the platform does not do it
        nav.key(&mut m, true, false, alt(KeyState::Pressed), &mut out);
        assert!(!nav.key(&mut m, true, false, alt(KeyState::Released), &mut out));
    }

    #[test]
    fn check_and_radio_picks() {
        let (mut m, _) = bar();
        let wrap = m.find("wrap").unwrap();
        let large = m.find("large").unwrap();
        let p = m.pick(wrap).unwrap();
        assert_eq!(p.activated, Some(("wrap".into(), true)));
        assert_eq!(p.toggled, vec![("wrap".into(), true)]);
        assert_eq!(m.pick(wrap).unwrap().activated, Some(("wrap".into(), false)));
        let p = m.pick(large).unwrap();
        assert_eq!(p.toggled, vec![("small".into(), false), ("large".into(), true)]);
        // a radio pick on the checked one changes nothing but still activates
        let p = m.pick(large).unwrap();
        assert!(p.toggled.is_empty());
        assert_eq!(p.activated, Some(("large".into(), true)));
        // disabled and submenus are not picked
        assert_eq!(m.pick(m.find("open").unwrap()), None);
        assert_eq!(m.pick(m.find("recent").unwrap()), None);
    }

    #[test]
    fn typeahead_cycles_and_times_out() {
        let labels = ["Alpha", "Beta", "beer", "Gamma", "Bravo"];
        let all = |_| true;
        assert_eq!(typeahead_find(&labels, all, "b", None), Some(1));
        assert_eq!(typeahead_find(&labels, all, "b", Some(1)), Some(2));
        assert_eq!(typeahead_find(&labels, all, "bb", Some(2)), Some(4));
        assert_eq!(typeahead_find(&labels, all, "be", Some(1)), Some(1));
        assert_eq!(typeahead_find(&labels, all, "bee", Some(1)), Some(2));
        assert_eq!(typeahead_find(&labels, |i| i != 1, "b", None), Some(2));
        let mut ta = Typeahead::default();
        assert_eq!(ta.push("b", t(0)).as_deref(), Some("b"));
        assert_eq!(ta.push("e", t(500)).as_deref(), Some("be"));
        assert_eq!(ta.push("g", t(2000)).as_deref(), Some("g"));
        assert!(ta.typing(t(2500)) && !ta.typing(t(4000)));
    }
}
