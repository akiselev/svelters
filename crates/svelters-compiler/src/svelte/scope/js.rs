//! JavaScript value semantics for `Evaluation` (`phases/scope.js`). Upstream evaluates constant
//! expressions by running the operators in the JS engine (`binary`, `unary`, `logical` tables,
//! `globals` like `Math.max`); this module implements the same ECMAScript operations on a small
//! value model.
//!
//! SVELTERS deviations (all on inputs where upstream throws a TypeError/RangeError inside the
//! compiler or depends on engine internals):
//! - operations that would need BigInt arithmetic, `in` / `instanceof` on primitives, and
//!   `String.fromCodePoint` out of range produce `UNKNOWN` instead of throwing;
//! - lone UTF-16 surrogates (`String.fromCharCode(0xd800)`) become U+FFFD (Rust strings are UTF-8);
//! - transcendental `Math.*` functions use Rust's libm, which may differ from V8 in the last ulp.

use crate::estree::{BinaryOperator, LogicalOperator, RegExpLiteral, UnaryOperator};
use std::rc::Rc;

/// The symbols upstream puts into an evaluation's value set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sym {
    /// `UNKNOWN`
    Unknown,
    /// `NUMBER` (includes `BigInt`)
    Number,
    /// `STRING`
    String,
    /// `FUNCTION`
    Function,
}

/// A value in an evaluation's value set.
#[derive(Clone, Debug)]
pub enum JsValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    /// Decimal digits (with a leading `-` when negative).
    BigInt(String),
    /// A RegExp object (compared by identity).
    RegExp(Rc<RegExpLiteral>),
    Symbol(Sym),
}

impl JsValue {
    pub fn is_symbol(&self) -> bool {
        matches!(self, JsValue::Symbol(_))
    }
}

/// `SameValueZero`, the equality of `Set`.
pub fn same_value_zero(a: &JsValue, b: &JsValue) -> bool {
    use JsValue::*;
    match (a, b) {
        (Undefined, Undefined) | (Null, Null) => true,
        (Bool(a), Bool(b)) => a == b,
        (Number(a), Number(b)) => (a.is_nan() && b.is_nan()) || a == b,
        (String(a), String(b)) => a == b,
        (BigInt(a), BigInt(b)) => a == b,
        (RegExp(a), RegExp(b)) => Rc::ptr_eq(a, b),
        (Symbol(a), Symbol(b)) => a == b,
        _ => false,
    }
}

/// An insertion-ordered `Set<any>`.
#[derive(Clone, Debug, Default)]
pub struct ValueSet(pub Vec<JsValue>);

impl ValueSet {
    pub fn add(&mut self, v: JsValue) {
        if !self.0.iter().any(|x| same_value_zero(x, &v)) {
            self.0.push(v);
        }
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, JsValue> {
        self.0.iter()
    }
}

const UNKNOWN: JsValue = JsValue::Symbol(Sym::Unknown);

// ---------------------------------------------------------------------------------------------
// abstract operations

pub fn to_boolean(v: &JsValue) -> bool {
    match v {
        JsValue::Undefined | JsValue::Null => false,
        JsValue::Bool(b) => *b,
        JsValue::Number(n) => !(n.is_nan() || *n == 0.0),
        JsValue::String(s) => !s.is_empty(),
        JsValue::BigInt(d) => d.trim_start_matches('-') != "0",
        JsValue::RegExp(_) | JsValue::Symbol(_) => true,
    }
}

pub fn type_of(v: &JsValue) -> &'static str {
    match v {
        JsValue::Undefined => "undefined",
        JsValue::Null | JsValue::RegExp(_) => "object",
        JsValue::Bool(_) => "boolean",
        JsValue::Number(_) => "number",
        JsValue::String(_) => "string",
        JsValue::BigInt(_) => "bigint",
        JsValue::Symbol(_) => "symbol",
    }
}

