//! Values that need no Windows types: scalars, enums and flags by name, records (WinRT structs) as plain objects,
//! `DateTime` / `TimeSpan`, GUIDs. Converted to and from their ABI bytes (little-endian, natural alignment, as the
//! generator laid them out). Platform-neutral and unit-tested on every target; `rt.rs` adds strings and objects.
use super::invoke::Word;
use super::model::{EnumT, RecordT, Ty, TyId, enum_t, kebab, norm_name};
use super::tables::{RECORDS, TYS};
use svelters_abi::Value;

pub type R<T> = Result<T, String>;

/// 100 ns ticks between 1601-01-01 and 1970-01-01 (WinRT `DateTime` epoch to the JS epoch).
const EPOCH_DELTA: i64 = 116_444_736_000_000_000;

pub fn ty(id: TyId) -> Ty {
    TYS[id as usize]
}

fn shown(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("{s:?}"),
        other => format!("{other:?}"),
    }
}

pub fn to_f64(v: &Value) -> R<f64> {
    match v {
        Value::F64(f) | Value::DateTime(f) => Ok(*f),
        Value::I64(i) => Ok(*i as f64),
        Value::U64(u) => Ok(*u as f64),
        Value::Bool(b) => Ok(*b as u8 as f64),
        Value::Str(s) => s.trim().parse().map_err(|_| format!("expected a number, got {s:?}")),
        other => Err(format!("expected a number, got {}", shown(other))),
    }
}

/// An integer in `[min, max]` (numbers must be integral).
pub fn to_int(v: &Value, min: i128, max: i128) -> R<i128> {
    let i: i128 = match v {
        Value::I64(i) => *i as i128,
        Value::U64(u) => *u as i128,
        Value::Bool(b) => *b as i128,
        other => {
            let f = to_f64(other)?;
            if f.fract() != 0.0 || !f.is_finite() {
                return Err(format!("expected an integer, got {f}"));
            }
            f as i128
        }
    };
    if i < min || i > max {
        return Err(format!("{i} is out of range [{min}, {max}]"));
    }
    Ok(i)
}

pub fn to_bool(v: &Value) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::I64(i) => Ok(*i != 0),
        Value::F64(f) => Ok(*f != 0.0),
        Value::Str(s) if s == "true" => Ok(true),
        Value::Str(s) if s == "false" => Ok(false),
        other => Err(format!("expected a boolean, got {}", shown(other))),
    }
}

pub fn to_str(v: &Value) -> R<String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        Value::Str16(w) => Ok(String::from_utf16_lossy(w)),
        Value::Null => Ok(String::new()),
        Value::I64(i) => Ok(i.to_string()),
        Value::F64(f) => Ok(f.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        other => Err(format!("expected a string, got {}", shown(other))),
    }
}

// ---- enums ----------------------------------------------------------------------------------------------------

fn member(e: &EnumT, name: &str) -> R<i64> {
    let n = norm_name(name.trim());
    e.members.iter().find(|(m, _)| norm_name(m) == n).map(|(_, v)| *v).ok_or_else(|| {
        let names: Vec<String> = e.members.iter().map(|(m, _)| kebab(m)).collect();
        format!("{name:?} is not a {} (one of: {})", e.name, names.join(", "))
    })
}

/// An enum from its native, camelCase or kebab-case name or its number; flags also from a list of names (or a
/// `|`/`,`-separated string).
pub fn enum_in(e: &EnumT, v: &Value) -> R<i64> {
    match v {
        Value::Str(s) if e.flags && (s.contains('|') || s.contains(',')) => {
            s.split(['|', ',']).filter(|p| !p.trim().is_empty()).map(|p| member(e, p)).try_fold(0i64, |a, m| Ok(a | m?))
        }
        Value::Str(s) => member(e, s),
        Value::List(items) if e.flags => items.iter().try_fold(0i64, |a, it| Ok(a | enum_in(e, it)?)),
        Value::Enum { value, .. } => Ok(*value),
        Value::Flags { mask, .. } => Ok(*mask as i64),
        other => Ok(to_int(other, i32::MIN as i128, u32::MAX as i128)? as i64),
    }
}

/// The kebab-case name (enums), the list of set member names (flags; a zero value is `[]` unless a member is 0),
/// or the number when no member matches.
pub fn enum_out(e: &EnumT, value: i64) -> Value {
    if e.flags {
        if let Some((m, _)) = e.members.iter().find(|(_, v)| *v == value && value == 0) {
            return Value::List(vec![Value::Str(kebab(m))]);
        }
        let mut out = Vec::new();
        let mut rest = value;
        for (m, v) in e.members {
            if *v != 0 && value & v == *v && v.count_ones() == 1 {
                out.push(Value::Str(kebab(m)));
                rest &= !v;
            }
        }
        if rest != 0 {
            out.push(Value::I64(rest));
        }
        return Value::List(out);
    }
    match e.members.iter().find(|(_, v)| *v == value) {
        Some((m, _)) => Value::Str(kebab(m)),
        None => Value::I64(value),
    }
}

