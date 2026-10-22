//! RA2: serde values crossing as structured JS values ([`Json<T>`]), typed from `schemars`.
//!
//! A `Json<T>` crosses as a plain JS value (objects, arrays, numbers, strings, booleans, `null`), with no
//! JSON text in between: the host converts the JS value structurally (a variant slot), and serde reads
//! the owned [`Value`] tree directly. serde's errors keep their kind: a wrong type is `type`, an integer
//! out of range `range`, a missing field `missing-field`, an unknown tag or variant `unknown-variant`
//! (`NativeError { domain: 'svelters' }`), and the message starts with the path (`at shape.r: ...`).
//! The path goes through serde's internally tagged, adjacently tagged, untagged and flattened types
//! too, which serde reads from a buffer (SV48, [`locate`]).
//!
//! The typings come from `T`'s [`JsonSchema`]: named schemas (`$defs`) become exported TS types of the
//! module that uses them, and the JSON Schema itself goes into `exports.json`.

use super::*;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::{self, DeserializeOwned, DeserializeSeed, IntoDeserializer, Visitor};
use serde_json::Value as J;
use std::collections::BTreeSet;
use std::fmt;

mod locate;

/// A serde value crossing as a structured JS value, typed from its [`JsonSchema`].
///
/// For types that cannot derive svelters' own `Record`/`Enum` (a headless crate, or the orphan rule).
/// Errors reading a JS value are `NativeError { domain: 'svelters', code }` with `code` one of `type`,
/// `range`, `missing-field`, `unknown-variant`, `unknown-field`, `invalid` (a serde `custom` error).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Json<T>(pub T);

/// [`Json<T>`] for a serde type without a [`JsonSchema`]: typed `unknown`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JsonAny<T>(pub T);

macro_rules! wrapper {
    ($($w:ident),*) => {$(
        impl<T> $w<T> {
            pub fn into_inner(self) -> T {
                self.0
            }
        }
        impl<T> From<T> for $w<T> {
            fn from(v: T) -> Self {
                $w(v)
            }
        }
        impl<T> std::ops::Deref for $w<T> {
            type Target = T;
            fn deref(&self) -> &T {
                &self.0
            }
        }
        impl<T> std::ops::DerefMut for $w<T> {
            fn deref_mut(&mut self) -> &mut T {
                &mut self.0
            }
        }
    )*};
}
wrapper!(Json, JsonAny);

/// The slot of a JSON value: anything JSON-like, converted structurally (objects become maps).
pub(crate) fn json_slot() -> SlotDesc {
    SlotDesc::new(SVL_NULL).typed(SVL_T_VARIANT).flags(SVL_D_NULLABLE)
}

impl<T: Serialize + DeserializeOwned + JsonSchema> JsType for Json<T> {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        b.json_schema::<T>()
    }
}
impl<T: Serialize + DeserializeOwned + JsonSchema> FromJs for Json<T> {
    fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
        from_value_schema(v).map(Json)
    }
}
impl<T: Serialize + DeserializeOwned + JsonSchema> IntoJs for Json<T> {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        to_value(&self.0)
    }
}

impl<T: Serialize + DeserializeOwned> JsType for JsonAny<T> {
    fn describe(_: &mut TypeBuilder) -> TypeRef {
        TypeRef::new(json_slot(), "unknown")
    }
}
impl<T: Serialize + DeserializeOwned> FromJs for JsonAny<T> {
    fn from_js(v: Value, _: &CallCx<'_>) -> Result<Self, NativeError> {
        from_value(v).map(JsonAny)
    }
}
impl<T: Serialize + DeserializeOwned> IntoJs for JsonAny<T> {
    fn into_js(self, _: &CallCx<'_>) -> Result<Value, NativeError> {
        to_value(&self.0)
    }
}

// ---- value errors with a path ----

/// The `NativeError` codes of a value that does not fit its Rust type (domain `svelters`).
const VALUE_CODES: &[&str] = &["type", "range", "missing-field", "unknown-variant", "unknown-field", "invalid"];

/// Prefixes the path of a value error with `seg` (a field name, or `[i]`): `at a.b[2]: ...`.
pub(crate) fn at(mut e: NativeError, seg: &str) -> NativeError {
    let value = e.domain == "svelters" && matches!(&e.code, Value::Str(c) if VALUE_CODES.contains(&c.as_str()));
    if value {
        e.message = prefix_path(&e.message, seg);
    }
    e
}

