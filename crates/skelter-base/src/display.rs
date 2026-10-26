//! Display lists (architecture §6.1): what a node records when its content changes. Coordinates are in
//! the node's local space (points); the compositor applies the node's transform, clip and effect from
//! the property trees.

use crate::color::{Color, Fill};
use crate::geom::{Affine, Corners, Insets, Point, Rect, RoundedRect};
use crate::ids::{CanvasId, ImageId, PathId, RunId, TextureId};
use smallvec::SmallVec;

/// One node's recorded content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    /// Ink bounds in local coordinates (what damage uses).
    pub bounds: Rect,
    pub items: SmallVec<[Prim; 8]>,
}

/// The closed set of primitives (architecture §8.1). The renderer has one instanced pipeline per
/// kind; adding a kind is a coordinated change (render, compositor, widgets).
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Quad(Quad),
    Shadow(Shadow),
    Line(Line),
    Glyphs(Glyphs),
    Icon(Icon),
    Image(Image),
    Path(PathPrim),
    /// A texture another pass produces (offscreen viewport, egui island, cached tile).
    Surface(SurfacePrim),
    /// A canvas layer's content placed here (architecture §9.5).
    Canvas(CanvasPrim),
    /// Clip the following items of this list (until the matching [`Prim::PopClip`]).
    PushClip(RoundedRect),
    PopClip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

/// A rounded rectangle with per-corner radii, per-edge borders and a fill (SDF in the shader).
#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    pub rect: Rect,
    pub radii: Corners,
    pub fill: Fill,
    pub border_widths: Insets,
    pub border_color: Color,
    pub border_style: BorderStyle,
}

/// A drop or inset box shadow (closed-form blur).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub rect: Rect,
    pub radii: Corners,
    pub color: Color,
    pub blur: f32,
    pub spread: f32,
    pub offset: crate::geom::Vec2,
    pub inset: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// A straight segment; `dash` = (on, off) lengths; `wavy` for spell-check underlines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub from: Point,
    pub to: Point,
    pub width: f32,
    pub color: Color,
    pub cap: LineCap,
    pub dash: Option<(f32, f32)>,
    pub wavy: bool,
}

/// A shaped glyph run (held by the compositor under `run`) drawn with its baseline origin at `origin`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyphs {
    pub run: RunId,
    pub origin: Point,
    pub color: Color,
}

/// A monochrome icon (coverage mask tinted with `color`) or a colour icon (`color: None`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Icon {
    pub image: ImageId,
    pub rect: Rect,
    pub color: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sampling {
    #[default]
    Linear,
    Nearest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Image {
    pub image: ImageId,
    pub rect: Rect,
    /// Source rectangle in the image's pixels; `None` = the whole image.
    pub src: Option<Rect>,
    pub radii: Corners,
    pub opacity: f32,
    pub sampling: Sampling,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathPaint {
    Fill(Color),
    Stroke { color: Color, width: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPrim {
    pub path: PathId,
    pub transform: Affine,
    pub paint: PathPaint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePrim {
    pub texture: TextureId,
    pub rect: Rect,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPrim {
    pub canvas: CanvasId,
    pub rect: Rect,
}

impl DisplayList {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, p: Prim, ink: Rect) {
        self.bounds = self.bounds.union(&ink);
        self.items.push(p);
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Moves every primitive (and the bounds) by `v`: a list recorded in a part's own space placed
    /// in its node's space.
    pub fn translate(&mut self, v: crate::geom::Vec2) {
        if v == crate::geom::Vec2::ZERO {
            return;
        }
        let pt = |p: &mut Point| *p = Point::new(p.x + v.x, p.y + v.y);
        self.bounds = self.bounds.translate(v);
        for p in self.items.iter_mut() {
            match p {
                Prim::Quad(q) => q.rect = q.rect.translate(v),
                Prim::Shadow(s) => s.rect = s.rect.translate(v),
                Prim::Line(l) => {
                    pt(&mut l.from);
                    pt(&mut l.to);
                }
                Prim::Glyphs(g) => pt(&mut g.origin),
                Prim::Icon(i) => i.rect = i.rect.translate(v),
                Prim::Image(i) => i.rect = i.rect.translate(v),
                Prim::Path(p) => p.transform = Affine::translate(v.x, v.y).then(&p.transform),
                Prim::Surface(s) => s.rect = s.rect.translate(v),
                Prim::Canvas(c) => c.rect = c.rect.translate(v),
                Prim::PushClip(r) => r.rect = r.rect.translate(v),
                Prim::PopClip => {}
            }
        }
    }
}
