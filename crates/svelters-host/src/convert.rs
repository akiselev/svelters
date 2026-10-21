//! Type-directed conversion between JS values and the ABI's `svl_value`, written once over two
//! small engine traits ([`JsReader`], [`JsWriter`]).
//!
//! Inbound ([`encode`]): the backend's slot descriptor drives validation and coercion, and the
//! result is a raw `svl_value` tree whose strings and byte ranges are *borrowed* from the engine.
//! Whatever must stay alive until the backend call returns is parked in an [`Arena`].
//!
//! Outbound ([`decode`]): owned [`Value`]s become JS values; enum and flags values use the type
//! tables for names, records use field names from the descriptors.
//!
//! Nothing here names an engine type; `quickjs.rs` implements the traits.

use std::any::Any;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;
use svelters_abi::*;

pub(crate) use crate::exports::buffers::Take;
use crate::exports::buffers;

/// Arrays and records nest at most this deep (guards cyclic structures).
pub(crate) const MAX_DEPTH: u32 = 16;
const MAX_SAFE: f64 = 9_007_199_254_740_992.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JsKind {
    Undefined,
    Null,
    Bool,
    Number,
    String,
    BigInt,
    Array,
    Object,
    Function,
    Other,
}

/// A borrowed view of a JS string. The pointer stays valid while the guard that came with it
/// (parked in the [`Arena`]) is alive.
#[derive(Clone, Copy)]
pub(crate) struct StrView {
    pub ptr: *const u8,
    /// Code units (bytes for UTF-8, u16s for UTF-16).
    pub len: usize,
    pub utf16: bool,
    pub nul_term: bool,
}

pub(crate) type Guard = Box<dyn Any>;

/// What the converter needs from a JS engine to read values.
pub(crate) trait JsReader {
    type V: Clone;
    fn kind(&self, v: &Self::V) -> JsKind;
    fn boolean(&self, v: &Self::V) -> bool;
    fn number(&self, v: &Self::V) -> f64;
    /// `None` when the BigInt does not fit an i64.
    fn bigint(&self, v: &Self::V) -> Option<i64>;
    /// Borrows the string as UTF-8, or as UTF-16 when `want_utf16` (zero-copy for wide strings).
    fn string(&self, v: &Self::V, want_utf16: bool, guards: &mut Vec<Guard>) -> Result<StrView, String>;
    fn field(&self, obj: &Self::V, key: &str) -> Result<Self::V, String>;
    fn has_field(&self, obj: &Self::V, key: &str) -> Result<bool, String>;
    fn keys(&self, obj: &Self::V) -> Result<Vec<String>, String>;
    fn len(&self, arr: &Self::V) -> Result<usize, String>;
    fn index(&self, arr: &Self::V, i: usize) -> Result<Self::V, String>;
    /// Epoch milliseconds when `v` is a `Date`.
    fn date_ms(&self, v: &Self::V) -> Option<f64>;
    /// Stores the function behind `v` for the callback `id` (the engine keeps it alive).
    fn store_function(&self, v: &Self::V, id: u64) -> Result<(), String>;
    /// True for an ArrayBuffer or a typed array / DataView over one.
    fn is_bytes(&self, v: &Self::V) -> bool;
    /// Borrowed bytes of an ArrayBuffer / typed array, valid while the guard lives.
    fn bytes(&self, v: &Self::V, guards: &mut Vec<Guard>) -> Result<(*const u8, usize), String>;
    /// `(handle, type id)` when `v` is a boxed-value wrapper that has not been disposed.
    fn boxed(&self, v: &Self::V) -> Result<Option<(svl_handle, svl_id)>, String>;
    /// `(signature, value)` when `v` was made by `variant(signature, value)`.
    fn typed_variant(&self, _v: &Self::V) -> Result<Option<(String, Self::V)>, String> {
        Ok(None)
    }
    /// A renderer node standing for a native handle: `None` when `v` is not a node, `Some(None)`
    /// for a node that has no live native object.
    fn node_handle(&self, v: &Self::V) -> Result<Option<Option<f64>>, String>;
    /// E2: checks that `v` fits a buffer slot (a typed array of `elem`, usable as `take` says) and parks
    /// it; returns its index. Nothing is moved until [`JsReader::commit_buffers`], so a later argument
    /// that fails to convert leaves the caller's arrays untouched.
    fn stage_buffer(&self, _v: &Self::V, elem: Elem, _take: Take) -> Result<usize, String> {
        Err(format!("this engine cannot pass a {} to a Rust export", elem.js_name()))
    }
    /// E2: moves the staged arrays out of JS (transfer, or reclaim of Rust memory) or snapshots them,
    /// in staging order.
    fn commit_buffers(&self) -> Result<Vec<BufferValue>, String> {
        Ok(Vec::new())
    }
}

/// What async native methods need from a JS engine.
pub(crate) trait JsPromises: JsReader {
    /// Creates a Promise and stores, under callback `id`, the function that settles it: called with a
    /// result it resolves, called with a `NativeError` it rejects.
    fn new_completion(&self, id: u64) -> Result<Self::V, String>;
    /// Calls `backend->cancel(id)` when the AbortSignal `signal` aborts.
    fn watch_abort(&self, signal: &Self::V, id: u64) -> Result<(), String>;
    /// E1: creates the Promise of offloaded export call `id`. `options` is the call's
    /// `{ signal, onProgress }`: an abort of `signal` rejects the Promise at once (`cancelled`) and
    /// cancels the op; `onProgress` receives progress values.
    fn new_op(&self, id: u64, options: Option<&Self::V>) -> Result<Self::V, String>;
    /// E1: settles (or reports progress of) op `id` from inside a host op: one microtask later, so a
    /// rejection is not reported as unhandled before the caller could attach a handler.
    fn settle_op_soon(&self, s: &crate::engine::OpSettle) -> Result<(), String>;
}

