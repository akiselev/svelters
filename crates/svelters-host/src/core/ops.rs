//! JS host ops (called by the engine's `__svelters_host`): name and descriptor caches, the
//! type-directed conversion around each backend call, and boxed-value release.
//!
//! Every op converts its JS arguments completely *before* touching handle liveness a second time
//! and before calling the backend: reading a record field can run user JS (getters, Proxy traps),
//! which may destroy the target handle. No `RefCell` is borrowed across an engine or backend call.

use super::*;
use crate::convert::{self, Arena, ConvError, Cx, ErrKind, JsPromises, JsReader, JsWriter, TypeSource};
use svelters_abi::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpKind {
    /// A backend or host failure: thrown as `Error`.
    Error,
    /// A value of the wrong type: `TypeError`.
    Type,
    /// A value out of range: `RangeError`.
    Range,
}

#[derive(Clone, Debug)]
pub(crate) struct OpError {
    pub kind: OpKind,
    pub msg: String,
    /// Set when the backend reported an `SVL_ERROR` value.
    pub native: Option<NativeError>,
}

impl From<String> for OpError {
    fn from(msg: String) -> OpError {
        OpError {
            kind: OpKind::Error,
            msg,
            native: None,
        }
    }
}


/// A member named by string (resolved and cached here) or by a pre-resolved id.
#[derive(Clone, Copy)]
pub(crate) enum Name<'a> {
    Str(&'a str),
    Id(svl_id),
}

impl TypeSource for Core {
    fn type_info(&self, id: svl_id) -> Option<Rc<TypeInfo>> {
        if let Some(t) = self.st.borrow().types.get(&id) {
            return t.clone();
        }
        if crate::exports::is_app_id(id) {
            let t = crate::exports::schema().type_desc(id).map(|d| Rc::new(TypeInfo::new(d.clone())));
            self.st.borrow_mut().types.insert(id, t.clone());
            return t;
        }
        let t = self.backend.describe_type(id).map(|d| Rc::new(TypeInfo::new(d)));
        self.st.borrow_mut().types.insert(id, t.clone());
        t
    }
}

impl Core {
    pub(super) fn utf16(&self) -> bool {
        self.backend.caps() & SVL_CAP_UTF16_IN != 0
    }

    pub(super) fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> Result<svl_id, String> {
        if let Some(id) = self
            .st
            .borrow()
            .names
            .get(&(what as u32, kind))
            .and_then(|m| m.get(name))
        {
            return Ok(*id);
        }
        let id = if crate::exports::is_app_id(kind) {
            crate::exports::schema().resolve(what, kind, name)
        } else {
            self.backend.resolve(what, kind, name)
        };
        if id == 0 {
            return Err(format!("unknown {what:?} '{name}'"));
        }
        self.st
            .borrow_mut()
            .names
            .entry((what as u32, kind))
            .or_default()
            .insert(name.into(), id);
        Ok(id)
    }

    pub(super) fn resolve_name(&self, what: NameKind, kind: svl_id, n: Name<'_>) -> Result<svl_id, String> {
        match n {
            Name::Str(s) => self.resolve(what, kind, s),
            Name::Id(id) => Ok(id),
        }
    }

    /// The kind of a live handle; an error that says why for a dead one.
    pub(crate) fn kind_of(&self, h: svl_handle) -> Result<svl_id, String> {
        let st = self.st.borrow();
        if let Some(k) = st.handles.get(&h).or_else(|| st.boxed.get(&h)) {
            return Ok(*k);
        }
        Err(match st.dead.why.get(&h) {
            Some(d) => format!("handle #{h} was destroyed ({})", d.describe()),
            None => format!("stale or foreign handle {h}"),
        })
    }

    /// `Some(kind)` for a live handle, `None` for one that is known to be dead (teardown-shaped
    /// ops treat that as a no-op), an error for a handle the host never issued.
    pub(crate) fn live_kind(&self, h: svl_handle) -> Result<Option<svl_id>, String> {
        let st = self.st.borrow();
        if let Some(k) = st.handles.get(&h).or_else(|| st.boxed.get(&h)) {
            return Ok(Some(*k));
        }
        if st.dead.why.contains_key(&h) {
            return Ok(None);
        }
        Err(format!("stale or foreign handle {h}"))
    }

