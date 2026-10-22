//! RA2: serde values crossing as structured JS values ([`Json<T>`]), typed from `schemars`.
//!
//! A `Json<T>` crosses as a plain JS value (objects, arrays, numbers, strings, booleans, `null`), with no
//! JSON text in between: the host converts the JS value structurally (a variant slot), and serde reads
//! the owned [`Value`] tree directly. serde's errors keep their kind: a wrong type is `type`, an integer
//! out of range `range`, a missing field `missing-field`, an unknown tag or variant `unknown-variant`
//! (`NativeError { domain: 'svelters' }`), and the message starts with the path (`at shape.r: ...`).
//!
//! The typings come from `T`'s [`JsonSchema`]: named schemas (`$defs`) become exported TS types of the
//! module that uses them, and the JSON Schema itself goes into `exports.json`.

use super::*;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::{self, DeserializeOwned, DeserializeSeed, IntoDeserializer, Visitor};
use serde_json::Value as J;
use std::fmt;

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
        from_value(v).map(Json)
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
    /// Innermost segment last.
    path: Vec<String>,
    msg: String,
}

impl DeError {
    fn new(code: &'static str, msg: String) -> DeError {
        DeError { code, path: Vec::new(), msg }
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
        DeError::new("invalid", msg.to_string())
    }
    fn invalid_type(unexp: de::Unexpected<'_>, exp: &dyn de::Expected) -> Self {
        DeError::new("type", format!("invalid type: {unexp}, expected {exp}"))
    }
    fn invalid_value(unexp: de::Unexpected<'_>, exp: &dyn de::Expected) -> Self {
        // serde's integer visitors report a number that does not fit this way.
        let code = match unexp {
            de::Unexpected::Signed(_) | de::Unexpected::Unsigned(_) | de::Unexpected::Float(_) => "range",
            _ => "invalid",
        };
        DeError::new(code, format!("invalid value: {unexp}, expected {exp}"))
    }
    fn invalid_length(len: usize, exp: &dyn de::Expected) -> Self {
        DeError::new("type", format!("invalid length {len}, expected {exp}"))
    }
    fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self {
        DeError::new("unknown-variant", format!("unknown variant `{variant}`, expected {}", one_of(expected)))
    }
    fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
        DeError::new("unknown-field", format!("unknown field `{field}`, expected {}", one_of(expected)))
    }
    fn missing_field(field: &'static str) -> Self {
        DeError::new("missing-field", format!("missing field `{field}`"))
    }
    fn duplicate_field(field: &'static str) -> Self {
        DeError::new("type", format!("duplicate field `{field}`"))
    }
}

// ---- Value -> T ----

