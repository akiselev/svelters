//! [`KindRegistry`]: the built-in kinds plus the provider kinds of `#[svelters::widget]`, with the id
//! scheme over them (ARCH-EMBED §8.1). Writer lane: RUSTAPI.

use super::{Def, KINDS, KindDef, MethodDef, Native, P, PropDef, SignalDef, Ty};
use svelters_host::exports::TypeRef;

/// Kind ids of provider kinds: pack 62 (`62 << 22 | local`; CUSTOM-WIDGETS.md §3.2).
pub const PROVIDER_PACK: u32 = 62 << 22;
const PACK_MASK: u32 = 0x3F << 22;

/// A provider kind in the registry.
pub struct ProviderKind {
    pub entry: &'static crate::provider::KindEntry,
    /// Its definition in the kind table's shape (props `P::Ext(i)`, types `Ty::Provider`).
    pub def: &'static KindDef,
    /// The JS types of the props, then of the event payloads (from the export schema).
    pub types: &'static [TypeRef],
}

impl ProviderKind {
    /// The JS type of event `i`'s payload.
    pub fn event_type(&self, i: usize) -> Option<&TypeRef> {
        self.types.get(self.entry.props.len() + i)
    }
}

/// The kinds the backend knows, with the id scheme over them.
///
/// Built once per process: the built-in table [`KINDS`] (indexes and kind ids `index + 1`) plus the
/// provider kinds collected from `inventory`, sorted by spec id (indexes `KINDS.len() + local`, kind
/// ids `62 << 22 | (local + 1)`, so ids are stable for a given binary). A duplicate tag or spec id,
/// or one that collides with a built-in, fails at startup. Member ids are
/// `(defining kind index + 1) << 16 | (member index + 1)` for both.
///
/// Built-in definitions are references into [`KINDS`] itself (callers compare them by address,
/// `tests/spec.rs`); provider definitions are leaked once, so every definition is `'static`.
pub struct KindRegistry {
    providers: Vec<ProviderKind>,
    /// The base kind index of each kind (SG2: member lookups walk the chain without allocating).
    bases: Vec<Option<usize>>,
}

static REGISTRY: std::sync::OnceLock<KindRegistry> = std::sync::OnceLock::new();

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

