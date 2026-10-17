//! Value conversions that need no Windows types: numbers, booleans, enum names, the XAML string
//! forms of `Thickness` and `GridLength`. Platform-neutral and unit-tested on every target;
//! `conv.rs` wraps them in windows-rs types.
use crate::table::{Enum, kebab};
use svelters_abi::Value;

pub type R<T> = Result<T, String>;

fn shown(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("{s:?}"),
        other => format!("{other:?}"),
    }
}

pub fn to_string(v: &Value) -> R<String> {
    Ok(match v {
        Value::Str(s) => s.clone(),
        Value::Str16(w) => String::from_utf16_lossy(w),
        Value::Null => String::new(),
        Value::I64(i) => i.to_string(),
        Value::U64(u) => u.to_string(),
        Value::F64(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        other => return Err(format!("expected a string, got {}", shown(other))),
    })
}

/// Booleans also arrive as the text of a bare attribute (`""`), `"true"`/`"false"` or 0/1.
pub fn to_bool(v: &Value) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::I64(i) => Ok(*i != 0),
        Value::F64(f) => Ok(*f != 0.0),
        Value::Str(s) if s.is_empty() || s == "true" => Ok(true),
        Value::Str(s) if s == "false" => Ok(false),
        other => Err(format!("expected a boolean, got {}", shown(other))),
    }
}

pub fn to_f64(v: &Value) -> R<f64> {
    match v {
        Value::F64(f) => Ok(*f),
        Value::I64(i) => Ok(*i as f64),
        Value::U64(u) => Ok(*u as f64),
        Value::Str(s) if s.trim().eq_ignore_ascii_case("auto") || s.trim().eq_ignore_ascii_case("nan") => Ok(f64::NAN),
        Value::Str(s) => s.trim().parse().map_err(|_| format!("expected a number, got {s:?}")),
        other => Err(format!("expected a number, got {}", shown(other))),
    }
}

pub fn to_i32(v: &Value) -> R<i32> {
    let f = match v {
        Value::I64(i) => return i32::try_from(*i).map_err(|_| format!("{i} is out of range for a 32-bit integer")),
        other => to_f64(other)?,
    };
    if f.fract() != 0.0 || !(i32::MIN as f64..=i32::MAX as f64).contains(&f) {
        return Err(format!("expected a 32-bit integer, got {f}"));
    }
    Ok(f as i32)
}

/// Enum member by native name (`Collapsed`), kebab-case name (`collapsed`) or number.
pub fn enum_value(v: &Value, e: &Enum) -> R<i32> {
    let wanted = match v {
        Value::Str(s) => s.trim(),
        Value::I64(i) => {
            let i = *i as i32;
            return e.members.iter().any(|(_, m)| *m == i).then_some(i).ok_or_else(|| format!("{i} is not a value of {}", e.native));
        }
        other => return Err(format!("expected a {} name, got {}", e.native, shown(other))),
    };
    let norm = |s: &str| s.replace('-', "").to_ascii_lowercase();
    e.members.iter().find(|(n, _)| norm(n) == norm(wanted)).map(|(_, m)| *m).ok_or_else(|| {
        let names: Vec<String> = e.members.iter().map(|(n, _)| kebab(n)).collect();
        format!("{wanted:?} is not a {} (one of: {})", e.native, names.join(", "))
    })
}

/// The kebab-case name of an enum value, or its number when the table has no such member.
pub fn enum_name(e: &Enum, value: i32) -> Value {
    match e.members.iter().find(|(_, m)| *m == value) {
        Some((n, _)) => Value::Str(kebab(n)),
        None => Value::I64(value as i64),
    }
}

/// `Thickness` as `[left, top, right, bottom]`. XAML string forms: `"u"`, `"h,v"`, `"l,t,r,b"`
/// (commas or spaces); a number is uniform; a record or list is `{top, right, bottom, left}`
/// (the neutral `SVL_T_INSETS` order).
pub fn thickness(v: &Value) -> R<[f64; 4]> {
    let nums: Vec<f64> = match v {
        Value::F64(_) | Value::I64(_) | Value::U64(_) => vec![to_f64(v)?],
        Value::Str(s) => s
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<f64>().map_err(|_| format!("bad Thickness {s:?}: expected 1, 2 or 4 numbers")))
            .collect::<R<_>>()?,
        Value::Record { fields, .. } | Value::List(fields) if fields.len() == 4 => {
            let f: Vec<f64> = fields.iter().map(to_f64).collect::<R<_>>()?;
            return Ok([f[3], f[0], f[1], f[2]]);
        }
        Value::Map(kv) => {
            let get = |k: &str| kv.iter().find(|(n, _)| n == k).map(|(_, v)| to_f64(v)).transpose().map(|o| o.unwrap_or(0.0));
            return Ok([get("left")?, get("top")?, get("right")?, get("bottom")?]);
        }
        other => return Err(format!("expected a Thickness, got {}", shown(other))),
    };
    match nums[..] {
        [u] => Ok([u; 4]),
        [h, v] => Ok([h, v, h, v]),
        [l, t, r, b] => Ok([l, t, r, b]),
        _ => Err(format!("bad Thickness {}: expected 1, 2 or 4 numbers", shown(v))),
    }
}

fn num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 { format!("{}", f as i64) } else { f.to_string() }
}

/// `"l,t,r,b"`, the form the getter returns.
pub fn thickness_text(t: [f64; 4]) -> String {
    t.iter().map(|x| num(*x)).collect::<Vec<_>>().join(",")
}

