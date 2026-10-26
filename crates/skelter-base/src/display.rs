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
    /// The palette slot the tint comes from at draw time (a mask only): while the item's palette
    /// sets that slot, the compositor draws it in place of `color`.
    pub slot: Option<PaletteSlot>,
}

/// How many slots a [`Palette`] has. Slot 0 is `currentColor`; skelter-widgets' icon registry uses
/// 1, 2 and 3 for the icon kit's `--icon-subject`, `--icon-accent` and `--icon-badge`.
pub const PALETTE_SLOTS: usize = 4;

/// A primitive's colour taken from a palette slot at draw time (architecture §8.1, live icon
/// colours): the slot's colour with its alpha multiplied by `alpha` (the layer's own opacity). The
/// primitive's recorded colour is that same product for the colours it was recorded with, so it
/// draws unchanged until the palette changes.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaletteSlot {
    /// `0..PALETTE_SLOTS`.
    pub index: u8,
    pub alpha: f32,
}

impl PaletteSlot {
    pub fn new(index: u8, alpha: f32) -> PaletteSlot {
        PaletteSlot { index, alpha }
    }
    /// The colour this slot draws in when the palette gives `c`.
    pub fn resolve(&self, c: Color) -> Color {
        c.with_alpha(c.a * self.alpha)
    }
}

/// Colours by palette slot; `None` keeps a primitive's recorded colour. The compositor resolves a
/// display list's palette from its effect chain (the nearest effect node that sets a slot wins) and
/// animates it (`AnimProperty::Palette`) without the UI thread.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Palette {
    pub slots: [Option<Color>; PALETTE_SLOTS],
}

impl Palette {
    pub const EMPTY: Palette = Palette { slots: [None; PALETTE_SLOTS] };

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
    pub fn get(&self, slot: PaletteSlot) -> Option<Color> {
        self.slots.get(slot.index as usize).copied().flatten().map(|c| slot.resolve(c))
    }
    pub fn set(&mut self, index: u8, c: Option<Color>) {
        if let Some(s) = self.slots.get_mut(index as usize) {
            *s = c;
        }
    }
    /// `self` where it sets a slot, else `outer` (an inner effect node's palette over its parent's).
    pub fn over(&self, outer: &Palette) -> Palette {
        let mut p = *outer;
        for (i, c) in self.slots.iter().enumerate() {
            if c.is_some() {
                p.slots[i] = *c;
            }
        }
        p
    }

    /// `list` with its slotted primitives drawn in this palette's colours; `None` when nothing in it
    /// changes (no slotted primitive, or the palette sets none of their slots).
    pub fn apply(&self, list: &DisplayList) -> Option<DisplayList> {
        if self.is_empty() {
            return None;
        }
        let mut out: Option<DisplayList> = None;
        for (i, p) in list.items.iter().enumerate() {
            let repl = match p {
                Prim::Icon(ic) => ic.slot.and_then(|s| self.get(s)).filter(|c| ic.color.is_some() && Some(*c) != ic.color).map(|c| Prim::Icon(Icon { color: Some(c), ..*ic })),
                Prim::Path(pp) => pp.slot.and_then(|s| self.get(s)).filter(|c| *c != pp.paint.color()).map(|c| Prim::Path(PathPrim { paint: pp.paint.with_color(c), ..*pp })),
                _ => None,
            };
            if let Some(r) = repl {
                out.get_or_insert_with(|| list.clone()).items[i] = r;
            }
        }
        out
    }
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

impl PathPaint {
    pub fn color(&self) -> Color {
        match *self {
            PathPaint::Fill(c) | PathPaint::Stroke { color: c, .. } => c,
        }
    }
    /// The same paint in colour `c`.
    pub fn with_color(&self, c: Color) -> PathPaint {
        match *self {
            PathPaint::Fill(_) => PathPaint::Fill(c),
            PathPaint::Stroke { width, .. } => PathPaint::Stroke { color: c, width },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPrim {
    pub path: PathId,
    pub transform: Affine,
    pub paint: PathPaint,
    /// The palette slot the paint's colour comes from at draw time (see [`PaletteSlot`]).
    pub slot: Option<PaletteSlot>,
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
