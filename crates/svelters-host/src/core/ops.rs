//! JS host ops (called by the engine's `__svelters_host`): name and descriptor caches, the
//! type-directed conversion around each backend call, and boxed-value release.
//!
//! Every op converts its JS arguments completely *before* touching handle liveness a second time
//! and before calling the backend: reading a record field can run user JS (getters, Proxy traps),
//! which may destroy the target handle. No `RefCell` is borrowed across an engine or backend call.

use super::*;
use crate::convert::{self, Arena, ConvError, Cx, ErrKind, JsReader, JsWriter, TypeSource};
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

impl From<AbiError> for OpError {
    fn from(e: AbiError) -> OpError {
        OpError {
            kind: OpKind::Error,
            msg: e.message,
            native: e.native,
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
        let t = self.backend.describe_type(id).map(|d| Rc::new(TypeInfo::new(d)));
        self.st.borrow_mut().types.insert(id, t.clone());
        t
    }
}

impl Core {
    fn utf16(&self) -> bool {
        self.backend.caps() & SVL_CAP_UTF16_IN != 0
    }

    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> Result<svl_id, String> {
        if let Some(id) = self
            .st
            .borrow()
            .names
            .get(&(what as u32, kind))
            .and_then(|m| m.get(name))
        {
            return Ok(*id);
        }
        let id = self.backend.resolve(what, kind, name);
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

    fn resolve_name(&self, what: NameKind, kind: svl_id, n: Name<'_>) -> Result<svl_id, String> {
        match n {
            Name::Str(s) => self.resolve(what, kind, s),
            Name::Id(id) => Ok(id),
        }
    }

    pub(crate) fn kind_of(&self, h: svl_handle) -> Result<svl_id, String> {
        self.st
            .borrow()
            .handles
            .get(&h)
            .copied()
            .ok_or_else(|| format!("stale or foreign handle {h}"))
    }

    fn kind_name(&self, k: svl_id) -> String {
        self.st
            .borrow()
            .kind_names
            .get(&k)
            .cloned()
            .unwrap_or_else(|| format!("kind#{k}"))
    }

    /// The cached descriptor of a member, if the backend publishes one.
    fn member(&self, what: NameKind, kind: svl_id, id: svl_id) -> Option<Rc<MemberDesc>> {
        let key = (what as u32, kind, id);
        if let Some(m) = self.st.borrow().members.get(&key) {
            return m.clone();
        }
        let m = self.backend.describe(what, kind, id).map(Rc::new);
        self.st.borrow_mut().members.insert(key, m.clone());
        m
    }

    fn conv_err(&self, prefix: &str, e: ConvError) -> OpError {
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
        let k = self.backend.resolve(NameKind::Kind, 0, kind);
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
            let kid = self.op_kind_id(kind)?;
            self.create_kind(kid)
        })
    }

    pub(crate) fn op_create_id(&self, kid: svl_id) -> Result<svl_handle, OpError> {
        self.timed(|| self.create_kind(kid))
    }

