//! Objective-C type encodings and value conversion rules (pure Rust, tested on every platform).
//!
//! The dynamic bridge learns a property's or argument's type from the runtime (`property_getAttributes`,
//! `-[NSMethodSignature getArgumentTypeAtIndex:]`) or from the generated tables, as a [`Ty`]. The
//! functions here turn ABI values into native scalars / struct bytes for that type and back, so the
//! only platform code left is boxing (`NSNumber`, `NSValue`, `NSString`) and the message send.

use svelters_abi::Value;
use svelters_abi::{SVL_T_INSETS, SVL_T_POINT, SVL_T_RECT, SVL_T_SIZE};

/// The shape of a native value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Void,
    Bool,
    /// Signed integer of `n` bytes (`c` is `I(1)`: on x86-64 that is also `BOOL`).
    I(u8),
    /// Unsigned integer of `n` bytes.
    U(u8),
    F32,
    F64,
    /// An object; the class name when the encoding names one (`@"NSString"`).
    Object(Option<String>),
    /// A block (`@?`).
    Block,
    Class,
    Sel,
    CStr,
    Pointer,
    Struct(StructTy),
    Unknown(String),
}

/// Structs the bridge converts; everything else is `Other` (refused).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructTy {
    Rect,
    Size,
    Point,
    /// `NSEdgeInsets` {top, left, bottom, right}.
    Insets,
    /// `NSRange` {location, length}.
    Range,
    Other(String),
}

impl StructTy {
    pub fn from_name(name: &str) -> StructTy {
        match name {
            "CGRect" | "NSRect" | "_NSRect" => StructTy::Rect,
            "CGSize" | "NSSize" | "_NSSize" => StructTy::Size,
            "CGPoint" | "NSPoint" | "_NSPoint" => StructTy::Point,
            "NSEdgeInsets" => StructTy::Insets,
            "_NSRange" | "NSRange" => StructTy::Range,
            other => StructTy::Other(other.to_string()),
        }
    }

    /// Number of 8-byte fields in native order (all supported structs are made of `double` or `NSUInteger`).
    pub fn fields(&self) -> usize {
        match self {
            StructTy::Rect | StructTy::Insets => 4,
            StructTy::Size | StructTy::Point | StructTy::Range => 2,
            StructTy::Other(_) => 0,
        }
    }

    /// The ObjC encoding used for `+[NSValue valueWithBytes:objCType:]` on 64-bit Apple targets.
    pub fn encoding(&self) -> &'static str {
        match self {
            StructTy::Rect => "{CGRect={CGPoint=dd}{CGSize=dd}}",
            StructTy::Size => "{CGSize=dd}",
            StructTy::Point => "{CGPoint=dd}",
            StructTy::Insets => "{NSEdgeInsets=dddd}",
            StructTy::Range => "{_NSRange=QQ}",
            StructTy::Other(_) => "",
        }
    }
}

const QUALIFIERS: &[u8] = b"rnNoORVAj+";

/// Parses one type from the front of `enc` (qualifiers and trailing offset digits are skipped) and
/// returns it with the rest of the string.
pub fn parse_type(enc: &str) -> (Ty, &str) {
    let b = enc.as_bytes();
    let mut i = 0;
    while i < b.len() && QUALIFIERS.contains(&b[i]) {
        i += 1;
    }
    if i >= b.len() {
        return (Ty::Unknown(enc.to_string()), "");
    }
    let start = i;
    let c = b[i];
    i += 1;
    let ty = match c {
        b'v' => Ty::Void,
        b'B' => Ty::Bool,
        b'c' => Ty::I(1),
        b's' => Ty::I(2),
        b'i' => Ty::I(4),
        b'l' => Ty::I(4), // `long` is encoded `q` on LP64; `l` means 32 bits
        b'q' => Ty::I(8),
        b'C' => Ty::U(1),
        b'S' => Ty::U(2),
        b'I' => Ty::U(4),
        b'L' => Ty::U(4),
        b'Q' => Ty::U(8),
        b'f' => Ty::F32,
        b'd' => Ty::F64,
        b'*' => Ty::CStr,
        b'#' => Ty::Class,
        b':' => Ty::Sel,
        b'@' => {
            if b.get(i) == Some(&b'?') {
                i += 1;
                if b.get(i) == Some(&b'<') {
                    i = skip_balanced(b, i, b'<', b'>');
                }
                Ty::Block
            } else if b.get(i) == Some(&b'"') {
                let rest = &enc[i + 1..];
                let end = rest.find('"').unwrap_or(rest.len());
                let name = &rest[..end];
                i += end + 2;
                // `@"<NSCopying>"` names only protocols
                let cls = name.split('<').next().unwrap_or("");
                Ty::Object((!cls.is_empty()).then(|| cls.to_string()))
            } else {
                Ty::Object(None)
            }
        }
        b'^' => {
            let (_, rest) = parse_type(&enc[i..]);
            i = enc.len() - rest.len();
            Ty::Pointer
        }
        b'{' => {
            let end = skip_balanced(b, start, b'{', b'}');
            let inner = &enc[start + 1..end.saturating_sub(1).max(start + 1)];
            let name = inner.split('=').next().unwrap_or("");
            i = end;
            Ty::Struct(StructTy::from_name(name))
        }
        b'[' | b'(' | b'b' | b'?' => {
            let end = match c {
                b'[' => skip_balanced(b, start, b'[', b']'),
                b'(' => skip_balanced(b, start, b'(', b')'),
                _ => {
                    let mut j = i;
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    j
                }
            };
            i = end;
            Ty::Unknown(enc[start..end].to_string())
        }
        _ => Ty::Unknown(enc[start..i].to_string()),
    };
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'-') {
        i += 1;
    }
    (ty, &enc[i.min(enc.len())..])
}