/// What the converter needs from a JS engine to build values.
pub(crate) trait JsWriter {
    type V;
    fn null(&self) -> Result<Self::V, String>;
    fn boolean(&self, b: bool) -> Result<Self::V, String>;
    fn number(&self, n: f64) -> Result<Self::V, String>;
    fn string(&self, s: &str) -> Result<Self::V, String>;
    /// UTF-16 text from a backend with `SVL_CAP_UTF16_OUT`, without transcoding through UTF-8.
    fn string16(&self, s: &[u16]) -> Result<Self::V, String>;
    fn array(&self, items: Vec<Self::V>) -> Result<Self::V, String>;
    fn object(&self, entries: Vec<(String, Self::V)>) -> Result<Self::V, String>;
    fn bytes(&self, b: &[u8]) -> Result<Self::V, String>;
    fn date(&self, ms: f64) -> Result<Self::V, String>;
    fn boxed(&self, handle: svl_handle, ty: svl_id, type_name: &str) -> Result<Self::V, String>;
    /// A native object handle: the renderer's node for it when one is installed, else the number.
    fn handle(&self, h: svl_handle) -> Result<Self::V, String>;
    fn native_error(&self, domain: &str, code: Self::V, message: &str) -> Result<Self::V, String>;
    /// E2: a typed array over the buffer's memory (zero-copy where the engine can). The default copies.
    fn buffer(&self, b: &BufferValue) -> Result<Self::V, String> {
        b.with_bytes(|bytes| self.bytes(bytes))
    }
    /// A JS `bigint` (members marked `SVL_D_BIGINT`, e.g. `#[js(bigint)]` app exports).
    fn bigint(&self, i: i64) -> Result<Self::V, String> {
        Err(format!("RangeError: value {i} needs a bigint, which this engine does not support"))
    }
    fn bigint_u(&self, u: u64) -> Result<Self::V, String> {
        match i64::try_from(u) {
            Ok(i) => self.bigint(i),
            Err(_) => Err(format!("RangeError: value {u} needs a bigint, which this engine does not support")),
        }
    }
}

/// Descriptor access for the converter.
pub(crate) trait TypeSource {
    fn type_info(&self, id: svl_id) -> Option<Rc<TypeInfo>>;
}

/// A [`TypeDesc`] with name lookup tables built once.
pub(crate) struct TypeInfo {
    pub desc: TypeDesc,
    pub by_name: HashMap<String, i64>,
    pub by_value: HashMap<i64, usize>,
}

impl TypeInfo {
    pub(crate) fn new(desc: TypeDesc) -> TypeInfo {
        let mut by_name = HashMap::new();
        let mut by_value = HashMap::new();
        for (i, m) in desc.members.iter().enumerate() {
            by_name.insert(m.name.clone(), m.value);
            by_value.entry(m.value).or_insert(i);
        }
        TypeInfo {
            desc,
            by_name,
            by_value,
        }
    }

