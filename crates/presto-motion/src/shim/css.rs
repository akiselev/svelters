//! Stand-in for `presto_anim::css` (parsing the CSS values Svelte passes in keyframes): only the
//! computed `transform` the built-in transitions prepend, in `f64`.

use crate::transition::{Transform64, parse_float};

pub type TransformList = Vec<Transform64>;

/// A CSS transform list (`matrix(1, 0, 0, 1, 10, 0)`, `translate(10px, 0) scale(.9)`,
/// `rotate(45deg)`), or `None` for anything else (percentages, 3D functions, other units).
pub fn parse_transform(text: &str) -> Option<TransformList> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let close = rest[open..].find(')')? + open;
        let name = rest[..open].trim();
        let args: Vec<&str> = rest[open + 1..close].split(|c: char| c == ',' || c.is_whitespace()).filter(|a| !a.is_empty()).collect();
        let len = |a: &str| -> Option<f64> {
            let v = parse_float(a);
            let unit = a.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E');
            (!v.is_nan() && (unit.is_empty() || unit == "px")).then_some(v)
        };
        let num = |a: &str| -> Option<f64> { a.parse::<f64>().ok() };
        let angle = |a: &str| -> Option<f64> {
            let v = parse_float(a);
            if v.is_nan() {
                return None;
            }
            let unit = a.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == '+');
            Some(match unit {
                "deg" => v.to_radians(),
                "rad" => v,
                "turn" => v * std::f64::consts::TAU,
                "grad" => v * std::f64::consts::PI / 200.0,
                "" if v == 0.0 => 0.0,
                _ => return None,
            })
        };
        let f = match (name, args.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => Transform64::Matrix([num(a)?, num(b)?, num(c)?, num(d)?, num(e)?, num(f)?]),
            ("translate", [x]) => Transform64::Translate(len(x)?, 0.0),
            ("translate", [x, y]) => Transform64::Translate(len(x)?, len(y)?),
            ("translateX", [x]) => Transform64::Translate(len(x)?, 0.0),
            ("translateY", [y]) => Transform64::Translate(0.0, len(y)?),
            ("scale", [s]) => Transform64::Scale(num(s)?, num(s)?),
            ("scale", [x, y]) => Transform64::Scale(num(x)?, num(y)?),
            ("scaleX", [x]) => Transform64::Scale(num(x)?, 1.0),
            ("scaleY", [y]) => Transform64::Scale(1.0, num(y)?),
            ("rotate", [a]) => Transform64::Rotate(angle(a)?),
            ("skew", [x]) => Transform64::Skew(angle(x)?, 0.0),
            ("skew", [x, y]) => Transform64::Skew(angle(x)?, angle(y)?),
            ("skewX", [x]) => Transform64::Skew(angle(x)?, 0.0),
            ("skewY", [y]) => Transform64::Skew(0.0, angle(y)?),
            _ => return None,
        };
        out.push(f);
        rest = rest[close + 1..].trim_start();
    }
    Some(out)
}