fn skip_balanced(b: &[u8], start: usize, open: u8, close: u8) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while i < b.len() {
        if b[i] == open {
            depth += 1;
        } else if b[i] == close {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        } else if b[i] == b'"' {
            // quoted class names inside structs
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += 1;
            }
        }
        i += 1;
    }
    b.len()
}

/// Parses a whole method type encoding (`v24@0:8@16`): return type first, then every argument
/// including `self` and `_cmd`.
pub fn parse_method(enc: &str) -> Vec<Ty> {
    let mut out = Vec::new();
    let mut rest = enc;
    while !rest.is_empty() {
        let (t, r) = parse_type(rest);
        out.push(t);
        if r.len() == rest.len() {
            break;
        }
        rest = r;
    }
    out
}

/// What `property_getAttributes` says about a declared property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropAttrs {
    pub ty: Ty,
    pub readonly: bool,
    pub copy: bool,
    pub weak: bool,
    pub getter: Option<String>,
    pub setter: Option<String>,
}

/// Parses `T@"NSString",C,N,V_title` (see Apple's "Declared Properties" runtime guide).
pub fn parse_property_attrs(attrs: &str) -> PropAttrs {
    let mut p = PropAttrs { ty: Ty::Unknown(String::new()), readonly: false, copy: false, weak: false, getter: None, setter: None };
    let rest = match attrs.strip_prefix('T') {
        Some(t) => {
            let (ty, rest) = parse_type(t);
            p.ty = ty;
            rest
        }
        None => attrs,
    };
    for part in rest.split(',').filter(|s| !s.is_empty()) {
        match part.as_bytes()[0] {
            b'R' => p.readonly = true,
            b'C' => p.copy = true,
            b'W' => p.weak = true,
            b'G' => p.getter = Some(part[1..].to_string()),
            b'S' => p.setter = Some(part[1..].to_string()),
            _ => {}
        }
    }
    p
}

/// A native scalar ready to box into an `NSNumber` or write into an argument buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scalar {
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
}

impl Scalar {
    /// The little-endian bytes of this scalar as a value of type `ty` (argument buffers).
    pub fn bytes_for(&self, ty: &Ty) -> Vec<u8> {
        let as_i = || match *self {
            Scalar::Bool(b) => b as i64,
            Scalar::I64(i) => i,
            Scalar::U64(u) => u as i64,
            Scalar::F64(f) => f as i64,
        };
        let as_f = || match *self {
            Scalar::Bool(b) => b as i64 as f64,
            Scalar::I64(i) => i as f64,
            Scalar::U64(u) => u as f64,
            Scalar::F64(f) => f,
        };
        match ty {
            Ty::Bool => vec![(as_i() != 0) as u8],
            Ty::I(n) | Ty::U(n) => as_i().to_le_bytes()[..*n as usize].to_vec(),
            Ty::F32 => (as_f() as f32).to_le_bytes().to_vec(),
            Ty::F64 => as_f().to_le_bytes().to_vec(),
            _ => Vec::new(),
        }
    }
}

