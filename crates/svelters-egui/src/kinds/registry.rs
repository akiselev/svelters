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
    /// RA3: the kind's methods (`#[svelters::widget] impl`), if it has any.
    pub methods: Option<&'static crate::provider::KindMethods>,
    /// RA3: the JS types of the methods: each method's parameters, then its result.
    pub method_types: &'static [TypeRef],
}

impl ProviderKind {
    /// The JS type of event `i`'s payload.
    pub fn event_type(&self, i: usize) -> Option<&TypeRef> {
        self.types.get(self.entry.props.len() + i)
    }

    /// RA3: method `i`'s entry, parameter types and result type.
    pub fn method(&self, i: usize) -> Option<(&'static crate::provider::MethodEntry, &[TypeRef], &TypeRef)> {
        let ms = self.methods?.methods;
        let m = ms.get(i)?;
        let at: usize = ms[..i].iter().map(|m| m.params.len() + 1).sum();
        let params = self.method_types.get(at..at + m.params.len())?;
        let ret = self.method_types.get(at + m.params.len())?;
        Some((m, params, ret))
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
    /// OW0: own widgets that replace a built-in kind (`replaces = "<tag>"`), by built-in index.
    replacements: Vec<Replacement>,
    /// The base kind index of each kind (SG2: member lookups walk the chain without allocating).
    bases: Vec<Option<usize>>,
}

/// OW0 (ARCH-WIDGETS §6.2): an own widget implementing built-in kind `kind`. The kind's row stays
/// (tag, prop and signal ids); the struct's props and events are routed to it by name.
pub struct Replacement {
    /// The built-in kind index.
    pub kind: usize,
    pub entry: &'static crate::provider::KindEntry,
    /// Struct prop `i` → the built-in prop id.
    props: Vec<u32>,
    /// Struct event `i` → the built-in signal id.
    signals: Vec<u32>,
    /// The struct's props the row lacks: extra props of the built-in kind, ids after the row's.
    pub extra_props: &'static [PropDef],
    /// The struct's events the row lacks: extra signals of the kind.
    pub extra_signals: &'static [SignalDef],
    /// The row with the extras appended (the compiler's spec and the typings).
    pub merged: &'static KindDef,
}

impl Replacement {
    /// Checks that the struct covers built-in kind `kind`'s own props (those not stored in
    /// `Common`) and signals with the same names; its other props and events become extras of the
    /// kind (a kind-table type each). Returns the routing.
    fn new(reg: &KindRegistry, kind: usize, entry: &'static crate::provider::KindEntry) -> Result<Replacement, String> {
        let def = reg.kind(kind);
        let tag = def.tag.unwrap_or(def.id);
        let mut errors = Vec::new();
        for p in def.props.iter().filter(|p| !crate::scene::props::is_common(p.key)) {
            if !entry.props.iter().any(|e| e.name == p.name) {
                errors.push(format!("prop `{}` is missing", p.name));
            }
        }
        for s in def.signals {
            if !entry.events.iter().any(|e| e.name == s.name) {
                errors.push(format!("event `{}` is missing", s.name));
            }
        }
        // a fresh struct: the extras' defaults
        let fresh = (entry.new)();
        let mut extra_props: Vec<PropDef> = Vec::new();
        let props: Vec<u32> = entry
            .props
            .iter()
            .enumerate()
            .map(|(i, e)| match reg.find_prop(kind, e.name) {
                Some((id, d)) if !crate::scene::props::is_common(d.key) => id,
                Some(_) => {
                    errors.push(format!("prop `{}` is a common prop of every element", e.name));
                    0
                }
                None => {
                    let Some(ty) = row_ty(e.row_ty) else {
                        errors.push(format!("extra prop `{}`: not a kind-table type (bool, integer, float, String, `enumeration`)", e.name));
                        return 0;
                    };
                    let v = svelters_host::exports::__private::with_cx(entry.namespace, |cx| fresh.get_prop(i, cx)).unwrap_or(svelters_abi::Value::Null);
                    let default = match v {
                        svelters_abi::Value::Bool(b) => Def::Bool(b),
                        svelters_abi::Value::I64(n) => Def::I64(n),
                        svelters_abi::Value::F64(f) => Def::F64(f),
                        svelters_abi::Value::Str(s) => Def::Str(Box::leak(s.into_boxed_str())),
                        _ => Def::Null,
                    };
                    extra_props.push(PropDef {
                        name: e.name,
                        key: P::Ext(i as u16),
                        ty,
                        default,
                        nullable: e.nullable,
                        native: if e.bind { Native::Widget } else { Native::Never },
                        layout: false,
                        doc: e.docs,
                    });
                    KindRegistry::member_id(kind, def.props.len() + extra_props.len() - 1)
                }
            })
            .collect();
        let mut extra_signals: Vec<SignalDef> = Vec::new();
        let signals: Vec<u32> = entry
            .events
            .iter()
            .map(|e| match reg.find_signal(kind, e.name) {
                Some((id, _)) => id,
                None => {
                    let args: &'static [(&'static str, Ty)] = match e.arg.map(row_ty) {
                        None => &[],
                        Some(Some(t)) => leak(vec![("value", t)]),
                        Some(None) => {
                            errors.push(format!("extra event `{}`: its payload is not a kind-table type", e.name));
                            &[]
                        }
                    };
                    extra_signals.push(SignalDef { name: e.name, args, sync: false, doc: e.docs });
                    KindRegistry::member_id(kind, def.signals.len() + extra_signals.len() - 1)
                }
            })
            .collect();
        if errors.is_empty() {
            let extra_props = leak(extra_props);
            let extra_signals = leak(extra_signals);
            let merged: &'static KindDef = Box::leak(Box::new(KindDef {
                props: leak(def.props.iter().cloned().chain(extra_props.iter().cloned()).collect()),
                signals: leak(def.signals.iter().cloned().chain(extra_signals.iter().cloned()).collect()),
                ..def.clone()
            }));
            Ok(Replacement { kind, entry, props, signals, extra_props, extra_signals, merged })
        } else {
            Err(format!("svelters-egui: own widget {} does not replace <{tag}>: {}", entry.id, errors.join("; ")))
        }
    }
}

