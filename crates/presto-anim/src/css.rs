//! The CSS value strings Svelte writes into keyframes (`round2/animation-scroll.md` §1.1–§1.3): a
//! keyframe is `css_to_keyframe(css(t, u))`, a `prop: value; …` string split on `;` and `:`, and the
//! built-in transitions emit numbers (`opacity: 0.5`), lengths (`height: 12px`), transform lists
//! that start with the computed `matrix(…)` (`transform: matrix(…) translate(10px, 0) scale(.9)`),
//! filters (`blur(5px)`) and stroke dashes. Values are parsed once at `animate()` into [`CssValue`]
//! and resolved against the element ([`ResolveCx`]) into presto's [`AnimValue`].

use std::f32::consts::PI;

use presto_base::anim::{AnimProperty, AnimValue, TransformFn};
use presto_base::color::Color;
use presto_base::geom::{Affine, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Px,
    Percent,
    Em,
    Rem,
    Vw,
    Vh,
    Vmin,
    Vmax,
    Pt,
    Pc,
    In,
    Cm,
    Mm,
    Q,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Length {
    pub value: f32,
    pub unit: Unit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CssTransformFn {
    Translate(Length, Length),
    Scale(f32, f32),
    /// Radians.
    Rotate(f32),
    /// Radians.
    Skew(f32, f32),
    Matrix(Affine),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterFn {
    Blur(Length),
    /// `brightness`, `contrast`, `grayscale`, `invert`, `opacity`, `saturate`, `sepia`: an amount
    /// (percentages divided by 100).
    Amount(FilterAmount, f32),
    /// Radians.
    HueRotate(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterAmount {
    Brightness,
    Contrast,
    Grayscale,
    Invert,
    Opacity,
    Saturate,
    Sepia,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CssValue {
    Number(f32),
    Length(Length),
    Color(Color),
    Transform(Vec<CssTransformFn>),
    Filter(Vec<FilterFn>),
    /// Any other single identifier (`hidden`, `auto`, `none` outside transforms and filters).
    Keyword(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssError {
    pub at: usize,
    pub message: &'static str,
}

/// What lengths resolve against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolveCx {
    pub font_size: f32,
    pub root_font_size: f32,
    pub viewport: Size,
    /// The reference for `%` in a plain length (the containing block's width or height, per
    /// property); translate percentages use `box_size`.
    pub percent_base: f32,
    /// The element's border box (translate `%`).
    pub box_size: Size,
}

impl Default for ResolveCx {
    fn default() -> Self {
        ResolveCx { font_size: 16.0, root_font_size: 16.0, viewport: Size::new(0.0, 0.0), percent_base: 0.0, box_size: Size::new(0.0, 0.0) }
    }
}

impl Length {
    pub const fn px(v: f32) -> Self {
        Length { value: v, unit: Unit::Px }
    }

    /// In px; `percent_base` stands for 100%. Absolute units use CSS's 96 px per inch.
    pub fn resolve(&self, cx: &ResolveCx, percent_base: f32) -> f32 {
        let v = self.value;
        match self.unit {
            Unit::Px => v,
            Unit::Percent => v / 100.0 * percent_base,
            Unit::Em => v * cx.font_size,
            Unit::Rem => v * cx.root_font_size,
            Unit::Vw => v / 100.0 * cx.viewport.w,
            Unit::Vh => v / 100.0 * cx.viewport.h,
            Unit::Vmin => v / 100.0 * cx.viewport.w.min(cx.viewport.h),
            Unit::Vmax => v / 100.0 * cx.viewport.w.max(cx.viewport.h),
            Unit::Pt => v * 96.0 / 72.0,
            Unit::Pc => v * 16.0,
            Unit::In => v * 96.0,
            Unit::Cm => v * 96.0 / 2.54,
            Unit::Mm => v * 96.0 / 25.4,
            Unit::Q => v * 96.0 / 101.6,
        }
    }
}

/// Svelte's `css_to_keyframe` (svelte@5.57.2 `internal/client/dom/elements/transitions.js`): split on
/// `;`, then on the first `:`, trim; stop at the first part without a name or value. Property names
/// stay as written (kebab-case); [`camel_case`] gives the keyframe-object key Svelte uses.
pub fn parse_declarations(css: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in css.split(';') {
        let Some((name, value)) = part.split_once(':') else { break };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() || value.is_empty() {
            break;
        }
        out.push((name.to_string(), value.to_string()));
    }
    out
}

/// `background-color` → `backgroundColor`, `float` → `cssFloat`; custom properties stay.
pub fn camel_case(name: &str) -> String {
    if name.starts_with("--") {
        return name.to_string();
    }
    if name == "float" {
        return "cssFloat".to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut up = false;
    for c in name.chars() {
        if c == '-' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// The animated properties presto knows by name; other properties go through svelters-style's ids
/// ([`AnimProperty::Css`]) at the caller.
pub fn property(name: &str) -> Option<AnimProperty> {
    Some(match name {
        "opacity" => AnimProperty::Opacity,
        "transform" => AnimProperty::Transform,
        "clip" | "clip-path" => AnimProperty::Clip,
        "color" => AnimProperty::Color,
        "background-color" | "backgroundColor" => AnimProperty::BackgroundColor,
        "border-color" | "borderColor" => AnimProperty::BorderColor,
        "filter" => AnimProperty::Filter,
        "stroke-dashoffset" | "strokeDashoffset" => AnimProperty::StrokeDashOffset,
        "width" => AnimProperty::Width,
        "height" => AnimProperty::Height,
        _ => return None,
    })
}

/// Parses one declaration's value. `transform` and `filter` parse as function lists; anything else
/// as one number, length, colour or keyword.
pub fn parse_value(property: &str, value: &str) -> Result<CssValue, CssError> {
    let mut p = Parser { s: value.as_bytes(), i: 0 };
    p.ws();
    let v = match property {
        "transform" => CssValue::Transform(p.transform_list()?),
        "filter" => CssValue::Filter(p.filter_list()?),
        _ => p.single()?,
    };
    p.ws();
    if p.i < p.s.len() {
        return Err(p.err("trailing input"));
    }
    Ok(v)
}

/// Resolves a parsed value. Filters have no [`AnimValue`] yet and resolve to `None`.
pub fn resolve(v: &CssValue, cx: &ResolveCx) -> Option<AnimValue> {
    Some(match v {
        CssValue::Number(n) => AnimValue::Number(*n),
        CssValue::Length(l) => AnimValue::Length(l.resolve(cx, cx.percent_base)),
        CssValue::Color(c) => AnimValue::Color(*c),
        CssValue::Transform(fns) => AnimValue::Transform(
            fns.iter()
                .map(|f| match *f {
                    CssTransformFn::Translate(x, y) => TransformFn::Translate(x.resolve(cx, cx.box_size.w), y.resolve(cx, cx.box_size.h)),
                    CssTransformFn::Scale(x, y) => TransformFn::Scale(x, y),
                    CssTransformFn::Rotate(a) => TransformFn::Rotate(a),
                    CssTransformFn::Skew(x, y) => TransformFn::Skew(x, y),
                    CssTransformFn::Matrix(m) => TransformFn::Matrix(m),
                })
                .collect(),
        ),
        CssValue::Filter(_) | CssValue::Keyword(_) => return None,
    })
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

enum Dim {
    Number(f32),
    Percent(f32),
    Unit(f32, String),
}

impl<'a> Parser<'a> {
    fn err(&self, message: &'static str) -> CssError {
        CssError { at: self.i, message }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn ws(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Option<String> {
        self.ws();
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
            self.i += 1;
        }
        // an identifier does not start with a digit (or `-digit`)
        let s = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        let first = s.bytes().find(|&c| c != b'-');
        if s.is_empty() || first.is_none_or(|c| c.is_ascii_digit() || c == b'.') {
            self.i = start;
            return None;
        }
        Some(s.to_ascii_lowercase())
    }

    fn number(&mut self) -> Option<f32> {
        self.ws();
        let start = self.i;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.i += 1;
        }
        let digits = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.peek() == Some(b'.') && self.s.get(self.i + 1).is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if self.i == digits {
            self.i = start;
            return None;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            let save = self.i;
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
            } else {
                self.i = save;
            }
        }
        std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok()
    }

    fn dimension(&mut self) -> Option<Dim> {
        let n = self.number()?;
        if self.peek() == Some(b'%') {
            self.i += 1;
            return Some(Dim::Percent(n));
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.i += 1;
        }
        if self.i == start {
            return Some(Dim::Number(n));
        }
        let unit = std::str::from_utf8(&self.s[start..self.i]).ok()?.to_ascii_lowercase();
        Some(Dim::Unit(n, unit))
    }

    fn length(&mut self) -> Result<Length, CssError> {
        match self.dimension() {
            Some(Dim::Number(n)) if n == 0.0 => Ok(Length::px(0.0)),
            Some(Dim::Number(n)) => Ok(Length::px(n)), // Svelte's `split_css_unit` defaults to px
            Some(Dim::Percent(n)) => Ok(Length { value: n, unit: Unit::Percent }),
            Some(Dim::Unit(n, u)) => {
                let unit = match u.as_str() {
                    "px" => Unit::Px,
                    "em" => Unit::Em,
                    "rem" => Unit::Rem,
                    "vw" => Unit::Vw,
                    "vh" => Unit::Vh,
                    "vmin" => Unit::Vmin,
                    "vmax" => Unit::Vmax,
                    "pt" => Unit::Pt,
                    "pc" => Unit::Pc,
                    "in" => Unit::In,
                    "cm" => Unit::Cm,
                    "mm" => Unit::Mm,
                    "q" => Unit::Q,
                    _ => return Err(self.err("unknown length unit")),
                };
                Ok(Length { value: n, unit })
            }
            None => Err(self.err("expected a length")),
        }
    }

    fn angle(&mut self) -> Result<f32, CssError> {
        match self.dimension() {
            Some(Dim::Number(n)) if n == 0.0 => Ok(0.0),
            Some(Dim::Unit(n, u)) => match u.as_str() {
                "deg" => Ok(n * PI / 180.0),
                "rad" => Ok(n),
                "grad" => Ok(n * PI / 200.0),
                "turn" => Ok(n * 2.0 * PI),
                _ => Err(self.err("unknown angle unit")),
            },
            _ => Err(self.err("expected an angle")),
        }
    }

    /// A number or a percentage (as a fraction).
    fn amount(&mut self) -> Result<f32, CssError> {
        match self.dimension() {
            Some(Dim::Number(n)) => Ok(n),
            Some(Dim::Percent(n)) => Ok(n / 100.0),
            _ => Err(self.err("expected a number or percentage")),
        }
    }

    fn plain_number(&mut self) -> Result<f32, CssError> {
        match self.dimension() {
            Some(Dim::Number(n)) => Ok(n),
            _ => Err(self.err("expected a number")),
        }
    }

    fn comma(&mut self) -> bool {
        self.eat(b',')
    }

    fn close(&mut self) -> Result<(), CssError> {
        if self.eat(b')') { Ok(()) } else { Err(self.err("expected `)`")) }
    }

    fn numbers(&mut self, n: usize) -> Result<Vec<f32>, CssError> {
        let mut v = Vec::with_capacity(n);
        for k in 0..n {
            if k > 0 {
                self.comma();
            }
            v.push(self.plain_number()?);
        }
        Ok(v)
    }

    fn transform_list(&mut self) -> Result<Vec<CssTransformFn>, CssError> {
        let mut out = Vec::new();
        self.ws();
        if self.peek().is_none() {
            return Ok(out);
        }
        loop {
            self.ws();
            if self.peek().is_none() {
                return Ok(out);
            }
            let Some(name) = self.ident() else { return Err(self.err("expected a transform function")) };
            if name == "none" && out.is_empty() {
                return Ok(out);
            }
            if !self.eat(b'(') {
                return Err(self.err("expected `(`"));
            }
            let f = match name.as_str() {
                "matrix" => {
                    let v = self.numbers(6)?;
                    CssTransformFn::Matrix(Affine { a: v[0], b: v[1], c: v[2], d: v[3], e: v[4], f: v[5] })
                }
                "matrix3d" => {
                    let v = self.numbers(16)?;
                    // column-major; 2-D only when the z and perspective parts are the identity
                    let flat = v[2] == 0.0 && v[3] == 0.0 && v[6] == 0.0 && v[7] == 0.0 && v[8] == 0.0 && v[9] == 0.0
                        && v[10] == 1.0 && v[11] == 0.0 && v[14] == 0.0 && v[15] == 1.0;
                    if !flat {
                        return Err(self.err("3-D matrix3d() is not supported"));
                    }
                    CssTransformFn::Matrix(Affine { a: v[0], b: v[1], c: v[4], d: v[5], e: v[12], f: v[13] })
                }
                "translate" => {
                    let x = self.length()?;
                    let y = if self.comma() { self.length()? } else { Length::px(0.0) };
                    CssTransformFn::Translate(x, y)
                }
                "translatex" => CssTransformFn::Translate(self.length()?, Length::px(0.0)),
                "translatey" => CssTransformFn::Translate(Length::px(0.0), self.length()?),
                "translate3d" => {
                    let x = self.length()?;
                    self.comma();
                    let y = self.length()?;
                    self.comma();
                    let z = self.length()?;
                    if z.value != 0.0 {
                        return Err(self.err("3-D translate is not supported"));
                    }
                    CssTransformFn::Translate(x, y)
                }
                "scale" => {
                    let x = self.amount()?;
                    let y = if self.comma() { self.amount()? } else { x };
                    CssTransformFn::Scale(x, y)
                }
                "scalex" => CssTransformFn::Scale(self.amount()?, 1.0),
                "scaley" => CssTransformFn::Scale(1.0, self.amount()?),
                "scale3d" => {
                    let x = self.amount()?;
                    self.comma();
                    let y = self.amount()?;
                    self.comma();
                    let z = self.amount()?;
                    if z != 1.0 {
                        return Err(self.err("3-D scale is not supported"));
                    }
                    CssTransformFn::Scale(x, y)
                }
                "rotate" | "rotatez" => CssTransformFn::Rotate(self.angle()?),
                "skew" => {
                    let x = self.angle()?;
                    let y = if self.comma() { self.angle()? } else { 0.0 };
                    CssTransformFn::Skew(x, y)
                }
                "skewx" => CssTransformFn::Skew(self.angle()?, 0.0),
                "skewy" => CssTransformFn::Skew(0.0, self.angle()?),
                _ => return Err(self.err("unsupported transform function")),
            };
            self.close()?;
            out.push(f);
        }
    }

    fn filter_list(&mut self) -> Result<Vec<FilterFn>, CssError> {
        let mut out = Vec::new();
        loop {
            self.ws();
            if self.peek().is_none() {
                return Ok(out);
            }
            let Some(name) = self.ident() else { return Err(self.err("expected a filter function")) };
            if name == "none" && out.is_empty() {
                return Ok(out);
            }
            if !self.eat(b'(') {
                return Err(self.err("expected `(`"));
            }
            let f = match name.as_str() {
                "blur" => FilterFn::Blur(self.length()?),
                "hue-rotate" => FilterFn::HueRotate(self.angle()?),
                other => {
                    let kind = match other {
                        "brightness" => FilterAmount::Brightness,
                        "contrast" => FilterAmount::Contrast,
                        "grayscale" => FilterAmount::Grayscale,
                        "invert" => FilterAmount::Invert,
                        "opacity" => FilterAmount::Opacity,
                        "saturate" => FilterAmount::Saturate,
                        "sepia" => FilterAmount::Sepia,
                        _ => return Err(self.err("unsupported filter function")),
                    };
                    FilterFn::Amount(kind, self.amount()?)
                }
            };
            self.close()?;
            out.push(f);
        }
    }

    fn single(&mut self) -> Result<CssValue, CssError> {
        self.ws();
        if self.peek() == Some(b'#') {
            self.i += 1;
            return self.hex().map(CssValue::Color);
        }
        let save = self.i;
        if let Some(name) = self.ident() {
            if self.peek() == Some(b'(') {
                self.i += 1;
                let c = match name.as_str() {
                    "rgb" | "rgba" => self.rgb()?,
                    "hsl" | "hsla" => self.hsl()?,
                    _ => {
                        self.i = save;
                        return Err(self.err("unsupported function"));
                    }
                };
                return Ok(CssValue::Color(c));
            }
            return Ok(match named_color(&name) {
                Some(c) => CssValue::Color(c),
                None => CssValue::Keyword(name),
            });
        }
        match self.dimension() {
            Some(Dim::Number(n)) => Ok(CssValue::Number(n)),
            Some(_) => {
                self.i = save;
                Ok(CssValue::Length(self.length()?))
            }
            None => Err(self.err("expected a value")),
        }
    }

    fn hex(&mut self) -> Result<Color, CssError> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            self.i += 1;
        }
        let h = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("bad hex colour"))?;
        let d = |s: &str| u8::from_str_radix(s, 16).map_err(|_| CssError { at: start, message: "bad hex colour" });
        let dd = |c: char| -> Result<u8, CssError> { d(&format!("{c}{c}")) };
        let cs: Vec<char> = h.chars().collect();
        let (r, g, b, a) = match cs.len() {
            3 => (dd(cs[0])?, dd(cs[1])?, dd(cs[2])?, 255),
            4 => (dd(cs[0])?, dd(cs[1])?, dd(cs[2])?, dd(cs[3])?),
            6 => (d(&h[0..2])?, d(&h[2..4])?, d(&h[4..6])?, 255),
            8 => (d(&h[0..2])?, d(&h[2..4])?, d(&h[4..6])?, d(&h[6..8])?),
            _ => return Err(self.err("bad hex colour")),
        };
        Ok(Color::from_rgba8(r, g, b, a))
    }

    /// The channel list of `rgb()`/`hsl()`, comma or space separated, alpha after `,` or `/`.
    fn channels(&mut self) -> Result<(Vec<Dim>, Option<f32>), CssError> {
        let mut v = Vec::new();
        let mut alpha = None;
        loop {
            self.ws();
            if self.eat(b')') {
                break;
            }
            if !v.is_empty() && self.eat(b'/') {
                alpha = Some(self.amount()?);
                self.close()?;
                break;
            }
            self.comma();
            if v.len() == 3 {
                alpha = Some(self.amount()?);
                self.close()?;
                break;
            }
            let Some(dim) = self.dimension() else { return Err(self.err("expected a colour channel")) };
            v.push(dim);
        }
        if v.len() != 3 {
            return Err(self.err("expected three colour channels"));
        }
        Ok((v, alpha))
    }

    fn rgb(&mut self) -> Result<Color, CssError> {
        let (v, alpha) = self.channels()?;
        let ch = |d: &Dim| -> Result<f32, CssError> {
            match d {
                Dim::Number(n) => Ok((n / 255.0).clamp(0.0, 1.0)),
                Dim::Percent(n) => Ok((n / 100.0).clamp(0.0, 1.0)),
                Dim::Unit(..) => Err(CssError { at: 0, message: "bad rgb() channel" }),
            }
        };
        Ok(Color::rgba(ch(&v[0])?, ch(&v[1])?, ch(&v[2])?, alpha.unwrap_or(1.0).clamp(0.0, 1.0)))
    }

    fn hsl(&mut self) -> Result<Color, CssError> {
        let (v, alpha) = self.channels()?;
        let hue = match &v[0] {
            Dim::Number(n) => *n,
            Dim::Unit(n, u) if u == "deg" => *n,
            Dim::Unit(n, u) if u == "turn" => n * 360.0,
            Dim::Unit(n, u) if u == "rad" => n * 180.0 / PI,
            Dim::Unit(n, u) if u == "grad" => n * 0.9,
            _ => return Err(self.err("bad hsl() hue")),
        };
        let pct = |d: &Dim| match d {
            Dim::Percent(n) | Dim::Number(n) => Ok((n / 100.0).clamp(0.0, 1.0)),
            Dim::Unit(..) => Err(CssError { at: 0, message: "bad hsl() channel" }),
        };
        let (s, l) = (pct(&v[1])?, pct(&v[2])?);
        let [r, g, b] = hsl_to_rgb(hue, s, l);
        Ok(Color::rgba(r, g, b, alpha.unwrap_or(1.0).clamp(0.0, 1.0)))
    }
}

/// CSS Color 4 §7.1 `hslToRgb`.
fn hsl_to_rgb(hue: f32, sat: f32, light: f32) -> [f32; 3] {
    let h = hue.rem_euclid(360.0);
    let f = |n: f32| {
        let k = (n + h / 30.0) % 12.0;
        let a = sat * light.min(1.0 - light);
        light - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    [f(0.0), f(8.0), f(4.0)]
}

/// The colour keywords that appear in transition and animation keyframes in practice
/// (`getComputedStyle` serialises colours as `rgb()`/`rgba()`); others parse as keywords.
fn named_color(name: &str) -> Option<Color> {
    Some(match name {
        "transparent" => Color::TRANSPARENT,
        "black" => Color::BLACK,
        "white" => Color::WHITE,
        "red" => Color::from_rgba8(255, 0, 0, 255),
        "lime" => Color::from_rgba8(0, 255, 0, 255),
        "green" => Color::from_rgba8(0, 128, 0, 255),
        "blue" => Color::from_rgba8(0, 0, 255, 255),
        "yellow" => Color::from_rgba8(255, 255, 0, 255),
        "gray" | "grey" => Color::from_rgba8(128, 128, 128, 255),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_stop_at_first_malformed_part() {
        let d = parse_declarations("opacity: 0.5; transform: translate(10px, 0) ; bad ; height: 3px");
        assert_eq!(d, vec![("opacity".into(), "0.5".into()), ("transform".into(), "translate(10px, 0)".into())]);
        assert_eq!(camel_case("border-top-width"), "borderTopWidth");
        assert_eq!(camel_case("float"), "cssFloat");
        assert_eq!(camel_case("--x"), "--x");
    }

    #[test]
    fn svelte_fly_keyframe() {
        // fly at t = 0.25 over a computed matrix: `transform: matrix(…) translate(75px, 0px); opacity: 0.25`
        let v = parse_value("transform", "matrix(1, 0, 0, 1, 0, 0) translate(75px, 0px)").unwrap();
        let r = resolve(&v, &ResolveCx::default()).unwrap();
        assert_eq!(r, AnimValue::Transform(vec![TransformFn::Matrix(Affine::IDENTITY), TransformFn::Translate(75.0, 0.0)]));
        assert_eq!(parse_value("opacity", "0.25").unwrap(), CssValue::Number(0.25));
        // JS prints small numbers with exponents
        assert_eq!(parse_value("opacity", "1.5e-7").unwrap(), CssValue::Number(1.5e-7));
        let v = parse_value("transform", " translate(-1.2e-7px,50%) scale(.9)").unwrap();
        let cx = ResolveCx { box_size: Size::new(200.0, 40.0), ..ResolveCx::default() };
        assert_eq!(resolve(&v, &cx).unwrap(), AnimValue::Transform(vec![TransformFn::Translate(-1.2e-7, 20.0), TransformFn::Scale(0.9, 0.9)]));
    }

    #[test]
    fn lengths_and_units() {
        let cx = ResolveCx { font_size: 10.0, root_font_size: 20.0, viewport: Size::new(1000.0, 500.0), percent_base: 300.0, box_size: Size::ZERO };
        let px = |s: &str| match parse_value("height", s).unwrap() {
            CssValue::Length(l) => l.resolve(&cx, cx.percent_base),
            other => panic!("{other:?}"),
        };
        assert_eq!(px("12px"), 12.0);
        assert_eq!(px("2em"), 20.0);
        assert_eq!(px("2rem"), 40.0);
        assert_eq!(px("10vh"), 50.0);
        assert_eq!(px("50%"), 150.0);
        assert_eq!(px("1in"), 96.0);
        assert_eq!(parse_value("height", "0").unwrap(), CssValue::Number(0.0));
    }

    #[test]
    fn colours() {
        let c = |s: &str| match parse_value("color", s).unwrap() {
            CssValue::Color(c) => c,
            other => panic!("{other:?}"),
        };
        assert_eq!(c("#ff0000"), Color::from_rgba8(255, 0, 0, 255));
        assert_eq!(c("#f008"), Color::from_rgba8(255, 0, 0, 0x88));
        assert_eq!(c("rgb(255, 0, 0)"), Color::from_rgba8(255, 0, 0, 255));
        assert_eq!(c("rgba(0, 0, 255, 0.5)"), Color::rgba(0.0, 0.0, 1.0, 0.5));
        assert_eq!(c("rgb(0 0 255 / 50%)"), Color::rgba(0.0, 0.0, 1.0, 0.5));
        let g = c("hsl(120, 100%, 50%)");
        assert!((g.g - 1.0).abs() < 1e-6 && g.r.abs() < 1e-6 && g.b.abs() < 1e-6);
    }

    #[test]
    fn transforms_and_filters() {
        let v = parse_value("transform", "rotate(90deg) skewX(0.5rad) matrix3d(1,0,0,0, 0,1,0,0, 0,0,1,0, 5,6,0,1)").unwrap();
        let CssValue::Transform(f) = v else { panic!() };
        assert!(matches!(f[0], CssTransformFn::Rotate(a) if (a - PI / 2.0).abs() < 1e-6));
        assert_eq!(f[1], CssTransformFn::Skew(0.5, 0.0));
        assert_eq!(f[2], CssTransformFn::Matrix(Affine::translate(5.0, 6.0)));
        assert_eq!(parse_value("transform", "none").unwrap(), CssValue::Transform(vec![]));
        let v = parse_value("filter", "blur(2.5px) opacity(50%)").unwrap();
        assert_eq!(v, CssValue::Filter(vec![FilterFn::Blur(Length::px(2.5)), FilterFn::Amount(FilterAmount::Opacity, 0.5)]));
        assert!(parse_value("transform", "translate(10px").is_err());
        assert!(parse_value("transform", "perspective(10px)").is_err());
    }
}
