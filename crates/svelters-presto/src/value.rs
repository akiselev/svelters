//! Conversions between the ABI's [`svelters_abi::Value`], the surface [`Value`] (what JS sees, typed by
//! the surface spec) and presto-core's [`core::Value`] (what the tree stores): the host sends what JS
//! had (a number may arrive as `I64` or `F64`, an enum as its name), the tree stores exactly the
//! prop's type, and `null` is no value (a reset).

use crate::surface::{SignalSpec, Value, ValueType};
use presto_core as core;
use std::sync::Arc;
use presto_base::color::Color;
use svelters_abi::{SVL_BOOL, SVL_F64, SVL_I64, SVL_LIST, SVL_RECORD, SVL_STR, Value as Abi};

/// `v` as a value of `ty`, or why not.
pub fn from_abi(v: &Abi, ty: ValueType, nullable: bool) -> Result<Value, String> {
    let bad = || format!("expected {}{}, got {}", ty.name(), if nullable { " or null" } else { "" }, abi_kind(v));
    Ok(match (v, ty) {
        (Abi::Null, _) if nullable => Value::Null,
        (Abi::Bool(b), ValueType::Bool) => Value::Bool(*b),
        (Abi::I64(i), ValueType::I64) => Value::I64(*i),
        (Abi::F64(f), ValueType::I64) if f.fract() == 0.0 && f.abs() < 9.007_199_254_740_992e15 => Value::I64(*f as i64),
        (Abi::F64(f), ValueType::F64) => Value::F64(*f),
        (Abi::I64(i), ValueType::F64) => Value::F64(*i as f64),
        (Abi::Str(s), ValueType::Str) => Value::Str(s.clone()),
        (Abi::Str(s), ValueType::Enum(names)) => {
            let i = names.iter().position(|n| n == s).ok_or_else(|| format!("'{s}' is not one of {}", names.join(", ")))?;
            Value::Enum(i as u16)
        }
        (Abi::Enum { value, .. } | Abi::I64(value), ValueType::Enum(names)) if (*value as usize) < names.len() && *value >= 0 => Value::Enum(*value as u16),
        (Abi::Str(s), ValueType::Color) => Value::Color(parse_color(s).ok_or_else(|| format!("'{s}' is not a colour (#rgb, #rrggbb or #rrggbbaa)"))?),
        (Abi::List(items), ValueType::StrList) => Value::List(items.iter().map(|x| from_abi(x, ValueType::Str, false)).collect::<Result<_, _>>()?),
        (Abi::List(items), ValueType::NumList) => Value::List(items.iter().map(|x| from_abi(x, ValueType::F64, false)).collect::<Result<_, _>>()?),
        (Abi::List(items), ValueType::IdList) => Value::List(items.iter().map(|x| from_abi(x, ValueType::I64, false)).collect::<Result<_, _>>()?),
        (Abi::Map(fields), ValueType::Record) => Value::Record(fields.iter().map(|(k, v)| Ok((k.clone(), from_abi_any(v)?))).collect::<Result<_, String>>()?),
        _ => return Err(bad()),
    })
}

/// A record field or untyped argument: scalars, strings, lists and maps as they come.
fn from_abi_any(v: &Abi) -> Result<Value, String> {
    Ok(match v {
        Abi::Null => Value::Null,
        Abi::Bool(b) => Value::Bool(*b),
        Abi::I64(i) => Value::I64(*i),
        Abi::F64(f) => Value::F64(*f),
        Abi::Str(s) => Value::Str(s.clone()),
        Abi::List(items) => Value::List(items.iter().map(from_abi_any).collect::<Result<_, _>>()?),
        Abi::Map(fields) => Value::Record(fields.iter().map(|(k, v)| Ok((k.clone(), from_abi_any(v)?))).collect::<Result<_, String>>()?),
        other => return Err(format!("{} is not a plain value", abi_kind(other))),
    })
}

/// `v` (of type `ty`) for JS.
pub fn to_abi(v: &Value, ty: ValueType) -> Abi {
    match v {
        Value::Null => Abi::Null,
        Value::Bool(b) => Abi::Bool(*b),
        Value::I64(i) => Abi::I64(*i),
        Value::F64(f) => Abi::F64(*f),
        Value::Str(s) => Abi::Str(s.clone()),
        Value::Enum(i) => match ty {
            ValueType::Enum(names) => names.get(*i as usize).map_or(Abi::Null, |n| Abi::Str(n.to_string())),
            _ => Abi::I64(*i as i64),
        },
        Value::Color(c) => Abi::Str(color_text(*c)),
        Value::List(items) => {
            let of = match ty {
                ValueType::StrList => ValueType::Str,
                ValueType::IdList => ValueType::I64,
                _ => ValueType::F64,
            };
            Abi::List(items.iter().map(|x| to_abi(x, of)).collect())
        }
        Value::Record(fields) => Abi::Map(fields.iter().map(|(k, v)| (k.clone(), to_abi(v, ValueType::Record))).collect()),
    }
}

/// `v` for the tree; `None` for `null` (the prop is reset to its default).
pub fn to_core(v: Value) -> Option<core::Value> {
    Some(match v {
        Value::Null => return None,
        Value::Bool(b) => core::Value::Bool(b),
        Value::I64(i) => core::Value::I64(i),
        Value::F64(f) => core::Value::F64(f),
        Value::Str(s) => core::Value::Str(s.into()),
        Value::Enum(i) => core::Value::Enum(i as u32),
        Value::Color(c) => core::Value::Color(c),
        Value::List(items) => core::Value::List(items.into_iter().filter_map(to_core).collect::<Vec<_>>().into()),
        Value::Record(fields) => core::Value::Record(fields.into_iter().filter_map(|(k, v)| Some((Arc::from(k), to_core(v)?))).collect::<Vec<_>>().into()),
    })
}