/// `GridUnitType`: 0 Auto, 1 Pixel, 2 Star (winmd values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLen {
    pub value: f64,
    pub unit: i32,
}

pub const GRID_AUTO: i32 = 0;
pub const GRID_PIXEL: i32 = 1;
pub const GRID_STAR: i32 = 2;

/// `"Auto, *, 2*, 40"` -> lengths. An empty string is an empty collection.
pub fn grid_lengths(s: &str) -> R<Vec<GridLen>> {
    s.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| {
            if p.eq_ignore_ascii_case("auto") {
                Ok(GridLen { value: 1.0, unit: GRID_AUTO })
            } else if let Some(w) = p.strip_suffix('*') {
                let value = if w.trim().is_empty() { 1.0 } else { w.trim().parse().map_err(|_| format!("bad grid length {p:?}"))? };
                Ok(GridLen { value, unit: GRID_STAR })
            } else {
                Ok(GridLen { value: p.parse().map_err(|_| format!("bad grid length {p:?} (Auto, *, n* or pixels)"))?, unit: GRID_PIXEL })
            }
        })
        .collect()
}

pub fn grid_lengths_text(ls: &[GridLen]) -> String {
    ls.iter()
        .map(|l| match l.unit {
            GRID_AUTO => "Auto".to_string(),
            GRID_STAR if l.value == 1.0 => "*".to_string(),
            GRID_STAR => format!("{}*", num(l.value)),
            _ => num(l.value),
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Argument `i` of a method call.
pub fn arg(args: &[Value], i: usize) -> R<&Value> {
    args.get(i).ok_or_else(|| format!("missing argument {i}"))
}

pub fn arity(args: &[Value], n: usize) -> R<()> {
    if args.len() == n { Ok(()) } else { Err(format!("expected {n} argument(s), got {}", args.len())) }
}

/// A `call_sync` reply that asks for the result flag (`e.preventDefault()` -> `true`).
pub fn is_true(reply: &Option<Value>) -> bool {
    match reply {
        Some(Value::Bool(b)) => *b,
        Some(Value::I64(i)) => *i != 0,
        Some(Value::Map(kv)) => kv.iter().any(|(k, v)| k == "ret" && matches!(v, Value::Bool(true) | Value::I64(1))),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::ENUMS;

    fn s(x: &str) -> Value {
        Value::Str(x.into())
    }

    #[test]
    fn scalars() {
        assert!(to_bool(&s("")).unwrap());
        assert!(!to_bool(&s("false")).unwrap());
        assert_eq!(to_f64(&s("8")).unwrap(), 8.0);
        assert!(to_f64(&s("Auto")).unwrap().is_nan());
        assert_eq!(to_i32(&Value::F64(3.0)).unwrap(), 3);
        assert!(to_i32(&Value::F64(3.5)).is_err());
        assert!(to_bool(&s("yes")).is_err());
    }

    #[test]
    fn enums_by_name_kebab_or_number() {
        let vis = ENUMS.iter().find(|e| e.native == "Microsoft.UI.Xaml.Visibility").unwrap();
        assert_eq!(enum_value(&s("Collapsed"), vis).unwrap(), 1);
        assert_eq!(enum_value(&s("collapsed"), vis).unwrap(), 1);
        assert_eq!(enum_value(&Value::I64(0), vis).unwrap(), 0);
        assert!(enum_value(&s("hidden"), vis).unwrap_err().contains("visible, collapsed"));
        assert_eq!(enum_name(vis, 1), s("collapsed"));
        let ha = ENUMS.iter().find(|e| e.native == "Microsoft.UI.Xaml.HorizontalAlignment").unwrap();
        assert_eq!(enum_value(&s("stretch"), ha).unwrap(), 3);
    }

    #[test]
    fn thickness_forms() {
        assert_eq!(thickness(&s("4")).unwrap(), [4.0; 4]);
        assert_eq!(thickness(&s("2,4")).unwrap(), [2.0, 4.0, 2.0, 4.0]);
        assert_eq!(thickness(&s("1 2 3 4")).unwrap(), [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(thickness(&Value::I64(8)).unwrap(), [8.0; 4]);
        // INSETS record order is top, right, bottom, left
        let rec = Value::Record { ty: 4, fields: vec![Value::F64(1.0), Value::F64(2.0), Value::F64(3.0), Value::F64(4.0)] };
        assert_eq!(thickness(&rec).unwrap(), [4.0, 1.0, 2.0, 3.0]);
        assert!(thickness(&s("1,2,3")).is_err());
        assert_eq!(thickness_text([1.0, 2.0, 3.5, 4.0]), "1,2,3.5,4");
    }

    #[test]
    fn grid_length_lists() {
        let ls = grid_lengths("Auto, *, 2*, 40").unwrap();
        assert_eq!(ls.len(), 4);
        assert_eq!(ls[2], GridLen { value: 2.0, unit: GRID_STAR });
        assert_eq!(grid_lengths_text(&ls), "Auto,*,2*,40");
        assert!(grid_lengths("").unwrap().is_empty());
        assert!(grid_lengths("wide").is_err());
    }

    #[test]
    fn sync_replies() {
        assert!(is_true(&Some(Value::Bool(true))));
        assert!(!is_true(&Some(Value::Null)));
        assert!(!is_true(&None));
    }
}