/// Reads a `T` from a structurally converted JS value.
pub fn from_value<T: DeserializeOwned>(v: Value) -> Result<T, NativeError> {
    T::deserialize(De(v)).map_err(DeError::into_native)
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

struct De(Value);

impl<'de> de::Deserializer<'de> for De {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, DeError> {
        match self.0 {
            Value::Null => v.visit_unit(),
            Value::Bool(b) => v.visit_bool(b),
            Value::I64(i) => v.visit_i64(i),
            Value::U64(u) => v.visit_u64(u),
            Value::F64(f) | Value::DateTime(f) => v.visit_f64(f),
            Value::Str(s) => v.visit_string(s),
            Value::Str16(u) => v.visit_string(String::from_utf16_lossy(&u)),
            Value::Bytes(b) => v.visit_byte_buf(b),
            Value::List(items) => {
                let n = items.len();
                let mut seq = Seq { it: items.into_iter(), i: 0 };
                let out = v.visit_seq(&mut seq)?;
                if seq.it.len() != 0 {
                    return Err(de::Error::invalid_length(n, &"fewer elements"));
                }
                Ok(out)
            }
            Value::Map(entries) => v.visit_map(Map { it: entries.into_iter(), value: None }),
            other => Err(DeError::new("type", format!("{} cannot be read as a serde value", what(&other)))),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, DeError> {
        match self.0 {
            Value::Null => v.visit_none(),
            other => v.visit_some(De(other)),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _name: &'static str, v: V) -> Result<V::Value, DeError> {
        v.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(self, _name: &'static str, _variants: &'static [&'static str], v: V) -> Result<V::Value, DeError> {
        // Externally tagged: `"unit"` or `{ "variant": value }`.
        match self.0 {
            Value::Str(s) => v.visit_enum(s.into_deserializer()),
            Value::Map(mut m) if m.len() == 1 => {
                let (variant, value) = m.pop().expect("one entry");
                v.visit_enum(Enum { variant, value })
            }
            other => Err(de::Error::invalid_type(unexpected(&other), &"a variant name or an object with one key")),
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

struct Seq {
    it: std::vec::IntoIter<Value>,
    i: usize,
}

impl<'de> de::SeqAccess<'de> for Seq {
    type Error = DeError;
    fn next_element_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<Option<S::Value>, DeError> {
        match self.it.next() {
            Some(x) => {
                let i = self.i;
                self.i += 1;
                seed.deserialize(De(x)).map(Some).map_err(|e| e.at(format!("[{i}]")))
            }
            None => Ok(None),
        }
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.it.len())
    }
}

struct Map {
    it: std::vec::IntoIter<(String, Value)>,
    value: Option<(String, Value)>,
}

impl<'de> de::MapAccess<'de> for Map {
    type Error = DeError;
    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, DeError> {
        match self.it.next() {
            Some((k, v)) => {
                let key = seed.deserialize(de::value::StrDeserializer::<DeError>::new(&k)).map_err(|e| e.at(k.clone()))?;
                self.value = Some((k, v));
                Ok(Some(key))
            }
            None => Ok(None),
        }
    }
    fn next_value_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<S::Value, DeError> {
        let (k, v) = self.value.take().ok_or_else(|| DeError::new("invalid", "a map value without its key".into()))?;
        seed.deserialize(De(v)).map_err(|e| e.at(k))
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.it.len())
    }
}

struct Enum {
    variant: String,
    value: Value,
}

impl<'de> de::EnumAccess<'de> for Enum {
    type Error = DeError;
    type Variant = Variant;
    fn variant_seed<S: DeserializeSeed<'de>>(self, seed: S) -> Result<(S::Value, Variant), DeError> {
        let v = seed.deserialize(de::value::StrDeserializer::<DeError>::new(&self.variant))?;
        Ok((v, Variant { name: self.variant, value: self.value }))
    }
}

struct Variant {
    name: String,
    value: Value,
}

impl<'de> de::VariantAccess<'de> for Variant {
    type Error = DeError;
    fn unit_variant(self) -> Result<(), DeError> {
        match self.value {
            Value::Null => Ok(()),
            other => Err(DeError::invalid_type_at(&other, "a unit variant", self.name)),
        }
    }
    fn newtype_variant_seed<S: DeserializeSeed<'de>>(self, seed: S) -> Result<S::Value, DeError> {
        let name = self.name;
        seed.deserialize(De(self.value)).map_err(|e| e.at(name))
    }
    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, v: V) -> Result<V::Value, DeError> {
        let name = self.name;
        de::Deserializer::deserialize_seq(De(self.value), v).map_err(|e| e.at(name))
    }
    fn struct_variant<V: Visitor<'de>>(self, _fields: &'static [&'static str], v: V) -> Result<V::Value, DeError> {
        let name = self.name;
        de::Deserializer::deserialize_map(De(self.value), v).map_err(|e| e.at(name))
    }
}

impl DeError {
    fn invalid_type_at(v: &Value, exp: &str, seg: String) -> DeError {
        DeError::new("type", format!("invalid type: {}, expected {exp}", unexpected(v))).at(seg)
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
pub(crate) fn schema_ts(s: &J, refs: &mut Vec<String>) -> String {
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
        parts.push(ts_ident(&name));
        if !refs.contains(&name) {
            refs.push(name);
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
fn own_ts(o: &serde_json::Map<String, J>, refs: &mut Vec<String>) -> Option<String> {
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

fn array_ts(o: &serde_json::Map<String, J>, refs: &mut Vec<String>) -> String {
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

fn object_ts(o: &serde_json::Map<String, J>, refs: &mut Vec<String>) -> String {
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
        let mut refs = Vec::new();
        (schema_ts(&v, &mut refs), refs)
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
