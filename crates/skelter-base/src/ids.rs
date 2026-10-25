//! Ids. Every id is a plain integer newtype, `Copy`, hashable, and stable for its object's lifetime.

macro_rules! id {
    ($(#[$m:meta])* $name:ident($t:ty)) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub $t);
    };
}

id!(
    /// A tree node: `generation << 24 | index`, as svelters-egui's scene (`svelters-egui/src/scene/id.rs`).
    /// 0 is never a live node.
    NodeId(u64)
);
id!(
    /// An OS window.
    WindowId(u32)
);
id!(
    /// A compositor layer (architecture §6.3).
    LayerId(u32)
);
id!(
    /// A GPU surface (a window's or an OS layer's swapchain).
    SurfaceId(u32)
);
id!(
    /// A shaped text run held by the compositor (architecture §6.1).
    RunId(u64)
);
id!(
    /// An image or raster icon in the resource store.
    ImageId(u64)
);
id!(
    /// A cached tessellated path.
    PathId(u64)
);
id!(
    /// A canvas layer's content (typed instance chunks, architecture §9.5).
    CanvasId(u32)
);
id!(
    /// A compositor-side texture another pass produces (an offscreen viewport, an egui island).
    TextureId(u32)
);
id!(
    /// A running animation.
    AnimationId(u64)
);
id!(
    /// A font face in the font store (`skelter-text`).
    FontId(u32)
);
id!(
    /// A property-tree node (shared id space across the four trees; each tree indexes its own).
    PropNodeId(u32)
);
id!(
    /// Every platform input event gets one, in arrival order per process (architecture §16.3).
    InputSeq(u64)
);
id!(
    /// The tree's revision: one per transaction (architecture §4.4).
    Revision(u64)
);
id!(
    /// A compositor frame for one window, in order.
    FrameId(u64)
);

impl NodeId {
    pub const IDX_BITS: u32 = 24;
    pub const fn new(index: u32, generation: u32) -> Self {
        NodeId(((generation as u64) << Self::IDX_BITS) | index as u64)
    }
    pub const fn index(self) -> u32 {
        (self.0 & ((1 << Self::IDX_BITS) - 1)) as u32
    }
    pub const fn generation(self) -> u32 {
        (self.0 >> Self::IDX_BITS) as u32
    }
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}
