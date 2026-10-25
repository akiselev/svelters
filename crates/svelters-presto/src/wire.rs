//! The registry seen through the wire ids ([`crate::ids`]): which kind, prop, signal, method or slot
//! a `svl_id` names on a given kind, and back. Built once per registry; member lookups walk the kind's
//! chain to the declaring kind, whose row holds the id.

use crate::ids::{Ids, Key, What};
use crate::shim::{KindId, KindRegistry, KindSpec, MethodSpec, PropId, PropSpec, SignalId, SignalSpec, SlotId};
use std::collections::HashMap;
use svelters_abi::svl_id;

pub struct Wire {
    ids: &'static Ids,
    kind_of_wire: HashMap<svl_id, KindId>,
    wire_of_kind: HashMap<KindId, svl_id>,
}

/// A wire member resolved on one kind.
pub struct Member<T: 'static> {
    pub spec: &'static T,
    /// The kind that declares it (`Widget` for `enabled`).
    pub decl: &'static KindSpec,
}

impl Wire {
    pub fn new(registry: &KindRegistry, ids: &'static Ids) -> Wire {
        let mut kind_of_wire = HashMap::new();
        let mut wire_of_kind = HashMap::new();
        for (k, spec) in registry.kinds() {
            if let Some(id) = ids.get(&Key::kind(spec.name)) {
                kind_of_wire.insert(id, k);
                wire_of_kind.insert(k, id);
            }
        }
        Wire { ids, kind_of_wire, wire_of_kind }
    }

    pub fn ids(&self) -> &'static Ids {
        self.ids
    }

    pub fn kind(&self, id: svl_id) -> Option<KindId> {
        self.kind_of_wire.get(&id).copied()
    }

    pub fn kind_id(&self, k: KindId) -> svl_id {
        self.wire_of_kind.get(&k).copied().unwrap_or(0)
    }

    /// The wire id of member `name` of kind `spec` (through its chain); 0 when there is none.
    pub fn resolve(&self, spec: &'static KindSpec, what: What, name: &str) -> svl_id {
        for k in spec.chain() {
            let has = match what {
                What::Kind => false,
                What::Prop => k.props.iter().any(|p| p.name == name),
                What::Signal => k.signals.iter().any(|s| s.name == name),
                What::Method => k.methods.iter().any(|m| m.name == name),
                What::Slot => k.slots.iter().any(|s| s.name == name),
            };
            if has {
                return self.ids.get(&Key::member(what, k.name, name)).unwrap_or(0);
            }
        }
        0
    }

    /// The (declaring kind, member name) of wire id `id` when it names a `what` of `spec`'s chain.
    fn member(&self, spec: &'static KindSpec, what: What, id: svl_id) -> Option<(&'static KindSpec, String)> {
        let key = self.ids.key(id).filter(|k| k.what == what)?;
        let decl = spec.chain().find(|k| k.name == key.kind)?;
        Some((decl, key.member.clone()?))
    }

    pub fn prop(&self, spec: &'static KindSpec, id: svl_id) -> Option<Member<PropSpec>> {
        let (decl, name) = self.member(spec, What::Prop, id)?;
        Some(Member { spec: decl.props.iter().find(|p| p.name == name)?, decl })
    }

    pub fn signal(&self, spec: &'static KindSpec, id: svl_id) -> Option<Member<SignalSpec>> {
        let (decl, name) = self.member(spec, What::Signal, id)?;
        Some(Member { spec: decl.signals.iter().find(|s| s.name == name)?, decl })
    }

    pub fn method(&self, spec: &'static KindSpec, id: svl_id) -> Option<Member<MethodSpec>> {
        let (decl, name) = self.member(spec, What::Method, id)?;
        Some(Member { spec: decl.methods.iter().find(|m| m.name == name)?, decl })
    }

    /// Wire slot id → the tree's slot index on `spec`. 0 is the default slot.
    pub fn slot(&self, spec: &'static KindSpec, id: svl_id) -> Option<SlotId> {
        if id == 0 {
            return Some(SlotId(0));
        }
        let (decl, name) = self.member(spec, What::Slot, id)?;
        decl.slots.iter().position(|s| s.name == name).map(|i| SlotId(i as u32))
    }

    /// The tree's prop id of wire prop `id` on kind `k`.
    pub fn core_prop(&self, registry: &KindRegistry, k: KindId, id: svl_id) -> Option<(PropId, &'static PropSpec)> {
        let m = self.prop(registry.spec(k)?, id)?;
        registry.prop(k, m.spec.name)
    }

    /// The tree's signal id of wire signal `id` on kind `k`.
    pub fn core_signal(&self, registry: &KindRegistry, k: KindId, id: svl_id) -> Option<(SignalId, &'static SignalSpec)> {
        let m = self.signal(registry.spec(k)?, id)?;
        registry.signal(k, m.spec.name)
    }

    /// The wire id of the tree's signal `s` on kind `k`.
    pub fn signal_wire(&self, registry: &KindRegistry, k: KindId, s: SignalId) -> svl_id {
        match (registry.spec(k), registry.signal_spec(s)) {
            (Some(spec), Some(sig)) => self.resolve(spec, What::Signal, sig.name),
            _ => 0,
        }
    }

    /// The wire id of the tree's prop `p` on kind `k`.
    pub fn prop_wire(&self, registry: &KindRegistry, k: KindId, p: PropId) -> svl_id {
        match (registry.spec(k), registry.prop_spec(p)) {
            (Some(spec), Some(prop)) => self.resolve(spec, What::Prop, prop.name),
            _ => 0,
        }
    }
}
