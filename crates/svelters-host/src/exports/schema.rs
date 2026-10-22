//! The export schema: everything the macros registered, collected once per process.

use super::*;
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// One member as the host resolves it.
pub struct MemberSchema {
    pub name: String,
    pub rust: &'static str,
    pub kind: MemberKind,
    pub receiver: Receiver,
    pub docs: &'static str,
    pub sig: Sig,
    /// The host descriptor (params, ret); for a property: `params` = the setter's value slot, if any.
    pub desc: MemberDesc,
    pub thunk: Option<Thunk>,
    /// Property setter (props only).
    pub setter: Option<(Thunk, Receiver)>,
    /// Offloaded members (E1): a Promise-returning call that runs as this says.
    pub offload: Option<OffloadDef>,
}

/// An exported object type.
pub struct ClassSchema {
    pub name: &'static str,
    pub rust: &'static str,
    pub kind: svl_id,
    /// `app:<module>.<Name>`
    pub native: String,
    pub shared: bool,
    pub docs: &'static str,
    pub constructor: Option<MemberSchema>,
    pub methods: Vec<MemberSchema>,
    pub statics: Vec<MemberSchema>,
    pub props: Vec<MemberSchema>,
    /// E3: a `#[svelters::store]` class (its props are the store's fields, `exact` by construction).
    pub store: Option<StoreSchema>,
    /// E3: the events of an object with an `Emitter<E>` field (`obj.on(name, fn)`), in variant order.
    pub events: Vec<EventSchema>,
    /// E3: the `Emitter` of an object of this class.
    pub emitter: Option<fn(&AnyObj) -> Option<Arc<reactive::EmitterCore>>>,
}

/// E3: the store behind a store class.
pub struct StoreSchema {
    /// In field (declaration) order.
    pub fields: Vec<StoreFieldSchema>,
    pub subscribe: fn(&AnyObj, &Arc<reactive::StoreSub>) -> bool,
}

/// E3: one field of a store class.
pub struct StoreFieldSchema {
    pub name: &'static str,
    /// The prop id (`resolve(Prop, kind, name)`).
    pub prop: svl_id,
    pub writable: bool,
    pub proportional: bool,
}

impl StoreSchema {
    /// The field behind prop id `prop`: (field index, field).
    pub fn field_of_prop(&self, prop: svl_id) -> Option<(usize, &StoreFieldSchema)> {
        self.fields.iter().enumerate().find(|(_, f)| f.prop == prop)
    }
}

/// E3: one event of a class (`obj.on(name, fn)`).
pub struct EventSchema {
    pub name: &'static str,
    pub latest: bool,
    pub docs: &'static str,
    /// The handler's argument (`None`: no argument).
    pub payload: Option<TypeRef>,
}

/// E3: a module-level value (`export const`): an exported `static` or a service.
pub struct ValueSchema {
    /// `"static"` or `"service"`.
    pub what: &'static str,
    /// Read through `callStatic(module, name)`; `sig.ret` is its type.
    pub member: MemberSchema,
}

/// A module function.
pub type FnSchema = MemberSchema;

/// A declared type: a record, an enum, a tagged union (RA2) or a JSON Schema definition (RA2).
pub struct TypeSchema {
    /// 0 for a JSON Schema definition.
    pub id: svl_id,
    pub name: String,
    pub docs: String,
    /// `record`, `enum`, `union` (`#[js(tag)]` data enum) or `json` (a `$defs` entry of a `Json<T>`).
    pub kind: &'static str,
    pub decl: String,
    /// Records: (name, ts type, optional).
    pub fields: Vec<(String, String, bool)>,
    /// Enums: the values.
    pub values: Vec<String>,
    /// Unions: the tag key.
    pub tag: Option<String>,
    /// Unions: the variants, in declaration order.
    pub variants: Vec<VariantSchema>,
    /// JSON definitions: the JSON Schema (draft 2020-12; `$ref`s point at `#/$defs/<name>`).
    pub schema: Option<serde_json::Value>,
}

