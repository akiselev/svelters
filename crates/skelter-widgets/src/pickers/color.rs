//! Colour models of the colour kinds: unmultiplied sRGBA bytes (what hex and the R/G/B boxes show),
//! HSV in sRGB (what the square and the hue strip edit; svelters-egui used `ecolor`'s
//! `hsv_from_rgb`/`rgb_from_hsv`, the same formulas), HSL, and CSS hex.

use skelter_base::color::Color;

/// A colour in sRGB HSV, each 0..=1 (`h` below 1: 1.0 is the same hue as 0), unmultiplied alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
    pub a: f32,
}

/// The largest hue the controls store (a full turn wraps to red at 0).
pub const HUE_MAX: f32 = 0.9999;

/// `[r, g, b]` (0..=1) → `(h, s, v)` (0..=1).
pub fn hsv_from_rgb([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let range = max - min;
    if max == min {
        return (0.0, 0.0, max);
    }
    let s = range / max;
    let h = if max == r {
        (g - b) / (6.0 * range)
    } else if max == g {
        (b - r) / (6.0 * range) + 1.0 / 3.0
    } else {
        (r - g) / (6.0 * range) + 2.0 / 3.0
    };
    (h.rem_euclid(1.0), s, max)
}

/// `(h, s, v)` (0..=1) → `[r, g, b]` (0..=1).
pub fn rgb_from_hsv((h, s, v): (f32, f32, f32)) -> [f32; 3] {
    let h = (h.rem_euclid(1.0) * 6.0).min(6.0);
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match i as i32 {
        0 | 6 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// `[r, g, b]` (0..=1) → `(h, s, l)` (0..=1).
pub fn hsl_from_rgb([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let (h, _, v) = hsv_from_rgb([r, g, b]);
    let min = r.min(g).min(b);
    let l = (v + min) / 2.0;
    let s = if v == min { 0.0 } else if l <= 0.5 { (v - min) / (v + min) } else { (v - min) / (2.0 - v - min) };
    (h, s, l)
}

/// `(h, s, l)` (0..=1) → `[r, g, b]` (0..=1).
pub fn rgb_from_hsl((h, s, l): (f32, f32, f32)) -> [f32; 3] {
    let v = l + s * l.min(1.0 - l);
    let sv = if v == 0.0 { 0.0 } else { 2.0 * (1.0 - l / v) };
    rgb_from_hsv((h, sv, v))
}

fn q8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

impl Hsv {
    pub fn from_srgba([r, g, b, a]: [u8; 4]) -> Hsv {
        let (h, s, v) = hsv_from_rgb([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]);
        Hsv { h, s, v, a: a as f32 / 255.0 }
    }
    pub fn to_srgba(self) -> [u8; 4] {
        let [r, g, b] = rgb_from_hsv((self.h.rem_euclid(1.0), self.s.clamp(0.0, 1.0), self.v.clamp(0.0, 1.0)));
        [q8(r), q8(g), q8(b), q8(self.a)]
    }
    /// As a skelter colour (rounded through bytes, as the value is).
    pub fn color(self) -> Color {
        let [r, g, b, a] = self.to_srgba();
        Color::from_rgba8(r, g, b, a)
    }
    /// The same colour, opaque.
    pub fn opaque(self) -> Hsv {
        Hsv { a: 1.0, ..self }
    }
}

/// Parses CSS hex (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`; the `#` optional) into unmultiplied
/// sRGBA.
pub fn parse_hex(s: &str) -> Result<[u8; 4], String> {
    let t = s.trim();
    let h = t.strip_prefix('#').unwrap_or(t);
    let err = || format!("`{s}` is not a hex colour (#rrggbb, #rrggbbaa, #rgb, #rgba)");
    if !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(err());
    }
    let nib = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).map(|v| v * 17).map_err(|_| err());
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| err());
    match h.len() {
        3 => Ok([nib(0)?, nib(1)?, nib(2)?, 255]),
        4 => Ok([nib(0)?, nib(1)?, nib(2)?, nib(3)?]),
        6 => Ok([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Ok([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => Err(err()),
    }
}

/// Unmultiplied sRGBA as CSS hex: `#rrggbb` when opaque, else `#rrggbbaa`.
pub fn to_hex([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
}

/// A colour prop write: hex normalized (`#rrggbb`/`#rrggbbaa`), anything else refused.
pub fn normalize_hex(s: &str) -> Result<String, String> {
    parse_hex(s).map(to_hex)
}

/// A colour prop read leniently (a bad string is opaque black, the default).
pub fn rgba_or_black(s: &str) -> [u8; 4] {
    parse_hex(s).unwrap_or([0, 0, 0, 255])
}

/// `#rrggbb` bytes as a skelter colour.
pub fn rgba_color([r, g, b, a]: [u8; 4]) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// The `alpha` prop's values (`egui::color_picker::Alpha`'s names): `opaque` hides the alpha strip.
pub const ALPHA: &[&str] = &["opaque", "blend", "blend-or-additive"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_normalizes() {
        assert_eq!(parse_hex("#ff8000"), Ok([255, 128, 0, 255]));
        assert_eq!(parse_hex("#F80"), Ok([255, 136, 0, 255]));
        assert_eq!(parse_hex("#f808"), Ok([255, 136, 0, 136]));
        assert_eq!(parse_hex("12345678"), Ok([0x12, 0x34, 0x56, 0x78]));
        assert!(parse_hex("#12345").is_err());
        assert!(parse_hex("#ggg").is_err());
        assert!(parse_hex("").is_err());
        for s in ["#000000", "#ffffff", "#12ab34", "#12ab3480", "#00000000"] {
            assert_eq!(to_hex(parse_hex(s).unwrap()), s);
        }
        assert_eq!(normalize_hex("#ABC").unwrap(), "#aabbcc");
        assert_eq!(normalize_hex("#abcf").unwrap(), "#aabbcc", "an opaque #rgba drops its alpha");
    }

    #[test]
    fn hsv_round_trips_every_byte_colour_sampled() {
        for r in (0..=255u16).step_by(17) {
            for g in (0..=255u16).step_by(51) {
                for b in (0..=255u16).step_by(15) {
                    let c = [r as u8, g as u8, b as u8, 200];
                    assert_eq!(Hsv::from_srgba(c).to_srgba(), c, "{c:?}");
                }
            }
        }
        let red = Hsv::from_srgba([255, 0, 0, 255]);
        assert_eq!((red.h, red.s, red.v), (0.0, 1.0, 1.0));
        let blue = Hsv::from_srgba([0, 0, 255, 255]);
        assert!((blue.h - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(Hsv { h: 1.0, s: 1.0, v: 1.0, a: 1.0 }.to_srgba(), [255, 0, 0, 255], "a full turn is red");
    }

    #[test]
    fn hsl_matches_css() {
        let (h, s, l) = hsl_from_rgb([1.0, 0.0, 0.0]);
        assert_eq!((h, s, l), (0.0, 1.0, 0.5));
        let rgb = rgb_from_hsl((1.0 / 3.0, 1.0, 0.25));
        assert!((rgb[0]).abs() < 1e-6 && (rgb[1] - 0.5).abs() < 1e-6 && rgb[2].abs() < 1e-6);
        assert_eq!(hsl_from_rgb([0.5, 0.5, 0.5]), (0.0, 0.0, 0.5));
    }
}
