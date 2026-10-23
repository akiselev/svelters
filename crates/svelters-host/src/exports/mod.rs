//! Rust exports (E0, `docs/notes/egui-rust-exports.md`): the app's own Rust types and functions,
//! exposed to Svelte as `svelters:app/<module>` modules.
//!
//! Exports never go through the backend C ABI. The macros of the `svelters` facade crate
//! (`#[derive(svelters::Record/Enum/Error)]`, `#[svelters::object]`, `#[svelters::export]`) register
//! static descriptions with `inventory`; [`schema()`] collects them once per process into kinds named
//! `app:<module>.<Type>` (ids in the reserved pack 63) with ordinary [`MemberDesc`]s, so the host
//! converts and validates arguments exactly as for a backend. Each host keeps its own object table:
//! handles with [`APP_HANDLE_BIT`] set, routed by the host ops (`construct`, `call`, `callStatic`,
//! `getProp`, `setProp`, `destroy`) to thunks the macros generated.
//!
//! Nothing here is engine specific: thunks see owned [`Value`]s.
//!
//! The items marked `#[doc(hidden)]` are the macros' contract, not an API.

pub(crate) mod buffers;
mod codegen;
pub mod json;
pub(crate) mod objects;
pub mod offload;
pub mod reactive;
mod schema;
pub mod service;
pub mod stream;

pub use buffers::{Buffer, Element, Shared};
pub(crate) use codegen::builtin_module;
pub use codegen::{DumpReport, dump, dump_if_debug, javascript, json, typescript};
pub use json::{Json, JsonAny};
pub use offload::{Cancelled, CancelledFuture, Exec, Spawned, WorkCx, blocking, set_worker_pool, worker};
#[cfg(feature = "tokio")]
pub use offload::set_tokio_handle;
pub use reactive::{Emitter, EventInfo, EventSet, ExportStatic, Store, StoreField, StoreHooks, StoreState, StoreStatic};
pub use schema::{ClassSchema, EventSchema, FnSchema, MemberSchema, ModuleSchema, Schema, StoreSchema, TypeSchema, ValueSchema, VariantSchema, schema};
pub use service::{ServiceCx, StopToken, shutdown_services, start_services};
pub use stream::{Closed, Sink, Stream, TrySendError, channel};

use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
pub use svelters_abi::{NativeError, SlotDesc, Value};
use svelters_abi::*;

/// Kind and type ids of app exports live in pack 63 (bits 27..22, CUSTOM-WIDGETS.md §3.2).
pub const APP_PACK: u32 = 63 << 22;
const PACK_MASK: u32 = 0x3F << 22;
/// Handles of app objects have this bit set. Bit 63 (the design note's choice) cannot cross into JS,
/// whose handles are numbers below 2^53, so the app range is [2^52, 2^53).
pub const APP_HANDLE_BIT: u64 = 1 << 52;
const MAX_HANDLE: u64 = 1 << 53;

/// An id (kind or type) of the app export range.
pub fn is_app_id(id: svl_id) -> bool {
    id & PACK_MASK == APP_PACK
}

/// A handle of the app object range.
pub fn is_app_handle(h: svl_handle) -> bool {
    h & APP_HANDLE_BIT != 0 && h < MAX_HANDLE
}

// ---- type descriptions ----

/// What a Rust type is on the JS side: the host descriptor slot (drives conversion and validation),
/// the TypeScript type, and an optional JS expression `(x) => ...` that wraps a raw result (objects).
#[derive(Clone, Debug)]
pub struct TypeRef {
    pub slot: SlotDesc,
    pub ts: String,
    pub wrap: Option<String>,
}

impl TypeRef {
    pub fn new(slot: SlotDesc, ts: impl Into<String>) -> TypeRef {
        TypeRef { slot, ts: ts.into(), wrap: None }
    }
    fn void() -> TypeRef {
        TypeRef::new(SlotDesc::new(SVL_NULL), "void")
    }
}

/// A Rust type that has a JS shape.
pub trait JsType {
    fn describe(b: &mut TypeBuilder) -> TypeRef;
    /// Error codes a `Result` can carry (`@throws` in the typings).
    fn throws() -> Vec<&'static str> {
        Vec::new()
    }
}

/// A JS argument becomes `Self`. The host already validated the value against [`JsType::describe`]'s
/// slot, so a mismatch here is an internal error.
pub trait FromJs: JsType + Sized {
    fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError>;
}

/// `Self` becomes a JS result.
pub trait IntoJs: JsType {
    fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError>;
}

/// A field of a `#[derive(Record)]` struct.
pub struct FieldDef {
    pub name: &'static str,
    pub ty: TypeRef,
    /// `Option<T>` or `#[js(default)]`: may be missing on the JS side.
    pub optional: bool,
    pub docs: &'static str,
}

/// One variant of a `#[derive(Enum)] #[js(tag = "...")]` data enum (RA2).
pub struct VariantDef {
    /// The tag value (kebab-case, or `#[js(rename)]`).
    pub name: &'static str,
    pub docs: &'static str,
    /// Named fields (a struct variant).
    pub fields: Vec<FieldDef>,
    /// A newtype variant's inner type: its fields sit next to the tag.
    pub inner: Option<TypeRef>,
}

/// A declared TypeScript type (record interface, enum union or tagged union).
#[derive(Clone, Debug)]
pub(crate) struct NamedType {
    pub id: svl_id,
    pub name: String,
    pub docs: String,
    /// `interface X { ... }` or `type X = ...;`
    pub decl: String,
    pub desc: TypeDesc,
    /// Named types this one refers to.
    pub deps: Vec<TypeId>,
    pub kind: &'static str,
    pub fields: Vec<(String, String, bool)>,
    pub values: Vec<String>,
    /// The JS expression wrapping a raw value (objects inside), if any.
    pub wrap: Option<String>,
    /// RA2 tagged unions: the tag key and the variants.
    pub tag: Option<String>,
    pub variants: Vec<schema::VariantSchema>,
    /// RA2: JSON Schema definitions this type refers to (`Json<T>` fields).
    pub json_deps: Vec<String>,
}