    /// `'a' | 'b' | ...` for error messages.
    fn names(&self) -> String {
        let mut s = String::new();
        for (i, m) in self.desc.members.iter().enumerate() {
            if i == 8 {
                let _ = write!(s, " | ... ({} more)", self.desc.members.len() - 8);
                break;
            }
            if i > 0 {
                s.push_str(" | ");
            }
            let _ = write!(s, "'{}'", m.name);
        }
        s
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ErrKind {
    Type,
    Range,
}

/// A conversion failure: the message and the path of fields it occurred under.
#[derive(Clone, Debug)]
pub(crate) struct ConvError {
    pub kind: ErrKind,
    pub msg: String,
    /// Innermost first.
    pub path: Vec<String>,
}

impl ConvError {
    pub(crate) fn type_(msg: impl Into<String>) -> ConvError {
        ConvError {
            kind: ErrKind::Type,
            msg: msg.into(),
            path: Vec::new(),
        }
    }
    pub(crate) fn range(msg: impl Into<String>) -> ConvError {
        ConvError {
            kind: ErrKind::Range,
            msg: msg.into(),
            path: Vec::new(),
        }
    }
    fn at(mut self, seg: impl Into<String>) -> ConvError {
        self.path.push(seg.into());
        self
    }
    /// Message with the field path appended to `prefix` (`<box>.spacing`).
    pub(crate) fn render(&self, prefix: &str) -> String {
        let mut s = prefix.to_string();
        for seg in self.path.iter().rev() {
            if seg.starts_with('[') {
                s.push_str(seg);
            } else {
                s.push('.');
                s.push_str(seg);
            }
        }
        format!("{s}: {}", self.msg)
    }
}

impl From<String> for ConvError {
    fn from(m: String) -> ConvError {
        ConvError::type_(m)
    }
}

/// Registers callbacks the converter meets in an argument list.
pub(crate) trait CallbackSink {
    /// Reserves an id for a function that `owner`'s call hands to the backend.
    fn register(&self, owner: svl_handle, sig: svl_id, scope: u16) -> Result<u64, String>;
}

/// Keeps everything a raw `svl_value` tree points at alive until the backend call returns.
#[derive(Default)]
pub(crate) struct Arena {
    lists: Vec<Box<[svl_value]>>,
    guards: Vec<Guard>,
    /// Callbacks registered while encoding: `(id, scope)`.
    cbs: Vec<(u64, u16)>,
}

impl Arena {
    pub(crate) fn take_callbacks(&mut self) -> Vec<(u64, u16)> {
        std::mem::take(&mut self.cbs)
    }

    pub(crate) fn new() -> Arena {
        Arena::default()
    }
    fn list(&mut self, items: Vec<svl_value>) -> svl_list {
        let b = items.into_boxed_slice();
        let l = svl_list {
            ptr: b.as_ptr(),
            len: b.len(),
        };
        self.lists.push(b);
        l
    }
}

pub(crate) struct Cx<'c> {
    pub types: &'c dyn TypeSource,
    /// Backend accepts UTF-16 strings.
    pub utf16: bool,
    /// Where function arguments are registered; `None` refuses them.
    pub callbacks: Option<&'c dyn CallbackSink>,
    /// The handle whose op carries the callbacks (their lifetime is tied to it).
    pub owner: svl_handle,
}

/// Abbreviated JS value description for error messages: `"8px" (string)`, `3.5`, `null`.
fn describe<R: JsReader>(r: &R, v: &R::V) -> String {
    match r.kind(v) {
        JsKind::Undefined => "undefined".into(),
        JsKind::Null => "null".into(),
        JsKind::Bool => format!("{} (boolean)", r.boolean(v)),
        JsKind::Number => {
            let n = r.number(v);
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", n as i64)
            } else {
                format!("{n}")
            }
        }
        JsKind::String => {
            let mut g = Vec::new();
            match r.string(v, false, &mut g) {
                Ok(sv) => {
                    let bytes = unsafe { std::slice::from_raw_parts(sv.ptr, sv.len) };
                    let s = String::from_utf8_lossy(bytes);
                    let short: String = s.chars().take(24).collect();
                    let ell = if s.chars().count() > 24 { "..." } else { "" };
                    format!("{:?}{ell} (string)", short)
                }
                Err(_) => "string".into(),
            }
        }
        JsKind::BigInt => "bigint".into(),
        JsKind::Array => "array".into(),
        JsKind::Object => "object".into(),
        JsKind::Function => "function".into(),
        JsKind::Other => "unsupported value".into(),
    }
}

fn slot_type_name(slot: &SlotDesc, cx: &Cx) -> String {
    match slot.tag {
        SVL_BOOL => "boolean".into(),
        SVL_I64 => "integer".into(),
        SVL_F64 if slot.ty == SVL_T_DATETIME => "Date".into(),
        SVL_F64 => "number".into(),
        SVL_STR => "string".into(),
        SVL_HANDLE => "object handle".into(),
        SVL_LIST => "array".into(),
        SVL_BYTES => match buffers::spec(slot.ty) {
            Some((elem, _)) => elem.js_name().into(),
            None => "ArrayBuffer or typed array".into(),
        },
        SVL_ENUM | SVL_FLAGS | SVL_RECORD | SVL_BOXED => cx
            .types
            .type_info(slot.ty)
            .map(|t| t.desc.name.clone())
            .unwrap_or_else(|| "value".into()),
        _ => "value".into(),
    }
}

fn expected<R: JsReader>(r: &R, v: &R::V, what: &str) -> ConvError {
    ConvError::type_(format!("expected {what}, got {}", describe(r, v)))
}

/// Encodes `v` for `slot`.
pub(crate) fn encode<R: JsReader>(
    r: &R,
    v: &R::V,
    slot: &SlotDesc,
    cx: &Cx,
    ar: &mut Arena,
) -> Result<svl_value, ConvError> {
    encode_at(r, v, slot, cx, ar, 0)
}