/// A tree value as a surface value.
pub fn from_core(v: &core::Value) -> Value {
    match v {
        core::Value::Bool(b) => Value::Bool(*b),
        core::Value::I64(i) => Value::I64(*i),
        core::Value::F64(f) => Value::F64(*f),
        core::Value::Str(s) => Value::Str(s.to_string()),
        core::Value::Enum(i) => Value::Enum(*i as u16),
        core::Value::Color(c) => Value::Color(*c),
        core::Value::List(items) => Value::List(items.iter().map(from_core).collect()),
        core::Value::Record(fields) => Value::Record(fields.iter().map(|(k, v)| (k.to_string(), from_core(v))).collect()),
    }
}

/// A signal's surface arguments as presto-core's one argument (none, the one value, or a record by
/// the spec's argument names), as presto-widgets' host bridge emits them.
pub fn signal_arg(s: &SignalSpec, args: Vec<Value>) -> Option<core::Value> {
    match args.len() {
        0 => None,
        1 if s.args.len() <= 1 => to_core(args.into_iter().next().expect("one")),
        _ => to_core(Value::Record(s.args.iter().zip(args).map(|((n, _), v)| (n.to_string(), v)).collect())),
    }
}

/// The DOM `eventPhase` numbers.
pub const PHASE_CAPTURE: i64 = 1;
pub const PHASE_TARGET: i64 = 2;
pub const PHASE_BUBBLE: i64 = 3;

/// A core event argument as the surface signal's argument list for JS: a record's fields by the
/// spec's names, `target` and `phase` from the delivery (EV1), a single value as the one argument.
pub fn signal_args(s: &SignalSpec, arg: Option<&core::Value>, target: i64, phase: i64) -> Vec<Abi> {
    s.args
        .iter()
        .enumerate()
        .map(|(i, (name, ty))| match *name {
            "target" if s.bubbles => Abi::I64(target),
            "phase" if s.bubbles => Abi::I64(phase),
            _ => {
                let v = match arg {
                    Some(r @ core::Value::Record(_)) => r.field(name).map(from_core),
                    Some(v) if i == 0 => Some(from_core(v)),
                    _ => None,
                };
                v.map_or(Abi::Null, |v| to_abi(&v, *ty))
            }
        })
        .collect()
}

/// The ABI value tag a prop of type `ty` reads back as (`describe`).
pub fn tag_of(ty: ValueType) -> u16 {
    match ty {
        ValueType::Bool => SVL_BOOL,
        ValueType::I64 => SVL_I64,
        ValueType::F64 => SVL_F64,
        ValueType::Str | ValueType::Enum(_) | ValueType::Color => SVL_STR,
        ValueType::StrList | ValueType::NumList | ValueType::IdList => SVL_LIST,
        ValueType::Record => SVL_RECORD,
    }
}

fn abi_kind(v: &Abi) -> &'static str {
    match v {
        Abi::Null => "null",
        Abi::Bool(_) => "a boolean",
        Abi::I64(_) | Abi::U64(_) | Abi::F64(_) => "a number",
        Abi::Str(_) | Abi::Str16(_) => "a string",
        Abi::List(_) => "a list",
        Abi::Map(_) | Abi::Record { .. } => "an object",
        _ => "a native value",
    }
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
pub fn parse_color(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    let nib = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|n| n * 17);
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    if !h.is_ascii() {
        return None;
    }
    let [r, g, b, a] = match h.len() {
        3 => [nib(0)?, nib(1)?, nib(2)?, 255],
        4 => [nib(0)?, nib(1)?, nib(2)?, nib(3)?],
        6 => [byte(0)?, byte(2)?, byte(4)?, 255],
        8 => [byte(0)?, byte(2)?, byte(4)?, byte(6)?],
        _ => return None,
    };
    Some(Color::from_rgba8(r, g, b, a))
}

/// `#rrggbbaa`.
pub fn color_text(c: Color) -> String {
    let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", q(c.r), q(c.g), q(c.b), q(c.a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_cross_int_and_float() {
        assert_eq!(from_abi(&Abi::F64(3.0), ValueType::I64, false), Ok(Value::I64(3)));
        assert!(from_abi(&Abi::F64(3.5), ValueType::I64, false).is_err());
        assert_eq!(from_abi(&Abi::I64(2), ValueType::F64, false), Ok(Value::F64(2.0)));
    }

    #[test]
    fn enums_by_name_and_back() {
        let ty = ValueType::Enum(&["row", "column"]);
        let v = from_abi(&Abi::Str("column".into()), ty, false).unwrap();
        assert_eq!(v, Value::Enum(1));
        assert_eq!(to_abi(&v, ty), Abi::Str("column".into()));
        assert!(from_abi(&Abi::Str("grid".into()), ty, false).is_err());
    }

    #[test]
    fn null_only_where_nullable() {
        assert_eq!(from_abi(&Abi::Null, ValueType::F64, true), Ok(Value::Null));
        assert!(from_abi(&Abi::Null, ValueType::F64, false).is_err());
    }

    #[test]
    fn colours_round_trip() {
        let c = parse_color("#11223344").unwrap();
        assert_eq!(color_text(c), "#11223344");
        assert_eq!(color_text(parse_color("#fff").unwrap()), "#ffffffff");
        assert!(parse_color("red").is_none());
    }
}
