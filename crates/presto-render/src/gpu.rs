//! The instance records the shaders read from storage buffers. Each struct mirrors one WGSL struct in
//! `shaders.wgsl` field for field (std430: every field is 16 bytes, so `#[repr(C)]` matches).
//!
//! Geometry is in an instance's "pixel-local" space: the primitive's local space scaled so one unit is
//! about one device pixel; [`Xform`] maps it to device pixels. For the common translate-and-scale case
//! the transform is a pure translation, and the shaders' half-pixel antialiasing thresholds (from GPUI)
//! hold exactly.

use bytemuck::{Pod, Zeroable};

/// Pixel-local → device pixels: `m` = (a, b, c, d), `t` = (e, f, -, -) as in [`presto_base::geom::Affine`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct Xform {
    pub m: [f32; 4],
    pub t: [f32; 4],
}

/// A device-space clip: a hard rect plus at most one antialiased rounded rect.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct Clip {
    /// (x0, y0, x1, y1) in device pixels.
    pub rect: [f32; 4],
    /// (x, y, w, h); `w <= 0` = no rounded clip.
    pub rrect: [f32; 4],
    /// (tl, tr, br, bl).
    pub radii: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct Globals {
    /// (width, height, -, -) of the target in pixels.
    pub viewport: [f32; 4],
    /// Premultiplied colour the clear pipeline writes.
    pub clear: [f32; 4],
    pub gamma_ratios: [f32; 4],
    /// (grayscale enhanced contrast, -, -, -).
    pub text: [f32; 4],
}

pub const FILL_SOLID: u32 = 0;
pub const FILL_LINEAR: u32 = 1;
pub const MAX_STOPS: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct QuadInstance {
    pub xform: Xform,
    pub clip: Clip,
    /// (x, y, w, h).
    pub bounds: [f32; 4],
    pub radii: [f32; 4],
    /// (top, right, bottom, left).
    pub borders: [f32; 4],
    /// Straight sRGB-encoded RGBA (solid fill).
    pub fill: [f32; 4],
    pub border_color: [f32; 4],
    /// (fill kind, border style 0 solid / 1 dashed / 2 dotted, stop count, -).
    pub info: [u32; 4],
    /// (gradient angle in radians, CSS convention, -, -, -).
    pub params: [f32; 4],
    /// Gradient stop colours, straight RGBA8 packed little-endian (`unpack4x8unorm`).
    pub stop_colors: [u32; MAX_STOPS],
    pub stop_offsets: [f32; MAX_STOPS],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct ShadowInstance {
    pub xform: Xform,
    pub clip: Clip,
    /// The blurred rect: the shadow for drop shadows, the hole for inset ones.
    pub bounds: [f32; 4],
    pub radii: [f32; 4],
    pub color: [f32; 4],
    /// The element's own box (inset shadows are clipped to it).
    pub element: [f32; 4],
    pub element_radii: [f32; 4],
    /// (sigma, inset 0/1, -, -).
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct LineInstance {
    pub xform: Xform,
    pub clip: Clip,
    /// The drawn quad (x, y, w, h) in line space: x along the segment from 0 to `length`.
    pub bounds: [f32; 4],
    pub color: [f32; 4],
    /// (length, half width, cap 0 butt / 1 round / 2 square, wavy 0/1).
    pub geom: [f32; 4],
    /// (dash on, dash off, -, -); on = 0 for a solid line.
    pub dash: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct MaskInstance {
    pub xform: Xform,
    pub clip: Clip,
    pub bounds: [f32; 4],
    /// Atlas tile (x, y, w, h) in texels.
    pub uv: [f32; 4],
    pub color: [f32; 4],
    /// (gamma-correct 0/1, -, -, -): glyphs are corrected, icons are not.
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct ImageInstance {
    pub xform: Xform,
    pub clip: Clip,
    pub bounds: [f32; 4],
    /// Source (x, y, w, h) in texels.
    pub uv: [f32; 4],
    /// Sampling clamp (x0, y0, x1, y1) in texels, so linear filtering never reads a neighbour tile.
    pub uv_clamp: [f32; 4],
    pub radii: [f32; 4],
    /// (opacity, -, -, -).
    pub params: [f32; 4],
}

/// One path drawn in a path pre-pass (tessellated vertices are in the path's local space).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct PathInstance {
    /// Local → device pixels.
    pub xform: Xform,
    pub clip: Clip,
    /// Straight sRGB-encoded RGBA.
    pub color: [f32; 4],
    /// The intermediate's device rect (x, y, w, h).
    pub viewport_rect: [f32; 4],
}

/// The composite of one path batch's intermediate into the main pass.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct PathSprite {
    /// Device rect (x, y, w, h), equal to the intermediate's.
    pub bounds: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct PathVertex {
    pub pos: [f32; 2],
}

pub(crate) fn pack_rgba8(c: [f32; 4]) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    q(c[0]) | (q(c[1]) << 8) | (q(c[2]) << 16) | (q(c[3]) << 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_16_byte_multiples() {
        for (name, size) in [
            ("quad", size_of::<QuadInstance>()),
            ("shadow", size_of::<ShadowInstance>()),
            ("line", size_of::<LineInstance>()),
            ("mask", size_of::<MaskInstance>()),
            ("image", size_of::<ImageInstance>()),
            ("path", size_of::<PathInstance>()),
            ("path sprite", size_of::<PathSprite>()),
            ("globals", size_of::<Globals>()),
        ] {
            assert_eq!(size % 16, 0, "{name}: {size}");
        }
        assert_eq!(size_of::<QuadInstance>(), 256);
    }

    #[test]
    fn pack_is_little_endian_rgba() {
        assert_eq!(pack_rgba8([1.0, 0.0, 0.0, 1.0]), 0xff00_00ff);
    }
}
