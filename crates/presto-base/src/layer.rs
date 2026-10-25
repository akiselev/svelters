//! Compositor layers (architecture §6.3): subtrees the compositor treats as one unit with their own
//! damage and, for some kinds, their own GPU target or OS layer.

use crate::geom::Rect;
use crate::ids::{CanvasId, LayerId, NodeId, PropNodeId, SurfaceId, TextureId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerKind {
    /// A window's root layer.
    Root,
    /// Ordinary content (a floating pane, a popover, an animated subtree).
    Content,
    /// A scroll container's content; `tiles` = cache content in device-pixel tiles (architecture §7.5).
    Scroll { tiles: bool },
    /// Above all panes: tour overlay, adorners, menus, tooltips, the drag image.
    Overlay,
    /// A canvas with retained instance chunks.
    Canvas(CanvasId),
    /// A texture produced elsewhere (an offscreen viewport, an egui island) composited as a quad.
    Texture(TextureId),
    /// An OS compositor layer with its own swapchain (DComp visual, CAMetalLayer, wl_subsurface).
    OsLayer(SurfaceId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerDesc {
    pub id: LayerId,
    pub parent: Option<LayerId>,
    pub kind: LayerKind,
    pub transform: PropNodeId,
    pub clip: Option<PropNodeId>,
    pub effect: Option<PropNodeId>,
    /// Paint order among siblings.
    pub z: i32,
    /// Bounds in the layer's transform space.
    pub bounds: Rect,
    /// Fully opaque content (lets the renderer skip what is underneath).
    pub opaque: bool,
}

/// A node's display list placed in a layer, with the property nodes it draws under.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemRef {
    pub node: NodeId,
    pub transform: PropNodeId,
    pub clip: Option<PropNodeId>,
    pub effect: Option<PropNodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayerDelta {
    Add(LayerDesc),
    Update(LayerDesc),
    Remove(LayerId),
    /// The layer's items in paint order (sent when structure changed in it).
    Items {
        layer: LayerId,
        items: Vec<ItemRef>,
    },
    /// Extra damage in layer space (e.g. a canvas whose chunks changed).
    Damage {
        layer: LayerId,
        rects: Vec<Rect>,
    },
}