/// Builds descriptors while the schema is collected; generated `describe` functions call it.
pub struct TypeBuilder {
    pub(crate) module: String,
    pub(crate) next_type: u32,
    pub(crate) named: HashMap<TypeId, NamedType>,
    /// Anonymous LIST types, keyed by the Rust type.
    pub(crate) anon: HashMap<TypeId, (svl_id, TypeDesc)>,
    /// Named types each module refers to (closed over `deps` when the module is emitted).
    pub(crate) used: BTreeMap<String, Vec<TypeId>>,
    /// Classes each module refers to from other modules: module -> (other module, class).
    pub(crate) imports: BTreeMap<String, Vec<(String, String)>>,
    pub(crate) classes: HashMap<TypeId, (svl_id, &'static str, &'static str)>,
    /// Stack of named types being described (records refer to the types of their fields).
    stack: Vec<Vec<TypeId>>,
    /// RA2: the JSON Schemas of `Json<T>` types, one generator for the whole schema (unique `$defs`).
    pub(crate) json_gen: schemars::SchemaGenerator,
    /// RA2: JSON Schema definitions each module refers to (closed over their `$ref`s when emitted).
    pub(crate) json_used: BTreeMap<String, Vec<String>>,
    /// RA2: definitions the named types on `stack` refer to.
    json_stack: Vec<Vec<String>>,
    /// SV48: names of svelters' own types that JSON definitions must not take (see [`json::json_ident`]).
    pub(crate) json_reserved: std::collections::BTreeSet<String>,
}

impl TypeBuilder {
    pub(crate) fn new(classes: HashMap<TypeId, (svl_id, &'static str, &'static str)>) -> TypeBuilder {
        TypeBuilder {
            module: String::new(),
            next_type: 0,
            named: HashMap::new(),
            anon: HashMap::new(),
            used: BTreeMap::new(),
            imports: BTreeMap::new(),
            classes,
            stack: Vec::new(),
            json_gen: schemars::SchemaGenerator::new(schemars::generate::SchemaSettings::draft2020_12()),
            json_used: BTreeMap::new(),
            json_stack: Vec::new(),
            json_reserved: Default::default(),
        }
    }

    fn note_json_use(&mut self, name: &str) {
        if let Some(top) = self.json_stack.last_mut() {
            if !top.iter().any(|n| n == name) {
                top.push(name.to_string());
            }
        }
        let list = self.json_used.entry(self.module.clone()).or_default();
        if !list.iter().any(|n| n == name) {
            list.push(name.to_string());
        }
    }

    /// RA2: a `Json<T>`: a structured value typed from `T`'s JSON Schema. Named schemas become
    /// declared types of the module.
    pub fn json_schema<T: schemars::JsonSchema + ?Sized>(&mut self) -> TypeRef {
        let s = self.json_gen.subschema_for::<T>();
        let mut refs = json::Refs::new(&self.json_reserved);
        let ts = json::schema_ts(s.as_value(), &mut refs);
        let names = refs.names;
        for r in &names {
            self.note_json_use(r);
        }
        TypeRef::new(json::json_slot(), ts)
    }

    fn mint(&mut self) -> svl_id {
        self.next_type += 1;
        APP_PACK | 0x20_0000 | self.next_type
    }

    /// The description of `T`.
    pub fn ty<T: JsType + ?Sized>(&mut self) -> TypeRef {
        T::describe(self)
    }

    fn note_use(&mut self, t: TypeId) {
        if let Some(top) = self.stack.last_mut() {
            top.push(t);
        }
        let m = self.module.clone();
        let list = self.used.entry(m).or_default();
        if !list.contains(&t) {
            list.push(t);
        }
    }