/// Reads a scalar of type `ty` from little-endian bytes (a return buffer).
pub fn scalar_from_bytes(ty: &Ty, b: &[u8]) -> Option<Scalar> {
    let mut buf = [0u8; 8];
    match ty {
        Ty::Bool => Some(Scalar::Bool(b.first().copied().unwrap_or(0) != 0)),
        Ty::I(n) => {
            let n = *n as usize;
            buf[..n].copy_from_slice(b.get(..n)?);
            let raw = i64::from_le_bytes(buf);
            let shift = 64 - 8 * n as u32;
            Some(Scalar::I64(if shift == 0 { raw } else { (raw << shift) >> shift }))
        }
        Ty::U(n) => {
            let n = *n as usize;
            buf[..n].copy_from_slice(b.get(..n)?);
            Some(Scalar::U64(u64::from_le_bytes(buf)))
        }
        Ty::F32 => Some(Scalar::F64(f32::from_le_bytes(b.get(..4)?.try_into().ok()?) as f64)),
        Ty::F64 => Some(Scalar::F64(f64::from_le_bytes(b.get(..8)?.try_into().ok()?))),
        _ => None,
    }
}

/// Members of an enum or options type, JS name -> native value.
pub type Members<'a> = &'a [(&'a str, i64)];

fn member(members: Members, name: &str) -> Option<i64> {
    let name = name.trim();
    members.iter().find(|(n, _)| *n == name).map(|&(_, v)| v)
}

fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x") {
        return i64::from_str_radix(h, 16).ok();
    }
    s.parse().ok()
}

/// Converts a JS value for a scalar slot of type `ty`. `members` names enum values (`orientation =
/// "vertical"`); with `flags` a string may be `a|b` and a list of names is or-ed.
pub fn scalar_in(v: &Value, ty: &Ty, members: Members, flags: bool) -> Result<Scalar, String> {
    let int_of = |v: &Value| -> Result<i64, String> {
        match v {
            Value::Bool(b) => Ok(*b as i64),
            Value::I64(i) => Ok(*i),
            Value::U64(u) => Ok(*u as i64),
            Value::F64(f) if f.fract() == 0.0 => Ok(*f as i64),
            Value::F64(f) => Err(format!("expected an integer, got {f}")),
            Value::Enum { value, .. } => Ok(*value),
            Value::Flags { mask, .. } => Ok(*mask as i64),
            Value::Str(s) => {
                if let Some(i) = parse_int(s) {
                    return Ok(i);
                }
                if flags {
                    let mut m = 0;
                    for part in s.split('|').map(str::trim).filter(|p| !p.is_empty()) {
                        m |= member(members, part).ok_or_else(|| unknown_name(part, members))?;
                    }
                    return Ok(m);
                }
                member(members, s).ok_or_else(|| unknown_name(s, members))
            }
            Value::List(items) if flags => {
                let mut m = 0;
                for it in items {
                    m |= match it {
                        Value::Str(s) => member(members, s).ok_or_else(|| unknown_name(s, members))?,
                        other => return Err(format!("expected flag names, got {other:?}")),
                    };
                }
                Ok(m)
            }
            other => Err(format!("expected a number, got {}", value_kind(other))),
        }
    };
    match ty {
        Ty::Bool => match v {
            Value::Bool(b) => Ok(Scalar::Bool(*b)),
            Value::I64(i) => Ok(Scalar::Bool(*i != 0)),
            Value::F64(f) => Ok(Scalar::Bool(*f != 0.0)),
            Value::Str(s) if s.is_empty() || s == "true" => Ok(Scalar::Bool(true)),
            Value::Str(s) if s == "false" => Ok(Scalar::Bool(false)),
            Value::Null => Ok(Scalar::Bool(false)),
            other => Err(format!("expected a boolean, got {}", value_kind(other))),
        },
        Ty::I(_) => int_of(v).map(Scalar::I64),
        Ty::U(_) => match v {
            Value::U64(u) => Ok(Scalar::U64(*u)),
            other => int_of(other).map(|i| Scalar::U64(i as u64)),
        },
        Ty::F32 | Ty::F64 => match v {
            Value::F64(f) => Ok(Scalar::F64(*f)),
            Value::I64(i) => Ok(Scalar::F64(*i as f64)),
            Value::U64(u) => Ok(Scalar::F64(*u as f64)),
            Value::Bool(b) => Ok(Scalar::F64(*b as i64 as f64)),
            Value::Str(s) => s.trim().parse::<f64>().map(Scalar::F64).map_err(|_| format!("expected a number, got \"{s}\"")),
            other => Err(format!("expected a number, got {}", value_kind(other))),
        },
        other => Err(format!("{other:?} is not a scalar type")),
    }
}