fn prefix_path(msg: &str, seg: &str) -> String {
    if let Some(rest) = msg.strip_prefix("at ") {
        if let Some(i) = rest.find(": ") {
            let (p, tail) = rest.split_at(i);
            let join = if p.starts_with('[') { "" } else { "." };
            return format!("at {seg}{join}{p}{tail}");
        }
    }
    format!("at {seg}: {msg}")
}

/// A serde error reading a [`Value`], with its kind and path.
#[derive(Debug)]
pub struct DeError {
    code: &'static str,
    /// Outermost segment last.
    path: Vec<String>,
    msg: String,
    /// SV48: what the error is about, to find it in the value when serde lost the path.
    hint: locate::Hint,
    /// SV48: the error is about the value at `path` itself. Otherwise serde raised it from content it
    /// buffered (internally tagged, adjacently tagged, untagged and flattened types), somewhere at or
    /// below `path`, and [`locate`] finds where.
    exact: bool,
}

impl DeError {
    fn new(code: &'static str, msg: String, hint: locate::Hint) -> DeError {
        DeError { code, path: Vec::new(), msg, hint, exact: frames::exact() }
    }
    fn at(mut self, seg: String) -> DeError {
        self.path.push(seg);
        self
    }
    fn into_native(self) -> NativeError {
        let mut path = String::new();
        for s in self.path.iter().rev() {
            if !path.is_empty() && !s.starts_with('[') {
                path.push('.');
            }
            path.push_str(s);
        }
        let msg = if path.is_empty() { self.msg } else { format!("at {path}: {}", self.msg) };
        host_error(self.code, msg)
    }
}

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for DeError {}

fn one_of(names: &[&str]) -> String {
    match names {
        [] => "nothing".to_string(),
        [one] => format!("`{one}`"),
        _ => format!("one of {}", names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")),
    }
}

impl de::Error for DeError {
    fn custom<M: fmt::Display>(msg: M) -> Self {
        DeError::new("invalid", msg.to_string(), locate::Hint::Custom)
    }
    fn invalid_type(unexp: de::Unexpected<'_>, exp: &dyn de::Expected) -> Self {
        DeError::new("type", format!("invalid type: {unexp}, expected {exp}"), locate::Hint::Unexpected(unexp.to_string()))
    }
    fn invalid_value(unexp: de::Unexpected<'_>, exp: &dyn de::Expected) -> Self {
        // serde's integer visitors report a number that does not fit this way.
        let code = match unexp {
            de::Unexpected::Signed(_) | de::Unexpected::Unsigned(_) | de::Unexpected::Float(_) => "range",
            _ => "invalid",
        };
        DeError::new(code, format!("invalid value: {unexp}, expected {exp}"), locate::Hint::Unexpected(unexp.to_string()))
    }
    fn invalid_length(len: usize, exp: &dyn de::Expected) -> Self {
        DeError::new("type", format!("invalid length {len}, expected {exp}"), locate::Hint::Length(len))
    }
    fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self {
        DeError::new(
            "unknown-variant",
            format!("unknown variant `{variant}`, expected {}", one_of(expected)),
            locate::Hint::UnknownVariant(variant.to_string()),
        )
    }
    fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
        DeError::new("unknown-field", format!("unknown field `{field}`, expected {}", one_of(expected)), locate::Hint::UnknownField(field.to_string()))
    }
    fn missing_field(field: &'static str) -> Self {
        DeError::new("missing-field", format!("missing field `{field}`"), locate::Hint::MissingField(field.to_string()))
    }
    fn duplicate_field(field: &'static str) -> Self {
        DeError::new("type", format!("duplicate field `{field}`"), locate::Hint::None)
    }
}

// ---- where serde's errors come from (SV48) ----

/// serde reads internally tagged, adjacently tagged, untagged and flattened types from content it
/// buffered first (its private `Content`), so their errors do not pass through this deserializer and
/// lose their path. To tell them apart, each value the deserializer hands to serde is a frame: an
/// error made while the deserializer is visiting the frame's value (and, for an object, before its
/// entries ran out) is about that value; any other error (made after serde buffered the value) is
/// somewhere inside it.
mod frames {
    use std::cell::RefCell;