impl KindRegistry {
    /// The process's registry (built on first use).
    pub fn global() -> &'static KindRegistry {
        REGISTRY.get_or_init(KindRegistry::build)
    }

    fn build() -> KindRegistry {
        use crate::provider::KindRef;
        let mut entries: Vec<&'static crate::provider::KindEntry> = inventory::iter::<KindRef>.into_iter().map(|r| r.0).collect();
        entries.sort_by_key(|e| e.id);
        let mut providers: Vec<ProviderKind> = Vec::with_capacity(entries.len());
        if entries.is_empty() {
            return KindRegistry { providers, bases: Vec::new() }.with_bases();
        }
        let schema = svelters_host::exports::schema();
        for e in entries {
            assert!(
                KINDS.iter().all(|k| k.id != e.id && k.tag != Some(e.tag)),
                "svelters-egui: provider kind {} (<{}>) collides with a built-in kind",
                e.id,
                e.tag
            );
            assert!(
                providers.iter().all(|p| p.entry.id != e.id && p.entry.tag != e.tag),
                "svelters-egui: provider kind {} (<{}>) is registered twice (same spec id or tag)",
                e.id,
                e.tag
            );
            let types: Vec<TypeRef> = schema
                .kind_types(e.id)
                .unwrap_or_else(|| panic!("svelters-egui: provider kind {} has no types in the export schema", e.id))
                .to_vec();
            let props: Vec<PropDef> = e
                .props
                .iter()
                .enumerate()
                .map(|(i, p)| PropDef {
                    name: p.name,
                    key: P::Ext(i as u16),
                    ty: Ty::Provider,
                    default: Def::Null,
                    nullable: types.get(i).is_some_and(|t| t.slot.has(svelters_abi::SVL_D_NULLABLE)),
                    native: if p.bind { Native::Widget } else { Native::Never },
                    layout: false,
                    doc: p.docs,
                })
                .collect();
            let signals: Vec<SignalDef> =
                e.events.iter().map(|ev| SignalDef { name: ev.name, args: leak(vec![(ev.name, Ty::Provider)]), sync: false, doc: ev.docs }).collect();
            let def: &'static KindDef = Box::leak(Box::new(KindDef {
                id: e.id,
                tag: Some(e.tag),
                native: e.rust,
                // EV2b: a behaviour (attached kind) has its own props only, no `Widget` base
                base: if e.attaches_to.is_some() {
                    None
                } else {
                    Some("Widget")
                },
                toplevel: false,
                slot: None,
                text_content: None,
                present: None,
                props: leak(props),
                signals: leak(signals),
                methods: &[],
                doc: e.docs,
            }));
            providers.push(ProviderKind { entry: e, def, types: leak(types) });
        }
        KindRegistry { providers, bases: Vec::new() }.with_bases()
    }

    fn with_bases(mut self) -> KindRegistry {
        self.bases = self.indexes().map(|k| self.kind(k).base.map(|b| self.kind_index(b).expect("base kind"))).collect();
        self
    }

    /// The kind and its bases, most derived first, without allocating.
    pub fn chain_iter(&self, kind: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(kind), move |&k| self.bases.get(k).copied().flatten())
    }

    /// The built-in kinds, by index (provider kinds follow at `KINDS.len() + local`: [`Self::kind`]).
    pub fn kinds(&self) -> &'static [KindDef] {
        KINDS
    }

    /// The number of kinds, built-in and provider.
    pub fn len(&self) -> usize {
        KINDS.len() + self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The definition of kind index `k` (from [`Self::kind_index`], or a node's `kind`).
    pub fn kind(&self, k: usize) -> &'static KindDef {
        match k.checked_sub(KINDS.len()) {
            None => &KINDS[k],
            Some(local) => self.providers[local].def,
        }
    }

    /// Every kind index, built-in then provider.
    pub fn indexes(&self) -> std::ops::Range<usize> {
        0..self.len()
    }

    /// The provider kinds, sorted by spec id.
    pub fn providers(&self) -> &[ProviderKind] {
        &self.providers
    }

    /// Kind index `k` if it is a provider kind.
    pub fn provider(&self, k: usize) -> Option<&ProviderKind> {
        self.providers.get(k.checked_sub(KINDS.len())?)
    }

    /// Whether `k` is an attached kind (a behaviour, EV2b): inserted into any `Widget` with no slot
    /// or position, never laid out or painted.
    pub fn attaches(&self, k: usize) -> bool {
        self.provider(k)
            .is_some_and(|p| p.entry.attaches_to.is_some())
    }

    /// The behaviours (attached provider kinds), in spec id order.
    pub fn behaviors(&self) -> impl Iterator<Item = &ProviderKind> {
        self.providers
            .iter()
            .filter(|p| p.entry.attaches_to.is_some())
    }

    /// The provider kinds that are elements (not behaviours), in spec id order.
    pub fn elements(&self) -> impl Iterator<Item = &ProviderKind> {
        self.providers
            .iter()
            .filter(|p| p.entry.attaches_to.is_none())
    }

    /// The kind index of a provider kind's entry.
    pub fn provider_index(&self, id: &str) -> Option<usize> {
        self.providers.iter().position(|p| p.entry.id == id).map(|i| KINDS.len() + i)
    }

    /// The JS type of prop `i` of the provider kind with spec id `id`.
    pub fn provider_type(&self, id: &str, i: usize) -> Option<&TypeRef> {
        self.providers.iter().find(|p| p.entry.id == id)?.types.get(i)
    }

    /// Index of a kind by id.
    pub fn kind_index(&self, id: &str) -> Option<usize> {
        self.indexes().find(|&k| self.kind(k).id == id)
    }

    /// Index of a kind by tag.
    pub fn tag_index(&self, tag: &str) -> Option<usize> {
        self.indexes().find(|&k| self.kind(k).tag == Some(tag))
    }

    /// The kind and its bases, most derived first.
    pub fn chain(&self, kind: usize) -> Vec<usize> {
        self.chain_iter(kind).collect()
    }

    // ---- ids ----

    /// The kind id of index `kind`: `index + 1` for built-ins, `62 << 22 | (local + 1)` for providers.
    pub fn kind_id(&self, kind: usize) -> u32 {
        match kind.checked_sub(KINDS.len()) {
            None => kind as u32 + 1,
            Some(local) => PROVIDER_PACK | (local as u32 + 1),
        }
    }

    pub fn kind_of_id(&self, id: u32) -> Option<usize> {
        if id & PACK_MASK == PROVIDER_PACK {
            let local = ((id & !PACK_MASK) as usize).checked_sub(1)?;
            return (local < self.providers.len()).then_some(KINDS.len() + local);
        }
        let i = (id as usize).checked_sub(1)?;
        (i < KINDS.len()).then_some(i)
    }

    fn member_id(owner: usize, i: usize) -> u32 {
        ((owner as u32 + 1) << 16) | (i as u32 + 1)
    }

    fn split(&self, id: u32) -> Option<(usize, usize)> {
        let owner = ((id >> 16) as usize).checked_sub(1)?;
        let i = ((id & 0xffff) as usize).checked_sub(1)?;
        (owner < self.len()).then_some((owner, i))
    }

    /// A property of `kind` (or a base) by name: its id and definition.
    pub fn find_prop(&self, kind: usize, name: &str) -> Option<(u32, &'static PropDef)> {
        self.chain_iter(kind).find_map(|k| {
            let props = self.kind(k).props;
            props.iter().position(|p| p.name == name).map(|i| (Self::member_id(k, i), &props[i]))
        })
    }

    /// The property definition behind an id, if `kind` has it.
    pub fn prop_def(&self, kind: usize, id: u32) -> Option<&'static PropDef> {
        let (owner, i) = self.split(id)?;
        self.chain_iter(kind).any(|k| k == owner).then(|| self.kind(owner).props.get(i)).flatten()
    }

    pub fn find_signal(&self, kind: usize, name: &str) -> Option<(u32, &'static SignalDef)> {
        self.chain_iter(kind).find_map(|k| {
            let signals = self.kind(k).signals;
            signals.iter().position(|s| s.name == name).map(|i| (Self::member_id(k, i), &signals[i]))
        })
    }

    pub fn signal_def(&self, kind: usize, id: u32) -> Option<&'static SignalDef> {
        let (owner, i) = self.split(id)?;
        self.chain_iter(kind).any(|k| k == owner).then(|| self.kind(owner).signals.get(i)).flatten()
    }

    pub fn find_method(&self, kind: usize, name: &str) -> Option<(u32, &'static MethodDef)> {
        self.chain_iter(kind).find_map(|k| {
            let methods = self.kind(k).methods;
            methods.iter().position(|m| m.name == name).map(|i| (Self::member_id(k, i), &methods[i]))
        })
    }

    pub fn method_def(&self, kind: usize, id: u32) -> Option<&'static MethodDef> {
        let (owner, i) = self.split(id)?;
        self.chain_iter(kind).any(|k| k == owner).then(|| self.kind(owner).methods.get(i)).flatten()
    }

    /// The default slot of a container kind (slot id 1; 0 is "default" on the ABI).
    pub fn slot_of(&self, kind: usize) -> Option<&'static str> {
        self.chain_iter(kind).find_map(|k| self.kind(k).slot)
    }

    /// The signal id of `name` on `kind`, or 0.
    pub fn signal_id(&self, kind: usize, name: &str) -> u32 {
        self.find_signal(kind, name).map(|(id, _)| id).unwrap_or(0)
    }

    /// The property id of `key` on `kind`, or 0.
    pub fn prop_id(&self, kind: usize, key: P) -> u32 {
        for k in self.chain(kind) {
            if let Some(i) = self.kind(k).props.iter().position(|p| p.key == key) {
                return Self::member_id(k, i);
            }
        }
        0
    }

    /// The id of prop `i` of provider kind `kind` (0 if out of range).
    pub fn provider_prop_id(&self, kind: usize, i: usize) -> u32 {
        if self.provider(kind).is_some_and(|p| i < p.entry.props.len()) { Self::member_id(kind, i) } else { 0 }
    }

    /// The id of event `i` of provider kind `kind` (0 if out of range).
    pub fn provider_signal_id(&self, kind: usize, i: usize) -> u32 {
        if self.provider(kind).is_some_and(|p| i < p.entry.events.len()) { Self::member_id(kind, i) } else { 0 }
    }

    /// The host descriptor of prop `i` of provider kind `kind`: its export type's slot.
    pub fn provider_member_desc(&self, kind: usize, i: usize) -> Option<svelters_abi::MemberDesc> {
        let p = self.provider(kind)?;
        let name = p.entry.props.get(i)?.name;
        let slot = p.types.get(i)?.slot.clone().named(name);
        Some(svelters_abi::MemberDesc { name: name.to_string(), params: vec![slot.clone()], ret: slot, flags: svelters_abi::SVL_M_OBSERVABLE })
    }
}

/// The definition of kind index `k`, built-in or provider.
pub fn kind(k: usize) -> &'static KindDef {
    KindRegistry::global().kind(k)
}