/// RA2: one variant of a tagged union.
#[derive(Clone, Debug)]
pub struct VariantSchema {
    /// The tag value.
    pub name: String,
    pub docs: String,
    /// Struct variants: (name, ts type, optional).
    pub fields: Vec<(String, String, bool)>,
    /// Newtype variants: the inner type (its fields sit next to the tag).
    pub inner: Option<String>,
}

/// One `svelters:app/<module>`.
pub struct ModuleSchema {
    pub name: String,
    pub kind: svl_id,
    pub classes: Vec<ClassSchema>,
    pub functions: Vec<FnSchema>,
    /// E3: module-level values (`export const`), by name.
    pub values: Vec<ValueSchema>,
    /// Named types the module refers to (declared in its typings), in first-use order.
    pub types: Vec<TypeSchema>,
    /// Classes of other modules this one refers to: (module, class).
    pub imports: Vec<(String, String)>,
}

/// Every export of this binary.
pub struct Schema {
    pub modules: Vec<ModuleSchema>,
    kinds: HashMap<String, svl_id>,
    /// kind -> (module index, class index or None for the module kind)
    by_kind: HashMap<svl_id, (usize, Option<usize>)>,
    class_kinds: HashMap<TypeId, svl_id>,
    type_ids: HashMap<TypeId, svl_id>,
    types: HashMap<svl_id, TypeDesc>,
    /// The members a module kind or class kind resolves, per name kind: (methods+statics+ctor, props)
    names: HashMap<(u32, svl_id), Vec<String>>,
    /// RA1: the prop and event payload types of provider kinds, by kind spec id.
    kind_types: HashMap<&'static str, Vec<TypeRef>>,
}

/// The schema of this process (built on first use from the `inventory` registrations).
pub fn schema() -> &'static Schema {
    static S: OnceLock<Schema> = OnceLock::new();
    S.get_or_init(build)
}

fn camel_to_name(s: &str) -> String {
    s.to_string()
}

fn member(def: &MemberDef, b: &mut TypeBuilder) -> MemberSchema {
    let sig = (def.sig)(b);
    let mut params: Vec<SlotDesc> = sig.params.iter().map(|p| p.ty.slot.clone().named(p.name)).collect();
    // Only trailing optional parameters may be left out.
    let mut trailing = true;
    for p in params.iter_mut().rev() {
        if !(trailing && p.has(SVL_D_OPTIONAL)) {
            trailing = false;
            p.flags &= !SVL_D_OPTIONAL;
        }
    }
    let flags = match def.kind {
        MemberKind::Static => SVL_M_STATIC,
        MemberKind::Constructor => SVL_M_STATIC | SVL_M_CTOR,
        _ => 0,
    };
    let desc = MemberDesc { name: camel_to_name(def.name), params, ret: sig.ret.slot.clone(), flags: flags | SVL_M_THROWS };
    MemberSchema {
        name: def.name.to_string(),
        rust: def.rust,
        kind: def.kind,
        receiver: def.receiver,
        docs: def.docs,
        sig,
        desc,
        thunk: Some(def.thunk),
        setter: None,
        offload: def.offload,
    }
}

