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
    pub(crate) method_ids: Vec<MethodId>,
    pub(crate) slot_ids: Vec<SlotId>,
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
    pub fn method(&self, method: MethodId) -> Option<&'static MethodSpec> {
        self.method_ids.iter().position(|&m| m == method).map(|i| &self.spec.methods[i])
    }
    pub fn method_ids(&self) -> &[MethodId] {
        &self.method_ids
    }
    pub fn slot_ids(&self) -> &[SlotId] {
        &self.slot_ids
    }
    pub fn slot(&self, slot: SlotId) -> Option<&'static SlotSpec> {
        self.slot_ids.iter().position(|&s| s == slot).map(|i| &self.spec.slots[i])
    }
    pub fn prop_ids(&self) -> &[PropId] {
        &self.prop_ids
    }
    pub fn signal_ids(&self) -> &[SignalId] {
        &self.signal_ids
    }
}

/// What a wire id names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemberKind {
    Kind,
    Prop,
    Signal,
    Method,
    Slot,
}

impl MemberKind {
    pub const fn name(self) -> &'static str {
        match self {
            MemberKind::Kind => "kind",
            MemberKind::Prop => "prop",
            MemberKind::Signal => "signal",
            MemberKind::Method => "method",
            MemberKind::Slot => "slot",
        }
    }
}

/// The owner of a wire id: a kind, or a member of one (common members have the kind `"*"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Member {
    pub what: MemberKind,
    /// The declaring kind's name.
    pub kind: &'static str,
    /// The member's name (the kind's own for [`MemberKind::Kind`]).
    pub name: &'static str,
}

impl std::fmt::Display for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.what {
            MemberKind::Kind => write!(f, "kind `{}`", self.name),
            w => write!(f, "{} `{}` of kind `{}`", w.name(), self.name, self.kind),
        }
    }
}