    /// A `#[derive(Record)]` struct: a plain object, copied across.
    pub fn record<T: 'static>(&mut self, name: &str, docs: &str, fields: impl FnOnce(&mut TypeBuilder) -> Vec<FieldDef>) -> TypeRef {
        let t = TypeId::of::<T>();
        if let Some(n) = self.named.get(&t) {
            let r = TypeRef::new(SlotDesc::new(SVL_RECORD).typed(n.id), n.name.clone());
            let wrap = self.record_wrap(t);
            self.note_use(t);
            return TypeRef { wrap, ..r };
        }
        let id = self.mint();
        // A placeholder first: a recursive record refers to itself.
        self.named.insert(t, NamedType {
            id,
            name: name.to_string(),
            docs: docs.to_string(),
            decl: String::new(),
            desc: TypeDesc { name: name.to_string(), kind: SVL_RECORD, scope: 0, members: Vec::new(), fields: Vec::new(), ret: SlotDesc::new(SVL_NULL) },
            deps: Vec::new(),
            kind: "record",
            fields: Vec::new(),
            values: Vec::new(),
            wrap: None,
            tag: None,
            variants: Vec::new(),
            json_deps: Vec::new(),
        });
        self.stack.push(Vec::new());
        self.json_stack.push(Vec::new());
        let defs = fields(self);
        let deps = self.stack.pop().unwrap_or_default();
        let json_deps = self.json_stack.pop().unwrap_or_default();
        let mut slots = Vec::with_capacity(defs.len());
        let mut decl = format!("interface {name} {{");
        let mut wraps = Vec::new();
        let mut shown = Vec::new();
        for f in &defs {
            let mut s = f.ty.slot.clone();
            s.name = Some(f.name.to_string());
            if f.optional {
                s.flags |= SVL_D_OPTIONAL | SVL_D_NULLABLE;
            }
            slots.push(s);
            let doc = if f.docs.is_empty() { String::new() } else { format!(" /** {} */", f.docs.trim()) };
            let ts = f.ty.ts.clone();
            decl.push_str(&format!("{doc} {}{}: {};", prop_key(f.name), if f.optional { "?" } else { "" }, ts));
            shown.push((f.name.to_string(), ts, f.optional));
            if let Some(w) = &f.ty.wrap {
                wraps.push(format!("if (x.{0} != null) x.{0} = ({w})(x.{0});", prop_key(f.name)));
            }
        }
        decl.push_str(" }");
        let n = self.named.get_mut(&t).expect("placeholder");
        n.decl = decl;
        n.desc.fields = slots;
        n.deps = deps;
        n.json_deps = json_deps;
        n.fields = shown;
        if !wraps.is_empty() {
            n.wrap = Some(format!("(x) => {{ if (x !== null) {{ {} }} return x; }}", wraps.join(" ")));
        }
        self.note_use(t);
        TypeRef { slot: SlotDesc::new(SVL_RECORD).typed(id), ts: name.to_string(), wrap: self.record_wrap(t) }
    }

    fn record_wrap(&self, t: TypeId) -> Option<String> {
        self.named.get(&t).and_then(|n| n.wrap.clone())
    }

    /// RA2: a `#[derive(Enum)] #[js(tag = "...")]` data enum: a discriminated union of plain objects
    /// (serde's internally tagged form), copied across as a map.
    pub fn union<T: 'static>(&mut self, name: &str, docs: &str, tag: &str, variants: impl FnOnce(&mut TypeBuilder) -> Vec<VariantDef>) -> TypeRef {
        let t = TypeId::of::<T>();
        if let Some(n) = self.named.get(&t) {
            let r = TypeRef { slot: SlotDesc::new(SVL_MAP), ts: n.name.clone(), wrap: n.wrap.clone() };
            self.note_use(t);
            return r;
        }
        let id = self.mint();
        self.named.insert(t, NamedType {
            id,
            name: name.to_string(),
            docs: docs.to_string(),
            decl: String::new(),
            desc: TypeDesc { name: name.to_string(), kind: SVL_MAP, scope: 0, members: Vec::new(), fields: Vec::new(), ret: SlotDesc::new(SVL_NULL) },
            deps: Vec::new(),
            kind: "union",
            fields: Vec::new(),
            values: Vec::new(),
            wrap: None,
            tag: Some(tag.to_string()),
            variants: Vec::new(),
            json_deps: Vec::new(),
        });
        self.stack.push(Vec::new());
        self.json_stack.push(Vec::new());
        let defs = variants(self);
        let deps = self.stack.pop().unwrap_or_default();
        let json_deps = self.json_stack.pop().unwrap_or_default();
        let tag_key = prop_key(tag);
        let mut members = Vec::new();
        let mut shown = Vec::new();
        let mut cases = Vec::new();
        for v in &defs {
            let mut obj = format!("{{ {tag_key}: '{}';", v.name);
            let mut wraps = Vec::new();
            for f in &v.fields {
                let doc = if f.docs.is_empty() { String::new() } else { format!(" /** {} */", f.docs.trim()) };
                obj.push_str(&format!("{doc} {}{}: {};", prop_key(f.name), if f.optional { "?" } else { "" }, f.ty.ts));
                if let Some(w) = &f.ty.wrap {
                    wraps.push(format!("if (x[{0:?}] != null) x[{0:?}] = ({w})(x[{0:?}]);", f.name));
                }
            }
            obj.push_str(" }");
            match &v.inner {
                Some(inner) => {
                    members.push(format!("({obj} & {})", inner.ts));
                    if let Some(w) = &inner.wrap {
                        wraps.push(format!("({w})(x);"));
                    }
                }
                None => members.push(obj),
            }
            if !wraps.is_empty() {
                cases.push(format!("case '{}': {} break;", v.name, wraps.join(" ")));
            }
            shown.push(schema::VariantSchema {
                name: v.name.to_string(),
                docs: v.docs.to_string(),
                fields: v.fields.iter().map(|f| (f.name.to_string(), f.ty.ts.clone(), f.optional)).collect(),
                inner: v.inner.as_ref().map(|i| i.ts.clone()),
            });
        }
        let union = if members.is_empty() { "never".to_string() } else { members.join(" | ") };
        let n = self.named.get_mut(&t).expect("placeholder");
        n.decl = format!("type {name} = {union};");
        n.deps = deps;
        n.json_deps = json_deps;
        n.variants = shown;
        if !cases.is_empty() {
            n.wrap = Some(format!("(x) => {{ if (x !== null) {{ switch (x[{tag:?}]) {{ {} }} }} return x; }}", cases.join(" ")));
        }
        let wrap = n.wrap.clone();
        self.note_use(t);
        TypeRef { slot: SlotDesc::new(SVL_MAP), ts: name.to_string(), wrap }
    }

    /// A `#[derive(Enum)]` unit enum: one of the kebab-case strings `values` (variant index order).
    pub fn enumeration<T: 'static>(&mut self, name: &str, docs: &str, values: &[&str]) -> TypeRef {
        let t = TypeId::of::<T>();
        if let Some(n) = self.named.get(&t) {
            let r = TypeRef::new(SlotDesc::new(SVL_ENUM).typed(n.id), n.name.clone());
            self.note_use(t);
            return r;
        }
        let id = self.mint();
        let members = values.iter().enumerate().map(|(i, v)| EnumMember { name: v.to_string(), value: i as i64 }).collect();
        let union = if values.is_empty() { "never".to_string() } else { values.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(" | ") };
        self.named.insert(t, NamedType {
            id,
            name: name.to_string(),
            docs: docs.to_string(),
            decl: format!("type {name} = {union};"),
            desc: TypeDesc { name: name.to_string(), kind: SVL_ENUM, scope: 0, members, fields: Vec::new(), ret: SlotDesc::new(SVL_NULL) },
            deps: Vec::new(),
            kind: "enum",
            fields: Vec::new(),
            values: values.iter().map(|v| v.to_string()).collect(),
            wrap: None,
            tag: None,
            variants: Vec::new(),
            json_deps: Vec::new(),
        });
        self.note_use(t);
        TypeRef::new(SlotDesc::new(SVL_ENUM).typed(id), name)
    }

    /// An exported object type (`#[svelters::object]`).
    pub fn class<T: ExportObject>(&mut self) -> TypeRef {
        let (kind, module, name) = match self.classes.get(&TypeId::of::<T>()) {
            Some(c) => *c,
            None => (0, T::MODULE, T::NAME),
        };
        let local = module == self.module;
        let (ts, ident) = if local {
            (name.to_string(), name.to_string())
        } else {
            let list = self.imports.entry(self.module.clone()).or_default();
            if !list.iter().any(|(m, c)| m == module && c == name) {
                list.push((module.to_string(), name.to_string()));
            }
            (format!("import('svelters:app/{module}').{name}"), format!("{module}${name}"))
        };
        TypeRef { slot: SlotDesc::new(SVL_HANDLE).typed(kind), ts, wrap: Some(format!("(x) => wrapObject({ident}, x)")) }
    }

    /// An anonymous LIST type with element slot `elem`, one id per Rust type `L`.
    fn list<L: 'static>(&mut self, elem: &TypeRef) -> svl_id {
        let t = TypeId::of::<L>();
        if let Some((id, _)) = self.anon.get(&t) {
            return *id;
        }
        let id = self.mint();
        let desc = TypeDesc { name: "list".into(), kind: SVL_LIST, scope: 0, members: Vec::new(), fields: vec![elem.slot.clone()], ret: SlotDesc::new(SVL_NULL) };
        self.anon.insert(t, (id, desc));
        id
    }
}

/// A property key in TS/JS source: bare when it is an identifier.
pub(crate) fn prop_key(name: &str) -> String {
    let ident = !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if ident { name.to_string() } else { format!("{name:?}") }
}