// ---- GUIDs ----------------------------------------------------------------------------------------------------

pub fn guid_text(b: &[u8; 16]) -> String {
    let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let d2 = u16::from_le_bytes([b[4], b[5]]);
    let d3 = u16::from_le_bytes([b[6], b[7]]);
    format!(
        "{d1:08x}-{d2:04x}-{d3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

pub fn guid_bytes(s: &str) -> R<[u8; 16]> {
    let hex: String = s.trim().trim_start_matches('{').trim_end_matches('}').chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("expected a GUID, got {s:?}"));
    }
    let n = u128::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
    let d1 = (n >> 96) as u32;
    let d2 = (n >> 80) as u16;
    let d3 = (n >> 64) as u16;
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&d1.to_le_bytes());
    out[4..6].copy_from_slice(&d2.to_le_bytes());
    out[6..8].copy_from_slice(&d3.to_le_bytes());
    out[8..16].copy_from_slice(&(n as u64).to_be_bytes());
    Ok(out)
}

// ---- layout ---------------------------------------------------------------------------------------------------

/// Size and alignment of a value of `t` in a struct or an out buffer (pointers are 8 bytes: x86_64).
pub fn size_align(t: Ty) -> (usize, usize) {
    match t {
        Ty::Void => (0, 1),
        Ty::Bool | Ty::I8 | Ty::U8 => (1, 1),
        Ty::Char16 | Ty::I16 | Ty::U16 => (2, 2),
        Ty::I32 | Ty::U32 | Ty::F32 | Ty::Enum(_) => (4, 4),
        Ty::I64 | Ty::U64 | Ty::F64 | Ty::DateTime | Ty::TimeSpan => (8, 8),
        Ty::Guid => (16, 4),
        Ty::Record(r) => {
            let r = &RECORDS[r as usize];
            (r.size as usize, r.align as usize)
        }
        _ => (8, 8),
    }
}

/// Whether `t` is passed in one integer slot by value (as opposed to a pointer to a copy): structs of 1, 2, 4 or 8
/// bytes (Microsoft x64 convention).
pub fn struct_in_register(size: usize) -> bool {
    matches!(size, 1 | 2 | 4 | 8)
}

// ---- scalars and records to/from bytes -------------------------------------------------------------------------

/// Writes a plain value (scalar, enum, record, DateTime, TimeSpan, GUID) into `buf` (at least `size_align(t).0`).
pub fn write_plain(t: Ty, v: &Value, buf: &mut [u8]) -> R<()> {
    macro_rules! put {
        ($x:expr) => {{
            let b = $x.to_le_bytes();
            buf[..b.len()].copy_from_slice(&b);
        }};
    }
    match t {
        Ty::Bool => buf[0] = to_bool(v)? as u8,
        Ty::Char16 => {
            let s = to_str(v)?;
            let mut units = s.encode_utf16();
            let c = units.next().ok_or("expected one character, got an empty string")?;
            if units.next().is_some() {
                return Err(format!("expected one UTF-16 character, got {s:?}"));
            }
            put!(c)
        }
        Ty::I8 => put!(to_int(v, i8::MIN as i128, i8::MAX as i128)? as i8),
        Ty::U8 => put!(to_int(v, 0, u8::MAX as i128)? as u8),
        Ty::I16 => put!(to_int(v, i16::MIN as i128, i16::MAX as i128)? as i16),
        Ty::U16 => put!(to_int(v, 0, u16::MAX as i128)? as u16),
        Ty::I32 => put!(to_int(v, i32::MIN as i128, i32::MAX as i128)? as i32),
        Ty::U32 => put!(to_int(v, 0, u32::MAX as i128)? as u32),
        Ty::I64 => put!(to_int(v, i64::MIN as i128, i64::MAX as i128)? as i64),
        Ty::U64 => put!(to_int(v, 0, u64::MAX as i128)? as u64),
        Ty::F32 => put!(to_f64(v)? as f32),
        Ty::F64 => put!(to_f64(v)?),
        Ty::Enum(e) => {
            let e = enum_t(e);
            let x = enum_in(e, v)?;
            if e.unsigned { put!(x as u32) } else { put!(x as i32) }
        }
        Ty::DateTime => put!((to_f64(v)? * 10_000.0).round() as i64 + EPOCH_DELTA),
        Ty::TimeSpan => put!((to_f64(v)? * 10_000.0).round() as i64),
        Ty::Guid => buf[..16].copy_from_slice(&guid_bytes(&to_str(v)?)?),
        Ty::Record(r) => {
            let r = &RECORDS[r as usize];
            let fields: Vec<(String, &Value)> = match v {
                Value::Map(m) => m.iter().map(|(k, v)| (k.clone(), v)).collect(),
                Value::Record { fields, .. } => r.fields.iter().zip(fields).map(|(f, v)| (f.name.to_string(), v)).collect(),
                Value::List(items) if items.len() == r.fields.len() => r.fields.iter().zip(items).map(|(f, v)| (f.name.to_string(), v)).collect(),
                other => return Err(format!("expected a {} object, got {}", r.name, shown(other))),
            };
            for (k, _) in &fields {
                if !r.fields.iter().any(|f| f.name == k || norm_name(f.name) == norm_name(k)) {
                    let names: Vec<&str> = r.fields.iter().map(|f| f.name).collect();
                    return Err(format!("{} has no field {k:?} (fields: {})", r.name, names.join(", ")));
                }
            }
            for f in r.fields {
                let fv = fields.iter().find(|(k, _)| k == f.name || norm_name(f.name) == norm_name(k)).map(|(_, v)| *v);
                let fv = fv.cloned().unwrap_or(Value::I64(0));
                let o = f.offset as usize;
                write_plain(ty(f.ty), &fv, &mut buf[o..]).map_err(|e| format!("{}.{}: {e}", r.name, f.name))?;
            }
        }
        other => return Err(format!("not a plain type: {other:?}")),
    }
    Ok(())
}