fn encode_at<R: JsReader>(
    r: &R,
    v: &R::V,
    slot: &SlotDesc,
    cx: &Cx,
    ar: &mut Arena,
    depth: u32,
) -> Result<svl_value, ConvError> {
    if depth > MAX_DEPTH {
        return Err(ConvError::type_("value nested too deeply (or cyclic)"));
    }
    let kind = r.kind(v);
    match kind {
        JsKind::Null => {
            return if slot.tag == SVL_NULL || slot.has(SVL_D_NULLABLE) {
                Ok(svl_value::null())
            } else {
                Err(expected(r, v, &slot_type_name(slot, cx)))
            };
        }
        JsKind::Undefined => {
            return if slot.has(SVL_D_OPTIONAL) {
                Ok(svl_value::null())
            } else if slot.tag == SVL_NULL && slot.name.is_none() && slot.ty == 0 {
                Err(ConvError::type_("undefined is not a valid host value"))
            } else {
                Err(expected(r, v, &slot_type_name(slot, cx)))
            };
        }
        JsKind::Function | JsKind::Other if slot.tag != SVL_CALLBACK => {
            return Err(if slot.tag == SVL_NULL {
                ConvError::type_(format!("unsupported value type '{}' in host call", describe(r, v)))
            } else {
                expected(r, v, &slot_type_name(slot, cx))
            });
        }
        _ => {}
    }

    match slot.tag {
        SVL_NULL => dynamic(r, v, kind, slot.ty == SVL_T_VARIANT, cx, ar, depth),
        SVL_BOOL => encode_bool(r, v, kind),
        SVL_I64 => encode_int(r, v, kind, slot),
        SVL_F64 => encode_float(r, v, kind, slot),
        SVL_STR => encode_str(r, v, kind, slot, cx, ar),
        SVL_ENUM => encode_enum(r, v, kind, slot, cx, ar),
        SVL_FLAGS => encode_flags(r, v, kind, slot, cx, ar),
        SVL_RECORD => encode_record(r, v, kind, slot, cx, ar, depth),
        SVL_LIST => encode_list(r, v, kind, slot, cx, ar, depth),
        SVL_HANDLE => {
            if kind == JsKind::String && slot.has(SVL_D_STRING_FORM) {
                // An object that has a canonical string form (a shortcut trigger, a file path).
                return encode_str(r, v, kind, slot, cx, ar);
            }
            let n = if kind == JsKind::Object {
                match r.node_handle(v)? {
                    Some(Some(n)) => n,
                    Some(None) => return Err(ConvError::type_("node has no live native object (not created yet, or already dead)".to_string())),
                    None => return Err(expected(r, v, "object handle")),
                }
            } else if kind == JsKind::Number {
                r.number(v)
            } else {
                return Err(expected(r, v, "object handle"));
            };
            if !(n.is_finite() && n >= 1.0 && n.fract() == 0.0 && n < MAX_SAFE) {
                return Err(ConvError::type_(format!("invalid handle {}", describe(r, v))));
            }
            let mut out = svl_value::new(SVL_HANDLE, svl_value_u { h: n as u64 });
            out.type_ = slot.ty;
            Ok(out)
        }
        SVL_BYTES => {
            if let Some((elem, take)) = buffers::spec(slot.ty) {
                // E2: a typed array for a `Buffer<T>`/`Shared<[T]>` slot. The raw value only carries
                // the staging index; `owned` puts the committed buffer there.
                let i = r.stage_buffer(v, elem, take)?;
                let mut out = svl_value::new(SVL_BYTES, svl_value_u { bytes: svl_bytes { ptr: std::ptr::null(), len: i } });
                out.type_ = slot.ty;
                return Ok(out);
            }
            if !r.is_bytes(v) {
                return Err(expected(r, v, "ArrayBuffer or typed array"));
            }
            let (ptr, len) = r.bytes(v, &mut ar.guards)?;
            Ok(svl_value::new(
                SVL_BYTES,
                svl_value_u {
                    bytes: svl_bytes { ptr, len },
                },
            ))
        }
        SVL_BOXED => match r.boxed(v)? {
            Some((h, ty)) if slot.ty == 0 || ty == slot.ty => {
                Ok(svl_value::typed(SVL_BOXED, ty, svl_value_u { h }))
            }
            Some(_) => Err(ConvError::type_(format!(
                "expected {}, got a different boxed type",
                slot_type_name(slot, cx)
            ))),
            None if slot.has(SVL_D_STRING_FORM) && kind == JsKind::String => {
                let mut out = encode_str(r, v, kind, slot, cx, ar)?;
                out.type_ = slot.ty;
                Ok(out)
            }
            None => Err(expected(r, v, &format!("a live {}", slot_type_name(slot, cx)))),
        },
        SVL_MAP => encode_map(r, v, kind, cx, ar, depth),
        SVL_CALLBACK => encode_callback(r, v, kind, slot, cx, ar),
        t => Err(ConvError::type_(format!("descriptor has unsupported tag {t}"))),
    }
}

fn encode_callback<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    slot: &SlotDesc,
    cx: &Cx,
    ar: &mut Arena,
) -> Result<svl_value, ConvError> {
    if kind != JsKind::Function {
        return Err(expected(r, v, "function"));
    }
    let sink = cx
        .callbacks
        .ok_or_else(|| ConvError::type_("a function is not valid here (no callback support in this context)"))?;
    let ti = type_of(slot, cx)?;
    let id = sink.register(cx.owner, slot.ty, ti.desc.scope)?;
    ar.cbs.push((id, ti.desc.scope));
    r.store_function(v, id)?;
    Ok(svl_value::typed(SVL_CALLBACK, slot.ty, svl_value_u { u: id }))
}

fn encode_bool<R: JsReader>(r: &R, v: &R::V, kind: JsKind) -> Result<svl_value, ConvError> {
    let b = match kind {
        JsKind::Bool => r.boolean(v),
        JsKind::String => match str_of(r, v)?.as_str() {
            "" | "true" => true,
            "false" => false,
            _ => return Err(expected(r, v, "boolean")),
        },
        _ => return Err(expected(r, v, "boolean")),
    };
    Ok(svl_value::new(SVL_BOOL, svl_value_u { b }))
}