/// Folds getters and setters into one property each (a setter alone is a write-only prop).
fn add_accessor(props: &mut Vec<MemberSchema>, def: &MemberDef, b: &mut TypeBuilder) {
    let m = member(def, b);
    match def.kind {
        MemberKind::Getter => match props.iter_mut().find(|p| p.name == def.name) {
            Some(p) => {
                p.thunk = m.thunk;
                p.sig.ret = m.sig.ret.clone();
                p.desc.ret = m.desc.ret;
                p.receiver = m.receiver;
                if p.docs.is_empty() {
                    p.docs = m.docs;
                }
            }
            None => {
                let mut m = m;
                m.desc.params.clear();
                m.sig.params.clear();
                props.push(m);
            }
        },
        _ => {
            let value = m.desc.params.first().cloned().unwrap_or_else(|| SlotDesc::new(SVL_NULL));
            let vparam = m.sig.params.first().cloned();
            match props.iter_mut().find(|p| p.name == def.name) {
                Some(p) => {
                    p.setter = Some((def.thunk, def.receiver));
                    p.desc.params = vec![value];
                    p.sig.params = vparam.into_iter().collect();
                }
                None => props.push(MemberSchema {
                    name: def.name.to_string(),
                    rust: def.rust,
                    kind: MemberKind::Getter,
                    receiver: def.receiver,
                    docs: def.docs,
                    sig: Sig { params: vparam.into_iter().collect(), ret: TypeRef::void(), throws: m.sig.throws.clone(), progress: None, items: None },
                    desc: MemberDesc { name: def.name.to_string(), params: vec![value], ret: SlotDesc::new(SVL_NULL), flags: 0 },
                    thunk: None,
                    setter: Some((def.thunk, def.receiver)),
                    offload: None,
                }),
            }
        }
    }
}

fn build() -> Schema {
    // SV48: a JSON definition named like a svelters type or class would be declared twice. The names
    // are known only once everything is described, so build again with them reserved.
    let (s, names) = build_with(BTreeSet::new());
    if names.is_empty() { s } else { build_with(names).0 }
}