/// `ToPrimitive` (objects are RegExps, whose `toString` gives `/source/flags`).
fn to_primitive(v: &JsValue) -> JsValue {
    match v {
        JsValue::RegExp(r) => JsValue::String(format!("/{}/{}", r.pattern, r.flags)),
        v => v.clone(),
    }
}

/// `ToNumber`; `None` where JS throws (BigInt, Symbol).
pub fn to_number(v: &JsValue) -> Option<f64> {
    Some(match v {
        JsValue::Undefined => f64::NAN,
        JsValue::Null => 0.0,
        JsValue::Bool(b) => f64::from(u8::from(*b)),
        JsValue::Number(n) => *n,
        JsValue::String(s) => string_to_number(s),
        JsValue::RegExp(_) => return to_number(&to_primitive(v)),
        JsValue::BigInt(_) | JsValue::Symbol(_) => return None,
    })
}

/// `ToString`; `None` for symbols.
pub fn to_js_string(v: &JsValue) -> Option<String> {
    Some(match v {
        JsValue::Undefined => "undefined".into(),
        JsValue::Null => "null".into(),
        JsValue::Bool(b) => b.to_string(),
        JsValue::Number(n) => number_to_string(*n),
        JsValue::String(s) => s.clone(),
        JsValue::BigInt(d) => d.clone(),
        JsValue::RegExp(_) => return to_js_string(&to_primitive(v)),
        JsValue::Symbol(_) => return None,
    })
}

fn to_int32(n: f64) -> i32 {
    to_uint32(n) as i32
}

fn to_uint32(n: f64) -> u32 {
    if !n.is_finite() {
        return 0;
    }
    let t = n.trunc();
    let m = t.rem_euclid(4294967296.0);
    m as u32
}

fn to_uint16(n: f64) -> u16 {
    if !n.is_finite() {
        return 0;
    }
    n.trunc().rem_euclid(65536.0) as u16
}

/// `Number.prototype.toString()` (radix 10).
pub fn number_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x < 0.0 {
        return format!("-{}", number_to_string(-x));
    }
    if x.is_infinite() {
        return "Infinity".into();
    }
    // shortest round-trip digits: `d.ddde±x`
    let e = format!("{x:e}");
    let (mantissa, exp) = e.split_once('e').unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().unwrap() + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let sign = if n - 1 < 0 { '-' } else { '+' };
        let e = (n - 1).abs();
        if k == 1 {
            format!("{digits}e{sign}{e}")
        } else {
            format!("{}.{}e{sign}{e}", &digits[..1], &digits[1..])
        }
    }
}

/// `StrWhiteSpaceChar`: WhiteSpace and LineTerminator.
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\u{b}'
            | '\u{c}'
            | ' '
            | '\u{a0}'
            | '\u{feff}'
            | '\n'
            | '\r'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// Length of the longest prefix of `s` that is a `StrDecimalLiteral` (with optional sign).
fn decimal_prefix_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    if s[i..].starts_with("Infinity") {
        return i + "Infinity".len();
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut mantissa_digits = i - int_start;
    if i < b.len() && b[i] == b'.' {
        let frac_start = i + 1;
        let mut j = frac_start;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if mantissa_digits > 0 || j > frac_start {
            mantissa_digits += j - frac_start;
            i = j;
        }
    }
    if mantissa_digits == 0 {
        return 0;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            i = j;
        }
    }
    i
}

fn parse_decimal(s: &str) -> f64 {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let v = if body == "Infinity" {
        f64::INFINITY
    } else {
        // Rust's float parser is correctly rounded like JS; normalise `5.` / `.5`
        let mut t = body.to_string();
        if t.starts_with('.') {
            t.insert(0, '0');
        }
        t = t.replace(".e", ".0e").replace(".E", ".0E");
        if t.ends_with('.') {
            t.push('0');
        }
        t.parse::<f64>().unwrap_or(f64::NAN)
    };
    if neg { -v } else { v }
}