    #[derive(Default)]
    struct Frame {
        /// Deserializer calls on this value in progress.
        visiting: u32,
        /// The value is an object whose entries serde has read to the end: what serde reports now
        /// (a missing field, or an error in a flattened field) came from the buffered entries.
        drained: bool,
    }

    thread_local! {
        static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    }

    /// Whether an error made now is about the innermost frame's value itself.
    pub(super) fn exact() -> bool {
        FRAMES.with(|f| f.borrow().last().map_or(true, |fr| fr.visiting > 0 && !fr.drained))
    }

    /// A value handed to serde (the root, a list element, an object value, a variant's content).
    pub(super) struct Value;

    impl Value {
        pub(super) fn enter() -> Value {
            FRAMES.with(|f| f.borrow_mut().push(Frame::default()));
            Value
        }
    }

    impl Drop for Value {
        fn drop(&mut self) {
            FRAMES.with(|f| f.borrow_mut().pop());
        }
    }

    /// A deserializer call on the innermost frame's value.
    pub(super) struct Visit;

    impl Visit {
        pub(super) fn enter() -> Visit {
            FRAMES.with(|f| {
                if let Some(fr) = f.borrow_mut().last_mut() {
                    fr.visiting += 1;
                }
            });
            Visit
        }
    }

    impl Drop for Visit {
        fn drop(&mut self) {
            FRAMES.with(|f| {
                if let Some(fr) = f.borrow_mut().last_mut() {
                    fr.visiting = fr.visiting.saturating_sub(1);
                }
            });
        }
    }

    pub(super) fn drained() {
        FRAMES.with(|f| {
            if let Some(fr) = f.borrow_mut().last_mut() {
                fr.drained = true;
            }
        });
    }
}

/// Hands `x` to serde as a frame of its own.
fn child<'de, S: DeserializeSeed<'de>>(seed: S, x: &Value) -> Result<S::Value, DeError> {
    let _frame = frames::Value::enter();
    seed.deserialize(De(x))
}

// ---- Value -> T ----

/// Reads a `T` from a structurally converted JS value.
///
/// An error inside serde's buffered content (an internally tagged enum, for one) is placed from the
/// value alone: the first value below the buffered one that serde would show the same way. A missing
/// field cannot be placed so; [`from_value_schema`] places every kind.
pub fn from_value<T: DeserializeOwned>(v: Value) -> Result<T, NativeError> {
    child(std::marker::PhantomData::<T>, &v).map_err(|e| locate::locate(&v, None, e).into_native())
}

/// [`from_value`] for a type with a [`JsonSchema`]: an error inside serde's buffered content gets its
/// full path (`at body.extent.depth: ...`), found in the value with the schema's help.
pub fn from_value_schema<T: DeserializeOwned + JsonSchema>(v: Value) -> Result<T, NativeError> {
    child(std::marker::PhantomData::<T>, &v).map_err(|e| {
        // The common case, a wrong value the deserializer saw itself, needs no schema.
        if locate::settled(&v, &e) {
            return e.into_native();
        }
        let schema = schemars::SchemaGenerator::new(schemars::generate::SchemaSettings::draft2020_12()).into_root_schema_for::<T>();
        locate::locate(&v, Some(schema.as_value()), e).into_native()
    })
}

fn what(v: &Value) -> &'static str {
    match v {
        Value::Handle(_) => "an object handle",
        Value::Enum { .. } | Value::Flags { .. } => "an enum value",
        Value::Record { .. } => "a record",
        Value::Boxed { .. } => "a boxed value",
        Value::Callback { .. } => "a function",
        Value::Error(_) => "an error",
        Value::Buffer(_) => "a typed array",
        _ => "this value",
    }
}

struct De<'a>(&'a Value);

