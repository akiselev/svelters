//! Prop values: what JS (through svelters-presto) and Rust write into the tree, typed by the kind's
//! [`crate::PropSpec`].

use presto_base::color::Color;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(Arc<str>),
    /// An index into the prop's `enum_values`.
    Enum(u32),
    Color(Color),
    List(Arc<[Value]>),
    /// Named fields, in the order the spec declares them.
    Record(Arc<[(Arc<str>, Value)]>),
}

/// The type of a prop, as the spec spells it (`tessel.json`/`presto.json` `type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueType {
    Bool,
    I64,
    F64,
    Str,
    Enum,
    Color,
    List,
    Record,
}

impl ValueType {
    pub const fn name(self) -> &'static str {
        match self {
            ValueType::Bool => "bool",
            ValueType::I64 => "int",
            ValueType::F64 => "number",
            ValueType::Str => "string",
            ValueType::Enum => "enum",
            ValueType::Color => "color",
            ValueType::List => "list",
            ValueType::Record => "record",
        }
    }
}

impl Value {
    pub fn ty(&self) -> ValueType {
        match self {
            Value::Bool(_) => ValueType::Bool,
            Value::I64(_) => ValueType::I64,
            Value::F64(_) => ValueType::F64,
            Value::Str(_) => ValueType::Str,
            Value::Enum(_) => ValueType::Enum,
            Value::Color(_) => ValueType::Color,
            Value::List(_) => ValueType::List,
            Value::Record(_) => ValueType::Record,
        }
    }

    /// `v` converted to `ty` where the conversion loses nothing a UI cares about (an integer for a
    /// number prop, a whole number for an int prop); `None` otherwise.
    pub fn coerce(self, ty: ValueType) -> Option<Value> {
        if self.ty() == ty {
            return Some(self);
        }
        match (self, ty) {
            (Value::I64(i), ValueType::F64) => Some(Value::F64(i as f64)),
            (Value::F64(f), ValueType::I64) if f.fract() == 0.0 && f.abs() < 9.0e15 => Some(Value::I64(f as i64)),
            (Value::I64(i), ValueType::Enum) if (0..=u32::MAX as i64).contains(&i) => Some(Value::Enum(i as u32)),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::F64(f) => Some(f),
            Value::I64(i) => Some(i as f64),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::I64(i) => Some(i),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_enum(&self) -> Option<u32> {
        match *self {
            Value::Enum(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_color(&self) -> Option<Color> {
        match *self {
            Value::Color(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }
    /// The field `name` of a record.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Record(r) => r.iter().find(|(k, _)| &**k == name).map(|(_, v)| v),
            _ => None,
        }
    }
    /// Text for attribute selectors (`[value="3"]`) and accessibility values.
    pub fn to_text(&self) -> Option<String> {
        Some(match self {
            Value::Bool(b) => b.to_string(),
            Value::I64(i) => i.to_string(),
            Value::F64(f) => f.to_string(),
            Value::Str(s) => s.to_string(),
            Value::Enum(e) => e.to_string(),
            Value::Color(_) | Value::List(_) | Value::Record(_) => return None,
        })
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::I64(i)
    }
}
impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::F64(f)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.into())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s.into())
    }
}
impl From<Color> for Value {
    fn from(c: Color) -> Self {
        Value::Color(c)
    }
}

/// A prop's default in a `'static` spec table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropDefault {
    /// No value until written (reads as `None`).
    Unset,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(&'static str),
    Enum(u32),
    Color(Color),
}

impl PropDefault {
    pub fn to_value(self) -> Option<Value> {
        Some(match self {
            PropDefault::Unset => return None,
            PropDefault::Bool(b) => Value::Bool(b),
            PropDefault::I64(i) => Value::I64(i),
            PropDefault::F64(f) => Value::F64(f),
            PropDefault::Str(s) => Value::Str(s.into()),
            PropDefault::Enum(e) => Value::Enum(e),
            PropDefault::Color(c) => Value::Color(c),
        })
    }
}