/// Why a kind could not be registered.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegistryError {
    /// A kind with this name (or hash) is registered already.
    DuplicateKind { name: &'static str, existing: &'static str },
    /// Two members of one kind (or a member and a common one) hash to the same id.
    MemberCollision { kind: &'static str, a: &'static str, b: &'static str },
    /// The spec's `text_prop` names no prop of the kind.
    UnknownTextProp { kind: &'static str, prop: &'static str },
    /// Two names of the registry (kinds or members of any kinds) hash to the same wire id. The ids
    /// are the single wire id authority (the compiler, the spec and the host compute them from the
    /// names), so one of the two must be renamed.
    IdCollision { id: u32, existing: Member, new: Member },
    /// A `NameFrom::Prop` names no prop of the kind.
    UnknownNameProp { kind: &'static str, prop: &'static str },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::DuplicateKind { name, existing } => write!(f, "kind `{name}` collides with registered kind `{existing}`"),
            RegistryError::MemberCollision { kind, a, b } => write!(f, "kind `{kind}`: members `{a}` and `{b}` have the same id"),
            RegistryError::UnknownTextProp { kind, prop } => write!(f, "kind `{kind}`: text prop `{prop}` is not one of its props"),
            RegistryError::IdCollision { id, existing, new } => {
                write!(f, "wire id {id:#010x} of {new} is already the id of {existing} (FNV-1a collision): rename one of them")
            }
            RegistryError::UnknownNameProp { kind, prop } => write!(f, "kind `{kind}`: accessible name prop `{prop}` is not one of its props"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Every kind a document can create.
pub struct KindRegistry {
    kinds: Vec<KindEntry>,
    by_id: HashMap<KindId, usize>,
    by_name: HashMap<&'static str, usize>,
    /// Every wire id in use (kinds, their members, the common members) and what it names.
    ids: HashMap<u32, Member>,
}

impl Default for KindRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn common_members() -> impl Iterator<Item = (u32, Member)> {
    let c = &common::COMMON;
    let props = (0..c.props.len()).map(|i| (c.prop_id(i).0, Member { what: MemberKind::Prop, kind: c.name, name: c.props[i].name }));
    let signals = (0..c.signals.len()).map(|i| (c.signal_id(i).0, Member { what: MemberKind::Signal, kind: c.name, name: c.signals[i].name }));
    let methods = (0..c.methods.len()).map(|i| (c.method_id(i).0, Member { what: MemberKind::Method, kind: c.name, name: c.methods[i].name }));
    props.chain(signals).chain(methods)
}

impl KindRegistry {
    /// A registry with no kinds (the common props and signals of [`crate::common`] are always known).
    pub fn new() -> Self {
        let ids = common_members().collect();
        KindRegistry { kinds: Vec::new(), by_id: HashMap::new(), by_name: HashMap::new(), ids }
    }

    /// Registers `K`, built with `K::default()`.
    pub fn register<K: Kind + Default>(&mut self) -> Result<KindId, RegistryError> {
        self.register_with(K::spec(), || Box::new(K::default()))
    }

    /// Registers a kind from its spec and a factory (for kinds built from data, like provider kinds).
    /// Fails, registering nothing, when any of its ids collides with an id already in the registry.
    pub fn register_with(&mut self, spec: &'static KindSpec, factory: KindFactory) -> Result<KindId, RegistryError> {
        let id = spec.id();
        if let Some(&i) = self.by_id.get(&id).or_else(|| self.by_name.get(spec.name)) {
            return Err(RegistryError::DuplicateKind { name: spec.name, existing: self.kinds[i].spec.name });
        }
        let prop_ids: Vec<PropId> = (0..spec.props.len()).map(|i| spec.prop_id(i)).collect();
        let signal_ids: Vec<SignalId> = (0..spec.signals.len()).map(|i| spec.signal_id(i)).collect();
        let method_ids: Vec<MethodId> = (0..spec.methods.len()).map(|i| spec.method_id(i)).collect();
        let slot_ids: Vec<SlotId> = (0..spec.slots.len()).map(|i| spec.slot_id(i)).collect();
        // Members must be unique within the kind and against the common members every node has.
        let mut seen: HashMap<u32, &'static str> = HashMap::new();
        let common_props = common::COMMON.props.iter().enumerate().map(|(i, p)| (common::COMMON.prop_id(i).0, p.name));
        let common_signals = common::COMMON.signals.iter().enumerate().map(|(i, s)| (common::COMMON.signal_id(i).0, s.name));
        let own = prop_ids.iter().zip(spec.props).map(|(id, p)| (id.0, p.name));
        let own_signals = signal_ids.iter().zip(spec.signals).map(|(id, s)| (id.0, s.name));
        let own_methods = method_ids.iter().zip(spec.methods).map(|(id, m)| (id.0, m.name));
        let own_slots = slot_ids.iter().zip(spec.slots).map(|(id, s)| (id.0, s.name));
        for (h, name) in common_props.chain(common_signals).chain(own).chain(own_signals).chain(own_methods).chain(own_slots) {
            if let Some(prev) = seen.insert(h, name) {
                return Err(RegistryError::MemberCollision { kind: spec.name, a: prev, b: name });
            }
        }
        // ... and every id must be new to the whole registry: the hashes are the wire ids
        let member = |what, name| Member { what, kind: spec.name, name };
        let mut new_ids: Vec<(u32, Member)> = vec![(id.0, member(MemberKind::Kind, spec.name))];
        new_ids.extend(prop_ids.iter().zip(spec.props).map(|(i, p)| (i.0, member(MemberKind::Prop, p.name))));
        new_ids.extend(signal_ids.iter().zip(spec.signals).map(|(i, s)| (i.0, member(MemberKind::Signal, s.name))));
        new_ids.extend(method_ids.iter().zip(spec.methods).map(|(i, m)| (i.0, member(MemberKind::Method, m.name))));
        new_ids.extend(slot_ids.iter().zip(spec.slots).map(|(i, s)| (i.0, member(MemberKind::Slot, s.name))));
        for &(h, m) in &new_ids {
            if let Some(&existing) = self.ids.get(&h) {
                return Err(RegistryError::IdCollision { id: h, existing, new: m });
            }
        }
        let text_prop = match spec.text_prop {
            Some(t) => Some(spec.prop_named(t).ok_or(RegistryError::UnknownTextProp { kind: spec.name, prop: t })?),
            None => None,
        };
        for n in spec.name_from {
            if let crate::kind::NameFrom::Prop(p) = n
                && spec.prop_named(p).is_none()
            {
                return Err(RegistryError::UnknownNameProp { kind: spec.name, prop: p });
            }
        }
        self.ids.extend(new_ids);
        let defaults = spec.props.iter().map(|p| p.default.to_value()).collect();
        let ix = self.kinds.len();
        self.kinds.push(KindEntry { id, spec, factory, defaults, prop_ids, signal_ids, method_ids, slot_ids, text_prop });
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

    /// What a wire id names (a kind, a member of a registered kind, or a common member).
    pub fn member(&self, id: u32) -> Option<Member> {
        self.ids.get(&id).copied()
    }

    /// A hash of every wire id and what it names, independent of registration order: the host
    /// compares it with the compiled spec's to detect a stale spec.
    pub fn spec_hash(&self) -> u32 {
        let mut rows: Vec<(u32, Member)> = self.ids.iter().map(|(&k, &m)| (k, m)).collect();
        rows.sort_by_key(|r| r.0);
        let mut h = crate::ids::fingerprint(&["spec"]);
        for (id, m) in rows {
            let id = format!("{id:08x}");
            h ^= crate::ids::fingerprint(&[&id, m.what.name(), m.kind, m.name]);
            h = h.wrapping_mul(0x0100_0193);
        }
        h
    }
}