impl<'de> de::Deserializer<'de> for De<'_> {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, DeError> {
        let _visit = frames::Visit::enter();
        match self.0 {
            Value::Null => v.visit_unit(),
            Value::Bool(b) => v.visit_bool(*b),
            Value::I64(i) => v.visit_i64(*i),
            Value::U64(u) => v.visit_u64(*u),
            Value::F64(f) | Value::DateTime(f) => v.visit_f64(*f),
            Value::Str(s) => v.visit_str(s),
            Value::Str16(u) => v.visit_string(String::from_utf16_lossy(u)),
            Value::Bytes(b) => v.visit_bytes(b),
            Value::List(items) => {
                let n = items.len();
                let mut seq = Seq { it: items.iter(), i: 0 };
                let out = v.visit_seq(&mut seq)?;
                if seq.it.len() != 0 {
                    return Err(de::Error::invalid_length(n, &"fewer elements"));
                }
                Ok(out)
            }
            Value::Map(entries) => v.visit_map(Map { it: entries.iter(), value: None }),
            other => Err(DeError::new("type", format!("{} cannot be read as a serde value", what(other)), locate::Hint::None)),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, DeError> {
        let _visit = frames::Visit::enter();
        match self.0 {
            Value::Null => v.visit_none(),
            other => v.visit_some(De(other)),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _name: &'static str, v: V) -> Result<V::Value, DeError> {
        let _visit = frames::Visit::enter();
        v.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(self, _name: &'static str, _variants: &'static [&'static str], v: V) -> Result<V::Value, DeError> {
        let _visit = frames::Visit::enter();
        // Externally tagged: `"unit"` or `{ "variant": value }`.
        match self.0 {
            Value::Str(s) => v.visit_enum(s.as_str().into_deserializer()),
            Value::Map(m) if m.len() == 1 => {
                let (variant, value) = &m[0];
                v.visit_enum(Enum { variant, value })
            }
            other => Err(de::Error::invalid_type(unexpected(other), &"a variant name or an object with one key")),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf unit unit_struct
        seq tuple tuple_struct map struct identifier ignored_any
    }
}

fn unexpected(v: &Value) -> de::Unexpected<'_> {
    match v {
        Value::Null => de::Unexpected::Unit,
        Value::Bool(b) => de::Unexpected::Bool(*b),
        Value::I64(i) => de::Unexpected::Signed(*i),
        Value::U64(u) => de::Unexpected::Unsigned(*u),
        Value::F64(f) => de::Unexpected::Float(*f),
        Value::Str(s) => de::Unexpected::Str(s),
        Value::List(_) => de::Unexpected::Seq,
        Value::Map(_) => de::Unexpected::Map,
        _ => de::Unexpected::Other(what(v)),
    }
}

struct Seq<'a> {
    it: std::slice::Iter<'a, Value>,
    i: usize,
}

impl<'de> de::SeqAccess<'de> for Seq<'_> {
    type Error = DeError;
    fn next_element_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<Option<S::Value>, DeError> {
        match self.it.next() {
            Some(x) => {
                let i = self.i;
                self.i += 1;
                child(seed, x).map(Some).map_err(|e| e.at(format!("[{i}]")))
            }
            None => Ok(None),
        }
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.it.len())
    }
}

struct Map<'a> {
    it: std::slice::Iter<'a, (String, Value)>,
    value: Option<&'a (String, Value)>,
}

impl<'de> de::MapAccess<'de> for Map<'_> {
    type Error = DeError;
    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, DeError> {
        match self.it.next() {
            Some(entry) => {
                let k = &entry.0;
                let key = seed.deserialize(de::value::StrDeserializer::<DeError>::new(k)).map_err(|e| e.at(k.clone()))?;
                self.value = Some(entry);
                Ok(Some(key))
            }
            None => {
                frames::drained();
                Ok(None)
            }
        }
    }
    fn next_value_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<S::Value, DeError> {
        let (k, v) = self.value.take().ok_or_else(|| DeError::new("invalid", "a map value without its key".into(), locate::Hint::None))?;
        child(seed, v).map_err(|e| e.at(k.clone()))
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.it.len())
    }
}

struct Enum<'a> {
    variant: &'a str,
    value: &'a Value,
}

impl<'de, 'a> de::EnumAccess<'de> for Enum<'a> {
    type Error = DeError;
    type Variant = Variant<'a>;
    fn variant_seed<S: DeserializeSeed<'de>>(self, seed: S) -> Result<(S::Value, Variant<'a>), DeError> {
        let v = seed.deserialize(de::value::StrDeserializer::<DeError>::new(self.variant))?;
        Ok((v, Variant { name: self.variant, value: self.value }))
    }
}