fn parse_radix_digits(digits: &str, radix: u32) -> f64 {
    let mut acc: u128 = 0;
    let mut overflow = false;
    let mut f = 0.0f64;
    for c in digits.chars() {
        let d = c.to_digit(radix).unwrap();
        if !overflow {
            match acc.checked_mul(radix as u128).and_then(|a| a.checked_add(d as u128)) {
                Some(a) => acc = a,
                None => {
                    overflow = true;
                    f = acc as f64 * radix as f64 + d as f64;
                }
            }
        } else {
            f = f * radix as f64 + d as f64;
        }
    }
    if overflow { f } else { acc as f64 }
}

/// `StringToNumber`.
pub fn string_to_number(s: &str) -> f64 {
    let s = s.trim_matches(is_js_whitespace);
    if s.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(|c| c.is_digit(radix)) {
                return parse_radix_digits(rest, radix);
            }
            return f64::NAN;
        }
    }
    if decimal_prefix_len(s) == s.len() {
        parse_decimal(s)
    } else {
        f64::NAN
    }
}

fn parse_float(s: &str) -> f64 {
    let s = s.trim_start_matches(is_js_whitespace);
    let n = decimal_prefix_len(s);
    if n == 0 { f64::NAN } else { parse_decimal(&s[..n]) }
}

fn parse_int(s: &str, radix: f64) -> f64 {
    let mut s = s.trim_start_matches(is_js_whitespace);
    let mut sign = 1.0;
    if let Some(r) = s.strip_prefix('-') {
        sign = -1.0;
        s = r;
    } else if let Some(r) = s.strip_prefix('+') {
        s = r;
    }
    let mut r = to_int32(radix);
    let mut strip_prefix = true;
    if r != 0 {
        if !(2..=36).contains(&r) {
            return f64::NAN;
        }
        if r != 16 {
            strip_prefix = false;
        }
    } else {
        r = 10;
    }
    if strip_prefix && (s.starts_with("0x") || s.starts_with("0X")) {
        s = &s[2..];
        r = 16;
    }
    let end = s.find(|c: char| !c.is_digit(r as u32)).unwrap_or(s.len());
    let digits = &s[..end];
    if digits.is_empty() {
        return f64::NAN;
    }
    let v = if r == 10 {
        digits.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        parse_radix_digits(digits, r as u32)
    };
    sign * v
}

fn strict_equals(a: &JsValue, b: &JsValue) -> bool {
    match (a, b) {
        (JsValue::Number(a), JsValue::Number(b)) => a == b,
        _ => same_value_zero(a, b),
    }
}

/// `IsLooselyEqual`; `None` where BigInt comparison would be needed.
fn loose_equals(a: &JsValue, b: &JsValue) -> Option<bool> {
    use JsValue::*;
    if std::mem::discriminant(a) == std::mem::discriminant(b) {
        return Some(strict_equals(a, b));
    }
    match (a, b) {
        (Null, Undefined) | (Undefined, Null) => Some(true),
        (BigInt(_), _) | (_, BigInt(_)) => None,
        (Number(_), String(s)) => loose_equals(a, &Number(string_to_number(s))),
        (String(s), Number(_)) => loose_equals(&Number(string_to_number(s)), b),
        (Bool(x), _) => loose_equals(&Number(f64::from(u8::from(*x))), b),
        (_, Bool(y)) => loose_equals(a, &Number(f64::from(u8::from(*y)))),
        (RegExp(_), Number(_) | String(_)) => loose_equals(&to_primitive(a), b),
        (Number(_) | String(_), RegExp(_)) => loose_equals(a, &to_primitive(b)),
        _ => Some(false),
    }
}

/// `IsLessThan(x, y)`: `Some(None)` is upstream's `undefined` (NaN involved).
fn less_than(x: &JsValue, y: &JsValue) -> Option<Option<bool>> {
    let px = to_primitive(x);
    let py = to_primitive(y);
    if let (JsValue::String(a), JsValue::String(b)) = (&px, &py) {
        return Some(Some(a.encode_utf16().lt(b.encode_utf16())));
    }
    let nx = to_number(&px)?;
    let ny = to_number(&py)?;
    if nx.is_nan() || ny.is_nan() {
        return Some(None);
    }
    Some(Some(nx < ny))
}