    fn create_kind(&self, kid: svl_id) -> Result<svl_handle, OpError> {
        let h = self.backend.create(kid).map_err(OpError::from)?;
        self.st.borrow_mut().handles.insert(h, kid);
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
                .map_err(OpError::from)
        })
    }

    pub(crate) fn op_remove(&self, parent: svl_handle, child: svl_handle) -> Result<(), OpError> {
        self.timed(|| {
            self.kind_of(parent)?;
            self.kind_of(child)?;
            self.backend.remove(parent, child).map_err(OpError::from)
        })
    }

    pub(crate) fn op_destroy(&self, h: svl_handle) -> Result<(), OpError> {
        self.timed(|| {
            self.kind_of(h)?;
            self.st.borrow_mut().handles.remove(&h);
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
            let k = self.kind_of(h)?;
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
            let cx = Cx {
                types: self,
                utf16: self.utf16(),
            };
            let raw = convert::encode(r, v, slot, &cx, &mut ar).map_err(|e| self.conv_err(&prefix(), e))?;
            // Conversion can run user JS that destroys the target: check again before dispatch.
            self.kind_of(h)?;
            let res = self.backend.set_prop_raw(h, p, &raw).map_err(OpError::from);
            drop(ar);
            res
        })
    }

    pub(crate) fn op_get_prop<W: JsWriter>(&self, w: &W, h: svl_handle, prop: Name<'_>) -> Result<W::V, OpError> {
        self.timed(|| {
            let k = self.kind_of(h)?;
            let p = self.resolve_name(NameKind::Prop, k, prop)?;
            let raw = self.backend.get_prop_raw(h, p).map_err(OpError::from)?;
            let v = unsafe { Value::from_raw(&raw) };
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
        E: JsReader + JsWriter<V = <E as JsReader>::V>,
    {
        self.timed(|| {
            let k = self.kind_of(h)?;
            let m = self.resolve_name(NameKind::Method, k, method)?;
            let desc = self.member(NameKind::Method, k, m);
            let mname = match (&desc, method) {
                (Some(d), _) => d.name.clone(),
                (None, Name::Str(s)) => s.to_string(),
                (None, Name::Id(i)) => format!("#{i}"),
            };
            let given = JsReader::len(e, args)?;
            let mut ar = Arena::new();
            let cx = Cx {
                types: self,
                utf16: self.utf16(),
            };
            let mut raw: Vec<svl_value> = Vec::with_capacity(given);
            match &desc {
                Some(d) => {
                    let required = d.params.iter().filter(|p| !p.has(SVL_D_OPTIONAL)).count();
                    if given < required || given > d.params.len() {
                        return Err(OpError {
                            kind: OpKind::Type,
                            msg: format!(
                                "<{}>.{mname}: expected {} argument(s), got {given}",
                                self.kind_name(k),
                                if required == d.params.len() {
                                    required.to_string()
                                } else {
                                    format!("{required} to {}", d.params.len())
                                }
                            ),
                            native: None,
                        });
                    }
                    for (i, slot) in d.params.iter().enumerate() {
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
                        raw.push(convert::encode(e, &v, slot, &cx, &mut ar).map_err(|c| self.conv_err(&prefix, c))?);
                    }
                }
                None => {
                    let any = SlotDesc::new(SVL_NULL);
                    for i in 0..given {
                        let v = JsReader::index(e, args, i)?;
                        let prefix = format!("<{}>.{mname}(arg {i})", self.kind_name(k));
                        raw.push(convert::encode(e, &v, &any, &cx, &mut ar).map_err(|c| self.conv_err(&prefix, c))?);
                    }
                }
            }
            self.kind_of(h)?;
            let out = self.backend.call_raw(h, m, &raw).map_err(OpError::from);
            drop(ar);
            let v = unsafe { Value::from_raw(&out?) };
            self.decode_result(e, &v)
        })
    }

    fn decode_result<W: JsWriter>(&self, w: &W, v: &Value) -> Result<W::V, OpError> {
        convert::decode(w, v, self).map_err(|m| match m.strip_prefix("RangeError: ") {
            Some(rest) => OpError {
                kind: OpKind::Range,
                msg: rest.to_string(),
                native: None,
            },
            None => OpError::from(m),
        })
    }

    pub(crate) fn op_listen(&self, h: svl_handle, signal: Name<'_>, on: bool) -> Result<(), OpError> {
        self.timed(|| {
            let k = self.kind_of(h)?;
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
            self.backend.listen(h, s, on).map_err(OpError::from)
        })
    }

    // ---- boxed values ----

    /// A boxed handle reached JS: track it so release is idempotent.
    pub(crate) fn note_boxed(&self, h: svl_handle) {
        self.st.borrow_mut().boxed.insert(h);
    }

    /// Explicit release (`dispose()`): immediately, once. A second call is a no-op.
    pub(crate) fn release_boxed(&self, h: svl_handle) {
        let live = self.st.borrow_mut().boxed.remove(&h);
        if live {
            self.backend.destroy(h);
        }
    }

    /// GC release: only records the handle. Never calls the backend (may run mid-conversion).
    pub(crate) fn queue_release(&self, h: svl_handle) {
        if let Ok(mut st) = self.st.try_borrow_mut() {
            if st.boxed.contains(&h) {
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
    }

    pub(crate) fn live_boxed(&self) -> usize {
        self.st.borrow().boxed.len()
    }
}