struct Variant<'a> {
    name: &'a str,
    value: &'a Value,
}

impl<'de> de::VariantAccess<'de> for Variant<'_> {
    type Error = DeError;
    fn unit_variant(self) -> Result<(), DeError> {
        match self.value {
            Value::Null => Ok(()),
            other => Err(DeError::invalid_type_at(other, "a unit variant", self.name.to_string())),
        }
    }
    fn newtype_variant_seed<S: DeserializeSeed<'de>>(self, seed: S) -> Result<S::Value, DeError> {
        child(seed, self.value).map_err(|e| e.at(self.name.to_string()))
    }
    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, v: V) -> Result<V::Value, DeError> {
        let _frame = frames::Value::enter();
        de::Deserializer::deserialize_seq(De(self.value), v).map_err(|e| e.at(self.name.to_string()))
    }
    fn struct_variant<V: Visitor<'de>>(self, _fields: &'static [&'static str], v: V) -> Result<V::Value, DeError> {
        let _frame = frames::Value::enter();
        de::Deserializer::deserialize_map(De(self.value), v).map_err(|e| e.at(self.name.to_string()))
    }
}

impl DeError {
    fn invalid_type_at(v: &Value, exp: &str, seg: String) -> DeError {
        DeError::new("type", format!("invalid type: {}, expected {exp}", unexpected(v)), locate::Hint::Unexpected(unexpected(v).to_string())).at(seg)
    }
}

// ---- T -> Value ----

/// Writes a `T` as a structured JS value.
pub fn to_value<T: Serialize + ?Sized>(v: &T) -> Result<Value, NativeError> {
    let j = serde_json::to_value(v).map_err(|e| host_error("type", format!("cannot convert to a JS value: {e}")))?;
    Ok(from_json(j))
}

/// A `serde_json::Value` as an owned [`Value`] (objects become maps).
pub fn from_json(j: J) -> Value {
    match j {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(b),
        J::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => Value::I64(i),
            (None, Some(u)) => Value::U64(u),
            _ => Value::F64(n.as_f64().unwrap_or(f64::NAN)),
        },
        J::String(s) => Value::Str(s),
        J::Array(items) => Value::List(items.into_iter().map(from_json).collect()),
        J::Object(m) => Value::Map(m.into_iter().map(|(k, v)| (k, from_json(v))).collect()),
    }
}

// ---- JSON Schema -> TypeScript ----

/// A schema name as a TS identifier.
pub(crate) fn ts_ident(name: &str) -> String {
    let mut s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' }).collect();
    if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert(0, '_');
    }
    s
}

/// The TS name of JSON definition `name`. A definition named like one of svelters' own types (a
/// `Record` and a `JsonSchema` struct both called `Point`) is declared as `PointJson`: `reserved`
/// holds those names.
pub(crate) fn json_ident(name: &str, reserved: &BTreeSet<String>) -> String {
    let id = ts_ident(name);
    if !reserved.contains(&id) {
        return id;
    }
    let mut n = format!("{id}Json");
    let mut i = 2;
    while reserved.contains(&n) {
        n = format!("{id}Json{i}");
        i += 1;
    }
    n
}

/// The JSON definitions a TS type refers to, collected while it is written.
pub(crate) struct Refs<'a> {
    pub(crate) names: Vec<String>,
    reserved: &'a BTreeSet<String>,
}