// ---- errors ----

/// An error type an export may return (`Result<T, E>`): rejects/throws as
/// `NativeError { domain: 'app:<module>', code, message }`. `#[derive(svelters::Error)]` implements it
/// with one kebab-case code per variant and `Display` as the message.
pub trait ExportError {
    fn code(&self) -> String;
    fn message(&self) -> String;
    /// Every code this type can produce (for the typings).
    fn codes() -> Vec<&'static str>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn into_native(self, domain: &str) -> NativeError
    where
        Self: Sized,
    {
        NativeError { domain: domain.to_string(), code: Value::Str(self.code()), message: self.message() }
    }
}

impl ExportError for NativeError {
    fn code(&self) -> String {
        match &self.code {
            Value::Str(s) => s.clone(),
            Value::I64(i) => i.to_string(),
            _ => "error".into(),
        }
    }
    fn message(&self) -> String {
        self.message.clone()
    }
    fn into_native(self, _domain: &str) -> NativeError {
        self
    }
}

impl ExportError for String {
    fn code(&self) -> String {
        "error".into()
    }
    fn message(&self) -> String {
        self.clone()
    }
    fn codes() -> Vec<&'static str> {
        vec!["error"]
    }
}

impl ExportError for &'static str {
    fn code(&self) -> String {
        "error".into()
    }
    fn message(&self) -> String {
        self.to_string()
    }
    fn codes() -> Vec<&'static str> {
        vec!["error"]
    }
}

impl ExportError for Box<dyn std::error::Error + Send + Sync> {
    fn code(&self) -> String {
        "error".into()
    }
    fn message(&self) -> String {
        self.to_string()
    }
    fn codes() -> Vec<&'static str> {
        vec!["error"]
    }
}

/// An internal failure (`domain: 'svelters'`).
pub fn host_error(code: &str, message: impl Into<String>) -> NativeError {
    NativeError { domain: "svelters".into(), code: Value::Str(code.into()), message: message.into() }
}

fn mismatch(what: &str, v: &Value) -> NativeError {
    host_error("type", format!("expected {what}, got {v:?}"))
}

// ---- objects ----

/// An exported object behind a handle: `Rc<RefCell<T>>` for UI objects, `Arc<T>` for shared ones.
#[derive(Clone)]
pub enum AnyObj {
    Ui(Rc<dyn Any>),
    Shared(Arc<dyn Any + Send + Sync>),
}

impl AnyObj {
    fn addr(&self) -> usize {
        match self {
            AnyObj::Ui(r) => Rc::as_ptr(r) as *const () as usize,
            AnyObj::Shared(a) => Arc::as_ptr(a) as *const () as usize,
        }
    }
}

/// Implemented by `#[svelters::object]`.
pub trait ExportObject: Sized + 'static {
    const MODULE: &'static str;
    const NAME: &'static str;
    const SHARED: bool;
    const DOCS: &'static str;
    /// `Rc<RefCell<Self>>` or `Arc<Self>`.
    type Holder: Clone + 'static;
    type Ref<'a>: Deref<Target = Self>
    where
        Self: 'a;
    #[doc(hidden)]
    fn __holder(o: &AnyObj) -> Option<Self::Holder>;
    #[doc(hidden)]
    fn __borrow(h: &Self::Holder) -> Result<Self::Ref<'_>, NativeError>;
    #[doc(hidden)]
    fn __erase(h: Self::Holder) -> AnyObj;
    #[doc(hidden)]
    fn __new(v: Self) -> Self::Holder;
}

/// A UI-thread object (`#[svelters::object]`): may be `!Send`, methods get `&mut self`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a UI object",
    note = "`&mut self`, `&mut {Self}` and `Obj<{Self}>` need `#[svelters::object]` without `shared`; a shared object is `Arc<Self>` and uses interior mutability"
)]
pub trait UiObject: ExportObject<Holder = Rc<RefCell<Self>>> {}

/// A shared object (`#[svelters::object(shared)]`): `Arc<Self>`, `Send + Sync`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a shared object",
    note = "`self: Arc<Self>` and `Arc<{Self}>` need `#[svelters::object(shared)]`"
)]
pub trait SharedObject: ExportObject<Holder = Arc<Self>> + Send + Sync {}

/// A reference to a UI object that Rust can keep and hand back (same JS object every time).
pub struct Obj<T: UiObject>(pub Rc<RefCell<T>>);

impl<T: UiObject> Clone for Obj<T> {
    fn clone(&self) -> Self {
        Obj(self.0.clone())
    }
}

impl<T: UiObject> Obj<T> {
    pub fn new(v: T) -> Obj<T> {
        Obj(Rc::new(RefCell::new(v)))
    }
    pub fn borrow(&self) -> Ref<'_, T> {
        self.0.borrow()
    }
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }
}

