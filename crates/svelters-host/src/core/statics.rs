//! ABI v2.5 host ops: `construct` (an instance with construct-time properties), `call_static`
//! (static and constructor functions of a kind) and the type chain of a handle.
//!
//! Both ops encode their arguments with the descriptors of the target kind exactly like
//! `setProp` / `call`, then hand the raw values to the backend. A missing backend member is a
//! clean error ("this backend does not implement ..."), never a crash.

use super::*;
use crate::convert::{self, Arena, Cx, JsReader, JsWriter};
use svelters_abi::*;

impl Core {
    fn kind_from(&self, kind: Name<'_>) -> Result<svl_id, OpError> {
        Ok(match kind {
            Name::Str(s) => self.op_kind_id(s)?,
            Name::Id(i) => i,
        })
    }

    /// `host.construct(kind, props)`: `props` is an object of property name -> value.
    pub(crate) fn op_construct<E>(&self, e: &E, kind: Name<'_>, props: &<E as JsReader>::V) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V>,
    {
        self.timed(|| {
            self.tree_ops_allowed()?;
            if !self.backend.has_construct() {
                return Err("this backend does not implement construct".to_string().into());
            }
            let k = self.kind_from(kind)?;
            let kname = self.kind_name(k);
            let mut ar = Arena::new();
            let registry = CallbackRegistry(self);
            let cx = Cx { types: self, utf16: self.utf16(), callbacks: Some(&registry), owner: 0 };
            let mut ids: Vec<svl_id> = Vec::new();
            let mut raw: Vec<svl_value> = Vec::new();
            let keys = match JsReader::kind(e, props) {
                convert::JsKind::Undefined | convert::JsKind::Null => Vec::new(),
                convert::JsKind::Object => JsReader::keys(e, props)?,
                _ => {
                    return Err(OpError {
                        kind: OpKind::Type,
                        msg: format!("<{kname}>: construct properties must be an object"),
                        native: None,
                    })
                }
            };
            for key in keys {
                let p = self.resolve(NameKind::Prop, k, &key)?;
                let desc = self.member(NameKind::Prop, k, p);
                let any = SlotDesc::new(SVL_NULL);
                let slot = match &desc {
                    Some(d) => d.params.first().ok_or_else(|| OpError {
                        kind: OpKind::Type,
                        msg: format!("<{kname}>.{key}: property is read-only"),
                        native: None,
                    })?,
                    None => &any,
                };
                let v = JsReader::field(e, props, &key)?;
                match convert::encode(e, &v, slot, &cx, &mut ar) {
                    Ok(x) => {
                        ids.push(p);
                        raw.push(x);
                    }
                    Err(c) => {
                        self.finish_callbacks(&mut ar, false);
                        return Err(self.conv_err(&format!("<{kname}>.{key}"), c));
                    }
                }
            }
            let out = self.backend.construct_raw(k, &ids, &raw).map_err(|e| self.backend_err(e));
            self.finish_callbacks(&mut ar, out.is_ok());
            drop(ar);
            let v = unsafe { Value::from_raw_wide(&out?) };
            if let Value::Handle(h) = v {
                if h >= MAX_HANDLE {
                    self.backend.destroy(h);
                    return Err(format!("backend issued handle {h}, which JS cannot represent exactly").into());
                }
            }
            self.decode_result(e, &v)
        })
    }

    /// `host.callStatic(kind, method, args)`.
    pub(crate) fn op_call_static<E>(
        &self,
        e: &E,
        kind: Name<'_>,
        method: Name<'_>,
        args: &<E as JsReader>::V,
    ) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V>,
    {
        self.timed(|| {
            if !self.backend.has_call_static() {
                return Err("this backend does not implement call_static".to_string().into());
            }
            let k = self.kind_from(kind)?;
            let m = self.resolve_name(NameKind::Method, k, method)?;
            let desc = self.member(NameKind::Method, k, m);
            let mname = match (&desc, method) {
                (Some(d), _) => d.name.clone(),
                (None, Name::Str(s)) => s.to_string(),
                (None, Name::Id(i)) => format!("#{i}"),
            };
            let kname = self.kind_name(k);
            if desc.as_ref().is_some_and(|d| d.flags & SVL_M_ASYNC != 0) {
                return Err(format!("<{kname}>.{mname}: asynchronous static methods are not supported yet").into());
            }
            if desc.as_ref().is_some_and(|d| d.flags & SVL_M_STATIC == 0) {
                return Err(format!("<{kname}>.{mname} is an instance method; call it on an object").into());
            }
            let given = JsReader::len(e, args)?;
            let mut ar = Arena::new();
            let registry = CallbackRegistry(self);
            let cx = Cx { types: self, utf16: self.utf16(), callbacks: Some(&registry), owner: 0 };
            let mut raw: Vec<svl_value> = Vec::with_capacity(given);
            match &desc {
                Some(d) => {
                    let ins: Vec<&SlotDesc> = d.params.iter().filter(|p| !p.has(SVL_D_OUT)).collect();
                    let required = ins.iter().filter(|p| !p.has(SVL_D_OPTIONAL)).count();
                    if given < required || given > ins.len() {
                        return Err(OpError {
                            kind: OpKind::Type,
                            msg: format!(
                                "<{kname}>.{mname}: expected {} argument(s), got {given}",
                                if required == ins.len() { required.to_string() } else { format!("{required} to {}", ins.len()) }
                            ),
                            native: None,
                        });
                    }
                    for (i, slot) in ins.iter().copied().enumerate() {
                        if i >= given {
                            raw.push(svl_value::null());
                            continue;
                        }
                        let v = JsReader::index(e, args, i)?;
                        let prefix = format!("<{kname}>.{mname}({})", slot.name.as_deref().unwrap_or("arg"));
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
                        match convert::encode(e, &v, &any, &cx, &mut ar) {
                            Ok(x) => raw.push(x),
                            Err(c) => {
                                self.finish_callbacks(&mut ar, false);
                                return Err(self.conv_err(&format!("<{kname}>.{mname}(arg {i})"), c));
                            }
                        }
                    }
                }
            }
            let out = self.backend.call_static_raw(k, m, &raw).map_err(|e| self.backend_err(e));
            self.finish_callbacks(&mut ar, out.is_ok());
            drop(ar);
            let v = unsafe { Value::from_raw_wide(&out?) };
            match desc.as_ref().filter(|d| d.params.iter().any(|p| p.has(SVL_D_OUT | SVL_D_INOUT))) {
                Some(d) => self.decode_outs(e, &mname, k, d, &v),
                None => self.decode_result(e, &v),
            }
        })
    }

    /// The qualified native type names of a live handle, most derived first, as far as the backend
    /// names its kinds (`describe(SVL_KIND, 0, kind)`). Empty when the backend does not say.
    pub(crate) fn op_type_chain(&self, h: svl_handle) -> Result<Vec<String>, OpError> {
        let mut k = self.kind_of(h)?;
        let mut out = Vec::new();
        for _ in 0..64 {
            if k == 0 {
                break;
            }
            let Some(d) = self.backend.describe(NameKind::Kind, 0, k) else { break };
            out.push(d.name.clone());
            k = d.ret.ty;
        }
        Ok(out)
    }
}