impl<'a> Refs<'a> {
    pub(crate) fn new(reserved: &'a BTreeSet<String>) -> Refs<'a> {
        Refs { names: Vec::new(), reserved }
    }
}

/// The definition name a `$ref` points at (`#/$defs/Name`, JSON-pointer escaped).
pub(crate) fn ref_name(r: &str) -> String {
    let last = r.rsplit('/').next().unwrap_or(r);
    last.replace("~1", "/").replace("~0", "~")
}

fn literal(v: &J) -> String {
    match v {
        J::String(s) => {
            let q = serde_json::to_string(s).unwrap_or_default();
            // Single quotes, like the rest of the typings.
            format!("'{}'", q[1..q.len() - 1].replace("\\\"", "\"").replace('\'', "\\'"))
        }
        J::Null | J::Bool(_) | J::Number(_) => v.to_string(),
        // An object or array constant: typed by its shape only.
        J::Array(_) => "unknown[]".into(),
        J::Object(_) => "Record<string, unknown>".into(),
    }
}

/// `s` as a member of a union, an intersection or an array element.
fn paren(s: String) -> String {
    let mut depth = 0i32;
    let b = s.as_bytes();
    for (i, c) in s.char_indices() {
        match c {
            '{' | '(' | '[' | '<' => depth += 1,
            '}' | ')' | ']' | '>' => depth -= 1,
            '|' | '&' if depth == 0 && i > 0 && b[i - 1] == b' ' => return format!("({s})"),
            _ => {}
        }
    }
    s
}

fn union(parts: Vec<String>) -> String {
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    // `null` last, like the rest of the typings (`T | null`).
    if let Some(i) = out.iter().position(|p| p == "null") {
        let n = out.remove(i);
        out.push(n);
    }
    match out.len() {
        0 => "never".into(),
        1 => out.pop().unwrap_or_default(),
        _ => out.into_iter().map(paren).collect::<Vec<_>>().join(" | "),
    }
}

/// The TypeScript type of a JSON Schema (draft 2020-12, as schemars writes it). `refs` collects the
/// definitions it refers to.
pub(crate) fn schema_ts(s: &J, refs: &mut Refs<'_>) -> String {
    let o = match s {
        J::Bool(false) => return "never".into(),
        J::Object(o) => o,
        _ => return "unknown".into(),
    };
    if let Some(c) = o.get("const") {
        return literal(c);
    }
    if let Some(J::Array(vs)) = o.get("enum") {
        return union(vs.iter().map(literal).collect());
    }
    let mut parts = Vec::new();
    // In draft 2020-12 a `$ref` sits next to other keywords (schemars: a newtype variant of an
    // internally tagged enum is the tag's `properties` plus a `$ref` to the inner struct).
    if let Some(r) = o.get("$ref").and_then(J::as_str) {
        let name = ref_name(r);
        parts.push(json_ident(&name, refs.reserved));
        if !refs.names.contains(&name) {
            refs.names.push(name);
        }
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(J::Array(xs)) = o.get(key) {
            parts.push(union(xs.iter().map(|x| schema_ts(x, refs)).collect()));
        }
    }
    if let Some(J::Array(xs)) = o.get("allOf") {
        parts.extend(xs.iter().map(|x| schema_ts(x, refs)));
    }
    if let Some(own) = own_ts(o, refs) {
        parts.push(own);
    }
    match parts.len() {
        0 => "unknown".into(),
        1 => parts.pop().unwrap_or_default(),
        _ => parts.into_iter().map(paren).collect::<Vec<_>>().join(" & "),
    }
}

/// The type a schema's own keywords (`type`, `properties`, `items`) give, if any.
fn own_ts(o: &serde_json::Map<String, J>, refs: &mut Refs<'_>) -> Option<String> {
    let types: Vec<&str> = match o.get("type") {
        Some(J::String(t)) => vec![t.as_str()],
        Some(J::Array(ts)) => ts.iter().filter_map(J::as_str).collect(),
        _ if o.contains_key("properties") || o.contains_key("additionalProperties") => vec!["object"],
        _ if o.contains_key("items") || o.contains_key("prefixItems") => vec!["array"],
        _ => return None,
    };
    Some(union(
        types
            .into_iter()
            .map(|t| match t {
                "string" => "string".to_string(),
                "number" | "integer" => "number".to_string(),
                "boolean" => "boolean".to_string(),
                "null" => "null".to_string(),
                "array" => array_ts(o, refs),
                "object" => object_ts(o, refs),
                _ => "unknown".to_string(),
            })
            .collect(),
    ))
}

fn array_ts(o: &serde_json::Map<String, J>, refs: &mut Refs<'_>) -> String {
    if let Some(J::Array(prefix)) = o.get("prefixItems") {
        let mut items: Vec<String> = prefix.iter().map(|x| schema_ts(x, refs)).collect();
        match o.get("items") {
            None | Some(J::Bool(false)) => {}
            Some(rest) => items.push(format!("...{}[]", paren(schema_ts(rest, refs)))),
        }
        return format!("[{}]", items.join(", "));
    }
    match o.get("items") {
        Some(x) => format!("{}[]", paren(schema_ts(x, refs))),
        None => "unknown[]".into(),
    }
}

fn object_ts(o: &serde_json::Map<String, J>, refs: &mut Refs<'_>) -> String {
    let required: Vec<&str> = o.get("required").and_then(J::as_array).map(|r| r.iter().filter_map(J::as_str).collect()).unwrap_or_default();
    let extra = match o.get("additionalProperties") {
        None | Some(J::Bool(false)) => None,
        Some(x) => Some(schema_ts(x, refs)),
    };
    let props = o.get("properties").and_then(J::as_object);
    let Some(props) = props.filter(|p| !p.is_empty()) else {
        return match extra {
            Some(t) => format!("Record<string, {t}>"),
            None if o.get("additionalProperties") == Some(&J::Bool(false)) => "Record<string, never>".into(),
            None => "Record<string, unknown>".into(),
        };
    };
    let mut s = String::from("{");
    for (k, v) in props {
        let doc = v.get("description").and_then(J::as_str).map(|d| d.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|d| !d.is_empty());
        if let Some(d) = doc {
            s.push_str(&format!(" /** {} */", d.replace("*/", "*\\/")));
        }
        let opt = if required.contains(&k.as_str()) { "" } else { "?" };
        s.push_str(&format!(" {}{opt}: {};", prop_key(k), schema_ts(v, refs)));
    }
    if extra.is_some() {
        // Every declared property must fit an index signature: `unknown` does.
        s.push_str(" [key: string]: unknown;");
    }
    s.push_str(" }");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ts(v: J) -> (String, Vec<String>) {
        let reserved = BTreeSet::new();
        let mut refs = Refs::new(&reserved);
        (schema_ts(&v, &mut refs), refs.names)
    }

    #[test]
    fn schema_to_typescript() {
        assert_eq!(ts(json!(true)).0, "unknown");
        assert_eq!(ts(json!({"type": ["string", "null"]})).0, "string | null");
        assert_eq!(ts(json!({"type": "integer", "format": "uint8"})).0, "number");
        assert_eq!(ts(json!({"type": "array", "items": {"type": ["number", "null"]}})).0, "(number | null)[]");
        assert_eq!(ts(json!({"type": "array", "prefixItems": [{"type": "string"}, {"type": "number"}], "items": false})).0, "[string, number]");
        assert_eq!(ts(json!({"type": "object", "additionalProperties": {"type": "number"}})).0, "Record<string, number>");
        assert_eq!(ts(json!({"enum": ["a", "b'c"]})).0, "'a' | 'b\\'c'");
        let (t, refs) = ts(json!({"anyOf": [{"$ref": "#/$defs/Foo"}, {"type": "null"}]}));
        assert_eq!((t.as_str(), refs), ("Foo | null", vec!["Foo".to_string()]));
        assert_eq!(
            ts(json!({"type": "object", "properties": {"kind": {"type": "string", "const": "x"}}, "required": ["kind"], "$ref": "#/$defs/Foo"})).0,
            "Foo & { kind: 'x'; }"
        );
        assert_eq!(
            ts(json!({"type": "object", "properties": {"a": {"type": "string", "description": "The a."}, "b-c": {"type": "boolean"}}, "required": ["a"]})).0,
            "{ /** The a. */ a: string; \"b-c\"?: boolean; }"
        );
        assert_eq!(
            ts(json!({"oneOf": [
                {"type": "object", "properties": {"kind": {"type": "string", "const": "a"}}, "required": ["kind"]},
                {"type": "object", "properties": {"kind": {"type": "string", "const": "b"}, "n": {"type": "integer"}}, "required": ["kind", "n"]}
            ]}))
            .0,
            "{ kind: 'a'; } | { kind: 'b'; n: number; }"
        );
    }

    #[test]
    fn paths_compose() {
        let e = at(at(host_error("range", "too big"), "[2]"), "items");
        assert_eq!(e.message, "at items[2]: too big");
        let e = at(at(host_error("type", "x"), "r"), "shape");
        assert_eq!(e.message, "at shape.r: x");
        // Errors of an app's own domain are left alone.
        let e = at(NativeError { domain: "app:m".into(), code: Value::Str("type".into()), message: "m".into() }, "x");
        assert_eq!(e.message, "m");
    }
}