fn unknown_name(name: &str, members: Members) -> String {
    let names: Vec<&str> = members.iter().map(|(n, _)| *n).collect();
    if names.is_empty() {
        format!("expected a number, got \"{name}\"")
    } else {
        format!("unknown value \"{name}\" (expected one of: {})", names.join(", "))
    }
}

/// A short name of a value's type for messages.
pub fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::I64(_) | Value::U64(_) | Value::F64(_) => "number",
        Value::DateTime(_) => "date",
        Value::Str(_) | Value::Str16(_) => "string",
        Value::Handle(_) => "object",
        Value::List(_) => "list",
        Value::Enum { .. } => "enum",
        Value::Flags { .. } => "flags",
        Value::Record { .. } => "record",
        Value::Bytes(_) => "bytes",
        Value::Boxed { .. } => "boxed",
        Value::Callback { .. } => "function",
        Value::Map(_) => "object",
        Value::Error(_) => "error",
    }
}

/// The JS value of a scalar read back: enum members by name when `members` has one for the value.
pub fn scalar_out(s: Scalar, ty: &Ty, members: Members, flags: bool) -> Value {
    match s {
        Scalar::Bool(b) => Value::Bool(b),
        Scalar::F64(f) => Value::F64(f),
        Scalar::I64(i) => int_out(i, ty, members, flags),
        Scalar::U64(u) => {
            if u <= i64::MAX as u64 {
                int_out(u as i64, ty, members, flags)
            } else {
                Value::U64(u)
            }
        }
    }
}

fn int_out(i: i64, ty: &Ty, members: Members, flags: bool) -> Value {
    if *ty == Ty::I(1) && members.is_empty() {
        // `BOOL` on x86-64 and `char` look the same; the generated tables type real BOOLs as Bool.
        return Value::I64(i);
    }
    if !flags {
        if let Some((n, _)) = members.iter().find(|(_, v)| *v == i) {
            return Value::Str((*n).to_string());
        }
    }
    Value::I64(i)
}

/// The fields of a struct value in native order. Accepts the well-known record (`Rect`, `Size`, ...),
/// a map with the field names, or a list of numbers in the JS field order.
pub fn struct_in(v: &Value, st: &StructTy) -> Result<Vec<f64>, String> {
    let names = js_fields(st);
    if names.is_empty() {
        return Err(format!("struct {st:?} is not supported"));
    }
    let num = |v: &Value| -> Result<f64, String> {
        match v {
            Value::F64(f) => Ok(*f),
            Value::I64(i) => Ok(*i as f64),
            Value::U64(u) => Ok(*u as f64),
            other => Err(format!("expected a number field, got {}", value_kind(other))),
        }
    };
    let js: Vec<f64> = match v {
        Value::Record { fields, .. } | Value::List(fields) => {
            if fields.len() != names.len() {
                return Err(format!("expected {} fields ({}), got {}", names.len(), names.join(", "), fields.len()));
            }
            fields.iter().map(num).collect::<Result<_, _>>()?
        }
        Value::Map(kv) => names
            .iter()
            .map(|n| kv.iter().find(|(k, _)| k == n).map(|(_, v)| num(v)).unwrap_or_else(|| Err(format!("missing field {n}"))))
            .collect::<Result<_, _>>()?,
        other => return Err(format!("expected {{{}}}, got {}", names.join(", "), value_kind(other))),
    };
    Ok(match st {
        // JS {top, right, bottom, left} -> NSEdgeInsets {top, left, bottom, right}
        StructTy::Insets => vec![js[0], js[3], js[2], js[1]],
        _ => js,
    })
}

