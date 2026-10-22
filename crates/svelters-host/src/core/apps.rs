//! Host ops on app exports (E0, `docs/notes/egui-rust-exports.md`): kinds named `app:<module>[.<Type>]`
//! (pack 63) and handles with `APP_HANDLE_BIT` go to the export registry, never to the backend.
//!
//! Arguments are encoded with the member's descriptor exactly like a backend call (`convert::encode`,
//! so validation, ranges, enums and records behave the same), copied into owned `Value`s and handed to
//! the generated thunk. Results go back through `convert::decode_slot`. Calls are logged to the op
//! log sink (decision 2026-10-21: in full, like native calls).

use super::*;
use crate::convert::{self, Arena, Cx, JsPromises, JsReader, JsWriter};
use crate::exports::{self, CallCx, MemberKind, MemberSchema, Receiver, Thunk, schema};
use std::panic::{AssertUnwindSafe, catch_unwind};
use svelters_abi::*;

/// `#a<n>`: how the op log names an app object.
fn tag(h: svl_handle) -> String {
    format!("#a{}", h & !exports::APP_HANDLE_BIT)
}

pub(crate) fn json_args(args: &[Value]) -> String {
    svelters_backend_log::json_string(&Value::List(args.to_vec()))
}

fn type_err(msg: String) -> OpError {
    OpError { kind: OpKind::Type, msg, native: None }
}

impl Core {
    pub(crate) fn app_kind(&self, name: &str) -> Result<svl_id, String> {
        schema().kind(name).ok_or_else(|| self.unknown_app_kind(name))
    }

    /// The error for an app kind that does not exist or is not visible to this host: the same text,
    /// listing only the visible modules, so a policy does not reveal what it hides.
    fn unknown_app_kind(&self, name: &str) -> String {
        format!("unknown kind '{name}': no such Rust export in this binary (svelters:app modules: {})", {
            let ms: Vec<&str> = schema().modules.iter().map(|m| m.name.as_str()).filter(|m| self.app_module_visible(m)).collect();
            if ms.is_empty() { "none".to_string() } else { ms.join(", ") }
        })
    }

    fn app_module_visible(&self, module: &str) -> bool {
        self.app_gate.borrow().as_ref().is_none_or(|g| g(module))
    }

    /// Imports are gated by the module provider; `construct` and `callStatic` reach an export with
    /// no import, so they ask the same policy here.
    fn app_gate_kind(&self, kind: svl_id) -> Result<(), OpError> {
        match schema().module_of(kind) {
            Some(m) if !self.app_module_visible(&m.name) => Err(type_err(self.unknown_app_kind(&self.app_kind_name(kind)))),
            _ => Ok(()),
        }
    }

    fn app_kind_name(&self, kind: svl_id) -> String {
        schema().kind_name(kind).unwrap_or_else(|| format!("app kind#{kind}"))
    }