#[doc(hidden)]
pub mod __private {
    //! Used by the code the `svelters` macros generate.
    pub use super::*;
    pub use inventory;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    pub fn ui_holder<T: 'static>(o: &AnyObj) -> Option<Rc<RefCell<T>>> {
        match o {
            AnyObj::Ui(r) => r.clone().downcast::<RefCell<T>>().ok(),
            AnyObj::Shared(_) => None,
        }
    }

    pub fn shared_holder<T: Send + Sync + 'static>(o: &AnyObj) -> Option<Arc<T>> {
        match o {
            AnyObj::Shared(a) => a.clone().downcast::<T>().ok(),
            AnyObj::Ui(_) => None,
        }
    }

    pub fn ui_borrow<T: ExportObject>(h: &Rc<RefCell<T>>) -> Result<std::cell::Ref<'_, T>, NativeError> {
        h.try_borrow().map_err(|_| busy::<T>())
    }

    pub fn borrow_mut<T: UiObject>(h: &Rc<RefCell<T>>) -> Result<std::cell::RefMut<'_, T>, NativeError> {
        h.try_borrow_mut().map_err(|_| busy::<T>())
    }

    fn busy<T: ExportObject>() -> NativeError {
        host_error("busy", format!("{} is already in use by a call in progress (re-entrant call)", T::NAME))
    }

    /// The object a method is called on.
    pub fn this<T: ExportObject>(cx: &CallCx<'_>) -> Result<T::Holder, NativeError> {
        let h = cx.this.ok_or_else(|| host_error("type", "instance method called without an object"))?;
        cx.objects.holder::<T>(h)
    }

    /// The receiver of a `self: Arc<Self>` method.
    pub fn this_arc<T: SharedObject>(cx: &CallCx<'_>) -> Result<Arc<T>, NativeError> {
        this::<T>(cx)
    }

    /// Compile-time check of `#[svelters::object(shared)]`.
    pub const fn assert_shared<T: Send + Sync + ?Sized>() {}

    pub use super::offload::{Injected, Job, OpShared, OffloadSelf, TokioFuture, WorkerSend, assert_offload_self, assert_worker_send, local_job, pool_job, tokio_job};
    // E3
    pub use super::reactive::{
        EmitterCore, EmitterDef, EventInfo, HookProbe, HookViaDefault, HookViaHooks, StoreCell, StoreDef, StoreField, StoreSub, ValueDef, emitter_of, store_get, store_set,
        subscribe_any,
    };
    pub use super::service::{ServiceDef, ServiceSlot};
    pub use super::stream::{DEFAULT_BUFFER, StreamItem, item_type, sink_for, stream_job};

    /// The value of an exported `static` (a module-level `export const`).
    pub fn static_value<T: ExportStatic>(v: &'static T, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        IntoJs::into_js(v.js(), cx)
    }

    /// The JS type of an exported `static`.
    pub fn static_type<T: ExportStatic>(b: &mut TypeBuilder) -> TypeRef {
        <T::Js as JsType>::describe(b)
    }

    /// A plain-object type (an event payload with named fields): copied as a map, typed `ts`.
    pub fn object_type(ts: String) -> TypeRef {
        TypeRef::new(SlotDesc::new(svelters_abi::SVL_MAP), ts)
    }

    /// RA1: runs `f` with a call context for module `module` (provider kinds convert their typed
    /// props, method arguments and event payloads with it). RA3: during a host op (a `set_prop`, a
    /// method call, a read of an observed prop) it has that host's object table, so exported objects
    /// (`Obj<T>`, `Arc<T>`) convert; outside any op (event payloads after a pass) the table is empty
    /// and a value holding an object handle does not convert. FIX-ALLOC: the empty table lives on
    /// the stack (an empty `HashMap` does not allocate), so a call outside any op, such as a Rust
    /// scene write of an own widget's prop, allocates nothing (SG2).
    pub fn with_cx<R>(module: &'static str, f: impl FnOnce(&CallCx<'_>) -> R) -> R {
        match objects::current() {
            Some(objects) => f(&CallCx { objects: &objects, schema: schema(), this: None, module }),
            None => f(&CallCx { objects: &objects::AppObjects::default(), schema: schema(), this: None, module }),
        }
    }

    /// The sync thunk of an offloaded member (never called: the host starts it with its `start`).
    pub fn offload_thunk(_: &CallCx<'_>, _: Vec<Value>) -> Result<Value, NativeError> {
        Err(host_error("internal", "an offloaded export was called synchronously"))
    }

    /// An object argument (`&T`, `&mut T`, `Arc<T>`, `Obj<T>`).
    pub fn object<T: ExportObject>(v: Value, cx: &CallCx<'_>) -> Result<T::Holder, NativeError> {
        match v {
            Value::Handle(h) => cx.objects.holder::<T>(h),
            other => Err(mismatch(T::NAME, &other)),
        }
    }

    /// Positional fields of a record value: `(name, optional)` per field. A plain object (a record inside
    /// a map, a tagged union or a `Json` value, RA2) is read by name; a missing required field is a
    /// `missing-field` error.
    pub fn record_fields(v: Value, names: &[(&str, bool)]) -> Result<std::vec::IntoIter<Value>, NativeError> {
        match v {
            Value::Record { fields, .. } if fields.len() == names.len() => Ok(fields.into_iter()),
            Value::List(fields) if fields.len() == names.len() => Ok(fields.into_iter()),
            Value::Map(mut entries) => {
                let mut out = Vec::with_capacity(names.len());
                for (name, optional) in names {
                    match entries.iter().position(|(k, _)| k == name) {
                        Some(i) => out.push(entries.swap_remove(i).1),
                        None if *optional => out.push(Value::Null),
                        None => return Err(host_error("missing-field", format!("missing field `{name}`"))),
                    }
                }
                Ok(out.into_iter())
            }
            other => Err(mismatch("record", &other)),
        }
    }

    /// RA2: prefixes the path of a value error (`type`, `range`, `missing-field`, ...) with a field or
    /// parameter name: `at shape.r: ...`.
    pub fn at(e: NativeError, seg: &str) -> NativeError {
        super::json::at(e, seg)
    }

    /// A unit enum value: its index, or (inside a plain object, RA2) its name.
    pub fn enum_value(v: Value, values: &[&str]) -> Result<i64, NativeError> {
        match v {
            Value::Enum { value, .. } | Value::I64(value) => Ok(value),
            Value::Str(s) => values
                .iter()
                .position(|x| *x == s)
                .map(|i| i as i64)
                .ok_or_else(|| host_error("unknown-variant", format!("unknown variant `{s}`, expected {}", one_of(values)))),
            other => Err(mismatch("enum value", &other)),
        }
    }

    fn one_of(values: &[&str]) -> String {
        values.iter().map(|v| format!("`{v}`")).collect::<Vec<_>>().join(", ")
    }

    /// RA2: a tagged union value being read (`#[derive(Enum)] #[js(tag = "...")]`).
    pub struct Tagged {
        pub tag: String,
        key: &'static str,
        fields: Vec<(String, Value)>,
    }

    /// Splits a plain object into its tag and the other fields.
    pub fn tagged(v: Value, key: &'static str, ty: &str) -> Result<Tagged, NativeError> {
        let mut fields = match v {
            Value::Map(m) => m,
            other => return Err(host_error("type", format!("expected {ty} (an object with a `{key}` tag), got {other:?}"))),
        };
        let Some(i) = fields.iter().position(|(k, _)| k == key) else {
            return Err(host_error("missing-field", format!("missing field `{key}` (the tag of {ty})")));
        };
        match fields.remove(i).1 {
            Value::Str(tag) => Ok(Tagged { tag, key, fields }),
            other => Err(at(host_error("type", format!("expected a string tag, got {other:?}")), key)),
        }
    }

    impl Tagged {
        fn take(&mut self, name: &str) -> Option<Value> {
            let i = self.fields.iter().position(|(k, _)| k == name)?;
            Some(self.fields.swap_remove(i).1)
        }

        /// A field of a struct variant; `optional` fields may be missing (`null`).
        pub fn field<T: FromJs>(&mut self, name: &str, optional: bool, cx: &CallCx<'_>) -> Result<T, NativeError> {
            match self.take(name) {
                Some(v) => T::from_js(v, cx).map_err(|e| at(e, name)),
                None if optional => T::from_js(Value::Null, cx).map_err(|e| at(e, name)),
                None => Err(host_error("missing-field", format!("missing field `{name}`"))),
            }
        }

        /// A `#[js(default)]` field.
        pub fn field_or<T: FromJs>(&mut self, name: &str, cx: &CallCx<'_>, default: impl FnOnce() -> T) -> Result<T, NativeError> {
            match self.take(name) {
                None | Some(Value::Null) => Ok(default()),
                Some(v) => T::from_js(v, cx).map_err(|e| at(e, name)),
            }
        }

        /// A newtype variant's inner value: the fields next to the tag.
        pub fn rest<T: FromJs>(self, cx: &CallCx<'_>) -> Result<T, NativeError> {
            T::from_js(Value::Map(self.fields), cx)
        }

        /// The error for a tag that names no variant.
        pub fn unknown(&self, tag: &str, expected: &[&str]) -> NativeError {
            at(host_error("unknown-variant", format!("unknown variant `{tag}`, expected one of {}", one_of(expected))), self.key)
        }
    }

    /// RA2: a struct or unit variant as a plain object: the tag first, then the fields.
    pub fn tagged_value(key: &str, tag: &str, fields: Vec<(&str, Value)>) -> Value {
        let mut m = Vec::with_capacity(fields.len() + 1);
        m.push((key.to_string(), Value::Str(tag.to_string())));
        m.extend(fields.into_iter().map(|(k, v)| (k.to_string(), v)));
        Value::Map(m)
    }

    /// RA2: a newtype variant as a plain object: the tag next to the inner record's fields.
    pub fn tagged_inner(key: &str, tag: &str, inner: Value, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        let fields: Vec<(String, Value)> = match inner {
            Value::Map(m) => m,
            Value::Record { ty, fields } => {
                let desc = cx.schema.type_desc(ty).ok_or_else(|| host_error("type", "a newtype variant's record is not in the export schema"))?;
                desc.fields.iter().map(|f| f.name.clone().unwrap_or_default()).zip(fields).collect()
            }
            other => return Err(host_error("type", format!("a newtype variant of a tagged union needs a record or an object inside, got {other:?}"))),
        };
        let mut m = Vec::with_capacity(fields.len() + 1);
        m.push((key.to_string(), Value::Str(tag.to_string())));
        m.extend(fields.into_iter().filter(|(k, _)| k != key));
        Ok(Value::Map(m))
    }

    /// A field that may be missing: `default()` when the value is null.
    pub fn or_default<T: FromJs>(v: Value, cx: &CallCx<'_>, default: impl FnOnce() -> T) -> Result<T, NativeError> {
        match v {
            Value::Null => Ok(default()),
            v => T::from_js(v, cx),
        }
    }

    pub fn enum_index(v: Value) -> Result<i64, NativeError> {
        match v {
            Value::Enum { value, .. } | Value::I64(value) => Ok(value),
            other => Err(mismatch("enum value", &other)),
        }
    }

    pub fn bad_enum(name: &str, i: i64) -> NativeError {
        host_error("type", format!("{i} is not a variant of {name}"))
    }

    /// `#[js(bigint)]`: the member crosses as a JS `bigint`.
    pub fn bigint(mut t: TypeRef) -> TypeRef {
        t.slot.flags |= svelters_abi::SVL_D_BIGINT;
        t.slot.min = if t.slot.has(svelters_abi::SVL_D_UNSIGNED) { 0.0 } else { i64::MIN as f64 };
        t.slot.max = if t.slot.has(svelters_abi::SVL_D_UNSIGNED) { u64::MAX as f64 } else { i64::MAX as f64 };
        t.ts = t.ts.replace("number", "bigint");
        t
    }

    /// Runs a UI-object's `&mut self` method; a panic inside poisons the object.
    pub fn record_type<T: 'static>(cx: &CallCx<'_>) -> Result<svelters_abi::svl_id, NativeError> {
        cx.schema.type_id_of(std::any::TypeId::of::<T>()).ok_or_else(|| host_error("type", format!("type {} is not in the export schema", std::any::type_name::<T>())))
    }
}

