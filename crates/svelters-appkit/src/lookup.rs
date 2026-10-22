//! Name resolution over the generated tables (pure; tested on every platform).
//!
//! Kind, prop, signal, method and slot ids are 1-based indexes into `KINDS`, `PROPS`, `SIGNALS`,
//! `METHODS` and `SLOTS`. Members are looked up along the kind's superclass chain, most derived
//! first, so `string-value` (declared on NSControl) resolves for `text-field`.

use crate::generated::tables::*;
use crate::tables::*;

/// Ids at and above this are minted at run time from the ObjC runtime (dynamic props/methods).
pub const DYN_BASE: u32 = 1 << 20;

pub fn kind_by_tag(tag: &str) -> u32 {
    KINDS.iter().position(|k| !k.tag.is_empty() && k.tag == tag).map_or(0, |i| i as u32 + 1)
}

pub fn kind_by_class(class: &str) -> u32 {
    KINDS.iter().position(|k| k.class == class).map_or(0, |i| i as u32 + 1)
}

pub fn kind(id: u32) -> Option<&'static KindInfo> {
    KINDS.get((id as usize).wrapping_sub(1))
}

/// `kind` and its ingested superclasses, most derived first.
pub fn chain(kind_id: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut k = kind_id;
    while let Some(info) = kind(k) {
        out.push(k);
        k = info.parent as u32;
    }
    out
}

fn find<T>(table: &'static [T], kind_id: u32, name: &str, kind_of: impl Fn(&T) -> u16, name_of: impl Fn(&T) -> &str) -> u32 {
    for k in chain(kind_id) {
        if let Some(i) = table.iter().position(|t| kind_of(t) as u32 == k && name_of(t) == name) {
            return i as u32 + 1;
        }
    }
    0
}

pub fn prop(kind_id: u32, name: &str) -> u32 {
    find(PROPS, kind_id, name, |p| p.kind, |p| p.name)
}

pub fn signal(kind_id: u32, name: &str) -> u32 {
    find(SIGNALS, kind_id, name, |s| s.kind, |s| s.name)
}

pub fn method(kind_id: u32, name: &str) -> u32 {
    find(METHODS, kind_id, name, |m| m.kind, |m| m.name)
}

pub fn slot(kind_id: u32, name: &str) -> u32 {
    find(SLOTS, kind_id, name, |s| s.kind, |s| s.name)
}

/// The nearest slot marked default along the chain.
pub fn default_slot(kind_id: u32) -> u32 {
    for k in chain(kind_id) {
        if let Some(i) = SLOTS.iter().position(|s| s.kind as u32 == k && s.default) {
            return i as u32 + 1;
        }
    }
    0
}

/// The observe fact of prop `name` for an element of `kind_id`: the most derived class along the chain that has one.
pub fn observe_info(kind_id: u32, name: &str) -> Option<&'static ObserveInfo> {
    match find(OBSERVE, kind_id, name, |o| o.kind, |o| o.name) {
        0 => None,
        i => OBSERVE.get(i as usize - 1),
    }
}

/// `SVL_M_OBSERVABLE`: an exact or partial fact with at least one mechanism the backend installs.
pub fn observable(kind_id: u32, name: &str) -> bool {
    observe_info(kind_id, name).is_some_and(|o| {
        matches!(o.class, ObsClass::Exact | ObsClass::Partial) && (!o.kvo.is_empty() || !o.notifications.is_empty() || o.action)
    })
}

pub fn prop_info(id: u32) -> Option<&'static PropInfo> {
    PROPS.get((id as usize).wrapping_sub(1))
}

/// How a prop is written when it is not a plain KVC set (`PROP_WRITES`).
pub fn prop_write(id: u32) -> Option<&'static PropWrite> {
    PROP_WRITES.iter().find(|w| w.prop == id)
}

pub fn signal_info(id: u32) -> Option<&'static SignalInfo> {
    SIGNALS.get((id as usize).wrapping_sub(1))
}

pub fn method_info(id: u32) -> Option<&'static MethodInfo> {
    METHODS.get((id as usize).wrapping_sub(1))
}

pub fn slot_info(id: u32) -> Option<&'static SlotInfo> {
    SLOTS.get((id as usize).wrapping_sub(1))
}