/// UTF-8 text of a string value (transient copy; used for coercion and names).
fn str_of<R: JsReader>(r: &R, v: &R::V) -> Result<String, ConvError> {
    let mut g = Vec::new();
    let sv = r.string(v, false, &mut g)?;
    let bytes = unsafe { std::slice::from_raw_parts(sv.ptr, sv.len) };
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

/// A JS number from a number, or from a numeric attribute string (`"8"`, `"1.5"`).
fn number_of<R: JsReader>(r: &R, v: &R::V, kind: JsKind, what: &str) -> Result<f64, ConvError> {
    match kind {
        JsKind::Number => Ok(r.number(v)),
        JsKind::String => {
            let s = str_of(r, v)?;
            let t = s.trim();
            let ok = !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'));
            match (ok, t.parse::<f64>()) {
                (true, Ok(n)) if n.is_finite() => Ok(n),
                _ => Err(expected(r, v, what)),
            }
        }
        _ => Err(expected(r, v, what)),
    }
}

fn range_of(slot: &SlotDesc) -> (f64, f64) {
    let min = if slot.min.is_nan() { -MAX_SAFE } else { slot.min };
    let max = if slot.max.is_nan() { MAX_SAFE } else { slot.max };
    (min, max)
}

fn encode_int<R: JsReader>(r: &R, v: &R::V, kind: JsKind, slot: &SlotDesc) -> Result<svl_value, ConvError> {
    let n = if kind == JsKind::BigInt {
        r.bigint(v)
            .ok_or_else(|| ConvError::range("bigint does not fit a 64-bit integer"))? as f64
    } else {
        number_of(r, v, kind, "integer")?
    };
    if !n.is_finite() || n.fract() != 0.0 {
        return Err(if kind == JsKind::Number {
            ConvError::range(format!("{n} is not an integer"))
        } else {
            expected(r, v, "integer")
        });
    }
    let (min, max) = range_of(slot);
    if n < min || n > max {
        return Err(ConvError::range(format!(
            "{} is out of range ({}..{})",
            n as i64, min as i64, max as i64
        )));
    }
    if n.abs() >= MAX_SAFE && kind != JsKind::BigInt {
        return Err(ConvError::range(format!("{n} exceeds the safe integer range")));
    }
    let mut out = svl_value::new(SVL_I64, svl_value_u { i: n as i64 });
    if slot.has(SVL_D_UNSIGNED) {
        out.flags |= SVL_F_UNSIGNED;
    }
    Ok(out)
}

fn encode_float<R: JsReader>(r: &R, v: &R::V, kind: JsKind, slot: &SlotDesc) -> Result<svl_value, ConvError> {
    if slot.ty == SVL_T_DATETIME {
        // A datetime slot takes a Date (or epoch milliseconds) and travels as typed F64.
        let ms = match r.date_ms(v) {
            Some(ms) => ms,
            None if kind == JsKind::Number => r.number(v),
            None => return Err(expected(r, v, "Date")),
        };
        if !ms.is_finite() {
            return Err(ConvError::range("invalid Date"));
        }
        return Ok(svl_value::typed(SVL_F64, SVL_T_DATETIME, svl_value_u { f: ms }));
    }
    let n = number_of(r, v, kind, "number")?;
    if !slot.min.is_nan() && n < slot.min || !slot.max.is_nan() && n > slot.max {
        return Err(ConvError::range(format!("{n} is out of range")));
    }
    Ok(svl_value::new(SVL_F64, svl_value_u { f: n }))
}

fn encode_str<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    slot: &SlotDesc,
    cx: &Cx,
    ar: &mut Arena,
) -> Result<svl_value, ConvError> {
    if kind != JsKind::String {
        return Err(expected(r, v, &slot_type_name(slot, cx)));
    }
    let sv = r.string(v, cx.utf16, &mut ar.guards)?;
    let mut out = svl_value::new(
        SVL_STR,
        svl_value_u {
            s: svl_str {
                ptr: sv.ptr.cast(),
                len: sv.len,
            },
        },
    );
    if sv.utf16 {
        out.flags |= SVL_F_UTF16;
    }
    if sv.nul_term {
        out.flags |= SVL_F_NUL_TERM;
    }
    Ok(out)
}

fn type_of(slot: &SlotDesc, cx: &Cx) -> Result<Rc<TypeInfo>, ConvError> {
    cx.types
        .type_info(slot.ty)
        .ok_or_else(|| ConvError::type_(format!("no descriptor for type {}", slot.ty)))
}

fn enum_member<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    ti: &TypeInfo,
) -> Result<i64, ConvError> {
    match kind {
        JsKind::String => {
            let s = str_of(r, v)?;
            ti.by_name.get(&s).copied().ok_or_else(|| {
                ConvError::type_(format!("expected one of {}, got {}", ti.names(), describe(r, v)))
            })
        }
        JsKind::Number => {
            let n = r.number(v);
            if n.fract() == 0.0 && ti.by_value.contains_key(&(n as i64)) {
                Ok(n as i64)
            } else {
                Err(ConvError::type_(format!("expected one of {}, got {}", ti.names(), describe(r, v))))
            }
        }
        _ => Err(ConvError::type_(format!("expected one of {}, got {}", ti.names(), describe(r, v)))),
    }
}

fn encode_enum<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    slot: &SlotDesc,
    cx: &Cx,
    _ar: &mut Arena,
) -> Result<svl_value, ConvError> {
    let ti = type_of(slot, cx)?;
    let value = enum_member(r, v, kind, &ti)?;
    Ok(svl_value::typed(SVL_ENUM, slot.ty, svl_value_u { i: value }))
}

