//! The kind registry: every kind a document can create, with its spec, its factory and the id tables
//! the document looks props, signals and slots up in. Shared (`Arc`) between documents and with the
//! spec generator.

use crate::common;
use crate::ids::{KindId, MethodId, PropId, SignalId, SlotId};
use crate::kind::{Kind, KindFactory, KindSpec, MethodSpec, PropSpec, SignalSpec, SlotSpec};
use crate::value::Value;
use std::collections::HashMap;

/// A registered kind.
pub struct KindEntry {
    pub id: KindId,
    pub spec: &'static KindSpec,
    pub factory: KindFactory,
    /// Each prop's default value, by prop index.
    pub defaults: Vec<Option<Value>>,
    pub(crate) prop_ids: Vec<PropId>,
    pub(crate) signal_ids: Vec<SignalId>,
    pub(crate) slot_ids: Vec<SlotId>,
    pub(crate) method_ids: Vec<MethodId>,
    /// The text-content prop's id.
    pub text_prop: Option<PropId>,
}

impl KindEntry {
    /// The index of `prop` in the spec.
    pub fn prop_index(&self, prop: PropId) -> Option<usize> {
        self.prop_ids.iter().position(|&p| p == prop)
    }
    pub fn prop(&self, prop: PropId) -> Option<&'static PropSpec> {
        self.prop_index(prop).map(|i| &self.spec.props[i])
    }
    pub fn signal(&self, signal: SignalId) -> Option<&'static SignalSpec> {
        self.signal_ids.iter().position(|&s| s == signal).map(|i| &self.spec.signals[i])
    }
    pub fn slot(&self, slot: SlotId) -> Option<&'static SlotSpec> {
        self.slot_ids.iter().position(|&s| s == slot).map(|i| &self.spec.slots[i])
    }
    pub fn method(&self, method: MethodId) -> Option<&'static MethodSpec> {
        self.method_ids.iter().position(|&m| m == method).map(|i| &self.spec.methods[i])
    }
    pub fn method_ids(&self) -> &[MethodId] {
        &self.method_ids
    }
    pub fn prop_ids(&self) -> &[PropId] {
        &self.prop_ids
    }
    pub fn signal_ids(&self) -> &[SignalId] {
        &self.signal_ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// A kind with this name (or hash) is registered already.
    DuplicateKind { name: &'static str, existing: &'static str },
    /// Two members of one kind (or a member and a common one) hash to the same id.
    MemberCollision { kind: &'static str, a: &'static str, b: &'static str },
    /// A member's id equals a member id of another registered kind (the ids are the wire's
    /// authority, so they must be unique across kinds): `kind.member` against `other_kind.other`.
    CrossKindCollision { kind: &'static str, member: &'static str, other_kind: &'static str, other: &'static str, id: u32 },
    /// The spec's `text_prop` names no prop of the kind.
    UnknownTextProp { kind: &'static str, prop: &'static str },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::DuplicateKind { name, existing } => write!(f, "kind `{name}` collides with registered kind `{existing}`"),
            RegistryError::MemberCollision { kind, a, b } => write!(f, "kind `{kind}`: members `{a}` and `{b}` have the same id"),
            RegistryError::CrossKindCollision { kind, member, other_kind, other, id } => {
                write!(f, "`{kind}.{member}` and `{other_kind}.{other}` hash to the same id {id:#010x}: rename one")
            }
            RegistryError::UnknownTextProp { kind, prop } => write!(f, "kind `{kind}`: text prop `{prop}` is not one of its props"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Every kind a document can create.
pub struct KindRegistry {
    kinds: Vec<KindEntry>,
    /// Every member id of every registered kind (and of the common members): id → (kind, member).
    members: HashMap<u32, (&'static str, &'static str)>,
    by_id: HashMap<KindId, usize>,
    by_name: HashMap<&'static str, usize>,
}

impl Default for KindRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl KindRegistry {
    /// A registry with no kinds (the common props and signals of [`crate::common`] are always known).
    pub fn new() -> Self {
        let mut members = HashMap::new();
        for (i, p) in common::COMMON.props.iter().enumerate() {
            members.insert(common::COMMON.prop_id(i).0, ("*", p.name));
        }
        for (i, s) in common::COMMON.signals.iter().enumerate() {
            members.insert(common::COMMON.signal_id(i).0, ("*", s.name));
        }
        KindRegistry { kinds: Vec::new(), members, by_id: HashMap::new(), by_name: HashMap::new() }
    }

    /// Registers `K`, built with `K::default()`.
    pub fn register<K: Kind + Default>(&mut self) -> Result<KindId, RegistryError> {
        self.register_with(K::spec(), || Box::new(K::default()))
    }

    /// Registers a kind from its spec and a factory (for kinds built from data, like provider kinds).
    pub fn register_with(&mut self, spec: &'static KindSpec, factory: KindFactory) -> Result<KindId, RegistryError> {
        let id = spec.id();
        if let Some(&i) = self.by_id.get(&id).or_else(|| self.by_name.get(spec.name)) {
            return Err(RegistryError::DuplicateKind { name: spec.name, existing: self.kinds[i].spec.name });
        }
        let prop_ids: Vec<PropId> = (0..spec.props.len()).map(|i| spec.prop_id(i)).collect();
        let signal_ids: Vec<SignalId> = (0..spec.signals.len()).map(|i| spec.signal_id(i)).collect();
        let slot_ids: Vec<SlotId> = (0..spec.slots.len()).map(|i| spec.slot_id(i)).collect();
        // Members must be unique within the kind and against the common members every node has.
        let mut seen: HashMap<u32, &'static str> = HashMap::new();
        let common_props = common::COMMON.props.iter().enumerate().map(|(i, p)| (common::COMMON.prop_id(i).0, p.name));
        let common_signals = common::COMMON.signals.iter().enumerate().map(|(i, s)| (common::COMMON.signal_id(i).0, s.name));
        let own = prop_ids.iter().zip(spec.props).map(|(id, p)| (id.0, p.name));
        let own_signals = signal_ids.iter().zip(spec.signals).map(|(id, s)| (id.0, s.name));
        let own_slots = slot_ids.iter().zip(spec.slots).map(|(id, s)| (id.0, s.name));
        let method_ids: Vec<MethodId> = (0..spec.methods.len()).map(|i| spec.method_id(i)).collect();
        let own_methods = method_ids.iter().zip(spec.methods).map(|(id, m)| (id.0, m.name));
        let mut mine = Vec::new();
        for (h, name) in common_props.chain(common_signals).chain(own.chain(own_signals).chain(own_slots).chain(own_methods).inspect(|&(h, n)| mine.push((h, n)))) {
            if let Some(prev) = seen.insert(h, name) {
                return Err(RegistryError::MemberCollision { kind: spec.name, a: prev, b: name });
            }
        }
        // and against every other kind's members (the kind's own ids hash its name, so equal ids
        // across kinds are true collisions)
        let kid = spec.id().0;
        if let Some(&(other_kind, other)) = self.members.get(&kid) {
            return Err(RegistryError::CrossKindCollision { kind: spec.name, member: "(kind id)", other_kind, other, id: kid });
        }
        for &(h, name) in &mine {
            if let Some(&(other_kind, other)) = self.members.get(&h) {
                return Err(RegistryError::CrossKindCollision { kind: spec.name, member: name, other_kind, other, id: h });
            }
        }
        let text_prop = match spec.text_prop {
            Some(t) => Some(spec.prop_named(t).ok_or(RegistryError::UnknownTextProp { kind: spec.name, prop: t })?),
            None => None,
        };
        let defaults = spec.props.iter().map(|p| p.default.to_value()).collect();
        let ix = self.kinds.len();
        for &(h, name) in &mine {
            self.members.insert(h, (spec.name, name));
        }
        self.members.insert(kid, (spec.name, "(kind id)"));
        self.kinds.push(KindEntry { id, spec, factory, defaults, prop_ids, signal_ids, slot_ids, method_ids, text_prop });
        self.by_id.insert(id, ix);
        self.by_name.insert(spec.name, ix);
        Ok(id)
    }

    pub fn get(&self, id: KindId) -> Option<&KindEntry> {
        self.by_id.get(&id).map(|&i| &self.kinds[i])
    }
    pub fn by_name(&self, name: &str) -> Option<&KindEntry> {
        self.by_name.get(name).map(|&i| &self.kinds[i])
    }
    pub fn iter(&self) -> impl Iterator<Item = &KindEntry> {
        self.kinds.iter()
    }
    pub fn len(&self) -> usize {
        self.kinds.len()
    }
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}
