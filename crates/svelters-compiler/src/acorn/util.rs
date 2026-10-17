//! Port of acorn 8.19.0 `src/util.js` (+ JS runtime helpers acorn relies on implicitly).
//!
//! SVELTERS: `codePointToString` builds UTF-16 strings in acorn; Rust strings are UTF-8, so
//! `JsStr` collects code points and pairs surrogate escapes (`"😀"`). A lone
//! surrogate cannot be represented and becomes U+FFFD (recorded in `lone_surrogate`).
//! `js_number_to_string` is `Number.prototype.toString()`, `bigint_to_decimal` is
//! `BigInt(str).toString()`.

#[derive(Default)]
pub struct JsStr {
    pub out: String,
    pending_high: Option<u32>,
    pub lone_surrogate: bool,
}

impl JsStr {
    pub fn new() -> JsStr {
        JsStr::default()
    }

    fn flush(&mut self) {
        if self.pending_high.take().is_some() {
            self.out.push('\u{fffd}');
            self.lone_surrogate = true;
        }
    }

    pub fn push_str(&mut self, s: &str) {
        if !s.is_empty() {
            self.flush();
            self.out.push_str(s);
        }
    }

    /// `codePointToString(code)` / `String.fromCharCode(code)` appended.
    pub fn push_code(&mut self, code: u32) {
        if (0xD800..=0xDBFF).contains(&code) {
            self.flush();
            self.pending_high = Some(code);
            return;
        }
        if (0xDC00..=0xDFFF).contains(&code) {
            if let Some(hi) = self.pending_high.take() {
                let c = 0x10000 + ((hi - 0xD800) << 10) + (code - 0xDC00);
                self.out.push(char::from_u32(c).unwrap_or('\u{fffd}'));
            } else {
                self.out.push('\u{fffd}');
                self.lone_surrogate = true;
            }
            return;
        }
        self.flush();
        self.out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
    }

    pub fn finish(mut self) -> (String, bool) {
        self.flush();
        (self.out, self.lone_surrogate)
    }
}

/// `codePointToString(code)` for a code point that is not a surrogate.
pub fn code_point_to_string(code: u32) -> String {
    let mut s = JsStr::new();
    s.push_code(code);
    s.finish().0
}

/// JS `Number.prototype.toString()` (radix 10).
pub fn js_number_to_string(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v < 0.0 {
        return format!("-{}", js_number_to_string(-v));
    }
    // Shortest round-trip digits.
    let e = format!("{:e}", v);
    let (mantissa, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    if k <= n && n <= 21 {
        let mut s = digits;
        for _ in 0..(n - k) {
            s.push('0');
        }
        s
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        let mut s = String::from("0.");
        for _ in 0..(-n) {
            s.push('0');
        }
        s.push_str(&digits);
        s
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{}e{}{}", digits, sign, e.abs())
        } else {
            format!("{}.{}e{}{}", &digits[..1], &digits[1..], sign, e.abs())
        }
    }
}

/// `BigInt(str.replace(/_/g, "")).toString()` for a numeric literal body (decimal, `0x`, `0o`,
/// `0b`), without the trailing `n`.
pub fn bigint_to_decimal(raw: &str) -> String {
    let s: String = raw.chars().filter(|c| *c != '_').collect();
    let (radix, digits) = if s.len() > 2 && s.as_bytes()[0] == b'0' {
        match s.as_bytes()[1] {
            b'x' | b'X' => (16, &s[2..]),
            b'o' | b'O' => (8, &s[2..]),
            b'b' | b'B' => (2, &s[2..]),
            _ => (10, &s[..]),
        }
    } else {
        (10, &s[..])
    };
    // Little-endian base 1e9 limbs.
    let mut limbs: Vec<u64> = vec![0];
    for c in digits.chars() {
        let d = c.to_digit(radix).unwrap_or(0) as u64;
        let mut carry = d;
        for l in limbs.iter_mut() {
            let v = *l * radix as u64 + carry;
            *l = v % 1_000_000_000;
            carry = v / 1_000_000_000;
        }
        while carry > 0 {
            limbs.push(carry % 1_000_000_000);
            carry /= 1_000_000_000;
        }
    }
    let mut out = String::new();
    for (i, l) in limbs.iter().rev().enumerate() {
        if i == 0 {
            out.push_str(&l.to_string());
        } else {
            out.push_str(&format!("{:09}", l));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_to_string() {
        assert_eq!(js_number_to_string(1.0), "1");
        assert_eq!(js_number_to_string(1.5), "1.5");
        assert_eq!(js_number_to_string(1e21), "1e+21");
        assert_eq!(js_number_to_string(1e20), "100000000000000000000");
        assert_eq!(js_number_to_string(0.000001), "0.000001");
        assert_eq!(js_number_to_string(0.0000001), "1e-7");
        assert_eq!(js_number_to_string(123.456e-10), "1.23456e-8");
    }

    #[test]
    fn bigint() {
        assert_eq!(bigint_to_decimal("0xFF"), "255");
        assert_eq!(bigint_to_decimal("1_000"), "1000");
        assert_eq!(bigint_to_decimal("0"), "0");
        assert_eq!(bigint_to_decimal("0xFFFFFFFFFFFFFFFFFFFF"), "1208925819614629174706175");
    }
}
