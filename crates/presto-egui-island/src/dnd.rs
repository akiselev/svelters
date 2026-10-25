//! The `egui::DragAndDrop` ↔ presto drag-and-drop bridge (architecture §11.7), so cadatomic's Rust panels
//! in islands and presto's own drag sources and targets interoperate during the migration.
//!
//! - **presto → island.** While a presto drag is over the island the host calls
//!   [`EguiIsland::drag_enter`](crate::EguiIsland::drag_enter), `drag_move` and `drag_drop` or
//!   `drag_leave`. The bridge puts the payload into egui's `DragAndDrop` through its [`PayloadCodec`],
//!   so egui drop zones (`Response::dnd_hover_payload`/`dnd_release_payload`) see it. A drop is a
//!   pointer release with no press before it: egui's `any_released` is true, so drop zones take the
//!   payload, and no widget sees a click. The target calls [`IslandDrag::accept`] (svelters-egui's
//!   `DragPayload::accept` contract): egui's own `DragAndDrop` clears the payload on every release, so
//!   a payload gone after the drop doesn't tell a taken drop from an ignored one.
//! - **island → presto.** When an egui drag the codec can export leaves the island's rect with its
//!   button held, the pass output carries it ([`DndOutput::drag_out`]) and the host starts a presto
//!   drag with it; the host reports the outcome with
//!   [`EguiIsland::drag_out_finished`](crate::EguiIsland::drag_out_finished).
//!
//! The default codec carries [`IslandDrag`]. svelters-egui's panels use its `DragPayload`; an embedder
//! that still links svelters-egui supplies a codec for that type (the island doesn't depend on it).

use crate::shim::PrestoDrag;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The payload egui code in an island sees for a presto drag (the default codec's type), and the type an
/// island panel sets to drag something out to presto.
#[derive(Debug)]
pub struct IslandDrag {
    pub drag: PrestoDrag,
    accepted: Arc<AtomicBool>,
}

impl IslandDrag {
    pub fn new(drag: PrestoDrag) -> IslandDrag {
        IslandDrag { drag, accepted: Arc::default() }
    }

    /// A drop target took the drag (egui code calls it after `dnd_release_payload`, or presto's target
    /// took an exported drag).
    pub fn accept(&self) {
        self.accepted.store(true, Ordering::Relaxed);
    }

    pub fn was_accepted(&self) -> bool {
        self.accepted.load(Ordering::Relaxed)
    }
}

/// How presto drags become egui payloads and back.
pub trait PayloadCodec: Send + Sync {
    /// Put `drag` into `egui::DragAndDrop`; `accepted` is set when an egui target takes it.
    fn import(&self, ctx: &egui::Context, drag: &PrestoDrag, accepted: Arc<AtomicBool>);
    /// egui's current payload as a presto drag (`None`: no payload, or one presto can't carry), and the
    /// flag that reports the drop back to its egui source.
    fn export(&self, ctx: &egui::Context) -> Option<(PrestoDrag, Arc<AtomicBool>)>;
}

/// Carries [`IslandDrag`].
#[derive(Debug, Default)]
pub struct IslandDragCodec;

impl PayloadCodec for IslandDragCodec {
    fn import(&self, ctx: &egui::Context, drag: &PrestoDrag, accepted: Arc<AtomicBool>) {
        egui::DragAndDrop::set_payload(ctx, IslandDrag { drag: drag.clone(), accepted });
    }

    fn export(&self, ctx: &egui::Context) -> Option<(PrestoDrag, Arc<AtomicBool>)> {
        egui::DragAndDrop::payload::<IslandDrag>(ctx).map(|p| (p.drag.clone(), p.accepted.clone()))
    }
}

/// What the bridge reports after a pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DndOutput {
    /// An egui drag left the island: the host starts a presto drag carrying it.
    pub drag_out: Option<PrestoDrag>,
    /// The presto drag dropped on the island this pass, and whether an egui target took it.
    pub dropped: Option<bool>,
}

struct Incoming {
    accepted: Arc<AtomicBool>,
    dropping: bool,
}

pub struct DndBridge {
    codec: Box<dyn PayloadCodec>,
    incoming: Option<Incoming>,
    /// The egui drag currently exported to presto (its drop flag).
    outgoing: Option<Arc<AtomicBool>>,
}

impl Default for DndBridge {
    fn default() -> Self {
        DndBridge::new(Box::new(IslandDragCodec))
    }
}

impl DndBridge {
    pub fn new(codec: Box<dyn PayloadCodec>) -> Self {
        DndBridge { codec, incoming: None, outgoing: None }
    }

    pub fn set_codec(&mut self, codec: Box<dyn PayloadCodec>) {
        self.codec = codec;
    }

    /// A presto drag (re)entered the island.
    pub fn enter(&mut self, ctx: &egui::Context, drag: &PrestoDrag) {
        let accepted = Arc::new(AtomicBool::new(false));
        self.codec.import(ctx, drag, accepted.clone());
        self.incoming = Some(Incoming { accepted, dropping: false });
    }

    /// The presto drag left the island (or was cancelled) without dropping here.
    pub fn leave(&mut self, ctx: &egui::Context) {
        if self.incoming.take().is_some() {
            egui::DragAndDrop::clear_payload(ctx);
        }
    }

    /// The presto drag is released over the island; the next pass delivers it.
    pub fn drop_here(&mut self) {
        if let Some(i) = &mut self.incoming {
            i.dropping = true;
        }
    }

    pub fn has_incoming(&self) -> bool {
        self.incoming.is_some()
    }

    /// After each pass. `leaving`: the pointer is outside the island's rect with a button pressed inside
    /// it still held.
    pub fn after_pass(&mut self, ctx: &egui::Context, leaving: bool) -> DndOutput {
        let mut out = DndOutput::default();
        if let Some(i) = &self.incoming {
            if i.dropping {
                out.dropped = Some(i.accepted.load(Ordering::Relaxed));
                egui::DragAndDrop::clear_payload(ctx);
                self.incoming = None;
            }
            return out;
        }
        if self.outgoing.is_none() {
            if leaving && let Some((drag, flag)) = self.codec.export(ctx) {
                out.drag_out = Some(drag);
                self.outgoing = Some(flag);
            }
        } else if !egui::DragAndDrop::has_any_payload(ctx) && !leaving {
            // egui ended its drag (the release reached the island) and the host already reported or
            // dropped the outcome.
            self.outgoing = None;
        }
        out
    }

    /// The presto drag started from an island drag ended; `accepted`: a presto target took it.
    pub fn drag_out_finished(&mut self, accepted: bool) {
        if let Some(flag) = self.outgoing.take() {
            if accepted {
                flag.store(true, Ordering::Relaxed);
            }
        }
    }
}