fn encode_flags<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    slot: &SlotDesc,
    cx: &Cx,
    _ar: &mut Arena,
) -> Result<svl_value, ConvError> {
    let ti = type_of(slot, cx)?;
    let bad = |r: &R, v: &R::V| {
        ConvError::type_(format!(
            "expected an array of {}, got {}",
            ti.names(),
            describe(r, v)
        ))
    };
    let mut mask: u64 = 0;
    let add = |mask: &mut u64, name: &str, r: &R, v: &R::V| -> Result<(), ConvError> {
        match ti.by_name.get(name) {
            Some(bits) => {
                *mask |= *bits as u64;
                Ok(())
            }
            None => Err(bad(r, v)),
        }
    };
    match kind {
        JsKind::String => {
            // A space-separated list (attribute form) or a single name.
            for part in str_of(r, v)?.split_whitespace() {
                add(&mut mask, part, r, v)?;
            }
        }
        JsKind::Number => {
            let n = r.number(v);
            if !(n >= 0.0 && n.fract() == 0.0 && n < 4_294_967_296.0) {
                return Err(bad(r, v));
            }
            mask = n as u64;
        }
        JsKind::Array => {
            for i in 0..r.len(v)? {
                let e = r.index(v, i)?;
                match r.kind(&e) {
                    JsKind::String => add(&mut mask, &str_of(r, &e)?, r, &e)?,
                    JsKind::Number => mask |= r.number(&e) as u64,
                    _ => return Err(bad(r, &e).at(format!("[{i}]"))),
                }
            }
        }
        _ => return Err(bad(r, v)),
    }
    Ok(svl_value::typed(SVL_FLAGS, slot.ty, svl_value_u { u: mask }))
}

fn encode_record<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    slot: &SlotDesc,
    cx: &Cx,
    ar: &mut Arena,
    depth: u32,
) -> Result<svl_value, ConvError> {
    let ti = type_of(slot, cx)?;
    if kind == JsKind::String && slot.has(SVL_D_STRING_FORM) {
        let mut out = encode_str(r, v, kind, slot, cx, ar)?;
        out.type_ = slot.ty;
        return Ok(out);
    }
    if kind != JsKind::Object {
        return Err(expected(r, v, &ti.desc.name));
    }
    let mut fields = Vec::with_capacity(ti.desc.fields.len());
    for f in &ti.desc.fields {
        let name = f.name.as_deref().unwrap_or("");
        if !r.has_field(v, name)? {
            // An optional field (an app export's `Option<T>` / `#[js(default)]`) may be left out.
            if f.has(SVL_D_OPTIONAL) {
                fields.push(svl_value::null());
                continue;
            }
            return Err(ConvError::type_(format!("missing field '{name}' in {}", ti.desc.name)));
        }
        let fv = r.field(v, name)?;
        fields.push(encode_at(r, &fv, f, cx, ar, depth + 1).map_err(|e| e.at(name))?);
    }
    let l = ar.list(fields);
    Ok(svl_value::typed(SVL_RECORD, slot.ty, svl_value_u { l }))
}

fn encode_list<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    slot: &SlotDesc,
    cx: &Cx,
    ar: &mut Arena,
    depth: u32,
) -> Result<svl_value, ConvError> {
    if kind != JsKind::Array {
        return Err(expected(r, v, "array"));
    }
    // The element slot comes from the LIST type descriptor, or is dynamic.
    let elem = if slot.ty != 0 {
        cx.types
            .type_info(slot.ty)
            .and_then(|t| t.desc.fields.first().cloned())
    } else {
        None
    }
    .unwrap_or_else(|| SlotDesc::new(SVL_NULL));
    let n = r.len(v)?;
    let mut items = Vec::with_capacity(n);
    for i in 0..n {
        let e = r.index(v, i)?;
        items.push(encode_at(r, &e, &elem, cx, ar, depth + 1).map_err(|e| e.at(format!("[{i}]")))?);
    }
    let l = ar.list(items);
    Ok(svl_value::new(SVL_LIST, svl_value_u { l }))
}

fn encode_map<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    cx: &Cx,
    ar: &mut Arena,
    depth: u32,
) -> Result<svl_value, ConvError> {
    if kind != JsKind::Object {
        return Err(expected(r, v, "object"));
    }
    let any = variant_slot();
    let mut items = Vec::new();
    for k in r.keys(v)? {
        let fv = r.field(v, &k)?;
        if r.kind(&fv) == JsKind::Undefined {
            continue;
        }
        // Keys are copied: they are short and not engine-owned.
        let kb: Box<str> = k.clone().into_boxed_str();
        let key = svl_value::new(
            SVL_STR,
            svl_value_u {
                s: svl_str {
                    ptr: kb.as_ptr().cast(),
                    len: kb.len(),
                },
            },
        );
        ar.guards.push(Box::new(kb));
        items.push(key);
        items.push(encode_at(r, &fv, &any, cx, ar, depth + 1).map_err(|e| e.at(k))?);
    }
    let l = ar.list(items);
    Ok(svl_value::new(SVL_MAP, svl_value_u { l }))
}

/// The slot of an element inside a variant: anything JSON-like, plus Date, bytes and boxed values.
fn variant_slot() -> SlotDesc {
    SlotDesc::new(SVL_NULL).typed(SVL_T_VARIANT).flags(SVL_D_NULLABLE)
}