fn exponentiate(base: f64, exponent: f64) -> f64 {
    if exponent.is_nan() {
        return f64::NAN;
    }
    if exponent == 0.0 {
        return 1.0;
    }
    if base.is_nan() {
        return f64::NAN;
    }
    if base.abs() == 1.0 && exponent.is_infinite() {
        return f64::NAN;
    }
    base.powf(exponent)
}

/// The `binary` table.
pub fn binary(op: BinaryOperator, left: &JsValue, right: &JsValue) -> JsValue {
    use BinaryOperator as B;
    let r = (|| -> Option<JsValue> {
        Some(match op {
            B::NotEq => JsValue::Bool(!loose_equals(left, right)?),
            B::StrictNotEq => JsValue::Bool(!strict_equals(left, right)),
            B::Eq => JsValue::Bool(loose_equals(left, right)?),
            B::StrictEq => JsValue::Bool(strict_equals(left, right)),
            B::Lt => JsValue::Bool(less_than(left, right)?.unwrap_or(false)),
            B::Gt => JsValue::Bool(less_than(right, left)?.unwrap_or(false)),
            B::LtEq => JsValue::Bool(less_than(right, left)?.is_some_and(|r| !r)),
            B::GtEq => JsValue::Bool(less_than(left, right)?.is_some_and(|r| !r)),
            // TypeError for primitive right-hand sides upstream
            B::In | B::Instanceof => return None,
            B::Add => {
                let l = to_primitive(left);
                let r = to_primitive(right);
                if matches!(l, JsValue::String(_)) || matches!(r, JsValue::String(_)) {
                    JsValue::String(to_js_string(&l)? + &to_js_string(&r)?)
                } else {
                    JsValue::Number(to_number(&l)? + to_number(&r)?)
                }
            }
            _ => {
                let a = to_number(left)?;
                let b = to_number(right)?;
                JsValue::Number(match op {
                    B::Rem => a % b,
                    B::Mul => a * b,
                    B::Exp => exponentiate(a, b),
                    B::Sub => a - b,
                    B::Div => a / b,
                    B::BitAnd => f64::from(to_int32(a) & to_int32(b)),
                    B::BitOr => f64::from(to_int32(a) | to_int32(b)),
                    B::BitXor => f64::from(to_int32(a) ^ to_int32(b)),
                    B::Shl => f64::from(to_int32(a).wrapping_shl(to_uint32(b) & 31)),
                    B::Shr => f64::from(to_int32(a) >> (to_uint32(b) & 31)),
                    B::UShr => f64::from(to_uint32(a) >> (to_uint32(b) & 31)),
                    _ => unreachable!(),
                })
            }
        })
    })();
    r.unwrap_or(UNKNOWN)
}

/// The `unary` table.
pub fn unary(op: UnaryOperator, argument: &JsValue) -> JsValue {
    use UnaryOperator as U;
    match op {
        U::Minus => match argument {
            JsValue::BigInt(d) => JsValue::BigInt(match d.strip_prefix('-') {
                Some(p) => p.to_string(),
                None if d == "0" => d.clone(),
                None => format!("-{d}"),
            }),
            v => to_number(v).map_or(UNKNOWN, |n| JsValue::Number(-n)),
        },
        U::Plus => to_number(argument).map_or(UNKNOWN, JsValue::Number),
        U::Not => JsValue::Bool(!to_boolean(argument)),
        U::BitNot => to_number(argument).map_or(UNKNOWN, |n| JsValue::Number(f64::from(!to_int32(n)))),
        U::Typeof => JsValue::String(type_of(argument).to_string()),
        U::Void => JsValue::Undefined,
        U::Delete => JsValue::Bool(true),
    }
}