/// The JS value of native struct fields (inverse of [`struct_in`]).
pub fn struct_out(st: &StructTy, native: &[f64]) -> Value {
    let f = |i: usize| Value::F64(native.get(i).copied().unwrap_or(0.0));
    match st {
        StructTy::Rect => Value::Record { ty: SVL_T_RECT, fields: vec![f(0), f(1), f(2), f(3)] },
        StructTy::Size => Value::Record { ty: SVL_T_SIZE, fields: vec![f(0), f(1)] },
        StructTy::Point => Value::Record { ty: SVL_T_POINT, fields: vec![f(0), f(1)] },
        StructTy::Insets => Value::Record { ty: SVL_T_INSETS, fields: vec![f(0), f(3), f(2), f(1)] },
        StructTy::Range => Value::Map(vec![
            ("location".into(), Value::I64(native.first().copied().unwrap_or(0.0) as i64)),
            ("length".into(), Value::I64(native.get(1).copied().unwrap_or(0.0) as i64)),
        ]),
        StructTy::Other(_) => Value::Null,
    }
}

/// JS field names of a struct, in JS order.
pub fn js_fields(st: &StructTy) -> &'static [&'static str] {
    match st {
        StructTy::Rect => &["x", "y", "width", "height"],
        StructTy::Size => &["width", "height"],
        StructTy::Point => &["x", "y"],
        StructTy::Insets => &["top", "right", "bottom", "left"],
        StructTy::Range => &["location", "length"],
        StructTy::Other(_) => &[],
    }
}

