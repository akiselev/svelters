//! Reflected kinds, properties and signals (NT0): everything the generated widget tables do not
//! list is read from the GType system on demand, so a handle of any GObject or boxed type has a kind
//! and non-widget types need no per-type table at all.
//!
//! - **Kinds.** A type with a generated kind keeps that id. Any other type gets `KIND_BASE + n` the
//!   first time it is seen (a handle is minted for it, or `resolve(SVL_KIND, "Gio.ListStore")`).
//! - **Properties and signals.** Ids `DYN_BASE + n` index the `GParamSpec` / signal found by name on
//!   the kind's type. They are only offered for non-widget types: widgets keep the curated tables.

use crate::generated::tables::KINDS;
use crate::lookup;
use gtk::glib::{self, translate::*, Type};
use gtk::prelude::*;
use std::collections::HashMap;

/// Reflected kinds are `KIND_BASE + index`. Generated kinds are small, extras are 0x4001.. and the
/// menu kinds are 0x4401.., so this range is free.
pub const KIND_BASE: u32 = 0x10000;
/// Reflected property and signal ids. Signal ids must stay below `SVL_SIGNAL_ID_MASK`.
pub const DYN_BASE: u32 = 0x0100_0000;

#[derive(Default)]
pub struct Reflect {
    kinds: Vec<Type>,
    kind_ids: HashMap<Type, u32>,
    props: Vec<glib::ParamSpec>,
    prop_ids: HashMap<usize, u32>,
    signals: Vec<(glib::subclass::SignalId, String)>,
    signal_ids: HashMap<u32, u32>,
}

/// `fooBar` and `foo_bar` to `foo-bar`.
pub fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else if c == '_' {
            out.push('-');
        } else {
            out.push(c);
        }
    }
    out
}

fn find_pspec(t: Type, name: &str) -> Option<glib::ParamSpec> {
    if t.is_a(Type::INTERFACE) {
        let c = std::ffi::CString::new(name).ok()?;
        unsafe {
            let iface = glib::gobject_ffi::g_type_default_interface_ref(t.into_glib());
            if iface.is_null() {
                return None;
            }
            let ps = glib::gobject_ffi::g_object_interface_find_property(iface, c.as_ptr());
            let out = if ps.is_null() { None } else { Some(from_glib_none(ps)) };
            glib::gobject_ffi::g_type_default_interface_unref(iface);
            out
        }
    } else if t.is_a(Type::OBJECT) {
        glib::object::ObjectClass::from_type(t)?.find_property(name)
    } else {
        None
    }
}

impl Reflect {
    /// The kind of a type: its generated kind, else a reflected one.
    pub fn kind_for_type(&mut self, t: Type) -> u32 {
        let g = lookup::kind_for_gtype(t.name());
        if g != 0 {
            return g;
        }
        if let Some(k) = self.kind_ids.get(&t) {
            return *k;
        }
        let id = KIND_BASE + self.kinds.len() as u32;
        self.kinds.push(t);
        self.kind_ids.insert(t, id);
        id
    }

    /// The type behind a generated or reflected kind.
    pub fn type_of_kind(&self, kind: u32) -> Option<Type> {
        if kind >= KIND_BASE {
            return self.kinds.get((kind - KIND_BASE) as usize).copied();
        }
        let k = KINDS.get((kind as usize).checked_sub(1)?)?;
        crate::generated::types::ensure_registered(k.gtype);
        Type::from_name(k.gtype)
    }

    /// Whether dynamic properties, signals and statics are offered for this kind.
    pub fn is_dynamic_kind(&self, kind: u32) -> bool {
        self.type_of_kind(kind).is_some_and(|t| !t.is_a(gtk::Widget::static_type()))
    }

    pub fn find_prop(&mut self, kind: u32, name: &str) -> u32 {
        if !self.is_dynamic_kind(kind) {
            return 0;
        }
        let Some(t) = self.type_of_kind(kind) else { return 0 };
        let ps = find_pspec(t, name).or_else(|| {
            let k = kebab(name);
            (k != name).then(|| find_pspec(t, &k)).flatten()
        });
        let Some(ps) = ps else { return 0 };
        let key = ps.as_ptr() as usize;
        if let Some(id) = self.prop_ids.get(&key) {
            return *id;
        }
        let id = DYN_BASE + self.props.len() as u32;
        self.props.push(ps);
        self.prop_ids.insert(key, id);
        id
    }

    pub fn prop(&self, id: u32) -> Option<&glib::ParamSpec> {
        self.props.get(id.checked_sub(DYN_BASE)? as usize)
    }

    pub fn find_signal(&mut self, kind: u32, name: &str) -> u32 {
        if !self.is_dynamic_kind(kind) {
            return 0;
        }
        let Some(t) = self.type_of_kind(kind) else { return 0 };
        let sid = glib::subclass::SignalId::lookup(name, t).or_else(|| {
            let k = kebab(name);
            (k != name).then(|| glib::subclass::SignalId::lookup(&k, t)).flatten()
        });
        let Some(sid) = sid else { return 0 };
        let raw: u32 = sid.into_glib();
        if let Some(id) = self.signal_ids.get(&raw) {
            return *id;
        }
        let id = DYN_BASE + self.signals.len() as u32;
        self.signals.push((sid, sid.name().to_string()));
        self.signal_ids.insert(raw, id);
        id
    }

    pub fn signal(&self, id: u32) -> Option<&(glib::subclass::SignalId, String)> {
        self.signals.get((id & svelters_abi::SVL_SIGNAL_ID_MASK).checked_sub(DYN_BASE)? as usize)
    }
}
