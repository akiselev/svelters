//! Computed values and their parsers: lengths with `calc()`/`min()`/`max()`/`clamp()`, colours
//! (CSS Color 4 through `cssparser-color`), keywords, numbers and insets.

use crate::table::{Prop, Table, ValueType};
use cssparser::{ParseError, Parser, Token};
use std::fmt;

/// A length: `px` in the toolkit's unit plus `percent` of the reference box (which the layout
/// resolves). `calc(100% - 8px)` is `{ px: -8, percent: 100 }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Length {
    pub px: f32,
    pub percent: f32,
}

impl Length {
    pub const fn px(px: f32) -> Length {
        Length { px, percent: 0.0 }
    }

    pub const fn percent(percent: f32) -> Length {
        Length { px: 0.0, percent }
    }

    /// The length against a reference size.
    pub fn resolve(self, reference: f32) -> f32 {
        self.px + self.percent / 100.0 * reference
    }
}

/// An sRGB colour, components and alpha in 0–1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn rgba8(self) -> [u8; 4] {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [c(self.r), c(self.g), c(self.b), c(self.a)]
    }
}

/// A property's computed value, typed by its table entry's [`ValueType`].
#[derive(Clone, Debug, PartialEq)]
pub enum ComputedValue {
    Length(Length),
    /// `auto` for a length.
    Auto,
    Number(f32),
    Int(i32),
    Bool(bool),
    String(Box<str>),
    /// One of the property's [`Prop::values`].
    Keyword(&'static str),
    Color(Color),
    /// Top, right, bottom, left.
    Insets([Length; 4]),
    /// 1–1000.
    FontWeight(u16),
    Raw(Box<str>),
}

impl fmt::Display for ComputedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn len(f: &mut fmt::Formatter<'_>, l: &Length) -> fmt::Result {
            match (l.px != 0.0, l.percent != 0.0) {
                (_, false) => write!(f, "{}px", l.px),
                (false, true) => write!(f, "{}%", l.percent),
                (true, true) => write!(f, "calc({}% + {}px)", l.percent, l.px),
            }
        }
        match self {
            ComputedValue::Length(l) => len(f, l),
            ComputedValue::Auto => f.write_str("auto"),
            ComputedValue::Number(n) => write!(f, "{n}"),
            ComputedValue::Int(n) => write!(f, "{n}"),
            ComputedValue::Bool(b) => write!(f, "{b}"),
            ComputedValue::String(s) => write!(f, "{s:?}"),
            ComputedValue::Keyword(k) => f.write_str(k),
            ComputedValue::Color(c) => {
                let [r, g, b, _] = c.rgba8();
                write!(f, "rgba({r}, {g}, {b}, {})", c.a)
            }
            ComputedValue::Insets(s) => {
                for (i, l) in s.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    len(f, l)?;
                }
                Ok(())
            }
            ComputedValue::FontWeight(w) => write!(f, "{w}"),
            ComputedValue::Raw(s) => f.write_str(s),
        }
    }
}

/// What a value needs from the element being computed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValueCx<'a> {
    pub table: &'a Table,
    /// `1em` in px: the element's font size (its parent's, for `font-size` itself).
    pub em: f32,
    pub rem: f32,
    /// The element's `color`, for `currentcolor`.
    pub current_color: Option<Color>,
    /// The parent's font weight, for `bolder`/`lighter`.
    pub parent_weight: u16,
    /// When set, `%` resolves against it into px (`font-size: 120%`).
    pub percent_base: Option<f32>,
}

pub(crate) type VResult<T> = Result<T, String>;

/// Parses a declaration value (after `var()` substitution) as `prop`'s type.
pub(crate) fn parse_value(prop: &'static Prop, text: &str, cx: &ValueCx<'_>) -> VResult<ComputedValue> {
    let text = text.trim();
    if prop.ty == ValueType::Raw {
        return Ok(ComputedValue::Raw(text.into()));
    }
    let mut input = Parser::new(text);
    let v = parse_typed(prop, &mut input, cx)?;
    if !input.is_exhausted() {
        return Err(format!("{}: unexpected input after the value in '{text}'", prop.name));
    }
    Ok(v)
}