/// Conversion without a descriptor: the P-era rules (numbers, strings, booleans, null, arrays).
fn dynamic<R: JsReader>(
    r: &R,
    v: &R::V,
    kind: JsKind,
    variant: bool,
    cx: &Cx,
    ar: &mut Arena,
    depth: u32,
) -> Result<svl_value, ConvError> {
    match kind {
        JsKind::Bool => Ok(svl_value::new(SVL_BOOL, svl_value_u { b: r.boolean(v) })),
        JsKind::Number => {
            let f = r.number(v);
            if f.is_finite() && f.fract() == 0.0 && f.abs() < MAX_SAFE {
                Ok(svl_value::new(SVL_I64, svl_value_u { i: f as i64 }))
            } else {
                Ok(svl_value::new(SVL_F64, svl_value_u { f }))
            }
        }
        JsKind::BigInt => match r.bigint(v) {
            Some(i) => Ok(svl_value::new(SVL_I64, svl_value_u { i })),
            None => Err(ConvError::range("bigint does not fit a 64-bit integer")),
        },
        JsKind::String => encode_str(r, v, kind, &SlotDesc::new(SVL_STR), cx, ar),
        JsKind::Array => {
            // Inside a variant the elements are variants too (objects become MAPs).
            let any = if variant { variant_slot() } else { SlotDesc::new(SVL_NULL) };
            let n = r.len(v)?;
            let mut items = Vec::with_capacity(n);
            for i in 0..n {
                let e = r.index(v, i)?;
                items.push(encode_at(r, &e, &any, cx, ar, depth + 1).map_err(|e| e.at(format!("[{i}]")))?);
            }
            let l = ar.list(items);
            Ok(svl_value::new(SVL_LIST, svl_value_u { l }))
        }
        JsKind::Object if r.typed_variant(v)?.is_some() => {
            let (sig, inner) = r.typed_variant(v)?.expect("checked above");
            let sb: Box<str> = sig.into_boxed_str();
            let sv = svl_value::new(SVL_STR, svl_value_u { s: svl_str { ptr: sb.as_ptr().cast(), len: sb.len() } });
            ar.guards.push(Box::new(sb));
            let val = encode_at(r, &inner, &variant_slot(), cx, ar, depth + 1).map_err(|e| e.at("value".to_string()))?;
            let l = ar.list(vec![sv, val]);
            Ok(svl_value::typed(SVL_RECORD, SVL_T_TYPED_VARIANT, svl_value_u { l }))
        }
        JsKind::Object if r.date_ms(v).is_some() => Ok(svl_value::typed(
            SVL_F64,
            SVL_T_DATETIME,
            svl_value_u {
                f: r.date_ms(v).unwrap_or(f64::NAN),
            },
        )),
        JsKind::Object if r.is_bytes(v) => {
            let (ptr, len) = r.bytes(v, &mut ar.guards)?;
            Ok(svl_value::new(
                SVL_BYTES,
                svl_value_u {
                    bytes: svl_bytes { ptr, len },
                },
            ))
        }
        JsKind::Object => match r.boxed(v)? {
            Some((h, ty)) => Ok(svl_value::typed(SVL_BOXED, ty, svl_value_u { h })),
            None if r.node_handle(v)?.is_some() => match r.node_handle(v)? {
                Some(Some(n)) if n >= 1.0 && n.fract() == 0.0 && n < MAX_SAFE => Ok(svl_value::new(SVL_HANDLE, svl_value_u { h: n as u64 })),
                _ => Err(ConvError::type_("node has no live native object (not created yet, or already dead)".to_string())),
            },
            // A plain object in a variant slot is a string-keyed map.
            None if variant => encode_map(r, v, kind, cx, ar, depth),
            None => Err(ConvError::type_(
                "unsupported value type 'object' in host call (no descriptor for this slot)",
            )),
        },
        other => Err(ConvError::type_(format!("unsupported value type {other:?}"))),
    }
}

// ---- outbound ----

fn js_int<W: JsWriter>(w: &W, i: i64) -> Result<W::V, String> {
    if (i as f64).abs() >= MAX_SAFE {
        return Err(format!("RangeError: value {i} exceeds the safe integer range"));
    }
    w.number(i as f64)
}

/// Builds the JS value for a backend result.
pub(crate) fn decode<W: JsWriter>(w: &W, v: &Value, types: &dyn TypeSource) -> Result<W::V, String> {
    decode_at(w, v, types, 0)
}