    pub(super) fn kind_name(&self, k: svl_id) -> String {
        self.st
            .borrow()
            .kind_names
            .get(&k)
            .cloned()
            .unwrap_or_else(|| format!("kind#{k}"))
    }

    /// The cached descriptor of a member, if the backend publishes one.
    pub(super) fn member(&self, what: NameKind, kind: svl_id, id: svl_id) -> Option<Rc<MemberDesc>> {
        let key = (what as u32, kind, id);
        if let Some(m) = self.st.borrow().members.get(&key) {
            return m.clone();
        }
        let m = if crate::exports::is_app_id(kind) {
            crate::exports::schema().member(what, kind, id).map(|m| Rc::new(m.desc.clone()))
        } else {
            self.backend.describe(what, kind, id).map(Rc::new)
        };
        self.st.borrow_mut().members.insert(key, m.clone());
        m
    }

    /// A backend failure. A native error's code is named here when the backend describes the error
    /// enum (a `GIOErrorEnum` value becomes `"not-found"`), so the JS side needs no type tables.
    pub(super) fn backend_err(&self, e: AbiError) -> OpError {
        let native = e.native.map(|mut n| {
            n.code = match &n.code {
                Value::Enum { ty, value } => match self.type_info(*ty).and_then(|t| t.by_value.get(value).map(|i| t.desc.members[*i].name.clone())) {
                    Some(name) => Value::Str(name),
                    None => Value::I64(*value),
                },
                Value::I64(_) | Value::Str(_) => n.code.clone(),
                _ => Value::Null,
            };
            n
        });
        OpError {
            kind: OpKind::Error,
            msg: e.message,
            native,
        }
    }

    pub(super) fn conv_err(&self, prefix: &str, e: ConvError) -> OpError {
        OpError {
            kind: match e.kind {
                ErrKind::Type => OpKind::Type,
                ErrKind::Range => OpKind::Range,
            },
            msg: e.render(prefix),
            native: None,
        }
    }

    // ---- ids and descriptors for the JS layer ----

    /// Resolves an element kind name to its id (cached).
    pub(crate) fn op_kind_id(&self, kind: &str) -> Result<svl_id, String> {
        let cached = self.st.borrow().kinds.get(kind).copied();
        if let Some(k) = cached {
            return Ok(k);
        }
        let k = if kind.starts_with("app:") { self.app_kind(kind)? } else { self.backend.resolve(NameKind::Kind, 0, kind) };
        if k == 0 {
            return Err(format!("unknown kind '{kind}'"));
        }
        let mut st = self.st.borrow_mut();
        st.kinds.insert(kind.to_string(), k);
        st.kind_names.entry(k).or_insert_with(|| kind.to_string());
        Ok(k)
    }

    /// Resolves a prop/signal/method/slot name within a kind to its id (cached).
    pub(crate) fn op_member_id(&self, what: NameKind, kind: svl_id, name: &str) -> Result<svl_id, String> {
        self.resolve(what, kind, name)
    }

