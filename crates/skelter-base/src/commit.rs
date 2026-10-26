//! The UI → compositor [`Commit`] (architecture §6.4), fast commits (§7.3), and the compositor → UI
//! messages ([`ToUi`]).

use crate::anim::{AnimationCmd, AnimationEvent};
use crate::cursor::CursorIcon;
use crate::display::DisplayList;
use crate::geom::{Affine, DeviceSize, Rect, Scale, Vec2};
use crate::ids::{CanvasId, FrameId, ImageId, InputSeq, NodeId, PathId, PropNodeId, Revision, RunId, WindowId};
use crate::layer::LayerDelta;
use crate::props::PropertyTreeDelta;
use crate::text::GlyphRun;
use crate::time::Time;
use std::sync::Arc;

/// One UI frame's changes for one window. Building it clones only what changed; the compositor folds
/// commits that arrive before it draws (newer over older).
#[derive(Clone, Debug, Default)]
pub struct Commit {
    pub window: WindowId,
    pub revision: Revision,
    /// The last input this commit has seen.
    pub input_seq: InputSeq,
    /// The `BeginUiFrame` this answers; `None` = an urgent frame (discrete input).
    pub frame: Option<FrameId>,
    pub layers: Vec<LayerDelta>,
    pub props: PropertyTreeDelta,
    /// Re-recorded display lists only.
    pub lists: Vec<(NodeId, Arc<DisplayList>)>,
    /// Nodes whose lists are gone (removed from the tree).
    pub removed: Vec<NodeId>,
    /// Newly shaped runs; runs no list references any more are listed in `dropped_runs`.
    pub runs: Vec<(RunId, Arc<GlyphRun>)>,
    pub dropped_runs: Vec<RunId>,
    pub resources: ResourceDelta,
    pub animations: Vec<AnimationCmd>,
    pub a11y: Option<accesskit::TreeUpdate>,
    /// AccessKit updates of subtrees grafted under nodes of this commit's tree (an egui island's tree,
    /// under its node's `tree_id`), sent to the platform adapter after [`Commit::a11y`], in order.
    pub a11y_subtrees: Vec<accesskit::TreeUpdate>,
    pub cursor: Option<CursorIcon>,
    /// The IME candidate area (window points), when it moved.
    pub ime_area: Option<Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    /// Premultiplied RGBA8, sRGB-encoded.
    Rgba8Premul,
    /// 8-bit coverage (monochrome icons, masks).
    A8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageData {
    pub size: DeviceSize,
    pub format: ImageFormat,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PathData {
    pub cmds: Vec<PathCmd>,
    pub fill_rule: FillRule,
}

#[derive(Clone, Debug, Default)]
pub struct ResourceDelta {
    pub images: Vec<(ImageId, Arc<ImageData>)>,
    pub removed_images: Vec<ImageId>,
    pub paths: Vec<(PathId, Arc<PathData>)>,
    pub removed_paths: Vec<PathId>,
}

/// A native-owned property written by its owner straight to the compositor, without UI phases
/// (architecture §7.3). The same write is recorded in the tree, so the next [`Commit`] agrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FastCommit {
    ScrollOffset { window: WindowId, node: PropNodeId, offset: Vec2, revision: Revision, input_seq: InputSeq },
    Transform { window: WindowId, node: PropNodeId, local: Affine, revision: Revision, input_seq: InputSeq },
    Opacity { window: WindowId, node: PropNodeId, opacity: f32, revision: Revision },
    CanvasView { window: WindowId, canvas: CanvasId, view: Affine, input_seq: InputSeq },
    /// Show or move the caret (the compositor blinks it).
    Caret { window: WindowId, rect: Option<Rect>, revision: Revision },
}

/// Compositor → UI thread.
#[derive(Clone, Debug, PartialEq)]
pub enum ToUi {
    /// Run the UI phases now for a frame presenting at `target`; commit before `deadline`.
    BeginFrame { window: WindowId, frame: FrameId, target: Time, deadline: Time },
    /// The commit with this revision is applied (back-pressure release).
    CommitApplied { window: WindowId, revision: Revision },
    /// A frame reached the screen at `at` (presentation feedback, or an estimate when the OS gives none).
    Presented { window: WindowId, frame: FrameId, at: Time, estimated: bool, inputs: Vec<InputSeq> },
    Animation { window: WindowId, event: AnimationEvent },
    /// The compositor moved a scroll offset (fling, smooth scroll); the tree takes it as native state.
    ScrollMoved { window: WindowId, node: PropNodeId, offset: Vec2 },
    /// The resize handshake finished (architecture §7.6).
    ResizeDone { window: WindowId, seq: u64, size: DeviceSize, scale: Scale },
    /// The GPU device was lost and recreated: re-send every resource.
    DeviceReset,
    /// The compositor side of a frame's latency (architecture §16.3), sent when it was presented:
    /// when the draw started, the CPU submitted, the GPU finished, and the frame reached the screen
    /// (`estimated` when the OS gave no time). `gpu_done` is `None` when it was not reported.
    FrameTimings {
        window: WindowId,
        frame: FrameId,
        draw: Time,
        submit: Time,
        gpu_done: Option<Time>,
        present: Time,
        estimated: bool,
    },
    /// The platform's frame source reports the window covered or minimised (`true`) or visible
    /// again; the compositor stops drawing it meanwhile.
    Occluded { window: WindowId, occluded: bool },
}