/// The `logical` table.
pub fn logical(op: LogicalOperator, left: &JsValue, right: &JsValue) -> JsValue {
    match op {
        LogicalOperator::Or => {
            if to_boolean(left) {
                left.clone()
            } else {
                right.clone()
            }
        }
        LogicalOperator::And => {
            if to_boolean(left) {
                right.clone()
            } else {
                left.clone()
            }
        }
        LogicalOperator::Nullish => match left {
            JsValue::Undefined | JsValue::Null => right.clone(),
            _ => left.clone(),
        },
    }
}

// ---------------------------------------------------------------------------------------------
// globals

/// The type half of upstream's `globals` table: `NUMBER` or `STRING`.
pub fn global_type(keypath: &str) -> Option<Sym> {
    Some(match keypath {
        "BigInt"
        | "Math.min"
        | "Math.max"
        | "Math.random"
        | "Math.floor"
        | "Math.f16round"
        | "Math.round"
        | "Math.abs"
        | "Math.acos"
        | "Math.asin"
        | "Math.atan"
        | "Math.atan2"
        | "Math.ceil"
        | "Math.cos"
        | "Math.sin"
        | "Math.tan"
        | "Math.exp"
        | "Math.log"
        | "Math.pow"
        | "Math.sqrt"
        | "Math.clz32"
        | "Math.imul"
        | "Math.sign"
        | "Math.log10"
        | "Math.log2"
        | "Math.log1p"
        | "Math.expm1"
        | "Math.cosh"
        | "Math.sinh"
        | "Math.tanh"
        | "Math.acosh"
        | "Math.asinh"
        | "Math.atanh"
        | "Math.trunc"
        | "Math.fround"
        | "Math.cbrt"
        | "Number"
        | "Number.isInteger"
        | "Number.isFinite"
        | "Number.isNaN"
        | "Number.isSafeInteger"
        | "Number.parseFloat"
        | "Number.parseInt" => Sym::Number,
        "String" | "String.fromCharCode" | "String.fromCodePoint" => Sym::String,
        _ => return None,
    })
}

/// Whether the `globals` entry has a function (`BigInt` and `Math.random` do not).
pub fn global_has_fn(keypath: &str) -> bool {
    global_type(keypath).is_some() && keypath != "BigInt" && keypath != "Math.random"
}

fn f16round(x: f64) -> f64 {
    if x.is_nan() || x.is_infinite() || x == 0.0 {
        return x;
    }
    let a = x.abs();
    let r = if a >= 65520.0 {
        f64::INFINITY
    } else {
        // exponent of the leading bit
        let e = ((a.to_bits() >> 52) & 0x7ff) as i32 - 1023;
        let q = if e < -14 { 2f64.powi(-24) } else { 2f64.powi(e - 10) };
        (a / q).round_ties_even() * q
    };
    if x < 0.0 { -r } else { r }
}

fn math_round(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    if r == 0.0 && x < 0.0 { -0.0 } else { r }
}

fn math_max(args: &[f64]) -> f64 {
    let mut r = f64::NEG_INFINITY;
    for &a in args {
        if a.is_nan() {
            return f64::NAN;
        }
        if a > r || (a == 0.0 && r == 0.0 && r.is_sign_negative() && a.is_sign_positive()) {
            r = a;
        }
    }
    r
}

fn math_min(args: &[f64]) -> f64 {
    let mut r = f64::INFINITY;
    for &a in args {
        if a.is_nan() {
            return f64::NAN;
        }
        if a < r || (a == 0.0 && r == 0.0 && a.is_sign_negative()) {
            r = a;
        }
    }
    r
}