// ---- the call context ----

/// What a thunk sees of the host: the object table, the schema, the receiver.
pub struct CallCx<'a> {
    pub(crate) objects: &'a objects::AppObjects,
    pub(crate) schema: &'static Schema,
    pub(crate) this: Option<svl_handle>,
    pub(crate) module: &'static str,
}

impl CallCx<'_> {
    /// The `NativeError` domain of this call: `app:<module>`.
    pub fn domain(&self) -> String {
        format!("app:{}", self.module)
    }

    /// The handle of `h` (identity: the same `Rc`/`Arc` is always the same handle while it lives).
    pub fn adopt<T: ExportObject>(&self, h: T::Holder) -> Result<Value, NativeError> {
        let kind = self.schema.class_kind(TypeId::of::<T>()).ok_or_else(|| host_error("type", format!("{} is not an exported object type", T::NAME)))?;
        Ok(Value::Handle(self.objects.adopt(kind, TypeId::of::<T>(), T::__erase(h))))
    }
}

// ---- JsType impls ----

macro_rules! int_type {
    ($($t:ty => $min:expr, $max:expr, $unsigned:expr;)*) => {$(
        impl JsType for $t {
            fn describe(_: &mut TypeBuilder) -> TypeRef {
                let mut s = SlotDesc::new(SVL_I64).flags(SVL_D_INTEGRAL).range($min, $max);
                if $unsigned {
                    s = s.flags(SVL_D_UNSIGNED);
                }
                TypeRef::new(s, "number")
            }
        }
        impl FromJs for $t {
            fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
                let r: Option<$t> = match &v {
                    Value::I64(i) => <$t>::try_from(*i).ok(),
                    Value::U64(u) => <$t>::try_from(*u).ok(),
                    Value::F64(f) if f.fract() == 0.0 => <$t>::try_from(*f as i128).ok(),
                    _ => None,
                };
                r.ok_or_else(|| match &v {
                    Value::I64(_) | Value::U64(_) | Value::F64(_) => host_error("range", format!("{} does not fit {}", number(&v), stringify!($t))),
                    _ => mismatch("integer", &v),
                })
            }
        }
        impl IntoJs for $t {
            #[allow(unused_comparisons)]
            fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
                Ok(match i64::try_from(self) {
                    Ok(i) => Value::I64(i),
                    Err(_) => Value::U64(self as u64),
                })
            }
        }
    )*};
}