/// The kind-table type of an own widget's prop.
fn row_ty(t: crate::provider::RowTy) -> Option<Ty> {
    use crate::provider::RowTy as R;
    Some(match t {
        R::Bool => Ty::Bool,
        R::I64 => Ty::I64,
        R::F64 => Ty::F64,
        R::Str => Ty::Str,
        R::Enum(v) => Ty::Enum(v),
        R::Other => return None,
    })
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
        // OW0: own widgets replacing a built-in are routed after the bases are known
        let own: Vec<&'static crate::provider::KindEntry> = entries.iter().copied().filter(|e| e.replaces.is_some()).collect();
        entries.retain(|e| e.replaces.is_none());
        let mut providers: Vec<ProviderKind> = Vec::with_capacity(entries.len());
        if entries.is_empty() {
            return KindRegistry { providers, replacements: Vec::new(), bases: Vec::new() }.with_bases().with_replacements(own);
        }
        let schema = svelters_host::exports::schema();
        // RA3: the kinds' methods, by spec id (one `#[svelters::widget] impl` per kind)
        let mut methods: std::collections::HashMap<&'static str, &'static crate::provider::KindMethods> = std::collections::HashMap::new();
        for km in inventory::iter::<crate::provider::KindMethods> {
            let id = (km.kind)().id;
            assert!(methods.insert(id, km).is_none(), "svelters-egui: provider kind {id} has two #[svelters::widget] impl blocks (put its methods in one)");
        }
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
            let km = methods.remove(e.id);
            let method_types: Vec<TypeRef> = match km {
                Some(_) => schema
                    .kind_types(&crate::provider::methods_types_key(e.id))
                    .unwrap_or_else(|| panic!("svelters-egui: the methods of provider kind {} have no types in the export schema", e.id))
                    .to_vec(),
                None => Vec::new(),
            };
            let method_defs: Vec<MethodDef> = km.map_or(&[][..], |km| km.methods).iter().map(|m| MethodDef { name: m.name, doc: m.docs }).collect();
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
                // RA3: a container kind (`caps = CONTAINER`) takes JS children in its default slot;
                // taffy lays them out from its style (or `Widget::layout`), `cx.children` draws them
                slot: (e.caps.contains(crate::provider::Caps::CONTAINER) && e.attaches_to.is_none()).then_some("children"),
                text_content: None,
                present: None,
                props: leak(props),
                signals: leak(signals),
                methods: leak(method_defs),
                doc: e.docs,
            }));
            providers.push(ProviderKind { entry: e, def, types: leak(types), methods: km, method_types: leak(method_types) });
        }
        if let Some(id) = methods.keys().next() {
            panic!("svelters-egui: #[svelters::widget] impl for {id}, which is not a registered provider kind");
        }
        KindRegistry { providers, replacements: Vec::new(), bases: Vec::new() }.with_bases().with_replacements(own)
    }

    /// OW0: routes each own widget to the built-in kind it replaces; a tag that is not a built-in,
    /// a kind replaced twice or a struct that does not cover the row fails at startup.
    fn with_replacements(mut self, own: Vec<&'static crate::provider::KindEntry>) -> KindRegistry {
        for e in own {
            let tag = e.replaces.expect("filtered");
            let kind = KINDS.iter().position(|k| k.tag == Some(tag)).unwrap_or_else(|| panic!("svelters-egui: own widget {} replaces <{tag}>, which is not a built-in kind", e.id));
            assert!(self.replacements.iter().all(|r| r.kind != kind), "svelters-egui: <{tag}> is replaced twice ({})", e.id);
            let r = Replacement::new(&self, kind, e).unwrap_or_else(|m| panic!("{m}"));
            self.replacements.push(r);
        }
        self
    }

    /// OW0: the own widget that replaces built-in kind `kind`, if any.
    pub fn replacement(&self, kind: usize) -> Option<&Replacement> {
        self.replacements.iter().find(|r| r.kind == kind)
    }

    /// OW0: every own widget that replaces a built-in kind.
    pub fn replacements(&self) -> &[Replacement] {
        &self.replacements
    }

    /// The id of prop `i` of the `W::Ext` struct of a node of kind `kind`: a provider kind's
    /// member, or (OW0) the built-in prop an own widget's prop stands for. 0 if out of range.
    pub fn ext_prop_id(&self, kind: usize, i: usize) -> u32 {
        match self.replacement(kind) {
            Some(r) => r.props.get(i).copied().unwrap_or(0),
            None => self.provider_prop_id(kind, i),
        }
    }

    /// The id of event `i` of the `W::Ext` struct of a node of kind `kind` (see
    /// [`Self::ext_prop_id`]).
    pub fn ext_signal_id(&self, kind: usize, i: usize) -> u32 {
        match self.replacement(kind) {
            Some(r) => r.signals.get(i).copied().unwrap_or(0),
            None => self.provider_signal_id(kind, i),
        }
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

    /// OW0: the extra props of kind `k` (an own widget's props its built-in row lacks).
    fn extra_props(&self, k: usize) -> &'static [PropDef] {
        self.replacement(k).map_or(&[], |r| r.extra_props)
    }

    fn extra_signals(&self, k: usize) -> &'static [SignalDef] {
        self.replacement(k).map_or(&[], |r| r.extra_signals)
    }

    /// The definition of kind `k` as the compiler and the typings see it: the row with an own
    /// widget's extras appended (the row itself otherwise).
    pub fn spec_kind(&self, k: usize) -> &'static KindDef {
        self.replacement(k).map_or_else(|| self.kind(k), |r| r.merged)
    }

    /// A property of `kind` (or a base) by name: its id and definition.
    pub fn find_prop(&self, kind: usize, name: &str) -> Option<(u32, &'static PropDef)> {
        self.chain_iter(kind).find_map(|k| {
            let props = self.kind(k).props;
            props.iter().position(|p| p.name == name).map(|i| (Self::member_id(k, i), &props[i])).or_else(|| {
                let extra = self.extra_props(k);
                extra.iter().position(|p| p.name == name).map(|i| (Self::member_id(k, props.len() + i), &extra[i]))
            })
        })
    }

    /// The property definition behind an id, if `kind` has it.
    pub fn prop_def(&self, kind: usize, id: u32) -> Option<&'static PropDef> {
        let (owner, i) = self.split(id)?;
        if !self.chain_iter(kind).any(|k| k == owner) {
            return None;
        }
        let props = self.kind(owner).props;
        props.get(i).or_else(|| self.extra_props(owner).get(i.checked_sub(props.len())?))
    }

    pub fn find_signal(&self, kind: usize, name: &str) -> Option<(u32, &'static SignalDef)> {
        self.chain_iter(kind).find_map(|k| {
            let signals = self.kind(k).signals;
            signals.iter().position(|s| s.name == name).map(|i| (Self::member_id(k, i), &signals[i])).or_else(|| {
                let extra = self.extra_signals(k);
                extra.iter().position(|s| s.name == name).map(|i| (Self::member_id(k, signals.len() + i), &extra[i]))
            })
        })
    }

    pub fn signal_def(&self, kind: usize, id: u32) -> Option<&'static SignalDef> {
        let (owner, i) = self.split(id)?;
        if !self.chain_iter(kind).any(|k| k == owner) {
            return None;
        }
        let signals = self.kind(owner).signals;
        signals.get(i).or_else(|| self.extra_signals(owner).get(i.checked_sub(signals.len())?))
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

    /// RA3: the host descriptor of method `i` of provider kind `kind`: its parameters' slots (only
    /// trailing `Option`s may be left out) and its result's.
    pub fn provider_method_desc(&self, kind: usize, i: usize) -> Option<svelters_abi::MemberDesc> {
        use svelters_abi::SVL_D_OPTIONAL;
        let (m, params, ret) = self.provider(kind)?.method(i)?;
        let mut slots: Vec<svelters_abi::SlotDesc> = m.params.iter().zip(params).map(|(n, t)| t.slot.clone().named(n)).collect();
        let mut trailing = true;
        for p in slots.iter_mut().rev() {
            if !(trailing && p.has(SVL_D_OPTIONAL)) {
                trailing = false;
                p.flags &= !SVL_D_OPTIONAL;
            }
        }
        Some(svelters_abi::MemberDesc { name: m.name.to_string(), params: slots, ret: ret.slot.clone(), flags: svelters_abi::SVL_M_THROWS })
    }

    /// RA3: the index of method id `id` among provider kind `kind`'s own methods.
    pub fn provider_method_index(&self, kind: usize, id: u32) -> Option<usize> {
        let (owner, i) = self.split(id)?;
        (owner == kind && self.provider(kind)?.methods.is_some_and(|m| i < m.methods.len())).then_some(i)
    }
}

/// The definition of kind index `k`, built-in or provider.
pub fn kind(k: usize) -> &'static KindDef {
    KindRegistry::global().kind(k)
}