fn parse_typed<'i>(prop: &'static Prop, input: &mut Parser<'i>, cx: &ValueCx<'_>) -> VResult<ComputedValue> {
    let name = prop.name;
    match prop.ty {
        ValueType::Length => {
            if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
                return Ok(ComputedValue::Auto);
            }
            length(input, cx).map(ComputedValue::Length).map_err(|e| format!("{name}: {e}"))
        }
        ValueType::Number => number(input, cx).map(ComputedValue::Number).map_err(|e| format!("{name}: {e}")),
        ValueType::Int => {
            let n = number(input, cx).map_err(|e| format!("{name}: {e}"))?;
            if n.fract() != 0.0 {
                return Err(format!("{name}: {n} is not an integer"));
            }
            Ok(ComputedValue::Int(n as i32))
        }
        ValueType::Bool => match ident(input)?.as_str() {
            "true" => Ok(ComputedValue::Bool(true)),
            "false" => Ok(ComputedValue::Bool(false)),
            other => Err(format!("{name}: '{other}' is not true or false")),
        },
        ValueType::Enum => {
            let k = ident(input)?;
            prop.values
                .iter()
                .find(|v| v.eq_ignore_ascii_case(&k))
                .map(|v| ComputedValue::Keyword(v))
                .ok_or_else(|| format!("{name}: '{k}' is not one of {}", prop.values.join(", ")))
        }
        ValueType::String => {
            if let Ok(s) = input.try_parse(|i| i.expect_string_cloned()) {
                return Ok(ComputedValue::String(s.as_ref().into()));
            }
            let mut words = Vec::new();
            while let Ok(w) = input.try_parse(|i| i.expect_ident_cloned()) {
                words.push(w.as_ref().to_string());
            }
            if words.is_empty() {
                return Err(format!("{name}: expected a string"));
            }
            Ok(ComputedValue::String(words.join(" ").into()))
        }
        ValueType::Color => color(input, cx).map(ComputedValue::Color).map_err(|e| format!("{name}: {e}")),
        ValueType::Insets => {
            let mut sides = Vec::new();
            while !input.is_exhausted() && sides.len() < 4 {
                sides.push(length(input, cx).map_err(|e| format!("{name}: {e}"))?);
            }
            let [t, r, b, l] = match sides[..] {
                [a] => [a, a, a, a],
                [a, b] => [a, b, a, b],
                [a, b, c] => [a, b, c, b],
                [a, b, c, d] => [a, b, c, d],
                _ => return Err(format!("{name}: expected 1 to 4 lengths (top right bottom left)")),
            };
            Ok(ComputedValue::Insets([t, r, b, l]))
        }
        ValueType::FontWeight => {
            if let Ok(k) = input.try_parse(|i| i.expect_ident_cloned()) {
                let p = cx.parent_weight;
                let w = match k.to_ascii_lowercase().as_str() {
                    "normal" => 400,
                    "bold" => 700,
                    // CSS Fonts 4 §2.2.1, the relative-weight table.
                    "bolder" => match p {
                        0..350 => 400,
                        350..550 => 700,
                        _ => 900,
                    },
                    "lighter" => match p {
                        0..550 => 100,
                        550..750 => 400,
                        _ => 700,
                    },
                    other => return Err(format!("{name}: '{other}' is not a font weight")),
                };
                return Ok(ComputedValue::FontWeight(w));
            }
            let n = number(input, cx).map_err(|e| format!("{name}: {e}"))?;
            if !(1.0..=1000.0).contains(&n) {
                return Err(format!("{name}: {n} is outside 1 to 1000"));
            }
            Ok(ComputedValue::FontWeight(n.round() as u16))
        }
        ValueType::Raw => unreachable!("handled by parse_value"),
    }
}

fn ident(input: &mut Parser<'_>) -> VResult<String> {
    input.expect_ident_cloned().map(|s| s.as_ref().to_ascii_lowercase()).map_err(|_| "expected a keyword".to_string())
}

// ---- lengths, numbers and math functions ----

/// A linear combination produced by math functions: `px` + `percent`%, or a plain number.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Lin {
    px: f32,
    percent: f32,
    num: f32,
    /// Whether this is a length (`px`/`percent`) or a number (`num`).
    is_len: bool,
}

impl Lin {
    fn number(n: f32) -> Lin {
        Lin { num: n, ..Default::default() }
    }
    fn len(px: f32, percent: f32) -> Lin {
        Lin { px, percent, num: 0.0, is_len: true }
    }
    fn scale(self, k: f32) -> Lin {
        Lin { px: self.px * k, percent: self.percent * k, num: self.num * k, is_len: self.is_len }
    }
}