/// Native struct bytes from fields in native order (`double`s, or `NSUInteger`s for a range).
pub fn struct_bytes(st: &StructTy, native: &[f64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(native.len() * 8);
    for &x in native {
        match st {
            StructTy::Range => out.extend_from_slice(&(x as u64).to_le_bytes()),
            _ => out.extend_from_slice(&x.to_le_bytes()),
        }
    }
    out
}

/// Fields in native order from native struct bytes.
pub fn struct_from_bytes(st: &StructTy, b: &[u8]) -> Vec<f64> {
    b.chunks_exact(8)
        .take(st.fields())
        .map(|c| {
            let a: [u8; 8] = c.try_into().unwrap_or([0; 8]);
            match st {
                StructTy::Range => u64::from_le_bytes(a) as f64,
                _ => f64::from_le_bytes(a),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_types() {
        assert_eq!(parse_type("B").0, Ty::Bool);
        assert_eq!(parse_type("q").0, Ty::I(8));
        assert_eq!(parse_type("Q").0, Ty::U(8));
        assert_eq!(parse_type("d").0, Ty::F64);
        assert_eq!(parse_type("@").0, Ty::Object(None));
        assert_eq!(parse_type("@\"NSString\"").0, Ty::Object(Some("NSString".into())));
        assert_eq!(parse_type("@\"<NSStackViewDelegate>\"").0, Ty::Object(None));
        assert_eq!(parse_type("@\"NSView<NSTextInputClient>\"").0, Ty::Object(Some("NSView".into())));
        assert_eq!(parse_type("@?").0, Ty::Block);
        assert_eq!(parse_type("r*").0, Ty::CStr);
        assert_eq!(parse_type("^{CGColor=}").0, Ty::Pointer);
        assert_eq!(parse_type(":").0, Ty::Sel);
    }

    #[test]
    fn structs() {
        assert_eq!(parse_type("{CGRect={CGPoint=dd}{CGSize=dd}}").0, Ty::Struct(StructTy::Rect));
        assert_eq!(parse_type("{NSEdgeInsets=dddd}").0, Ty::Struct(StructTy::Insets));
        assert_eq!(parse_type("{_NSRange=QQ}").0, Ty::Struct(StructTy::Range));
        assert_eq!(parse_type("{CGAffineTransform=dddddd}").0, Ty::Struct(StructTy::Other("CGAffineTransform".into())));
    }

    #[test]
    fn method_signature() {
        let t = parse_method("v40@0:8@16q24@32");
        assert_eq!(t, vec![Ty::Void, Ty::Object(None), Ty::Sel, Ty::Object(None), Ty::I(8), Ty::Object(None)]);
        let t = parse_method("{CGSize=dd}16@0:8");
        assert_eq!(t[0], Ty::Struct(StructTy::Size));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn property_attributes() {
        let p = parse_property_attrs("T@\"NSString\",C,N,V_title");
        assert_eq!(p.ty, Ty::Object(Some("NSString".into())));
        assert!(p.copy && !p.readonly);
        let p = parse_property_attrs("TB,N,GisEnabled");
        assert_eq!(p.ty, Ty::Bool);
        assert_eq!(p.getter.as_deref(), Some("isEnabled"));
        let p = parse_property_attrs("T{CGRect={CGPoint=dd}{CGSize=dd}},R,N");
        assert_eq!(p.ty, Ty::Struct(StructTy::Rect));
        assert!(p.readonly);
        let p = parse_property_attrs("T@\"<NSWindowDelegate>\",W,N");
        assert!(p.weak);
    }

    const ORIENT: Members = &[("horizontal", 0), ("vertical", 1)];
    const MASK: Members = &[("titled", 1), ("closable", 2), ("miniaturizable", 4), ("resizable", 8)];

    #[test]
    fn scalars_in() {
        assert_eq!(scalar_in(&Value::Str("vertical".into()), &Ty::I(8), ORIENT, false), Ok(Scalar::I64(1)));
        assert_eq!(scalar_in(&Value::I64(0), &Ty::I(8), ORIENT, false), Ok(Scalar::I64(0)));
        assert!(scalar_in(&Value::Str("diagonal".into()), &Ty::I(8), ORIENT, false).unwrap_err().contains("horizontal, vertical"));
        assert_eq!(scalar_in(&Value::Str("titled|closable".into()), &Ty::U(8), MASK, true), Ok(Scalar::U64(3)));
        assert_eq!(
            scalar_in(&Value::List(vec![Value::Str("resizable".into()), Value::Str("titled".into())]), &Ty::U(8), MASK, true),
            Ok(Scalar::U64(9))
        );
        assert_eq!(scalar_in(&Value::Str("".into()), &Ty::Bool, &[], false), Ok(Scalar::Bool(true)));
        assert_eq!(scalar_in(&Value::Str("8".into()), &Ty::F64, &[], false), Ok(Scalar::F64(8.0)));
        assert_eq!(scalar_in(&Value::Str("8".into()), &Ty::I(8), &[], false), Ok(Scalar::I64(8)));
        assert!(scalar_in(&Value::F64(1.5), &Ty::I(8), &[], false).is_err());
        assert!(scalar_in(&Value::Str("x".into()), &Ty::Bool, &[], false).is_err());
    }

    #[test]
    fn scalars_out() {
        assert_eq!(scalar_out(Scalar::I64(1), &Ty::I(8), ORIENT, false), Value::Str("vertical".into()));
        assert_eq!(scalar_out(Scalar::I64(7), &Ty::I(8), ORIENT, false), Value::I64(7));
        assert_eq!(scalar_out(Scalar::U64(3), &Ty::U(8), MASK, true), Value::I64(3));
        assert_eq!(scalar_out(Scalar::Bool(true), &Ty::Bool, &[], false), Value::Bool(true));
    }

    #[test]
    fn scalar_bytes_roundtrip() {
        for (s, ty) in [(Scalar::I64(-3), Ty::I(8)), (Scalar::I64(-3), Ty::I(4)), (Scalar::U64(250), Ty::U(1)), (Scalar::Bool(true), Ty::Bool)] {
            let b = s.bytes_for(&ty);
            let back = scalar_from_bytes(&ty, &b).unwrap();
            match (s, back) {
                (Scalar::I64(a), Scalar::I64(b)) => assert_eq!(a, b),
                (Scalar::U64(a), Scalar::U64(b)) => assert_eq!(a, b),
                (Scalar::Bool(a), Scalar::Bool(b)) => assert_eq!(a, b),
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(scalar_from_bytes(&Ty::F64, &Scalar::F64(2.5).bytes_for(&Ty::F64)), Some(Scalar::F64(2.5)));
    }

    #[test]
    fn insets_reorder() {
        let js = Value::Record { ty: SVL_T_INSETS, fields: vec![Value::F64(1.0), Value::F64(2.0), Value::F64(3.0), Value::F64(4.0)] };
        let native = struct_in(&js, &StructTy::Insets).unwrap();
        assert_eq!(native, vec![1.0, 4.0, 3.0, 2.0]); // top, left, bottom, right
        assert_eq!(struct_out(&StructTy::Insets, &native), js);
        let m = Value::Map(vec![("width".into(), Value::I64(10)), ("height".into(), Value::F64(5.5))]);
        assert_eq!(struct_in(&m, &StructTy::Size).unwrap(), vec![10.0, 5.5]);
        let b = struct_bytes(&StructTy::Rect, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(struct_from_bytes(&StructTy::Rect, &b), vec![1.0, 2.0, 3.0, 4.0]);
        let r = struct_bytes(&StructTy::Range, &[3.0, 9.0]);
        assert_eq!(r.len(), 16);
        assert_eq!(struct_from_bytes(&StructTy::Range, &r), vec![3.0, 9.0]);
    }
}