/// A number value as JS would print it.
fn number(v: &Value) -> String {
    match v {
        Value::I64(i) => i.to_string(),
        Value::U64(u) => u.to_string(),
        Value::F64(f) => f.to_string(),
        other => format!("{other:?}"),
    }
}

const SAFE: f64 = 9_007_199_254_740_991.0;
int_type! {
    i8 => i8::MIN as f64, i8::MAX as f64, false;
    i16 => i16::MIN as f64, i16::MAX as f64, false;
    i32 => i32::MIN as f64, i32::MAX as f64, false;
    i64 => -SAFE, SAFE, false;
    isize => -SAFE, SAFE, false;
    u8 => 0.0, u8::MAX as f64, true;
    u16 => 0.0, u16::MAX as f64, true;
    u32 => 0.0, u32::MAX as f64, true;
    u64 => 0.0, SAFE, true;
    usize => 0.0, SAFE, true;
}

macro_rules! float_type {
    ($($t:ty),*) => {$(
        impl JsType for $t {
            fn describe(_: &mut TypeBuilder) -> TypeRef {
                TypeRef::new(SlotDesc::new(SVL_F64), "number")
            }
        }
        impl FromJs for $t {
            fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
                match v {
                    Value::F64(f) => Ok(f as $t),
                    Value::I64(i) => Ok(i as $t),
                    Value::U64(u) => Ok(u as $t),
                    other => Err(mismatch("number", &other)),
                }
            }
        }
        impl IntoJs for $t {
            fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
                Ok(Value::F64(self as f64))
            }
        }
    )*};
}
float_type!(f32, f64);

impl JsType for bool {
    fn describe(_: &mut TypeBuilder) -> TypeRef {
        TypeRef::new(SlotDesc::new(SVL_BOOL), "boolean")
    }
}
impl FromJs for bool {
    fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
        match v {
            Value::Bool(b) => Ok(b),
            other => Err(mismatch("boolean", &other)),
        }
    }
}
impl IntoJs for bool {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        Ok(Value::Bool(self))
    }
}

impl JsType for String {
    fn describe(_: &mut TypeBuilder) -> TypeRef {
        TypeRef::new(SlotDesc::new(SVL_STR), "string")
    }
}
impl FromJs for String {
    fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
        match v {
            Value::Str(s) => Ok(s),
            Value::Str16(u) => Ok(String::from_utf16_lossy(&u)),
            other => Err(mismatch("string", &other)),
        }
    }
}
impl IntoJs for String {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        Ok(Value::Str(self))
    }
}
/// AS1: an asset ref crosses as its `asset://` string; JS passes an `Asset` handle (the converter
/// takes its ref) or the string. A string that is not a valid ref is refused like a bad enum value.
/// Typed as `Asset` in the typings.
impl JsType for svelters_modules::AssetRef {
    fn describe(_: &mut TypeBuilder) -> TypeRef {
        // a result becomes the `Asset` handle once `svelters:assets` is loaded (it defines the global)
        let mut t = TypeRef::new(SlotDesc::new(SVL_STR), "import('@svelters/native-runtime/assets').Asset");
        t.wrap = Some("(r) => (typeof r === 'string' && globalThis.__svelters_asset ? globalThis.__svelters_asset(r) : r)".into());
        t
    }
}
impl FromJs for svelters_modules::AssetRef {
    fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError> {
        let s = String::from_js(v, cx)?;
        svelters_modules::AssetRef::parse(&s).map_err(|e| host_error("type", e.0))
    }
}
impl IntoJs for svelters_modules::AssetRef {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        Ok(Value::Str(self.into()))
    }
}
impl JsType for str {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        String::describe(b)
    }
}
impl JsType for &str {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        String::describe(b)
    }
}
impl IntoJs for &str {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        Ok(Value::Str(self.to_string()))
    }
}

impl JsType for () {
    fn describe(_: &mut TypeBuilder) -> TypeRef {
        TypeRef::void()
    }
}
impl IntoJs for () {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        Ok(Value::Null)
    }
}

impl<T: JsType> JsType for Option<T> {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        let t = T::describe(b);
        let ts = if t.ts.ends_with(" | null") { t.ts.clone() } else { format!("{} | null", t.ts) };
        TypeRef {
            slot: t.slot.flags(SVL_D_NULLABLE | SVL_D_OPTIONAL),
            ts,
            wrap: t.wrap.map(|w| format!("(x) => x === null || x === undefined ? null : ({w})(x)")),
        }
    }
}
impl<T: FromJs> FromJs for Option<T> {
    fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError> {
        match v {
            Value::Null => Ok(None),
            v => T::from_js(v, cx).map(Some),
        }
    }
}
impl<T: IntoJs> IntoJs for Option<T> {
    fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        match self {
            None => Ok(Value::Null),
            Some(v) => v.into_js(cx),
        }
    }
}

fn array_ts(elem: &str) -> String {
    if elem.contains(' ') { format!("({elem})[]") } else { format!("{elem}[]") }
}

impl<T: JsType + 'static> JsType for Vec<T> {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        let e = T::describe(b);
        let id = b.list::<Vec<T>>(&e);
        TypeRef {
            slot: SlotDesc::new(SVL_LIST).typed(id),
            ts: array_ts(&e.ts),
            wrap: e.wrap.map(|w| format!("(x) => x.map({w})")),
        }
    }
    fn throws() -> Vec<&'static str> {
        Vec::new()
    }
}
impl<T: FromJs + 'static> FromJs for Vec<T> {
    fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError> {
        match v {
            Value::List(items) => items.into_iter().enumerate().map(|(n, i)| T::from_js(i, cx).map_err(|e| json::at(e, &format!("[{n}]")))).collect(),
            other => Err(mismatch("array", &other)),
        }
    }
}
impl<T: IntoJs + 'static> IntoJs for Vec<T> {
    fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        Ok(Value::List(self.into_iter().map(|i| i.into_js(cx)).collect::<Result<_, _>>()?))
    }
}
impl<T: JsType + 'static> JsType for &[T] {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        Vec::<T>::describe(b)
    }
}

