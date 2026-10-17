//! Name and chain lookups over the generated tables.

use crate::generated::tables::*;
use gtk::prelude::StaticType;

fn kind(k: u32) -> Option<&'static KindInfo> {
    KINDS.get((k as usize).checked_sub(1)?)
}

/// `kind`, its interfaces, then the same for each ancestor.
pub fn chain(k: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cur = k;
    while let Some(info) = kind(cur) {
        out.push(cur);
        out.extend_from_slice(info.ifaces);
        cur = info.parent;
    }
    out
}

pub fn in_chain(k: u32, owner: u32) -> bool {
    chain(k).contains(&owner)
}

/// Resolve by element tag, GType name (`GtkButton`) or GIR name (`Gtk.Button`, `Button`).
pub fn kind_by_name(name: &str) -> u32 {
    let gir = name.strip_prefix("Gtk.").unwrap_or(name);
    for (i, k) in KINDS.iter().enumerate() {
        if (!k.tag.is_empty() && k.tag == name) || k.gtype == name {
            return i as u32 + 1;
        }
    }
    for (i, k) in KINDS.iter().enumerate() {
        if k.name == gir {
            return i as u32 + 1;
        }
    }
    0
}

pub fn find_prop(k: u32, name: &str) -> u32 {
    for owner in chain(k) {
        if let Some(i) = PROPS.iter().position(|p| p.kind == owner && (p.name == name || p.js_name == name)) {
            return i as u32 + 1;
        }
    }
    0
}

pub fn find_signal(k: u32, name: &str) -> u32 {
    for owner in chain(k) {
        if let Some(i) = SIGNALS.iter().position(|s| s.kind == owner && (s.name == name || s.js_event == name)) {
            return i as u32 + 1;
        }
    }
    0
}

pub fn find_method(k: u32, name: &str) -> u32 {
    for owner in chain(k) {
        if let Some(i) = METHODS.iter().position(|m| m.kind == owner && m.name == name) {
            return i as u32 + 1;
        }
    }
    0
}

pub fn find_slot(k: u32, name: &str) -> u32 {
    for owner in chain(k) {
        if let Some(i) = SLOTS.iter().position(|s| s.kind == owner && s.name == name) {
            return i as u32 + 1;
        }
    }
    0
}

/// A child attribute by JS name: the id of any `ATTRS` entry of that name on `k` or its bases. The
/// backend picks the entry of the child's own slot at call time (`attr_for_slot`).
pub fn find_attr(k: u32, name: &str) -> u32 {
    for owner in chain(k) {
        if let Some(i) = ATTRS.iter().position(|a| a.kind == owner && a.name == name) {
            return i as u32 + 1;
        }
    }
    0
}

/// The `ATTRS` id of attribute `attr` as it applies to a child in `slot` of a parent of kind `k`.
pub fn attr_for_slot(k: u32, attr: u32, slot: u32) -> Option<u32> {
    let name = ATTRS.get((attr as usize).checked_sub(1)?)?.name;
    ATTRS.iter().position(|a| a.name == name && a.slot == slot && in_chain(k, a.kind)).map(|i| i as u32 + 1)
}

/// The default slot of `k`: the nearest ancestor that declares one.
pub fn default_slot(k: u32) -> u32 {
    for owner in chain(k) {
        let name = kind(owner).map_or("", |i| i.default_slot);
        if !name.is_empty() {
            return find_slot(k, name);
        }
    }
    0
}

/// GType name -> generated kind id.
pub fn kind_for_gtype(name: &str) -> u32 {
    static MAP: std::sync::OnceLock<std::collections::HashMap<&'static str, u32>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| KINDS.iter().enumerate().map(|(i, k)| (k.gtype, i as u32 + 1)).collect()).get(name).copied().unwrap_or(0)
}

/// The kind of the nearest ancestor type that has a generated kind (0 when there is none).
/// Objects of this type travel as handles (owned or borrowed): widgets and types that have a
/// generated kind. Other objects are opaque `BOXED` values.
pub fn is_handle_type(t: gtk::glib::Type) -> bool {
    kind_for_gtype(t.name()) != 0 || t.is_a(gtk::Widget::static_type())
}

pub fn nearest_kind(t: gtk::glib::Type) -> u32 {
    let mut cur = Some(t);
    while let Some(ty) = cur {
        let k = kind_for_gtype(ty.name());
        if k != 0 {
            return k;
        }
        cur = ty.parent();
    }
    0
}