/// Reads a plain value written by WinRT into `buf`.
pub fn read_plain(t: Ty, buf: &[u8]) -> Value {
    macro_rules! get {
        ($t:ty) => {{
            const N: usize = std::mem::size_of::<$t>();
            let mut a = [0u8; N];
            a.copy_from_slice(&buf[..N]);
            <$t>::from_le_bytes(a)
        }};
    }
    match t {
        Ty::Bool => Value::Bool(buf[0] != 0),
        Ty::Char16 => Value::Str(String::from_utf16_lossy(&[get!(u16)])),
        Ty::I8 => Value::I64(get!(i8) as i64),
        Ty::U8 => Value::I64(get!(u8) as i64),
        Ty::I16 => Value::I64(get!(i16) as i64),
        Ty::U16 => Value::I64(get!(u16) as i64),
        Ty::I32 => Value::I64(get!(i32) as i64),
        Ty::U32 => Value::I64(get!(u32) as i64),
        Ty::I64 => Value::I64(get!(i64)),
        Ty::U64 => {
            let u = get!(u64);
            if u > i64::MAX as u64 { Value::U64(u) } else { Value::I64(u as i64) }
        }
        Ty::F32 => Value::F64(get!(f32) as f64),
        Ty::F64 => Value::F64(get!(f64)),
        Ty::Enum(e) => {
            let e = enum_t(e);
            let x = if e.unsigned { get!(u32) as i64 } else { get!(i32) as i64 };
            enum_out(e, x)
        }
        Ty::DateTime => Value::DateTime((get!(i64) - EPOCH_DELTA) as f64 / 10_000.0),
        Ty::TimeSpan => Value::F64(get!(i64) as f64 / 10_000.0),
        Ty::Guid => {
            let mut a = [0u8; 16];
            a.copy_from_slice(&buf[..16]);
            Value::Str(guid_text(&a))
        }
        Ty::Record(r) => {
            let r: &RecordT = &RECORDS[r as usize];
            Value::Map(r.fields.iter().map(|f| (f.name.to_string(), read_plain(ty(f.ty), &buf[f.offset as usize..]))).collect())
        }
        other => Value::Str(format!("<{other:?}>")),
    }
}

/// Whether `t` is converted by [`write_plain`] / [`read_plain`].
pub fn is_plain(t: Ty) -> bool {
    !matches!(
        t,
        Ty::Void
            | Ty::Str
            | Ty::Variant
            | Ty::Object { .. }
            | Ty::Widget { .. }
            | Ty::Nullable { .. }
            | Ty::View { .. }
            | Ty::Iterable { .. }
            | Ty::MapView { .. }
            | Ty::Pair { .. }
            | Ty::Async { .. }
    )
}

