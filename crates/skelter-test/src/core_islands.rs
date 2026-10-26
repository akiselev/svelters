//! [`CoreIslands`]: a skelter-core document whose `egui-island` nodes carry islands mounted through
//! `skelter_egui_island::IslandHost` (cadatomic's panes as real nodes, W-D1), with the island steps the app
//! runner does not take yet: `IslandHost::run` before each frame, its AccessKit subtrees carried by the
//! frame's commit (`Commit::a11y_subtrees`, which the app sends to the platform adapter), `IslandHost::render` before `render`,
//! AccessKit actions aimed at an island's tree routed to that island, the islands' wakeups, and
//! core's in-app drags bridged into the islands (`IslandHost::event_tap` on the `CoreDocument`).
//!
//! Build one with [`HarnessBuilder::build_core_islands`]; every kittest verb and query then works on the
//! skelter tree and the egui widgets inside its islands alike.

use crate::harness::{Harness, HarnessBuilder, Hooks};
use crate::island::{register_textures, Registrations};
use crate::render::GpuRender;
use skelter::backends::CoreDocument;
use skelter::seams::{Dispatched, DocumentApi};
use skelter_base::accesskit::{ActionRequest, TreeUpdate};
use skelter_base::anim::AnimationEvent;
use skelter_base::commit::Commit;
use skelter_base::event::{InputEnvelope, WindowEvent};
use skelter_base::geom::{Scale, Size, Vec2};
use skelter_base::ids::{NodeId, PropNodeId, WindowId};
use skelter_base::time::{Clock, Time};
use skelter_egui_island::{IslandHost, IslandOutput, island_tree_id};
use std::sync::Arc;

/// A skelter-core document with mounted islands; `ui(node, ui)` draws the island on `node`.
pub struct CoreIslands<U: FnMut(NodeId, &mut egui::Ui)> {
    pub core: CoreDocument,
    ui: U,
    clock: Arc<dyn Clock>,
    /// The islands' earliest pass, refreshed after every call that can change it.
    wakeup: Option<Time>,
    /// Island AccessKit updates waiting for the next commit (`Commit::a11y_subtrees`).
    subtree: Vec<TreeUpdate>,
    outputs: Vec<(NodeId, IslandOutput)>,
}

impl<U: FnMut(NodeId, &mut egui::Ui)> CoreIslands<U> {
    pub fn doc(&mut self) -> &mut skelter_core::Document {
        &mut self.core.doc
    }
    /// The outputs of the passes run since the last call (cursor, IME, copied text, links, drags out).
    pub fn take_outputs(&mut self) -> Vec<(NodeId, IslandOutput)> {
        std::mem::take(&mut self.outputs)
    }
    fn refresh(&mut self) {
        self.wakeup = IslandHost::next_wakeup(&mut self.core.doc);
    }
    fn due(&self, now: Time) -> bool {
        self.wakeup.is_some_and(|t| t <= now)
    }
}

impl<U: FnMut(NodeId, &mut egui::Ui)> DocumentApi for CoreIslands<U> {
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        let mut d = self.core.dispatch(ev);
        self.refresh();
        d.dirty |= self.due(ev.recv_time);
        d
    }

    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        let outs = IslandHost::run(&mut self.core.doc, target, &mut self.ui);
        for (node, out) in outs {
            if let Some(u) = &out.a11y {
                self.subtree.push(u.clone());
            }
            self.outputs.push((node, out));
        }
        self.refresh();
        let mut c = self.core.frame(window, target);
        // The subtrees ride the commit that carries their graft nodes (or the next one).
        if let Some(c) = &mut c {
            c.a11y_subtrees.append(&mut self.subtree);
        }
        c
    }

    fn needs_frame(&self, window: WindowId) -> bool {
        self.core.needs_frame(window) || self.due(self.clock.now())
    }

    fn next_wakeup(&self, _window: WindowId) -> Option<Time> {
        self.wakeup
    }

    fn resize(&mut self, window: WindowId, size: Size, scale: Scale) {
        self.core.resize(window, size, scale);
    }
    fn close_window(&mut self, window: WindowId) {
        self.core.close_window(window);
    }
    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2) {
        self.core.scroll_moved(window, node, offset);
    }
    fn animation_event(&mut self, window: WindowId, ev: AnimationEvent) {
        self.core.animation_event(window, ev);
    }

    /// An action for an island's tree goes to that island; others to the document.
    fn accessibility_action(&mut self, window: WindowId, req: &ActionRequest) -> bool {
        let doc = &mut self.core.doc;
        let target = IslandHost::mounted(doc)
            .into_iter()
            .find(|n| island_tree_id(n.0) == req.target_tree);
        let handled = match target {
            Some(node) => IslandHost::island_mut(doc, node)
                .is_some_and(|i| i.handle_a11y_action(req.clone())),
            None => self.core.accessibility_action(window, req),
        };
        self.refresh();
        handled
    }

    fn accessibility_activated(&mut self, window: WindowId) {
        self.core.accessibility_activated(window);
    }
    fn window_event(&mut self, window: WindowId, ev: &WindowEvent) {
        self.core.window_event(window, ev);
    }
    fn device_reset(&mut self) {
        self.core.device_reset();
    }
}

impl HarnessBuilder {
    /// A harness on a skelter-core document with islands mounted through `IslandHost` (mount them before
    /// building, or later through `state_mut().doc()`); `ui(node, ui)` draws each island's pass. It adds
    /// `IslandHost::event_tap` to `core` (don't add it yourself).
    #[track_caller]
    pub fn build_core_islands<U: FnMut(NodeId, &mut egui::Ui) + 'static>(
        self,
        mut core: CoreDocument,
        ui: U,
    ) -> Harness<CoreIslands<U>> {
        // In-app drags (skelter-core's drag-enter/leave/drop over island nodes) reach the islands.
        core.add_event_tap(IslandHost::event_tap);
        let hooks = Hooks {
            before_step: None,
            settled: None,
            clipboard: None,
            textures: Some(Box::new(|d: &mut CoreIslands<U>, g: &mut GpuRender| {
                let regs = Registrations::default();
                let mut encoder = g
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("skelter_test_islands"),
                    });
                let user = IslandHost::render(&mut d.core.doc, &g.device, &g.queue, &mut encoder, &regs);
                g.queue.submit(user.into_iter().chain([encoder.finish()]));
                register_textures(regs, g);
            })),
        };
        let mut h = self.build_with(
            move |clock, _| {
                let mut d = CoreIslands {
                    core,
                    ui,
                    clock,
                    wakeup: None,
                    subtree: Vec::new(),
                    outputs: Vec::new(),
                };
                d.refresh();
                d
            },
            hooks,
            None,
        );
        h.run();
        h
    }
}
