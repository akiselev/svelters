//! Prop storage for the built-in kinds: [`kind_props!`](crate::kind_props) declares a kind's props once
//! and generates the spec rows ([`PropSpec`]), a struct holding the values with their defaults, typed
//! `set`/`get` by spec index, and an enum naming each prop's index.
//!
//! ```ignore
//! kind_props! {
//!     /// The button's props.
//!     pub struct ButtonProps;
//!     /// The caption.
//!     text "text": String = Str("") ; Str, Layout;
//!     font_weight "font-weight": i64 = I64(400) ; I64, Layout;
//! }
//! ```

use crate::shim::core::{Default as Dv, PropError, PropSpec, Value, check};
use skelter_base::color::Color;

/// A Rust type a prop is stored as.
pub trait PropValue: Sized {
    fn from_value(v: &Value) -> Option<Self>;
    fn to_value(&self) -> Value;
    fn from_default(d: Dv) -> Self;
}

impl PropValue for bool {
    fn from_value(v: &Value) -> Option<bool> {
        v.as_bool()
    }
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_default(d: Dv) -> bool {
        matches!(d, Dv::Bool(true))
    }
}

impl PropValue for i64 {
    fn from_value(v: &Value) -> Option<i64> {
        v.as_i64()
    }
    fn to_value(&self) -> Value {
        Value::I64(*self)
    }
    fn from_default(d: Dv) -> i64 {
        match d {
            Dv::I64(v) => v,
            Dv::F64(v) => v as i64,
            _ => 0,
        }
    }
}

impl PropValue for f64 {
    fn from_value(v: &Value) -> Option<f64> {
        v.as_f64()
    }
    fn to_value(&self) -> Value {
        Value::F64(*self)
    }
    fn from_default(d: Dv) -> f64 {
        match d {
            Dv::F64(v) => v,
            Dv::I64(v) => v as f64,
            _ => 0.0,
        }
    }
}

impl PropValue for String {
    fn from_value(v: &Value) -> Option<String> {
        v.as_str().map(str::to_owned)
    }
    fn to_value(&self) -> Value {
        Value::str(self)
    }
    fn from_default(d: Dv) -> String {
        match d {
            Dv::Str(s) | Dv::Enum(s) => s.to_owned(),
            _ => String::new(),
        }
    }
}

/// An optional number (`value` of an indeterminate progress bar): unset is `None`.
impl PropValue for Option<f64> {
    fn from_value(v: &Value) -> Option<Option<f64>> {
        Some(v.as_f64())
    }
    fn to_value(&self) -> Value {
        match self {
            Some(v) => Value::F64(*v),
            None => Value::List(Vec::new()),
        }
    }
    fn from_default(d: Dv) -> Option<f64> {
        match d {
            Dv::F64(v) => Some(v),
            Dv::I64(v) => Some(v as f64),
            _ => None,
        }
    }
}

impl PropValue for Option<Color> {
    fn from_value(v: &Value) -> Option<Option<Color>> {
        match v {
            Value::Color(c) => Some(Some(*c)),
            _ => Some(None),
        }
    }
    fn to_value(&self) -> Value {
        match self {
            Some(c) => Value::Color(*c),
            None => Value::List(Vec::new()),
        }
    }
    fn from_default(_d: Dv) -> Option<Color> {
        None
    }
}

impl PropValue for Vec<Value> {
    fn from_value(v: &Value) -> Option<Vec<Value>> {
        match v {
            Value::List(l) => Some(l.clone()),
            _ => None,
        }
    }
    fn to_value(&self) -> Value {
        Value::List(self.clone())
    }
    fn from_default(_d: Dv) -> Vec<Value> {
        Vec::new()
    }
}

/// Writes `v` into `slot` after checking it against `spec`.
pub fn store<T: PropValue>(spec: &PropSpec, slot: &mut T, v: &Value) -> Result<(), PropError> {
    // optional props accept "unset" (an empty list) besides their type
    let unset = matches!(v, Value::List(l) if l.is_empty());
    if !unset {
        check(spec, v)?;
    }
    *slot = T::from_value(v).ok_or(PropError::Type { prop: spec.name, expected: spec.ty })?;
    Ok(())
}

/// Declares a kind's props: see the module docs. Each row is
/// `field "prop-name": Type = <Default variant> ; <ValueType>, <ChangeClass> [, bindable]`.
#[macro_export]
macro_rules! kind_props {
    (
        $(#[$m:meta])* pub struct $name:ident;
        $( $(#[doc = $doc:literal])* $field:ident $pname:literal : $ty:ty = $dv:expr ; $vt:expr , $change:ident $(, $bind:ident)? ; )*
    ) => {
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name { $( $(#[doc = $doc])* pub $field: $ty, )* }

        #[allow(non_camel_case_types, dead_code)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u32)]
        /// The props by spec index.
        pub enum P { $( $field, )* }

        impl P {
            pub const fn id(self) -> $crate::shim::core::PropId {
                $crate::shim::core::PropId(self as u32)
            }
        }

        impl ::core::default::Default for $name {
            fn default() -> Self {
                #[allow(unused_imports)]
                use $crate::shim::core::Default::*;
                $name { $( $field: <$ty as $crate::controls::props::PropValue>::from_default($dv), )* }
            }
        }

        impl $name {
            /// The spec rows, in index order.
            pub const SPECS: &'static [$crate::shim::core::PropSpec] = &[ $(
                {
                    let dv = {
                        #[allow(unused_imports)]
                        use $crate::shim::core::Default::*;
                        $dv
                    };
                    let vt = {
                        #[allow(unused_imports)]
                        use $crate::shim::core::ValueType::*;
                        $vt
                    };
                    let s = $crate::shim::core::PropSpec::new($pname, vt, ::skelter_base::change::ChangeClass::$change, dv, concat!($($doc, )* ""));
                    $( let _ = stringify!($bind); let s = s.bindable(); )?
                    s
                },
            )* ];

            /// Sets prop `id` to `v` (type-checked against its spec).
            pub fn set(&mut self, id: $crate::shim::core::PropId, v: &$crate::shim::core::Value) -> Result<(), $crate::shim::core::PropError> {
                let spec = Self::SPECS.get(id.0 as usize).ok_or($crate::shim::core::PropError::UnknownProp)?;
                let mut i = 0u32;
                $(
                    if id.0 == i {
                        return $crate::controls::props::store(spec, &mut self.$field, v);
                    }
                    i += 1;
                )*
                let _ = i;
                Err($crate::shim::core::PropError::UnknownProp)
            }

            /// The value of prop `id`.
            pub fn get(&self, id: $crate::shim::core::PropId) -> Option<$crate::shim::core::Value> {
                let mut i = 0u32;
                $(
                    if id.0 == i {
                        return Some($crate::controls::props::PropValue::to_value(&self.$field));
                    }
                    i += 1;
                )*
                let _ = i;
                None
            }
        }
    };
}