/// The argument word of a plain input value; `keep` receives the copy a large struct is passed by pointer to.
pub fn plain_word(t: Ty, v: &Value, keep: &mut Vec<Vec<u64>>) -> R<Word> {
    match t {
        Ty::F32 => return Ok(Word::F32(to_f64(v)? as f32)),
        Ty::F64 => return Ok(Word::F64(to_f64(v)?)),
        _ => {}
    }
    let (size, _) = size_align(t);
    let mut buf = vec![0u64; size.div_ceil(8).max(1)];
    {
        // SAFETY: a u64 buffer viewed as bytes, within its length.
        let bytes = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, buf.len() * 8) };
        write_plain(t, v, bytes)?;
    }
    if matches!(t, Ty::Record(_) | Ty::Guid) && !struct_in_register(size) {
        let p = buf.as_ptr();
        keep.push(buf);
        return Ok(Word::ptr(p));
    }
    Ok(Word::I(buf[0]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tables::*;

    fn rec_ty(name: &str) -> Ty {
        let i = RECORDS.iter().position(|r| r.name == name).unwrap_or_else(|| panic!("no record {name}"));
        Ty::Record(i as u16)
    }

    fn enum_ty(name: &str) -> &'static EnumT {
        ENUMS.iter().find(|e| e.name == name).unwrap_or_else(|| panic!("no enum {name}"))
    }

    #[test]
    fn records_round_trip_by_field_name() {
        let t = rec_ty("Windows.UI.Color");
        let v = Value::Map(vec![("a".into(), Value::I64(255)), ("r".into(), Value::I64(10)), ("g".into(), Value::I64(20)), ("b".into(), Value::I64(30))]);
        let mut buf = [0u8; 8];
        write_plain(t, &v, &mut buf).unwrap();
        assert_eq!(&buf[..4], &[255, 10, 20, 30]);
        assert_eq!(read_plain(t, &buf), v);
        assert!(write_plain(t, &Value::Map(vec![("alpha".into(), Value::I64(1))]), &mut buf).unwrap_err().contains("no field"));
        assert!(write_plain(t, &Value::Map(vec![("a".into(), Value::I64(256))]), &mut buf).is_err());
        let th = rec_ty("Microsoft.UI.Xaml.Thickness");
        assert_eq!(size_align(th), (32, 8));
        let mut keep = Vec::new();
        let w = plain_word(th, &Value::Map(vec![("left".into(), Value::F64(1.0)), ("bottom".into(), Value::F64(4.0))]), &mut keep).unwrap();
        assert!(matches!(w, Word::I(p) if p == keep[0].as_ptr() as u64));
        assert_eq!(f64::from_bits(keep[0][0]), 1.0);
        assert_eq!(f64::from_bits(keep[0][3]), 4.0);
        // An 8-byte struct goes by value.
        let w = plain_word(t, &v, &mut keep).unwrap();
        assert_eq!(w, Word::I(u32::from_le_bytes([255, 10, 20, 30]) as u64));
    }

    #[test]
    fn enums_by_any_spelling() {
        let e = enum_ty("Windows.Storage.CreationCollisionOption");
        assert_eq!(enum_in(e, &Value::Str("replace-existing".into())).unwrap(), 1);
        assert_eq!(enum_in(e, &Value::Str("ReplaceExisting".into())).unwrap(), 1);
        assert_eq!(enum_in(e, &Value::Str("replaceExisting".into())).unwrap(), 1);
        assert_eq!(enum_in(e, &Value::I64(2)).unwrap(), 2);
        assert!(enum_in(e, &Value::Str("nope".into())).unwrap_err().contains("replace-existing"));
        assert_eq!(enum_out(e, 1), Value::Str("replace-existing".into()));
        let f = enum_ty("Windows.Storage.FileAttributes");
        assert!(f.flags);
        let both = enum_in(f, &Value::List(vec![Value::Str("read-only".into()), Value::Str("archive".into())])).unwrap();
        assert_eq!(enum_out(f, both), Value::List(vec![Value::Str("read-only".into()), Value::Str("archive".into())]));
        assert_eq!(enum_out(f, 0), Value::List(vec![Value::Str("normal".into())]));
    }

    #[test]
    fn dates_spans_and_guids() {
        let mut b = [0u8; 16];
        write_plain(Ty::DateTime, &Value::DateTime(1_000.0), &mut b).unwrap();
        assert_eq!(i64::from_le_bytes(b[..8].try_into().unwrap()), EPOCH_DELTA + 10_000_000);
        assert_eq!(read_plain(Ty::DateTime, &b), Value::DateTime(1_000.0));
        write_plain(Ty::TimeSpan, &Value::F64(1.5), &mut b).unwrap();
        assert_eq!(i64::from_le_bytes(b[..8].try_into().unwrap()), 15_000);
        let g = "44a9796f-723e-4fdf-a218-033e75b0c084";
        let bytes = guid_bytes(g).unwrap();
        assert_eq!(&bytes[..4], &0x44a9796fu32.to_le_bytes());
        assert_eq!(guid_text(&bytes), g);
        write_plain(Ty::Char16, &Value::Str("é".into()), &mut b).unwrap();
        assert_eq!(read_plain(Ty::Char16, &b), Value::Str("é".into()));
        assert!(write_plain(Ty::I32, &Value::F64(1.5), &mut b).is_err());
        assert!(write_plain(Ty::U8, &Value::I64(-1), &mut b).is_err());
    }
}