/// Calls the function of upstream's `globals` table entry `keypath` (`fn(...values)`).
/// `None` when it has no function; `UNKNOWN` where JS would throw.
pub fn call_global(keypath: &str, args: &[JsValue]) -> Option<JsValue> {
    if !global_has_fn(keypath) {
        return None;
    }
    let arg = |i: usize| args.get(i).cloned().unwrap_or(JsValue::Undefined);
    let num = |i: usize| to_number(&arg(i));
    let r = (|| -> Option<JsValue> {
        let n1 = |f: fn(f64) -> f64| -> Option<JsValue> { Some(JsValue::Number(f(num(0)?))) };
        Some(match keypath {
            "Math.min" | "Math.max" => {
                let mut v = Vec::new();
                for a in args {
                    v.push(to_number(a)?);
                }
                JsValue::Number(if keypath == "Math.min" {
                    math_min(&v)
                } else {
                    math_max(&v)
                })
            }
            "Math.floor" => n1(f64::floor)?,
            "Math.f16round" => n1(f16round)?,
            "Math.round" => n1(math_round)?,
            "Math.abs" => n1(f64::abs)?,
            "Math.acos" => n1(f64::acos)?,
            "Math.asin" => n1(f64::asin)?,
            "Math.atan" => n1(f64::atan)?,
            "Math.atan2" => JsValue::Number(num(0)?.atan2(num(1)?)),
            "Math.ceil" => n1(f64::ceil)?,
            "Math.cos" => n1(f64::cos)?,
            "Math.sin" => n1(f64::sin)?,
            "Math.tan" => n1(f64::tan)?,
            "Math.exp" => n1(f64::exp)?,
            "Math.log" => n1(f64::ln)?,
            "Math.pow" => JsValue::Number(exponentiate(num(0)?, num(1)?)),
            "Math.sqrt" => n1(f64::sqrt)?,
            "Math.clz32" => JsValue::Number(f64::from(to_uint32(num(0)?).leading_zeros())),
            "Math.imul" => JsValue::Number(f64::from(to_int32(num(0)?).wrapping_mul(to_int32(num(1)?)))),
            "Math.sign" => n1(|x| if x.is_nan() || x == 0.0 { x } else { x.signum() })?,
            "Math.log10" => n1(f64::log10)?,
            "Math.log2" => n1(f64::log2)?,
            "Math.log1p" => n1(f64::ln_1p)?,
            "Math.expm1" => n1(f64::exp_m1)?,
            "Math.cosh" => n1(f64::cosh)?,
            "Math.sinh" => n1(f64::sinh)?,
            "Math.tanh" => n1(f64::tanh)?,
            "Math.acosh" => n1(f64::acosh)?,
            "Math.asinh" => n1(f64::asinh)?,
            "Math.atanh" => n1(f64::atanh)?,
            "Math.trunc" => n1(f64::trunc)?,
            "Math.fround" => n1(|x| x as f32 as f64)?,
            "Math.cbrt" => n1(f64::cbrt)?,
            "Number" => {
                if args.is_empty() {
                    JsValue::Number(0.0)
                } else {
                    JsValue::Number(num(0)?)
                }
            }
            "Number.isInteger" => {
                JsValue::Bool(matches!(arg(0), JsValue::Number(n) if n.is_finite() && n.trunc() == n))
            }
            "Number.isSafeInteger" => JsValue::Bool(
                matches!(arg(0), JsValue::Number(n) if n.is_finite() && n.trunc() == n && n.abs() <= 9007199254740991.0),
            ),
            "Number.isFinite" => JsValue::Bool(matches!(arg(0), JsValue::Number(n) if n.is_finite())),
            "Number.isNaN" => JsValue::Bool(matches!(arg(0), JsValue::Number(n) if n.is_nan())),
            "Number.parseFloat" => JsValue::Number(parse_float(&to_js_string(&arg(0))?)),
            "Number.parseInt" => {
                let radix = match arg(1) {
                    JsValue::Undefined => 0.0,
                    r => to_number(&r)?,
                };
                JsValue::Number(parse_int(&to_js_string(&arg(0))?, radix))
            }
            "String" => {
                if args.is_empty() {
                    JsValue::String(String::new())
                } else {
                    JsValue::String(to_js_string(&arg(0))?)
                }
            }
            "String.fromCharCode" => {
                let mut units = Vec::new();
                for a in args {
                    units.push(to_uint16(to_number(a)?));
                }
                JsValue::String(String::from_utf16_lossy(&units))
            }
            "String.fromCodePoint" => {
                let mut s = String::new();
                for a in args {
                    let n = to_number(a)?;
                    if n.trunc() != n || !(0.0..=1114111.0).contains(&n) {
                        return None; // RangeError
                    }
                    s.push(char::from_u32(n as u32).unwrap_or('\u{fffd}'));
                }
                JsValue::String(s)
            }
            _ => return None,
        })
    })();
    Some(r.unwrap_or(UNKNOWN))
}

