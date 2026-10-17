//! Name and chain lookups over the generated tables.

use crate::generated::tables::*;

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