/// The schema, and the svelters type and class names that JSON definitions took too (empty if none).
fn build_with(json_reserved: BTreeSet<String>) -> (Schema, BTreeSet<String>) {
    let mut objects: Vec<&ObjectDef> = inventory::iter::<ObjectDef>.into_iter().collect();
    objects.sort_by(|a, b| (a.module, a.name).cmp(&(b.module, b.name)));
    let mut fns: Vec<&FnDef> = inventory::iter::<FnDef>.into_iter().collect();
    fns.sort_by(|a, b| (a.module, a.member.name).cmp(&(b.module, b.member.name)));
    let impls: Vec<&ImplDef> = inventory::iter::<ImplDef>.into_iter().collect();
    let store_defs: Vec<&reactive::StoreDef> = inventory::iter::<reactive::StoreDef>.into_iter().collect();
    let emitter_defs: Vec<&reactive::EmitterDef> = inventory::iter::<reactive::EmitterDef>.into_iter().collect();
    let mut value_defs: Vec<&reactive::ValueDef> = inventory::iter::<reactive::ValueDef>.into_iter().collect();
    value_defs.sort_by(|a, b| (a.module, a.name).cmp(&(b.module, b.name)));
    let mut kind_defs: Vec<&KindTypesDef> = inventory::iter::<KindTypesDef>.into_iter().collect();
    kind_defs.sort_by(|a, b| (a.module, a.kind).cmp(&(b.module, b.kind)));
    let mut kind_types: HashMap<&'static str, Vec<TypeRef>> = HashMap::new();

    // Module names: every object's, function's and value's, and the namespaces of provider kinds (RA1).
    let mut module_names: Vec<&str> = objects
        .iter()
        .map(|o| o.module)
        .chain(fns.iter().map(|f| f.module))
        .chain(value_defs.iter().map(|v| v.module))
        .chain(kind_defs.iter().map(|k| k.module))
        .collect();
    module_names.sort_unstable();
    module_names.dedup();

    // Kind ids: module kinds and class kinds, in name order.
    let mut kind_names: Vec<String> = module_names.iter().map(|m| format!("app:{m}")).collect();
    kind_names.extend(objects.iter().map(|o| format!("app:{}.{}", o.module, o.name)));
    kind_names.sort();
    let kinds: HashMap<String, svl_id> = kind_names.iter().enumerate().map(|(i, n)| (n.clone(), APP_PACK | (i as u32 + 1))).collect();
    let class_list: HashMap<TypeId, (svl_id, &'static str, &'static str)> = objects
        .iter()
        .map(|o| ((o.type_id)(), (kinds[&format!("app:{}.{}", o.module, o.name)], o.module, o.name)))
        .collect();

    let mut b = TypeBuilder::new(class_list.clone());
    b.json_reserved = json_reserved;
    let mut modules: Vec<ModuleSchema> = Vec::new();
    for m in &module_names {
        b.module = m.to_string();
        let mut classes = Vec::new();
        for o in objects.iter().filter(|o| o.module == *m) {
            let tid = (o.type_id)();
            let mut c = ClassSchema {
                name: o.name,
                rust: o.rust,
                kind: kinds[&format!("app:{}.{}", o.module, o.name)],
                native: format!("app:{}.{}", o.module, o.name),
                shared: o.shared,
                docs: o.docs,
                constructor: None,
                methods: Vec::new(),
                statics: Vec::new(),
                props: Vec::new(),
                store: None,
                events: Vec::new(),
                emitter: None,
            };
            let mut defs: Vec<&MemberDef> = impls.iter().filter(|i| (i.type_id)() == tid).flat_map(|i| i.members.iter()).collect();
            defs.sort_by_key(|d| d.name);
            for d in defs {
                match d.kind {
                    MemberKind::Constructor => c.constructor = Some(member(d, &mut b)),
                    MemberKind::Method => c.methods.push(member(d, &mut b)),
                    MemberKind::Static => c.statics.push(member(d, &mut b)),
                    MemberKind::Getter | MemberKind::Setter => add_accessor(&mut c.props, d, &mut b),
                }
            }
            if let Some(sd) = store_defs.iter().find(|s| (s.type_id)() == tid) {
                let fields = sd
                    .fields
                    .iter()
                    .map(|f| StoreFieldSchema {
                        name: f.name,
                        prop: c.props.iter().position(|p| p.name == f.name).map_or(0, |i| i as svl_id + 1),
                        writable: f.writable,
                        proportional: f.proportional,
                    })
                    .collect();
                c.store = Some(StoreSchema { fields, subscribe: sd.subscribe });
            }
            if let Some(ed) = emitter_defs.iter().find(|e| (e.type_id)() == tid) {
                let payloads = (ed.describe)(&mut b);
                c.events = ed
                    .events
                    .iter()
                    .enumerate()
                    .map(|(i, e)| EventSchema { name: e.name, latest: e.latest, docs: e.docs, payload: payloads.get(i).cloned().flatten() })
                    .collect();
                c.emitter = Some(ed.emitter);
            }
            classes.push(c);
        }
        let functions = fns.iter().filter(|f| f.module == *m).map(|f| member(&f.member, &mut b)).collect();
        let values = value_defs
            .iter()
            .filter(|v| v.module == *m)
            .map(|v| {
                let ty = (v.ty)(&mut b);
                let sig = Sig { params: Vec::new(), ret: ty.clone(), throws: Vec::new(), progress: None, items: None };
                let desc = MemberDesc { name: v.name.to_string(), params: Vec::new(), ret: ty.slot.clone(), flags: SVL_M_STATIC | SVL_M_THROWS };
                ValueSchema {
                    what: v.what,
                    member: MemberSchema {
                        name: v.name.to_string(),
                        rust: v.rust,
                        kind: MemberKind::Static,
                        receiver: Receiver::None,
                        docs: v.docs,
                        sig,
                        desc,
                        thunk: Some(v.get),
                        setter: None,
                        offload: None,
                    },
                }
            })
            .collect();
        for k in kind_defs.iter().filter(|k| k.module == *m) {
            kind_types.insert(k.kind, (k.describe)(&mut b));
        }
        modules.push(ModuleSchema { name: m.to_string(), kind: kinds[&format!("app:{m}")], classes, functions, values, types: Vec::new(), imports: Vec::new() });
    }

    // Declared types per module: the used ones, closed over their dependencies.
    let defs = b.json_gen.definitions().clone();
    for m in &mut modules {
        let mut order: Vec<TypeId> = Vec::new();
        let mut todo: Vec<TypeId> = b.used.get(&m.name).cloned().unwrap_or_default();
        todo.reverse();
        while let Some(t) = todo.pop() {
            if order.contains(&t) {
                continue;
            }
            order.push(t);
            if let Some(n) = b.named.get(&t) {
                for d in n.deps.iter().rev() {
                    todo.push(*d);
                }
            }
        }
        m.types = order
            .iter()
            .filter_map(|t| b.named.get(t))
            .map(|n| TypeSchema {
                id: n.id,
                name: n.name.clone(),
                docs: n.docs.clone(),
                kind: n.kind,
                decl: n.decl.clone(),
                fields: n.fields.clone(),
                values: if n.kind == "enum" { n.values.clone() } else { Vec::new() },
                tag: n.tag.clone(),
                variants: n.variants.clone(),
                schema: None,
            })
            .collect();
        // RA2: the JSON Schema definitions the module (or its declared types) refers to, closed over
        // their own `$ref`s.
        let mut json_todo: Vec<String> = b.json_used.get(&m.name).cloned().unwrap_or_default();
        json_todo.extend(order.iter().filter_map(|t| b.named.get(t)).flat_map(|n| n.json_deps.iter().cloned()));
        let mut json_done: Vec<String> = Vec::new();
        let mut i = 0;
        while i < json_todo.len() {
            let name = json_todo[i].clone();
            i += 1;
            if json_done.contains(&name) {
                continue;
            }
            json_done.push(name.clone());
            let Some(def) = defs.get(&name) else { continue };
            let mut refs = json::Refs::new(&b.json_reserved);
            let ts = json::schema_ts(def, &mut refs);
            json_todo.extend(refs.names);
            let docs = def.get("description").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
            let ident = json::json_ident(&name, &b.json_reserved);
            m.types.push(TypeSchema {
                id: 0,
                name: ident.clone(),
                docs,
                kind: "json",
                decl: format!("type {ident} = {ts};"),
                fields: Vec::new(),
                values: Vec::new(),
                tag: None,
                variants: Vec::new(),
                schema: Some(def.clone()),
            });
        }
        m.imports = b.imports.get(&m.name).cloned().unwrap_or_default();
    }

    let mut by_kind = HashMap::new();
    let mut names = HashMap::new();
    for (mi, m) in modules.iter().enumerate() {
        by_kind.insert(m.kind, (mi, None));
        names.insert((NameKind::Method as u32, m.kind), m.functions.iter().map(|f| f.name.clone()).chain(m.values.iter().map(|v| v.member.name.clone())).collect());
        names.insert((NameKind::Prop as u32, m.kind), Vec::new());
        for (ci, c) in m.classes.iter().enumerate() {
            by_kind.insert(c.kind, (mi, Some(ci)));
            let mut methods: Vec<String> = c.methods.iter().chain(c.statics.iter()).map(|x| x.name.clone()).collect();
            if c.constructor.is_some() {
                methods.push("constructor".into());
            }
            names.insert((NameKind::Method as u32, c.kind), methods);
            names.insert((NameKind::Prop as u32, c.kind), c.props.iter().map(|p| p.name.clone()).collect());
            names.insert((NameKind::Signal as u32, c.kind), c.events.iter().map(|e| e.name.to_string()).collect());
        }
    }
    let mut type_ids = HashMap::new();
    let mut types = HashMap::new();
    for (t, n) in &b.named {
        type_ids.insert(*t, n.id);
        // A tagged union crosses as a plain map: the host needs no type info for it.
        if n.kind != "union" {
            types.insert(n.id, n.desc.clone());
        }
    }
    for (t, (id, d)) in &b.anon {
        type_ids.insert(*t, *id);
        types.insert(*id, d.clone());
    }
    // SV48: svelters type and class names that a JSON definition took as well.
    let svelters_names: BTreeSet<String> = b.named.values().map(|n| n.name.clone()).chain(class_list.values().map(|(_, _, c)| c.to_string())).collect();
    let clashes = defs.keys().any(|d| svelters_names.contains(&json::json_ident(d, &b.json_reserved)));
    let schema = Schema {
        modules,
        kinds,
        by_kind,
        class_kinds: class_list.into_iter().map(|(t, (k, _, _))| (t, k)).collect(),
        type_ids,
        types,
        names,
        kind_types,
    };
    (schema, if clashes { svelters_names } else { BTreeSet::new() })
}

