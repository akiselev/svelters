//! Colours. presto blends in sRGB-encoded space, as browsers do (architecture §8.3): a [`Color`] holds
//! sRGB-encoded channels with straight alpha; [`Color::premultiplied`] gives what shaders consume.

/// sRGB-encoded RGBA, straight (non-premultiplied) alpha, each channel 0..=1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { r, g, b, a }
    }
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }
    }
    /// Premultiplied, still sRGB-encoded: `[r*a, g*a, b*a, a]`.
    pub fn premultiplied(&self) -> [f32; 4] {
        [self.r * self.a, self.g * self.a, self.b * self.a, self.a]
    }
    pub fn with_alpha(self, a: f32) -> Self {
        Color { a, ..self }
    }
    pub fn is_opaque(&self) -> bool {
        self.a >= 1.0
    }
}

/// A gradient stop: `offset` 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

/// A fill: solid, or a linear gradient along `angle` (CSS degrees, 0 = to top) with up to 4 stops in
/// the quad shader; more stops go through a ramp texture (renderer's choice).
#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Solid(Color),
    Linear { angle_deg: f32, stops: smallvec::SmallVec<[GradientStop; 4]> },
}

impl Default for Fill {
    fn default() -> Self {
        Fill::Solid(Color::TRANSPARENT)
    }
}
