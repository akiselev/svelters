//! The compositor → renderer boundary: one frame's flattened draw input for one surface.

use crate::color::Color;
use crate::commit::{ImageData, PathData};
use crate::display::DisplayList;
use crate::geom::{Affine, DeviceRect, DeviceSize, Scale};
use crate::ids::{ImageId, PathId, RunId};
use crate::props::ClipShape;
use crate::text::GlyphRun;

/// One display list to draw, with its resolved properties (world space, points).
#[derive(Clone, Copy, Debug)]
pub struct DrawItem<'a> {
    pub transform: Affine,
    /// The intersection of the item's clip chain, in world points (the renderer intersects with damage).
    pub clip: Option<ClipShape>,
    pub opacity: f32,
    pub list: &'a DisplayList,
}

/// CPU-side resources the renderer may need while drawing (owned by the compositor).
pub trait SceneResources {
    fn run(&self, id: RunId) -> Option<&GlyphRun>;
    fn image(&self, id: ImageId) -> Option<&ImageData>;
    fn path(&self, id: PathId) -> Option<&PathData>;
}

/// Everything the renderer needs for one surface's frame.
pub struct FrameScene<'a> {
    pub target: DeviceSize,
    pub scale: Scale,
    /// Clear colour for damaged areas (`None` = keep: the target is persistent).
    pub clear: Option<Color>,
    /// Device rects to redraw; empty = the whole target.
    pub damage: &'a [DeviceRect],
    /// In paint order.
    pub items: &'a [DrawItem<'a>],
    pub resources: &'a dyn SceneResources,
}