impl Schema {
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// The id of `app:<module>` or `app:<module>.<Class>`.
    pub fn kind(&self, name: &str) -> Option<svl_id> {
        self.kinds.get(name).copied()
    }

    pub fn kind_name(&self, kind: svl_id) -> Option<String> {
        let (mi, ci) = self.by_kind.get(&kind)?;
        let m = &self.modules[*mi];
        Some(match ci {
            Some(ci) => m.classes[*ci].native.clone(),
            None => format!("app:{}", m.name),
        })
    }

    pub fn module_of(&self, kind: svl_id) -> Option<&ModuleSchema> {
        self.by_kind.get(&kind).map(|(mi, _)| &self.modules[*mi])
    }

    pub fn class(&self, kind: svl_id) -> Option<&ClassSchema> {
        let (mi, ci) = self.by_kind.get(&kind)?;
        ci.map(|ci| &self.modules[*mi].classes[ci])
    }

    pub fn module(&self, name: &str) -> Option<&ModuleSchema> {
        self.modules.iter().find(|m| m.name == name)
    }

    /// Member id (1-based) of `name` in `kind`'s method, prop or (E3) signal namespace.
    pub fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        let ns = match what {
            NameKind::Prop | NameKind::Signal => what,
            _ => NameKind::Method,
        };
        self.names
            .get(&(ns as u32, kind))
            .and_then(|v| v.iter().position(|n| n == name))
            .map_or(0, |i| i as svl_id + 1)
    }

    /// The member behind an id from [`Schema::resolve`].
    pub fn member(&self, what: NameKind, kind: svl_id, id: svl_id) -> Option<&MemberSchema> {
        if what == NameKind::Signal {
            return None;
        }
        let ns = if what == NameKind::Prop { NameKind::Prop } else { NameKind::Method };
        let name = self.names.get(&(ns as u32, kind))?.get((id as usize).checked_sub(1)?)?;
        let (mi, ci) = self.by_kind.get(&kind)?;
        let m = &self.modules[*mi];
        match ci {
            None => m.functions.iter().find(|f| &f.name == name).or_else(|| m.values.iter().map(|v| &v.member).find(|v| &v.name == name)),
            Some(ci) => {
                let c = &m.classes[*ci];
                if ns == NameKind::Prop {
                    return c.props.iter().find(|p| &p.name == name);
                }
                if name == "constructor" {
                    if let Some(k) = &c.constructor {
                        return Some(k);
                    }
                }
                c.methods.iter().chain(c.statics.iter()).find(|x| &x.name == name)
            }
        }
    }

    /// RA1: the prop and event payload types of a provider kind (props first, then events), as its
    /// [`KindTypesDef`] described them inside this schema.
    pub fn kind_types(&self, kind: &str) -> Option<&[TypeRef]> {
        self.kind_types.get(kind).map(Vec::as_slice)
    }

    pub fn type_desc(&self, id: svl_id) -> Option<&TypeDesc> {
        self.types.get(&id)
    }

    pub(crate) fn type_id_of(&self, t: TypeId) -> Option<svl_id> {
        self.type_ids.get(&t).copied()
    }

    pub(crate) fn class_kind(&self, t: TypeId) -> Option<svl_id> {
        self.class_kinds.get(&t).copied()
    }

    /// A stable fingerprint of the JS-visible schema (the JSON dump's hash).
    pub fn fingerprint(&self) -> String {
        let j = self.json();
        let mut h: u64 = 0xcbf29ce484222325;
        for b in j.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("{h:016x}")
    }
}