    /// Coarse value type of a prop for JS-side attribute coercion: `integer`, `number`,
    /// `boolean`, `string`, `enum`, `flags`, `record`, `list`, `bytes`, `any`; `None` without a
    /// descriptor.
    pub(crate) fn op_prop_type(&self, kind: svl_id, prop: svl_id) -> Option<&'static str> {
        let d = self.member(NameKind::Prop, kind, prop)?;
        let s = d.params.first().unwrap_or(&d.ret);
        Some(match s.tag {
            SVL_BOOL => "boolean",
            SVL_I64 => "integer",
            SVL_F64 => "number",
            SVL_STR => "string",
            SVL_ENUM => "enum",
            SVL_FLAGS => "flags",
            SVL_RECORD => "record",
            SVL_LIST => "list",
            SVL_BYTES => "bytes",
            _ => "any",
        })
    }

    // ---- tree ops ----

    pub(crate) fn op_create(&self, kind: &str) -> Result<svl_handle, OpError> {
        self.timed(|| {
            self.tree_ops_allowed()?;
            let kid = self.op_kind_id(kind)?;
            self.create_kind(kid)
        })
    }

    pub(crate) fn op_create_id(&self, kid: svl_id) -> Result<svl_handle, OpError> {
        self.timed(|| {
            self.tree_ops_allowed()?;
            self.create_kind(kid)
        })
    }

    fn create_kind(&self, kid: svl_id) -> Result<svl_handle, OpError> {
        let h = self.backend.create(kid).map_err(|e| self.backend_err(e))?;
        if h >= MAX_HANDLE {
            self.backend.destroy(h);
            return Err(format!("backend issued handle {h}, which JS cannot represent exactly").into());
        }
        let top = self
            .backend
            .handle_info(h)
            .is_ok_and(|i| i.lifetime == SVL_LIFE_TOPLEVEL);
        let mut st = self.st.borrow_mut();
        st.handles.insert(h, kid);
        if top {
            st.toplevels.insert(h);
            st.top_seen = true;
        }
        Ok(h)
    }

    pub(crate) fn op_insert(
        &self,
        parent: svl_handle,
        child: svl_handle,
        before: Option<svl_handle>,
        slot: Option<Name<'_>>,
    ) -> Result<(), OpError> {
        self.timed(|| {
            self.tree_ops_allowed()?;
            let pk = self.kind_of(parent)?;
            self.kind_of(child)?;
            if let Some(b) = before {
                self.kind_of(b)?;
            }
            let slot = match slot {
                Some(n) => self.resolve_name(NameKind::Slot, pk, n)?,
                None => 0,
            };
            self.backend
                .insert(parent, child, before, slot)
                .map_err(|e| self.backend_err(e))?;
            self.st.borrow_mut().tree.insert(child, parent);
            Ok(())
        })
    }

    pub(crate) fn op_remove(&self, parent: svl_handle, child: svl_handle) -> Result<(), OpError> {
        self.timed(|| {
            self.tree_ops_allowed()?;
            // A dead parent or child has nothing left to remove.
            if self.live_kind(parent)?.is_none() || self.live_kind(child)?.is_none() {
                return Ok(());
            }
            self.backend.remove(parent, child).map_err(|e| self.backend_err(e))?;
            self.st.borrow_mut().tree.remove(&child);
            Ok(())
        })
    }

    /// Sets a per-child attribute (ABI v2.2). `child` must be inserted under `parent`. `Null`
    /// resets the attribute.
    pub(crate) fn op_set_child_attr<R: JsReader>(
        &self,
        r: &R,
        parent: svl_handle,
        child: svl_handle,
        attr: Name<'_>,
        v: &R::V,
    ) -> Result<(), OpError> {
        self.timed(|| {
            let (Some(pk), Some(_)) = (self.live_kind(parent)?, self.live_kind(child)?) else {
                return Ok(());
            };
            if self.st.borrow().tree.get(&child) != Some(&parent) {
                return Err(format!("handle #{child} is not a child of #{parent}: child attributes need an inserted child").into());
            }
            let a = self.resolve_name(NameKind::Attr, pk, attr)?;
            let any = SlotDesc::new(SVL_NULL);
            let mut ar = Arena::new();
            let registry = CallbackRegistry(self);
            let cx = Cx {
                types: self,
                utf16: self.utf16(),
                callbacks: Some(&registry),
                owner: child,
            };
            let raw = match convert::encode(r, v, &any, &cx, &mut ar) {
                Ok(raw) => raw,
                Err(e) => {
                    self.finish_callbacks(&mut ar, false);
                    let name = match attr {
                        Name::Str(s) => s.to_string(),
                        Name::Id(i) => format!("#{i}"),
                    };
                    return Err(self.conv_err(&format!("<{}> child attribute {name}", self.kind_name(pk)), e));
                }
            };
            let res = self.backend.set_child_attr_raw(parent, child, a, &raw).map_err(|e| self.backend_err(e));
            self.finish_callbacks(&mut ar, res.is_ok());
            drop(ar);
            res
        })
    }

    pub(crate) fn op_get_child_attr<W: JsWriter>(
        &self,
        w: &W,
        parent: svl_handle,
        child: svl_handle,
        attr: Name<'_>,
    ) -> Result<W::V, OpError> {
        self.timed(|| {
            let pk = self.kind_of(parent)?;
            self.kind_of(child)?;
            let a = self.resolve_name(NameKind::Attr, pk, attr)?;
            let v = self.backend.get_child_attr(parent, child, a).map_err(|e| self.backend_err(e))?;
            self.decode_result(w, &v)
        })
    }

    pub(crate) fn op_destroy(&self, h: svl_handle) -> Result<(), OpError> {
        self.timed(|| {
            if crate::exports::is_app_handle(h) {
                self.app_destroy(h);
                return Ok(());
            }
            self.tree_ops_allowed()?;
            // Idempotent: destroying a dead handle (already destroyed, or died natively) is a no-op.
            if self.live_kind(h)?.is_none() {
                return Ok(());
            }
            // A queued event still carries `h` (its JS wrapper was collected meanwhile): the event
            // makes a new wrapper, whose collection destroys the handle later.
            if self.pinned(h) {
                return Ok(());
            }
            {
                let mut st = self.st.borrow_mut();
                st.handles.remove(&h);
                st.toplevels.remove(&h);
                st.tree.remove(&h);
                // The backend drops its native observations of `h` in `destroy`.
                st.observed.retain(|k, _| k.0 != h);
                st.dirty.retain(|k| k.0 != h);
                st.dead.push(h, Death::Destroyed);
            }
            self.cb_release_owner(h);
            self.backend.destroy(h);
            Ok(())
        })
    }

    // ---- values ----

    pub(crate) fn op_set_prop<R: JsReader>(
        &self,
        r: &R,
        h: svl_handle,
        prop: Name<'_>,
        v: &R::V,
    ) -> Result<(), OpError> {
        self.timed(|| {
            if crate::exports::is_app_handle(h) {
                return self.app_set_prop(r, h, prop, v);
            }
            let Some(k) = self.live_kind(h)? else {
                return Ok(());
            };
            let p = self.resolve_name(NameKind::Prop, k, prop)?;
            let desc = self.member(NameKind::Prop, k, p);
            let any = SlotDesc::new(SVL_NULL);
            let pname = match (&desc, prop) {
                (Some(d), _) => d.name.clone(),
                (None, Name::Str(s)) => s.to_string(),
                (None, Name::Id(i)) => format!("#{i}"),
            };
            let prefix = || format!("<{}>.{}", self.kind_name(k), pname);
            let slot = match &desc {
                Some(d) => d.params.first().ok_or_else(|| OpError {
                    kind: OpKind::Type,
                    msg: format!("{}: property is read-only", prefix()),
                    native: None,
                })?,
                None => &any,
            };
            let mut ar = Arena::new();
            let registry = CallbackRegistry(self);
            let cx = Cx {
                types: self,
                utf16: self.utf16(),
                callbacks: Some(&registry),
                owner: h,
            };
            let raw = match convert::encode(r, v, slot, &cx, &mut ar) {
                Ok(raw) => raw,
                Err(e) => {
                    self.finish_callbacks(&mut ar, false);
                    return Err(self.conv_err(&prefix(), e));
                }
            };
            // Conversion can run user JS that destroys the target: check again before dispatch.
            if let Err(e) = self.kind_of(h) {
                self.finish_callbacks(&mut ar, false);
                return Err(e.into());
            }
            let res = self.backend.set_prop_raw(h, p, &raw).map_err(|e| self.backend_err(e));
            self.finish_callbacks(&mut ar, res.is_ok());
            // ABI v2.6 set path (REACTIVITY.md §5.2): an observed prop is read back as its `last`.
            let observed = res.is_ok() && self.st.borrow().observed.contains_key(&(h, p));
            let written = observed.then(|| unsafe { Value::from_raw(&raw) });
            drop(ar);
            if observed {
                self.after_set(h, p, written);
            }
            res
        })
    }

    pub(crate) fn op_get_prop<W: JsWriter>(&self, w: &W, h: svl_handle, prop: Name<'_>) -> Result<W::V, OpError> {
        self.timed(|| {
            if crate::exports::is_app_handle(h) {
                return self.app_get_prop(w, h, prop);
            }
            let k = self.kind_of(h)?;
            let p = self.resolve_name(NameKind::Prop, k, prop)?;
            let raw = self.backend.get_prop_raw(h, p).map_err(|e| self.backend_err(e))?;
            let v = unsafe { Value::from_raw_wide(&raw) };
            self.decode_result(w, &v)
        })
    }

    pub(crate) fn op_call<E>(
        &self,
        e: &E,
        h: svl_handle,
        method: Name<'_>,
        args: &<E as JsReader>::V,
    ) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V> + JsPromises,
    {
        self.timed(|| {
            if crate::exports::is_app_handle(h) {
                return self.app_call(e, h, method, args);
            }
            let k = self.kind_of(h)?;
            let m = self.resolve_name(NameKind::Method, k, method)?;
            let desc = self.member(NameKind::Method, k, m);
            let mname = match (&desc, method) {
                (Some(d), _) => d.name.clone(),
                (None, Name::Str(s)) => s.to_string(),
                (None, Name::Id(i)) => format!("#{i}"),
            };
            if let Some(d) = desc.as_ref().filter(|d| d.flags & SVL_M_ASYNC != 0) {
                return self.op_call_async(e, h, m, k, d, &mname, args, false);
            }
            let given = JsReader::len(e, args)?;
            let mut ar = Arena::new();
            let registry = CallbackRegistry(self);
            let cx = Cx {
                types: self,
                utf16: self.utf16(),
                callbacks: Some(&registry),
                owner: h,
            };
            let mut raw: Vec<svl_value> = Vec::with_capacity(given);
            match &desc {
                Some(d) => {
                    // OUT-only parameters are not supplied by JS and not sent to the backend.
                    let ins: Vec<&SlotDesc> = d.params.iter().filter(|p| !p.has(SVL_D_OUT)).collect();
                    let required = ins.iter().filter(|p| !p.has(SVL_D_OPTIONAL)).count();
                    if given < required || given > ins.len() {
                        return Err(OpError {
                            kind: OpKind::Type,
                            msg: format!(
                                "<{}>.{mname}: expected {} argument(s), got {given}",
                                self.kind_name(k),
                                if required == ins.len() {
                                    required.to_string()
                                } else {
                                    format!("{required} to {}", ins.len())
                                }
                            ),
                            native: None,
                        });
                    }
                    for (i, slot) in ins.iter().copied().enumerate() {
                        let prefix = format!(
                            "<{}>.{mname}({})",
                            self.kind_name(k),
                            slot.name.as_deref().unwrap_or("arg")
                        );
                        let v = if i < given {
                            JsReader::index(e, args, i)?
                        } else {
                            raw.push(svl_value::null());
                            continue;
                        };
                        match convert::encode(e, &v, slot, &cx, &mut ar) {
                            Ok(x) => raw.push(x),
                            Err(c) => {
                                self.finish_callbacks(&mut ar, false);
                                return Err(self.conv_err(&prefix, c));
                            }
                        }
                    }
                }
                None => {
                    let any = SlotDesc::new(SVL_NULL);
                    for i in 0..given {
                        let v = JsReader::index(e, args, i)?;
                        let prefix = format!("<{}>.{mname}(arg {i})", self.kind_name(k));
                        match convert::encode(e, &v, &any, &cx, &mut ar) {
                            Ok(x) => raw.push(x),
                            Err(c) => {
                                self.finish_callbacks(&mut ar, false);
                                return Err(self.conv_err(&prefix, c));
                            }
                        }
                    }
                }
            }
            if let Err(err) = self.kind_of(h) {
                self.finish_callbacks(&mut ar, false);
                return Err(err.into());
            }
            let out = self.backend.call_raw(h, m, &raw).map_err(|e| self.backend_err(e));
            self.finish_callbacks(&mut ar, out.is_ok());
            drop(ar);
            let v = unsafe { Value::from_raw_wide(&out?) };
            match desc.as_ref().filter(|d| d.params.iter().any(|p| p.has(SVL_D_OUT | SVL_D_INOUT))) {
                Some(d) => self.decode_outs(e, &mname, k, d, &v),
                None => self.decode_result(e, &v),
            }
        })
    }

    /// A method with OUT / INOUT parameters answers `LIST [ret, out...]`, outs in parameter order.
    /// JS sees one object: `ret` (unless the method is void) and one key per out parameter.
    pub(super) fn decode_outs<W: JsWriter>(&self, w: &W, mname: &str, k: svl_id, d: &MemberDesc, v: &Value) -> Result<W::V, OpError> {
        let outs: Vec<(usize, &SlotDesc)> = d.params.iter().filter(|p| p.has(SVL_D_OUT | SVL_D_INOUT)).enumerate().collect();
        let items = match v {
            Value::List(items) if items.len() == outs.len() + 1 => items,
            _ => {
                return Err(OpError {
                    kind: OpKind::Error,
                    msg: format!(
                        "<{}>.{mname}: backend answered {v:?}; expected a list of the return value and {} out value(s)",
                        self.kind_name(k),
                        outs.len()
                    ),
                    native: None,
                })
            }
        };
        let mut entries = Vec::with_capacity(items.len());
        if !d.ret.is_void() {
            entries.push(("ret".to_string(), self.decode_result(w, &items[0])?));
        }
        for (i, slot) in outs {
            let key = slot.name.clone().unwrap_or_else(|| format!("out{i}"));
            entries.push((key, self.decode_result(w, &items[i + 1])?));
        }
        w.object(entries).map_err(OpError::from)
    }

    /// After the backend call: `call`-scope callbacks end with it; all of them end when it failed
    /// (the backend did not keep them).
    pub(super) fn finish_callbacks(&self, ar: &mut Arena, ok: bool) {
        for (id, scope) in ar.take_callbacks() {
            if !ok || scope == SVL_CB_CALL {
                self.cb_release(id);
            }
        }
    }

    /// Handles that come back inside a result and are new to the host (the backend minted them
    /// for native objects it did not create) join the handle table with the kind and lifetime
    /// class the backend reports. Known handles (an object we created) are left alone.
    pub(super) fn adopt_handles(&self, v: &Value) {
        match v {
            Value::Handle(h) => {
                if self.st.borrow().handles.contains_key(h) || *h >= MAX_HANDLE || crate::exports::is_app_handle(*h) {
                    return;
                }
                if let Ok(info) = self.backend.handle_info(*h) {
                    let mut st = self.st.borrow_mut();
                    st.handles.insert(*h, info.kind);
                    if info.lifetime == SVL_LIFE_TOPLEVEL {
                        st.toplevels.insert(*h);
                        st.top_seen = true;
                    }
                }
            }
            Value::List(items) => items.iter().for_each(|i| self.adopt_handles(i)),
            Value::Record { fields, .. } => fields.iter().for_each(|i| self.adopt_handles(i)),
            // Out-parameter objects folded into an async completion.
            Value::Map(entries) => entries.iter().for_each(|(_, v)| self.adopt_handles(v)),
            _ => {}
        }
    }

    pub(super) fn decode_result<W: JsWriter>(&self, w: &W, v: &Value) -> Result<W::V, OpError> {
        self.adopt_handles(v);
        convert::decode(w, v, self).map_err(|m| match m.strip_prefix("RangeError: ") {
            Some(rest) => OpError {
                kind: OpKind::Range,
                msg: rest.to_string(),
                native: None,
            },
            None => OpError::from(m),
        })
    }

    /// A styling op (ABI v2.3). `h == 0` ops are app-wide; a dead widget handle is a no-op.
    pub(crate) fn op_style(&self, op: u32, h: svl_handle, text: Option<&str>) -> Result<i32, OpError> {
        self.timed(|| {
            if h != 0 && self.live_kind(h)?.is_none() {
                return Ok(0);
            }
            self.backend.style(op, h, text).map_err(|e| self.backend_err(e))
        })
    }

    /// ABI v2.8 (TR1): the layout rect of a widget; `None` for a dead handle or a backend without
    /// geometry.
    pub(crate) fn op_bounds(&self, h: svl_handle) -> Result<Option<[f64; 4]>, OpError> {
        self.timed(|| {
            if self.live_kind(h)?.is_none() {
                return Ok(None);
            }
            self.backend.bounds(h).map_err(|e| self.backend_err(e))
        })
    }

    pub(crate) fn op_listen(&self, h: svl_handle, signal: Name<'_>, on: bool, flags: u32) -> Result<(), OpError> {
        self.timed(|| {
            if crate::exports::is_app_handle(h) {
                // E3: events of an object with an `Emitter` field.
                return self.app_listen(h, signal, on);
            }
            let k = match self.live_kind(h)? {
                Some(k) => k,
                None if !on => return Ok(()),
                None => return Err(self.kind_of(h).unwrap_err().into()),
            };
            let s = self.resolve_name(NameKind::Signal, k, signal)?;
            if let Name::Str(name) = signal {
                self.st.borrow_mut().signal_names.insert((k, s), name.to_string());
            } else if !self.st.borrow().signal_names.contains_key(&(k, s)) {
                let name = self
                    .member(NameKind::Signal, k, s)
                    .map(|d| d.name.clone())
                    .ok_or_else(|| "listen by id requires the signal to have been resolved by name first".to_string())?;
                self.st.borrow_mut().signal_names.insert((k, s), name);
            }
            self.backend.listen_ex(h, s, on, flags).map_err(|e| self.backend_err(e))
        })
    }

    /// ABI v2.6 `host.observe(h, prop, on)`: refcounted per (h, prop); the backend's `observe` is
    /// called on the 0→1 and 1→0 transitions only. Returns whether (h, prop) is observed after the
    /// call: `false` for `on` means the backend has no change notification for this property
    /// (`SVL_OBSERVE_UNSUPPORTED`, or no `observe` member), which is not an error. `off` on a dead
    /// handle or an unobserved prop is a no-op. Errors: a dead handle (for `on`), an unknown prop,
    /// or a backend failure.
    pub(crate) fn op_observe(&self, h: svl_handle, prop: Name<'_>, on: bool) -> Result<bool, OpError> {
        self.timed(|| {
            if crate::exports::is_app_handle(h) {
                // E3: store fields are observable (`exact`); other getters are plain reads.
                return self.app_observe(h, prop, on);
            }
            let k = match self.live_kind(h)? {
                Some(k) => k,
                None if !on => return Ok(false),
                None => return Err(self.kind_of(h).unwrap_err().into()),
            };
            let p = self.resolve_name(NameKind::Prop, k, prop)?;
            let count = self.st.borrow().observed.get(&(h, p)).map_or(0, |o| o.count);
            if on {
                if count > 0 {
                    self.st.borrow_mut().observed.get_mut(&(h, p)).unwrap().count += 1;
                    return Ok(true);
                }
                let desc = self.member(NameKind::Prop, k, p);
                let name = match prop {
                    Name::Str(s) => s.to_string(),
                    Name::Id(id) => desc.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| format!("#{id}")),
                };
                let proportional = desc.is_some_and(|d| d.flags & SVL_M_PROPORTIONAL != 0);
                match self.backend.observe(h, p, true) {
                    Ok(()) => {
                        // The baseline `last`: a notification that changes nothing (GTK re-notifies an
                        // identical set) is then dropped from the start.
                        let last = if proportional { None } else { self.read_last(h, p) };
                        let o = Observed { count: 1, name, proportional, last, force: false };
                        self.st.borrow_mut().observed.insert((h, p), o);
                        Ok(true)
                    }
                    Err(e) if e.code == SVL_OBSERVE_UNSUPPORTED => Ok(false),
                    Err(e) => Err(self.backend_err(e)),
                }
            } else {
                match count {
                    0 => Ok(false),
                    1 => {
                        self.st.borrow_mut().observed.remove(&(h, p));
                        self.backend.observe(h, p, false).map_err(|e| self.backend_err(e))?;
                        Ok(false)
                    }
                    _ => {
                        self.st.borrow_mut().observed.get_mut(&(h, p)).unwrap().count -= 1;
                        Ok(true)
                    }
                }
            }
        })
    }

    /// Number of (handle, prop) pairs observed natively (ABI v2.6).
    pub(crate) fn observed_count(&self) -> usize {
        self.st.borrow().observed.len()
    }

    // ---- boxed values ----

    /// A boxed handle reached JS: track it so release is idempotent.
    pub(crate) fn note_boxed(&self, h: svl_handle) {
        // A VALUE handle answers its kind in `handle_info` (0 for opaque values).
        let kind = self.backend.handle_info(h).map(|i| i.kind).unwrap_or(0);
        self.st.borrow_mut().boxed.insert(h, kind);
    }

    /// Explicit release (`dispose()`): immediately, once. A second call is a no-op.
    pub(crate) fn release_boxed(&self, h: svl_handle) {
        let live = self.st.borrow_mut().boxed.remove(&h).is_some();
        if live {
            self.backend.destroy(h);
        }
    }

    /// GC release: only records the handle. Never calls the backend (may run mid-conversion).
    pub(crate) fn queue_release(&self, h: svl_handle) {
        if let Ok(mut st) = self.st.try_borrow_mut() {
            if st.boxed.contains_key(&h) {
                st.release_queue.push(h);
            }
        }
    }

    /// Applies queued releases. Called at checkpoints, outside any JS entry.
    pub(crate) fn process_releases(&self) {
        let queue = std::mem::take(&mut self.st.borrow_mut().release_queue);
        for h in queue {
            self.release_boxed(h);
        }
        self.cb_process_frees();
    }

    pub(crate) fn live_boxed(&self) -> usize {
        self.st.borrow().boxed.len()
    }
}