/// Upstream's `global_constants`.
pub fn global_constant(keypath: &str) -> Option<f64> {
    use std::f64::consts;
    Some(match keypath {
        "Math.PI" => consts::PI,
        "Math.E" => consts::E,
        "Math.LN10" => consts::LN_10,
        "Math.LN2" => consts::LN_2,
        "Math.LOG10E" => consts::LOG10_E,
        "Math.LOG2E" => consts::LOG2_E,
        "Math.SQRT2" => consts::SQRT_2,
        "Math.SQRT1_2" => consts::FRAC_1_SQRT_2,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formatting() {
        for (v, s) in [
            (1.0, "1"),
            (0.1, "0.1"),
            (1.5e21, "1.5e+21"),
            (1e21, "1e+21"),
            (123456789012345680000.0, "123456789012345680000"),
            (1e-7, "1e-7"),
            (0.000001, "0.000001"),
            (-2.5, "-2.5"),
            (0.1 + 0.2, "0.30000000000000004"),
            (f64::NAN, "NaN"),
            (-0.0, "0"),
        ] {
            assert_eq!(number_to_string(v), s);
        }
    }

    #[test]
    fn string_numbers() {
        assert_eq!(string_to_number(" 12 "), 12.0);
        assert_eq!(string_to_number(""), 0.0);
        assert_eq!(string_to_number("0x1f"), 31.0);
        assert!(string_to_number("inf").is_nan());
        assert!(string_to_number("1_0").is_nan());
        assert_eq!(string_to_number(".5"), 0.5);
        assert_eq!(string_to_number("-Infinity"), f64::NEG_INFINITY);
        assert_eq!(parse_int("  -0x1F", 0.0), -31.0);
        assert_eq!(parse_float("2.5abc"), 2.5);
    }

    #[test]
    fn operators() {
        let n = JsValue::Number;
        let s = |x: &str| JsValue::String(x.into());
        assert!(matches!(binary(BinaryOperator::Add, &n(1.0), &s("a")), JsValue::String(x) if x == "1a"));
        assert!(matches!(
            binary(BinaryOperator::Eq, &n(1.0), &s("1")),
            JsValue::Bool(true)
        ));
        assert!(matches!(
            binary(BinaryOperator::Eq, &JsValue::Null, &n(0.0)),
            JsValue::Bool(false)
        ));
        assert!(matches!(
            binary(BinaryOperator::LtEq, &n(f64::NAN), &n(1.0)),
            JsValue::Bool(false)
        ));
        assert!(matches!(binary(BinaryOperator::UShr, &n(-1.0), &n(0.0)), JsValue::Number(x) if x == 4294967295.0));
        assert!(matches!(binary(BinaryOperator::Exp, &n(1.0), &n(f64::INFINITY)), JsValue::Number(x) if x.is_nan()));
        assert!(matches!(unary(UnaryOperator::Typeof, &JsValue::Null), JsValue::String(x) if x == "object"));
        assert!(
            matches!(call_global("Math.round", &[n(-0.5)]), Some(JsValue::Number(x)) if x == 0.0 && x.is_sign_negative())
        );
        assert!(matches!(call_global("Math.max", &[]), Some(JsValue::Number(x)) if x == f64::NEG_INFINITY));
        assert!(matches!(call_global("Math.f16round", &[n(5.0005)]), Some(JsValue::Number(x)) if x == 5.0));
        assert!(matches!(call_global("Math.f16round", &[n(65520.0)]), Some(JsValue::Number(x)) if x.is_infinite()));
        assert!(call_global("Math.random", &[]).is_none());
    }
}