fn decode_at<W: JsWriter>(w: &W, v: &Value, types: &dyn TypeSource, depth: u32) -> Result<W::V, String> {
    if depth > MAX_DEPTH {
        return Err("value nested too deeply".into());
    }
    match v {
        Value::Null => w.null(),
        Value::Bool(b) => w.boolean(*b),
        Value::I64(i) => js_int(w, *i),
        Value::U64(u) => {
            if *u as f64 >= MAX_SAFE {
                Err(format!("RangeError: value {u} exceeds the safe integer range"))
            } else {
                w.number(*u as f64)
            }
        }
        Value::F64(f) => w.number(*f),
        Value::DateTime(ms) => w.date(*ms),
        Value::Str(s) => w.string(s),
        Value::Str16(u) => w.string16(u),
        Value::Handle(h) => w.handle(*h),
        Value::List(items) => {
            let vs = items
                .iter()
                .map(|x| decode_at(w, x, types, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            w.array(vs)
        }
        Value::Enum { ty, value } => match types.type_info(*ty) {
            Some(ti) => match ti.by_value.get(value) {
                Some(i) => w.string(&ti.desc.members[*i].name),
                None => js_int(w, *value),
            },
            None => js_int(w, *value),
        },
        Value::Flags { ty, mask } => {
            let mut out = Vec::new();
            let mut rest = *mask;
            if let Some(ti) = types.type_info(*ty) {
                for m in &ti.desc.members {
                    let bits = m.value as u64;
                    // Multi-bit aliases only count when fully set; zero-valued members never do.
                    if bits != 0 && rest & bits == bits {
                        out.push(w.string(&m.name)?);
                        rest &= !bits;
                    }
                }
            }
            if rest != 0 || (out.is_empty() && *mask != 0) {
                out.push(w.number(rest as f64)?);
            }
            w.array(out)
        }
        Value::Record { ty, fields } => {
            let ti = types.type_info(*ty);
            match ti {
                Some(ti) if ti.desc.fields.len() == fields.len() => {
                    let mut entries = Vec::with_capacity(fields.len());
                    for (f, x) in ti.desc.fields.iter().zip(fields) {
                        entries.push((f.name.clone().unwrap_or_default(), decode_at(w, x, types, depth + 1)?));
                    }
                    w.object(entries)
                }
                _ => {
                    let vs = fields
                        .iter()
                        .map(|x| decode_at(w, x, types, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?;
                    w.array(vs)
                }
            }
        }
        Value::Bytes(b) => w.bytes(b),
        Value::Buffer(b) => w.buffer(b),
        Value::Boxed { ty, handle } => {
            let name = types.type_info(*ty).map(|t| t.desc.name.clone()).unwrap_or_default();
            w.boxed(*handle, *ty, &name)
        }
        Value::Callback { .. } => w.null(),
        Value::Map(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for (k, x) in entries {
                out.push((k.clone(), decode_at(w, x, types, depth + 1)?));
            }
            w.object(out)
        }
        Value::Error(e) => {
            let code = decode_at(w, &e.code, types, depth + 1)?;
            w.native_error(&e.domain, code, &e.message)
        }
    }
}

/// Like [`decode`], but shaped by the slot the value was declared with: `SVL_D_BIGINT` integers become
/// `bigint`s, also inside records and lists (app exports, whose results are typed by their member).
pub(crate) fn decode_slot<W: JsWriter>(w: &W, v: &Value, slot: &SlotDesc, types: &dyn TypeSource) -> Result<W::V, String> {
    decode_slot_at(w, v, slot, types, 0)
}

fn decode_slot_at<W: JsWriter>(w: &W, v: &Value, slot: &SlotDesc, types: &dyn TypeSource, depth: u32) -> Result<W::V, String> {
    if depth > MAX_DEPTH {
        return Err("value nested too deeply".into());
    }
    if slot.has(SVL_D_BIGINT) {
        match v {
            Value::I64(i) => return w.bigint(*i),
            Value::U64(u) => return w.bigint_u(*u),
            _ => {}
        }
    }
    match (v, slot.tag) {
        (Value::Record { ty, fields }, SVL_RECORD) => match types.type_info(*ty) {
            Some(ti) if ti.desc.fields.len() == fields.len() => {
                let mut entries = Vec::with_capacity(fields.len());
                for (f, x) in ti.desc.fields.iter().zip(fields) {
                    entries.push((f.name.clone().unwrap_or_default(), decode_slot_at(w, x, f, types, depth + 1)?));
                }
                w.object(entries)
            }
            _ => decode_at(w, v, types, depth),
        },
        (Value::List(items), SVL_LIST) if slot.ty != 0 => {
            let elem = types.type_info(slot.ty).and_then(|t| t.desc.fields.first().cloned()).unwrap_or_else(|| SlotDesc::new(SVL_NULL));
            let vs = items.iter().map(|x| decode_slot_at(w, x, &elem, types, depth + 1)).collect::<Result<Vec<_>, _>>()?;
            w.array(vs)
        }
        _ => decode_at(w, v, types, depth),
    }
}

// ---- E2: owned values with buffers ----

/// Copies an encoded raw value into an owned [`Value`], putting committed buffers where their staged
/// slots are (`encode` left a staging index there).
pub(crate) fn owned(raw: &svl_value, bufs: &mut [Option<BufferValue>]) -> Value {
    // SAFETY (all arms): `raw` was built by `encode` and its arena is still alive.
    unsafe {
        match raw.tag {
            SVL_BYTES if buffers::spec(raw.type_).is_some() => {
                let i = raw.u.bytes.len;
                bufs.get_mut(i).and_then(Option::take).map_or(Value::Null, Value::Buffer)
            }
            SVL_LIST => Value::List(owned_list(raw.u.l, bufs)),
            SVL_RECORD => Value::Record { ty: raw.type_, fields: owned_list(raw.u.l, bufs) },
            _ => Value::from_raw(raw),
        }
    }
}

unsafe fn owned_list(l: svl_list, bufs: &mut [Option<BufferValue>]) -> Vec<Value> {
    if l.ptr.is_null() || l.len == 0 {
        return Vec::new();
    }
    // SAFETY: the list was built by `encode` (arena alive).
    unsafe { std::slice::from_raw_parts(l.ptr, l.len) }.iter().map(|x| owned(x, bufs)).collect()
}

/// Commits the arrays staged while `raws` were encoded and copies `raws` into owned values.
pub(crate) fn owned_all<R: JsReader>(r: &R, raws: &[svl_value]) -> Result<Vec<Value>, String> {
    let mut bufs: Vec<Option<BufferValue>> = r.commit_buffers()?.into_iter().map(Some).collect();
    Ok(raws.iter().map(|x| owned(x, &mut bufs)).collect())
}