macro_rules! map_type {
    ($($m:ident),*) => {$(
        impl<T: JsType> JsType for std::collections::$m<String, T> {
            fn describe(b: &mut TypeBuilder) -> TypeRef {
                let e = T::describe(b);
                TypeRef::new(SlotDesc::new(SVL_MAP), format!("Record<string, {}>", e.ts))
            }
        }
        impl<T: FromJs> FromJs for std::collections::$m<String, T> {
            fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError> {
                match v {
                    Value::Map(entries) => entries
                        .into_iter()
                        .map(|(k, v)| {
                            let x = T::from_js(v, cx).map_err(|e| json::at(e, &k))?;
                            Ok((k, x))
                        })
                        .collect(),
                    other => Err(mismatch("object", &other)),
                }
            }
        }
        impl<T: IntoJs> IntoJs for std::collections::$m<String, T> {
            fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError> {
                let mut out: Vec<(String, Value)> = self.into_iter().map(|(k, v)| Ok((k, v.into_js(cx)?))).collect::<Result<_, NativeError>>()?;
                out.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(Value::Map(out))
            }
        }
    )*};
}
map_type!(HashMap, BTreeMap);

impl<T: JsType, E: ExportError> JsType for Result<T, E> {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        T::describe(b)
    }
    fn throws() -> Vec<&'static str> {
        E::codes()
    }
}
impl<T: IntoJs, E: ExportError> IntoJs for Result<T, E> {
    fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        match self {
            Ok(v) => v.into_js(cx),
            Err(e) => Err(e.into_native(&cx.domain())),
        }
    }
}

impl<T: SharedObject> JsType for Arc<T> {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        b.class::<T>()
    }
}
impl<T: SharedObject> FromJs for Arc<T> {
    fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError> {
        __private::object::<T>(v, cx)
    }
}
impl<T: SharedObject> IntoJs for Arc<T> {
    fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        cx.adopt::<T>(self)
    }
}

impl<T: UiObject> JsType for Obj<T> {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        b.class::<T>()
    }
}
impl<T: UiObject> FromJs for Obj<T> {
    fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError> {
        __private::object::<T>(v, cx).map(Obj)
    }
}
impl<T: UiObject> IntoJs for Obj<T> {
    fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        cx.adopt::<T>(self.0)
    }
}

// ---- what the macros register ----

/// How a member is reached from JS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    /// `obj.name(...)`: `host.call`.
    Method,
    /// `Class.name(...)` or a module function: `host.callStatic`.
    Static,
    /// `new Class(...)`: `host.construct`.
    Constructor,
    /// `obj.name`: `host.getProp`.
    Getter,
    /// `obj.name = v`: `host.setProp`.
    Setter,
}

/// The receiver of a method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receiver {
    None,
    /// `&self`
    Ref,
    /// `&mut self` (UI objects): a panic inside poisons the object.
    Mut,
    /// `self: Arc<Self>` (shared objects).
    Arc,
}

/// One parameter of a member.
#[derive(Clone, Debug)]
pub struct Param {
    pub name: &'static str,
    pub ty: TypeRef,
}

/// The JS signature of a member.
#[derive(Clone, Debug)]
pub struct Sig {
    pub params: Vec<Param>,
    pub ret: TypeRef,
    pub throws: Vec<&'static str>,
    /// Offloaded members with a `WorkCx<P>`: the progress type `P` (`options.onProgress`).
    pub progress: Option<TypeRef>,
    /// E3 stream members (a `Sink<T>` parameter or a `Stream<T>` result): the item type `T`. The JS
    /// call returns a `NativeStream<T>` (an async iterable) instead of a Promise.
    pub items: Option<TypeRef>,
}

/// The generated call: owned, validated arguments in, a result or a `NativeError` out.
pub type Thunk = fn(&CallCx<'_>, Vec<Value>) -> Result<Value, NativeError>;

/// The generated start of an offloaded call (E1): owned arguments in, the job to run out. Runs on
/// the JS thread; the job runs where [`OffloadDef::exec`] says.
pub type StartThunk = fn(&CallCx<'_>, Vec<Value>, &Arc<offload::OpShared>) -> Result<offload::Job, NativeError>;

/// How an offloaded member runs (`#[js(worker)]`, `#[js(blocking)]`, `async fn`, `#[js(tokio)]`).
#[derive(Clone, Copy, Debug)]
pub struct OffloadDef {
    pub exec: Exec,
    pub start: StartThunk,
    /// `#[js(supersede)]`: a new call cancels the previous in-flight calls of this member on the same
    /// object (they reject with `superseded`).
    pub supersede: bool,
    /// `#[js(max_in_flight = n)]`: at most `n` calls of this member on the same object run at once;
    /// further calls wait in a FIFO. 0 = unlimited.
    pub max_in_flight: u32,
}

/// One exported member, as `#[svelters::export]` emits it.
pub struct MemberDef {
    /// The JS name (camelCase, or `#[js(rename)]`); for accessors the property name.
    pub name: &'static str,
    pub rust: &'static str,
    pub kind: MemberKind,
    pub receiver: Receiver,
    pub docs: &'static str,
    pub sig: fn(&mut TypeBuilder) -> Sig,
    pub thunk: Thunk,
    /// Offloaded members (E1): the call returns a Promise and runs as [`OffloadDef`] says.
    pub offload: Option<OffloadDef>,
}

/// `#[svelters::object]` on a struct.
pub struct ObjectDef {
    pub module: &'static str,
    pub name: &'static str,
    pub rust: &'static str,
    pub docs: &'static str,
    pub shared: bool,
    pub type_id: fn() -> TypeId,
}

/// `#[svelters::export]` on an `impl` block (several per type are fine).
pub struct ImplDef {
    pub type_id: fn() -> TypeId,
    pub rust: &'static str,
    pub members: &'static [MemberDef],
}

/// `#[svelters::export(module = "...")]` on a free function.
pub struct FnDef {
    pub module: &'static str,
    pub member: MemberDef,
}

/// RA1: the JS types of a provider kind's props and event payloads (`#[svelters::widget]` in
/// svelters-egui). They are described in module `module` (the kind's namespace), so records and enums
/// a kind uses get export type ids the host converts with, and are declared in
/// `svelters:app/<module>` for the typings. [`Schema::kind_types`] returns the descriptions by `kind`.
pub struct KindTypesDef {
    pub module: &'static str,
    /// The kind's spec id (`cad.QuantityField`).
    pub kind: &'static str,
    /// Props first, then event payloads, in declaration order.
    pub describe: fn(&mut TypeBuilder) -> Vec<TypeRef>,
}

inventory::collect!(ObjectDef);
inventory::collect!(ImplDef);
inventory::collect!(FnDef);
inventory::collect!(KindTypesDef);