    fn app_resolve(&self, what: NameKind, kind: svl_id, n: Name<'_>) -> Result<&'static MemberSchema, OpError> {
        let id = match n {
            Name::Str(s) => schema().resolve(what, kind, s),
            Name::Id(i) => i,
        };
        schema().member(what, kind, id).ok_or_else(|| {
            let name = match n {
                Name::Str(s) => s.to_string(),
                Name::Id(i) => format!("#{i}"),
            };
            let what = if what == NameKind::Prop { "property" } else { "method" };
            type_err(format!("{} has no {what} '{name}'", self.app_kind_name(kind)))
        })
    }

    /// The kind of a live app handle, or why it is not live.
    fn app_live(&self, h: svl_handle) -> Result<svl_id, OpError> {
        self.apps.kind(h).ok_or_else(|| OpError {
            kind: OpKind::Error,
            msg: format!("app object {} was disposed", tag(h)),
            native: Some(exports::host_error("disposed", format!("app object {} was disposed", tag(h)))),
        })
    }

    /// A live, unpoisoned app object.
    pub(crate) fn app_check_live(&self, h: svl_handle) -> Result<svl_id, OpError> {
        let kind = self.app_live(h)?;
        if self.apps.is_poisoned(h) {
            let msg = format!("app object {} is poisoned: an earlier call panicked while it was borrowed mutably", tag(h));
            return Err(OpError { kind: OpKind::Error, msg: msg.clone(), native: Some(exports::host_error("poisoned", msg)) });
        }
        Ok(kind)
    }

    fn logging(&self) -> bool {
        self.op_log.borrow().is_some()
    }

    pub(crate) fn note_op(&self, line: impl FnOnce() -> String) {
        let sink = self.op_log.borrow().clone();
        if let Some(s) = sink {
            s(&line());
        }
    }

    /// Encodes a JS argument array against `m`'s parameters into owned values.
    fn app_args<E: JsReader>(&self, e: &E, m: &MemberSchema, prefix: &dyn Fn() -> String, args: &E::V) -> Result<Vec<Value>, OpError> {
        let slots = &m.desc.params;
        let given = match JsReader::kind(e, args) {
            convert::JsKind::Undefined | convert::JsKind::Null => 0,
            convert::JsKind::Array => JsReader::len(e, args)?,
            _ => return Err(type_err(format!("{}: arguments must be an array", prefix()))),
        };
        let required = slots.iter().filter(|p| !p.has(SVL_D_OPTIONAL)).count();
        // Trailing `undefined`s count as left out (generated stubs pass every declared parameter).
        if given < required || given > slots.len() {
            let n = if required == slots.len() { required.to_string() } else { format!("{required} to {}", slots.len()) };
            return Err(type_err(format!("{}: expected {n} argument(s), got {given}", prefix())));
        }
        let mut ar = Arena::new();
        let cx = Cx { types: self, utf16: false, callbacks: None, owner: 0 };
        let mut raws = Vec::with_capacity(slots.len());
        for (i, slot) in slots.iter().enumerate() {
            if i >= given {
                raws.push(svl_value::null());
                continue;
            }
            let v = JsReader::index(e, args, i)?;
            let raw = convert::encode(e, &v, slot, &cx, &mut ar)
                .map_err(|c| self.conv_err(&format!("{}({})", prefix(), slot.name.as_deref().unwrap_or("arg")), c))?;
            raws.push(raw);
        }
        // Every argument converted: only now are typed arrays moved out of JS (E2).
        let out = convert::owned_all(e, &raws).map_err(|m| type_err(format!("{}: {m}", prefix())))?;
        drop(ar);
        Ok(out)
    }

    /// Runs a thunk with the receiver `this`: panics become `NativeError { code: 'panic' }` and poison
    /// a UI object whose `&mut self` method panicked.
    fn app_invoke(&self, this: Option<svl_handle>, kind: svl_id, thunk: Thunk, receiver: Receiver, args: Vec<Value>) -> Result<Value, OpError> {
        if let Some(h) = this {
            self.app_live(h)?;
            if self.apps.is_poisoned(h) {
                let msg = format!("app object {} is poisoned: an earlier call panicked while it was borrowed mutably", tag(h));
                return Err(OpError { kind: OpKind::Error, msg: msg.clone(), native: Some(exports::host_error("poisoned", msg)) });
            }
        }
        let module = schema().module_of(kind).map(|m| m.name.as_str()).unwrap_or("");
        let cx = CallCx { objects: &self.apps, schema: schema(), this, module };
        match catch_unwind(AssertUnwindSafe(|| thunk(&cx, args))) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(n)) => Err(OpError { kind: OpKind::Error, msg: n.message.clone(), native: Some(n) }),
            Err(p) => {
                if receiver == Receiver::Mut {
                    if let Some(h) = this {
                        self.apps.poison(h);
                    }
                }
                let what = p
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| p.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "panic".into());
                let msg = format!("{}: Rust panic: {what}", self.app_kind_name(kind));
                Err(OpError { kind: OpKind::Error, msg: msg.clone(), native: Some(exports::host_error("panic", msg)) })
            }
        }
    }

    fn app_result<W: JsWriter>(&self, w: &W, slot: &SlotDesc, v: &Value) -> Result<W::V, OpError> {
        convert::decode_slot(w, v, slot, self).map_err(|m| match m.strip_prefix("RangeError: ") {
            Some(rest) => OpError { kind: OpKind::Range, msg: rest.to_string(), native: None },
            None => OpError::from(m),
        })
    }

    /// `host.construct('app:<module>.<Type>', [args])`.
    pub(crate) fn app_construct<E>(&self, e: &E, kind: svl_id, args: &<E as JsReader>::V) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V>,
    {
        self.app_gate_kind(kind)?;
        let Some(c) = schema().class(kind) else {
            return Err(type_err(format!("{} is a module, not a class: it cannot be constructed", self.app_kind_name(kind))));
        };
        let Some(k) = &c.constructor else {
            return Err(type_err(format!("{} has no constructor (#[js(constructor)])", c.native)));
        };
        let args = self.app_args(e, k, &|| format!("new {}", c.name), args)?;
        let line = self.logging().then(|| json_args(&args));
        let v = self.app_invoke(None, kind, k.thunk.expect("constructor thunk"), Receiver::None, args)?;
        if let (Value::Handle(h), Some(line)) = (&v, line) {
            self.note_op(|| format!("construct {} {} {line}", tag(*h), c.native));
        }
        self.app_result(e, &k.desc.ret, &v)
    }

    /// `host.call(h, method, args)` on an app object.
    pub(crate) fn app_call<E>(&self, e: &E, h: svl_handle, method: Name<'_>, args: &<E as JsReader>::V) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V> + JsPromises,
    {
        let kind = self.app_live(h)?;
        let m = self.app_resolve(NameKind::Method, kind, method)?;
        let kname = || self.app_kind_name(kind);
        if m.kind != MemberKind::Method {
            return Err(type_err(format!("{}.{} is static: call it on the class", kname(), m.name)));
        }
        if m.offload.is_some() {
            return self.app_start_op(e, Some(h), kind, m, &|| format!("{}.{}", kname(), m.name), args);
        }
        let args = self.app_args(e, m, &|| format!("{}.{}", kname(), m.name), args)?;
        self.note_op(|| format!("call {} {}.{} {}", tag(h), kname(), m.name, json_args(&args)));
        let v = self.app_invoke(Some(h), kind, m.thunk.expect("method thunk"), m.receiver, args)?;
        self.app_result(e, &m.desc.ret, &v)
    }

    /// `host.callStatic('app:<module>[.<Type>]', name, args)`: a static method or a module function.
    pub(crate) fn app_call_static<E>(&self, e: &E, kind: svl_id, method: Name<'_>, args: &<E as JsReader>::V) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V> + JsPromises,
    {
        self.app_gate_kind(kind)?;
        let m = self.app_resolve(NameKind::Method, kind, method)?;
        let kname = || self.app_kind_name(kind);
        if m.kind == MemberKind::Method {
            return Err(type_err(format!("{}.{} is an instance method; call it on an object", kname(), m.name)));
        }
        if m.offload.is_some() {
            return self.app_start_op(e, None, kind, m, &|| format!("{}.{}", kname(), m.name), args);
        }
        let args = self.app_args(e, m, &|| format!("{}.{}", kname(), m.name), args)?;
        let line = self.logging().then(|| json_args(&args));
        let v = self.app_invoke(None, kind, m.thunk.expect("static thunk"), Receiver::None, args)?;
        if let Some(line) = line {
            let made = match &v {
                Value::Handle(h) => format!(" {}", tag(*h)),
                _ => String::new(),
            };
            self.note_op(|| format!("static {}.{} {line}{made}", kname(), m.name));
        }
        self.app_result(e, &m.desc.ret, &v)
    }

    /// `host.getProp(h, prop)`: a `#[js(getter)]`. Not logged (native reads are not either).
    pub(crate) fn app_get_prop<W: JsWriter>(&self, w: &W, h: svl_handle, prop: Name<'_>) -> Result<W::V, OpError> {
        let kind = self.app_live(h)?;
        let p = self.app_resolve(NameKind::Prop, kind, prop)?;
        let Some(get) = p.thunk else {
            return Err(type_err(format!("{}.{} is write-only", self.app_kind_name(kind), p.name)));
        };
        let v = self.app_invoke(Some(h), kind, get, p.receiver, Vec::new())?;
        self.app_result(w, &p.desc.ret, &v)
    }

    /// `host.setProp(h, prop, v)`: a `#[js(setter)]`.
    pub(crate) fn app_set_prop<R: JsReader>(&self, r: &R, h: svl_handle, prop: Name<'_>, v: &R::V) -> Result<(), OpError> {
        let kind = self.app_live(h)?;
        let p = self.app_resolve(NameKind::Prop, kind, prop)?;
        let kname = || self.app_kind_name(kind);
        let Some((set, recv)) = p.setter else {
            return Err(type_err(format!("{}.{}: property is read-only", kname(), p.name)));
        };
        let slot = p.desc.params.first().cloned().unwrap_or_else(|| SlotDesc::new(SVL_NULL));
        let mut ar = Arena::new();
        let cx = Cx { types: self, utf16: false, callbacks: None, owner: 0 };
        let raw = convert::encode(r, v, &slot, &cx, &mut ar).map_err(|c| self.conv_err(&format!("{}.{}", kname(), p.name), c))?;
        let value = convert::owned_all(r, &[raw]).map_err(|m| type_err(format!("{}.{}: {m}", kname(), p.name)))?.pop().unwrap_or(Value::Null);
        drop(ar);
        self.note_op(|| format!("set {} {}.{} {}", tag(h), kname(), p.name, svelters_backend_log::json_string(&value)));
        let pid = schema().resolve(NameKind::Prop, kind, &p.name);
        let observed = self.st.borrow().observed.contains_key(&(h, pid));
        let written = observed.then(|| value.clone());
        self.app_invoke(Some(h), kind, set, recv, vec![value])?;
        // E3: R1's set path for an observed store field: the value is read back as `last`, so the echo of
        // this write is dropped and a value `on_write` normalized is delivered once.
        if observed {
            self.after_set(h, pid, written);
        }
        Ok(())
    }

    /// `host.destroy(h)` (`dispose()`, or a collected wrapper): drops our reference. Idempotent.
    /// Not logged: a release by the garbage collector is not deterministic.
    pub(crate) fn app_destroy(&self, h: svl_handle) {
        // In-flight calls on it reject with `disposed` (E1); their jobs keep their own reference.
        self.ops_owner_gone(h);
        self.app_forget(h);
        self.apps.release(h);
    }

    /// Number of live app objects.
    pub(crate) fn app_count(&self) -> usize {
        self.apps.len()
    }

    /// Live app handles (reload releases the old runtime's).
    pub(crate) fn app_handles(&self) -> Vec<svl_handle> {
        self.apps.handles()
    }

    pub(crate) fn app_release_all(&self, hs: &[svl_handle]) {
        for h in hs {
            self.app_forget(*h);
            self.apps.release(*h);
        }
    }

    /// A reload starts (E3): handles the new runtime receives are tagged with the next generation.
    pub(crate) fn app_begin_generation(&self) -> u64 {
        self.apps.begin_generation()
    }

    /// The reload succeeded: releases the old runtime's handles the new one did not receive again (a
    /// persisted object, an app-wide store and a service's object keep theirs).
    pub(crate) fn app_release_stale(&self, old: &[svl_handle]) {
        let stale = self.apps.not_received(old);
        self.app_release_all(&stale);
    }

    /// The reload failed: back to generation `old`; the handles the failed load made are released.
    pub(crate) fn app_restore_generation(&self, old: u64, old_handles: &[svl_handle]) {
        self.apps.restore_generation(old, old_handles);
        let new: Vec<_> = self.app_handles().into_iter().filter(|h| !old_handles.contains(h)).collect();
        self.app_release_all(&new);
    }
}