fn length(input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<Length> {
    let lin = term(input, cx, true)?;
    if !lin.is_len {
        return Err("expected a length".into());
    }
    Ok(Length { px: lin.px, percent: lin.percent })
}

fn number(input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<f32> {
    let lin = term(input, cx, false)?;
    if lin.is_len {
        return Err("expected a number, not a length".into());
    }
    Ok(lin.num)
}

/// One value outside a math function. A unitless number is a length in the toolkit's unit where a
/// length is expected (ST1's setter grammar), except inside `calc()`.
fn term(input: &mut Parser<'_>, cx: &ValueCx<'_>, want_len: bool) -> VResult<Lin> {
    let loc = input.state();
    let t = input.next().map_err(|_| "expected a value".to_string())?.clone();
    match t {
        Token::Number { value, .. } if want_len => Ok(Lin::len(value, 0.0)),
        Token::Function(ref f) if is_math(f) => {
            let f = f.to_ascii_lowercase();
            input.parse_nested_block(|i| math_fn(&f, i, cx).map_err(|e| ParseError::<String>::custom(e))).map_err(err_text)
        }
        _ => {
            input.reset(&loc);
            leaf(input, cx)
        }
    }
}

fn is_math(name: &str) -> bool {
    ["calc", "min", "max", "clamp"].iter().any(|f| name.eq_ignore_ascii_case(f))
}

fn err_text(e: cssparser::ParseError<String>) -> String {
    match e.kind {
        cssparser::ParseErrorKind::Custom(s) => s,
        cssparser::ParseErrorKind::Basic(b) => format!("{b:?}"),
    }
}

fn math_fn(name: &str, input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<Lin> {
    match name {
        "calc" => sum(input, cx),
        "min" | "max" => {
            let args = input
                .parse_comma_separated(|i| sum(i, cx).map_err(|e| ParseError::<String>::custom(e)))
                .map_err(err_text)?;
            pick(&args, name == "min")
        }
        "clamp" => {
            let args = input
                .parse_comma_separated(|i| sum(i, cx).map_err(|e| ParseError::<String>::custom(e)))
                .map_err(err_text)?;
            let [lo, v, hi] = args[..] else { return Err("clamp() takes three arguments".into()) };
            let v = pick(&[lo, v], false)?;
            pick(&[v, hi], true)
        }
        _ => unreachable!(),
    }
}

/// `min()`/`max()` of comparable values. Percentages can't be compared before layout, so a mix of
/// `%` and px is an error rather than a guess.
fn pick(args: &[Lin], min: bool) -> VResult<Lin> {
    let first = *args.first().ok_or("min()/max() need an argument")?;
    if args.iter().any(|a| a.is_len != first.is_len) {
        return Err("min()/max()/clamp() mix numbers and lengths".into());
    }
    let only_px = args.iter().all(|a| a.percent == 0.0);
    let only_pct = args.iter().all(|a| a.px == 0.0);
    if first.is_len && !only_px && !only_pct {
        return Err("min()/max()/clamp() can't compare % with px before layout".into());
    }
    let key = |a: &Lin| if !a.is_len { a.num } else if only_px { a.px } else { a.percent };
    let mut best = first;
    for a in &args[1..] {
        if (min && key(a) < key(&best)) || (!min && key(a) > key(&best)) {
            best = *a;
        }
    }
    Ok(best)
}

fn sum(input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<Lin> {
    let mut acc = product(input, cx)?;
    loop {
        let sign = match input.try_parse(|i| match i.next() {
            Ok(Token::Delim('+')) => Ok(1.0),
            Ok(Token::Delim('-')) => Ok(-1.0),
            _ => Err(()),
        }) {
            Ok(s) => s,
            Err(_) => break,
        };
        let rhs = product(input, cx)?;
        if rhs.is_len != acc.is_len {
            return Err("calc() adds a number to a length".into());
        }
        acc = Lin { px: acc.px + sign * rhs.px, percent: acc.percent + sign * rhs.percent, num: acc.num + sign * rhs.num, is_len: acc.is_len };
    }
    if !input.is_exhausted() {
        return Err("calc(): expected an operator (+ and - need spaces around them)".into());
    }
    Ok(acc)
}

fn product(input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<Lin> {
    let mut acc = math_leaf(input, cx)?;
    loop {
        let op = match input.try_parse(|i| match i.next() {
            Ok(Token::Delim('*')) => Ok('*'),
            Ok(Token::Delim('/')) => Ok('/'),
            _ => Err(()),
        }) {
            Ok(op) => op,
            Err(_) => break,
        };
        let rhs = math_leaf(input, cx)?;
        acc = match op {
            '*' if !rhs.is_len => acc.scale(rhs.num),
            '*' if !acc.is_len => rhs.scale(acc.num),
            '*' => return Err("calc() multiplies two lengths".into()),
            _ if rhs.is_len => return Err("calc() divides by a length".into()),
            _ if rhs.num == 0.0 => return Err("calc() divides by zero".into()),
            _ => acc.scale(1.0 / rhs.num),
        };
    }
    Ok(acc)
}

fn math_leaf(input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<Lin> {
    let state = input.state();
    match input.next().map_err(|_| "calc(): expected a value".to_string())?.clone() {
        Token::ParenthesisBlock => input.parse_nested_block(|i| sum(i, cx).map_err(|e| ParseError::<String>::custom(e))).map_err(err_text),
        Token::Function(ref f) if is_math(f) => {
            let f = f.to_ascii_lowercase();
            input.parse_nested_block(|i| math_fn(&f, i, cx).map_err(|e| ParseError::<String>::custom(e))).map_err(err_text)
        }
        Token::Ident(ref k) if k.eq_ignore_ascii_case("pi") => Ok(Lin::number(std::f32::consts::PI)),
        Token::Ident(ref k) if k.eq_ignore_ascii_case("e") => Ok(Lin::number(std::f32::consts::E)),
        _ => {
            input.reset(&state);
            leaf(input, cx)
        }
    }
}

/// A number, percentage or dimension.
fn leaf(input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<Lin> {
    match input.next().map_err(|_| "expected a value".to_string())?.clone() {
        Token::Number { value, .. } => Ok(Lin::number(value)),
        Token::Percentage { unit_value, .. } => {
            let pct = unit_value * 100.0;
            Ok(match cx.percent_base {
                Some(base) => Lin::len(pct / 100.0 * base, 0.0),
                None => Lin::len(0.0, pct),
            })
        }
        Token::Dimension { value, ref unit, .. } => {
            let u = unit.to_ascii_lowercase();
            let px = match u.as_str() {
                "em" => value * cx.em,
                "rem" => value * cx.rem,
                _ => match cx.table.unit_scale(&u) {
                    Some(scale) => value * scale,
                    None => {
                        let known: Vec<&str> = cx.table.units.iter().map(|(u, _)| *u).collect();
                        return Err(format!("unit '{unit}' is not one of {}, em, rem or %", known.join(", ")));
                    }
                },
            };
            Ok(Lin::len(px, 0.0))
        }
        t => Err(format!("unexpected {}", crate::selector::token_name(&t))),
    }
}

// ---- colours ----

fn color(input: &mut Parser<'_>, cx: &ValueCx<'_>) -> VResult<Color> {
    use cssparser_color::Color as C;
    let parsed = C::parse(input).map_err(|_| "not a colour".to_string())?;
    let opt = |v: Option<f32>| v.unwrap_or(0.0);
    let alpha = |a: Option<f32>| a.unwrap_or(1.0).clamp(0.0, 1.0);
    let srgb = |(r, g, b): (f32, f32, f32), a: f32| Color { r, g, b, a };
    Ok(match parsed {
        C::CurrentColor => cx.current_color.ok_or("currentcolor needs a `color` property on this backend")?,
        C::Rgba(c) => Color { r: c.red as f32 / 255.0, g: c.green as f32 / 255.0, b: c.blue as f32 / 255.0, a: c.alpha.clamp(0.0, 1.0) },
        C::Hsl(c) => srgb(cssparser_color::hsl_to_rgb(hue01(c.hue), opt(c.saturation), opt(c.lightness)), alpha(c.alpha)),
        C::Hwb(c) => srgb(cssparser_color::hwb_to_rgb(hue01(c.hue), opt(c.whiteness), opt(c.blackness)), alpha(c.alpha)),
        C::Oklab(c) => srgb(oklab_to_srgb(opt(c.lightness), opt(c.a), opt(c.b)), alpha(c.alpha)),
        C::Oklch(c) => {
            let (a, b) = polar(opt(c.chroma), opt(c.hue));
            srgb(oklab_to_srgb(opt(c.lightness), a, b), alpha(c.alpha))
        }
        C::Lab(c) => srgb(lab_to_srgb(opt(c.lightness), opt(c.a), opt(c.b)), alpha(c.alpha)),
        C::Lch(c) => {
            let (a, b) = polar(opt(c.chroma), opt(c.hue));
            srgb(lab_to_srgb(opt(c.lightness), a, b), alpha(c.alpha))
        }
        C::ColorFunction(f) => {
            use cssparser::color::PredefinedColorSpace as S;
            let (r, g, b) = (opt(f.c1), opt(f.c2), opt(f.c3));
            let rgb = match f.color_space {
                S::Srgb => (r, g, b),
                S::SrgbLinear => (gamma(r), gamma(g), gamma(b)),
                other => return Err(format!("color(): the {other:?} colour space is not supported")),
            };
            srgb(rgb, alpha(f.alpha))
        }
    })
}

fn hue01(h: Option<f32>) -> f32 {
    h.unwrap_or(0.0).rem_euclid(360.0) / 360.0
}

fn polar(chroma: f32, hue_deg: f32) -> (f32, f32) {
    let h = hue_deg.to_radians();
    (chroma * h.cos(), chroma * h.sin())
}

/// Linear-light sRGB → gamma-encoded sRGB.
fn gamma(c: f32) -> f32 {
    if c.abs() <= 0.0031308 { 12.92 * c } else { c.signum() * (1.055 * c.abs().powf(1.0 / 2.4) - 0.055) }
}

/// CSS Color 4 §17 (the OKLab matrices), into gamma-encoded sRGB.
fn oklab_to_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let bb = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    (gamma(r), gamma(g), gamma(bb))
}

/// CIE Lab (D50) → XYZ → Bradford D50→D65 → linear sRGB → sRGB (CSS Color 4 §17).
fn lab_to_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    const K: f32 = 24389.0 / 27.0;
    const E: f32 = 216.0 / 24389.0;
    let f1 = (l + 16.0) / 116.0;
    let f0 = a / 500.0 + f1;
    let f2 = f1 - b / 200.0;
    let x = if f0.powi(3) > E { f0.powi(3) } else { (116.0 * f0 - 16.0) / K };
    let y = if l > K * E { ((l + 16.0) / 116.0).powi(3) } else { l / K };
    let z = if f2.powi(3) > E { f2.powi(3) } else { (116.0 * f2 - 16.0) / K };
    let (x, y, z) = (x * 0.3457 / 0.3585, y, z * (1.0 - 0.3457 - 0.3585) / 0.3585);
    let (x, y, z) = (
        0.955_473_4 * x - 0.023_098_538 * y + 0.063_259_31 * z,
        -0.028_369_706 * x + 1.009_995_5 * y + 0.021_041_399 * z,
        0.012_314_002 * x - 0.020_507_696 * y + 1.330_365_9 * z,
    );
    let r = 3.240_97 * x - 1.537_383_2 * y - 0.498_610_76 * z;
    let g = -0.969_243_6 * x + 1.875_967_5 * y + 0.041_555_06 * z;
    let bb = 0.055_630_08 * x - 0.203_976_96 * y + 1.056_971_5 * z;
    (gamma(r), gamma(g), gamma(bb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{Dialect, Via};

    const fn p(name: &'static str, ty: ValueType, values: &'static [&'static str]) -> Prop {
        Prop { name, via: Via::Setter, target: name, ty, values, tags: &[], inline_only: false }
    }
    static LEN: Prop = p("width", ValueType::Length, &[]);
    static NUM: Prop = p("flex-grow", ValueType::Number, &[]);
    static INT: Prop = p("row", ValueType::Int, &[]);
    static COL: Prop = p("color", ValueType::Color, &[]);
    static INS: Prop = p("padding", ValueType::Insets, &[]);
    static DIR: Prop = p("flex-direction", ValueType::Enum, &["row", "column"]);
    static FW: Prop = p("font-weight", ValueType::FontWeight, &[]);
    static TABLE: Table = Table { toolkit: "t", dialect: Dialect::None, setter_selectors: &[], units: &[("px", 1.0), ("pt", 2.0)], props: &[] };

    fn cx() -> ValueCx<'static> {
        ValueCx { table: &TABLE, em: 10.0, rem: 16.0, current_color: None, parent_weight: 400, percent_base: None }
    }
    fn v(prop: &'static Prop, text: &str) -> VResult<ComputedValue> {
        parse_value(prop, text, &cx())
    }

    #[test]
    fn lengths_and_calc() {
        assert_eq!(v(&LEN, "12px"), Ok(ComputedValue::Length(Length::px(12.0))));
        assert_eq!(v(&LEN, "3pt"), Ok(ComputedValue::Length(Length::px(6.0))));
        assert_eq!(v(&LEN, "7"), Ok(ComputedValue::Length(Length::px(7.0))));
        assert_eq!(v(&LEN, "2em"), Ok(ComputedValue::Length(Length::px(20.0))));
        assert_eq!(v(&LEN, "1rem"), Ok(ComputedValue::Length(Length::px(16.0))));
        assert_eq!(v(&LEN, "auto"), Ok(ComputedValue::Auto));
        assert_eq!(v(&LEN, "calc(100% - 8px)"), Ok(ComputedValue::Length(Length { px: -8.0, percent: 100.0 })));
        assert_eq!(v(&LEN, "calc((2px + 1em) * 2)"), Ok(ComputedValue::Length(Length::px(24.0))));
        assert_eq!(v(&LEN, "calc(10px / 4)"), Ok(ComputedValue::Length(Length::px(2.5))));
        assert_eq!(v(&LEN, "min(10px, 2em, 30px)"), Ok(ComputedValue::Length(Length::px(10.0))));
        assert_eq!(v(&LEN, "clamp(4px, 1px, 8px)"), Ok(ComputedValue::Length(Length::px(4.0))));
        assert_eq!(v(&LEN, "calc(max(1px, 3px) + 1px)"), Ok(ComputedValue::Length(Length::px(4.0))));
        assert!(v(&LEN, "calc(1px + 2)").is_err());
        assert!(v(&LEN, "calc(1px * 2px)").is_err());
        assert!(v(&LEN, "calc(1px / 0)").is_err());
        assert!(v(&LEN, "calc(1px -2px)").is_err());
        assert!(v(&LEN, "min(10%, 2px)").is_err());
        assert!(v(&LEN, "3in").unwrap_err().contains("unit 'in'"));
        assert!(v(&LEN, "1px 2px").is_err());
    }

    #[test]
    fn numbers_keywords_insets() {
        assert_eq!(v(&NUM, "calc(1 / 4)"), Ok(ComputedValue::Number(0.25)));
        assert!(v(&NUM, "2px").is_err());
        assert_eq!(v(&INT, "3"), Ok(ComputedValue::Int(3)));
        assert!(v(&INT, "2.5").is_err());
        assert_eq!(v(&DIR, "Column"), Ok(ComputedValue::Keyword("column")));
        assert!(v(&DIR, "diagonal").unwrap_err().contains("row, column"));
        let px = Length::px;
        assert_eq!(v(&INS, "1px 2px 3px"), Ok(ComputedValue::Insets([px(1.0), px(2.0), px(3.0), px(2.0)])));
        assert_eq!(v(&INS, "4"), Ok(ComputedValue::Insets([px(4.0); 4])));
        assert!(v(&INS, "1px 2px 3px 4px 5px").is_err());
        assert_eq!(v(&FW, "bold"), Ok(ComputedValue::FontWeight(700)));
        assert_eq!(v(&FW, "bolder"), Ok(ComputedValue::FontWeight(700)));
        assert_eq!(v(&FW, "550"), Ok(ComputedValue::FontWeight(550)));
        assert!(v(&FW, "0").is_err());
    }

    #[test]
    fn colours() {
        let c = |t: &str| match v(&COL, t) {
            Ok(ComputedValue::Color(c)) => c.rgba8(),
            other => panic!("{t}: {other:?}"),
        };
        assert_eq!(c("#ff8000"), [255, 128, 0, 255]);
        assert_eq!(c("rgb(0 0 255 / 50%)"), [0, 0, 255, 128]);
        assert_eq!(c("transparent"), [0, 0, 0, 0]);
        assert_eq!(c("rebeccapurple"), [102, 51, 153, 255]);
        assert_eq!(c("hsl(120deg 100% 50%)"), [0, 255, 0, 255]);
        assert_eq!(c("hwb(0 0% 0%)"), [255, 0, 0, 255]);
        assert_eq!(c("oklch(62.8% 0.2577 29.23)"), [255, 0, 0, 255]);
        assert_eq!(c("oklab(1 0 0)"), [255, 255, 255, 255]);
        assert_eq!(c("lab(53.24 80.09 67.2)"), [255, 0, 0, 255]);
        assert_eq!(c("color(srgb 0 1 0)"), [0, 255, 0, 255]);
        assert!(v(&COL, "currentcolor").is_err());
        let with = ValueCx { current_color: Some(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }), ..cx() };
        assert_eq!(parse_value(&COL, "currentColor", &with), Ok(ComputedValue::Color(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 })));
    }
}