/// The slot along `kind`'s chain whose native child storage is the KVC key `key` (`subviews`,
/// `arrangedSubviews`, `contentView`). Such props are owned by the handle tree: writing them would
/// change children behind `insert`/`remove`, so they are read-only from JS whatever the header says.
pub fn slot_owning_key(kind_id: u32, key: &str) -> Option<&'static SlotInfo> {
    let ch = chain(kind_id);
    SLOTS.iter().find(|s| ch.contains(&(s.kind as u32)) && (s.list == key || s.get == key) && !key.is_empty())
}

/// JS names of the readable table props of a kind (most derived declaration wins).
pub fn readable_props(kind_id: u32) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for k in chain(kind_id) {
        for p in PROPS.iter().filter(|p| p.kind as u32 == k && p.setter.is_empty()) {
            if !out.contains(&p.name) {
                out.push(p.name);
            }
        }
    }
    out
}

/// Members and flag-ness of an enum-typed prop.
pub fn members(ty: Ty) -> (&'static [(&'static str, i64)], bool) {
    match ty {
        Ty::Enum(i) | Ty::Flags(i) => ENUMS.get(i as usize).map_or((&[][..], false), |e| (e.members, e.flags)),
        _ => (&[], false),
    }
}

/// The JS signal name a notification-backed signal of `kind` uses for a notification name.
pub fn notification_signal(kind_id: u32, notification: &str) -> u32 {
    for k in chain(kind_id) {
        if let Some(i) = SIGNALS.iter().position(|s| s.kind as u32 == k && s.via == Via::Notification && s.native == notification) {
            return i as u32 + 1;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use svelters_apple::names;

    #[test]
    fn tags_follow_the_naming_rule() {
        let mut seen = std::collections::HashSet::new();
        for k in KINDS.iter().filter(|k| !k.tag.is_empty()) {
            assert_eq!(k.tag, names::class_tag(k.class, "NS"), "{}", k.class);
            assert!(seen.insert(k.tag), "duplicate tag {}", k.tag);
        }
        for p in PROPS.iter().filter(|p| p.setter.is_empty()) {
            assert_eq!(p.name, names::kebab(p.key), "{}", p.key);
        }
        for m in METHODS {
            assert_eq!(m.name, names::selector_name(m.selector));
        }
    }

    #[test]
    fn resolution_walks_the_chain() {
        let tf = kind_by_tag("text-field");
        assert!(tf != 0);
        let sv = prop(tf, "string-value");
        assert!(sv != 0);
        assert_eq!(PROPS[sv as usize - 1].key, "stringValue");
        assert_eq!(KINDS[PROPS[sv as usize - 1].kind as usize - 1].class, "NSControl");
        assert!(signal(tf, "action") != 0, "inherited from NSControl");
        assert!(signal(tf, "control-text-did-change") != 0);
        assert_eq!(prop(tf, "no-such-prop"), 0);
        assert_eq!(kind_by_tag(""), 0);
        assert_eq!(kind_by_tag("control"), 0, "NSControl is not an element");
        let w = kind_by_tag("window");
        assert!(kind(w).unwrap().toplevel);
        assert_eq!(slot_info(default_slot(w)).unwrap().name, "content-view");
        let stack = kind_by_tag("stack-view");
        assert_eq!(slot_info(default_slot(stack)).unwrap().name, "arranged-subviews");
        assert!(slot(stack, "subviews") != 0, "NSView's slot is inherited");
        assert_eq!(slot_owning_key(stack, "subviews").unwrap().name, "subviews");
        assert_eq!(slot_owning_key(stack, "arrangedSubviews").unwrap().name, "arranged-subviews");
        assert_eq!(slot_owning_key(kind_by_tag("window"), "contentView").unwrap().name, "content-view");
        assert!(slot_owning_key(stack, "hidden").is_none());
        assert_eq!(slot_info(default_slot(kind_by_tag("button"))).unwrap().name, "subviews");
    }

    #[test]
    fn enums_and_readables() {
        let b = kind_by_tag("button");
        let st = prop_info(prop(b, "state")).unwrap();
        let (m, flags) = members(st.ty);
        assert!(!flags);
        assert!(m.contains(&("on", 1)) && m.contains(&("mixed", -1)));
        let bt = prop_info(prop(b, "button-type")).unwrap();
        assert_eq!(bt.setter, "setButtonType:");
        assert!(members(bt.ty).0.contains(&("switch", 3)));
        let mask = prop_info(prop(kind_by_tag("window"), "style-mask")).unwrap();
        assert!(members(mask.ty).1, "NS_OPTIONS");
        let r = readable_props(b);
        assert!(r.contains(&"title") && r.contains(&"enabled") && r.contains(&"frame"));
        assert!(!r.contains(&"button-type"));
    }
}
