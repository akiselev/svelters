//! The dock seam of ARCH-EMBED §7.1 as a test helper (EM2): the shape cadatomic's adapter between
//! berth (its dock crate) and an [`Embed`] takes. Every pane event is one `Embed` call and drawing
//! is one lookup, so moving a pane between windows moves nothing.
//!
//! [`PaneEvent`] and [`CloseReason`] mirror berth's (cadatomic CRATES §4.1, the canonical shape):
//! `PaneEvent` is `#[non_exhaustive]` there, so the adapter matches with a wildcard arm. Panes are
//! named by their id string here (berth's `PaneId::as_str`).
//!
//! The seam items it follows (ARCH-EMBED "Decided"): the close flow is `request_close` → the host's
//! dirty guard (`EmbedEvent::CloseDecided`) → berth's `DockCommand::Close` → `Closed` (2); a pane is
//! mounted lazily on its first `Shown`, from a pending table (3); the host owns the per-frame order
//! (4: it calls `Embed::logic` itself and runs with `prepass_plugin = false`).
//!
//! Writer lane: EMBED.

use crate::embed::{Embed, EmbedError, EntryId, MountSpec, Props, RootId, RootKey, RootOwner, SlotOptions, with_current};
use std::collections::HashMap;

/// Why berth closed a pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The user closed it (its tab's close button, after the host allowed it).
    User,
    /// `DockCommand::Close` from the host.
    Api,
    /// A layout restore dropped it.
    Restore,
    /// Its window closed.
    WindowClosed,
    /// The whole layout was replaced.
    LayoutReplaced,
}

/// What berth reports about a pane, after its frame (`dock.take_events()`).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PaneEvent {
    Shown { pane: String, viewport: egui::ViewportId },
    Hidden { pane: String },
    MovedToViewport { pane: String, from: egui::ViewportId, to: egui::ViewportId },
    Closed { pane: String, reason: CloseReason },
}

impl PaneEvent {
    pub fn pane(&self) -> &str {
        match self {
            PaneEvent::Shown { pane, .. }
            | PaneEvent::Hidden { pane }
            | PaneEvent::MovedToViewport { pane, .. }
            | PaneEvent::Closed { pane, .. } => pane,
        }
    }
}

struct Pending {
    entry: EntryId,
    export: String,
    props: Props,
}

/// Svelte panes of a dock on one [`Embed`] (cadatomic keeps one, beside its `PaneHost`s).
pub struct SvelteDock {
    pub embed: Embed,
    roots: HashMap<String, RootId>,
    /// Opened, not shown yet: mounted at the first `Shown` (seam 3).
    pending: HashMap<String, Pending>,
}

impl SvelteDock {
    pub fn new(embed: Embed) -> SvelteDock {
        SvelteDock { embed, roots: HashMap::new(), pending: HashMap::new() }
    }

    /// The host opens a pane with Svelte content (its view registry decides; the dock never mounts).
    /// The slot exists now, hidden; the root mounts when berth first shows the pane.
    pub fn open(&mut self, pane: &str, entry: EntryId, export: &str, props: Props) {
        let slot = self.embed.create_slot(pane, SlotOptions::fill().label(pane));
        self.embed.set_visible(slot, false);
        self.pending.insert(pane.to_string(), Pending { entry, export: export.to_string(), props });
    }

    /// One pane event (from `App::logic`, after `dock.take_events()`, before `Embed::logic`).
    pub fn on_event(&mut self, ev: &PaneEvent) -> Result<(), EmbedError> {
        let Some(slot) = self.embed.slot(ev.pane()) else { return Ok(()) }; // not a Svelte pane
        match ev {
            PaneEvent::Shown { pane, viewport } => {
                self.embed.set_viewport(slot, *viewport);
                self.embed.set_visible(slot, true);
                if let Some(p) = self.pending.remove(pane) {
                    let spec = MountSpec { entry: p.entry, export: p.export.into(), slot, key: RootKey::from(pane.as_str()), props: p.props, owner: RootOwner::InProcess };
                    self.roots.insert(pane.clone(), self.embed.mount(spec)?);
                }
            }
            PaneEvent::Hidden { .. } => self.embed.set_visible(slot, false),
            PaneEvent::MovedToViewport { to, .. } => self.embed.set_viewport(slot, *to),
            PaneEvent::Closed { pane, .. } => {
                self.pending.remove(pane);
                self.roots.remove(pane);
                self.embed.destroy_slot(slot);
            }
            #[allow(unreachable_patterns)] // berth's enum is non_exhaustive: new events are ignored
            _ => {}
        }
        Ok(())
    }

    /// The close flow's first step (the user clicked the tab's close button): the root's
    /// `onCloseRequest` handlers answer with `EmbedEvent::CloseDecided`; on `allow` the host sends
    /// berth `DockCommand::Close`, and berth's `Closed` frees the slot. A pane not mounted yet has no
    /// handlers: `None` (close it right away).
    pub fn request_close(&self, pane: &str) -> Option<RootId> {
        let r = *self.roots.get(pane)?;
        self.embed.request_close(r);
        Some(r)
    }

    pub fn root(&self, pane: &str) -> Option<RootId> {
        self.roots.get(pane).copied()
    }

    /// The pane a root belongs to (to route `EmbedEvent`s: titles, close decisions).
    pub fn pane_of(&self, root: RootId) -> Option<&str> {
        self.roots.iter().find(|(_, r)| **r == root).map(|(p, _)| p.as_str())
    }

    /// berth's draw callback for a pane. It needs only the pane id, so a deferred viewport's
    /// `Send + Sync` closure can call it (on the UI thread). False: not a Svelte pane.
    pub fn pane_ui(ui: &mut egui::Ui, pane: &str) -> bool {
        with_current(|e| match e.slot(pane) {
            Some(s) => {
                e.show_slot(ui, s);
                true
            }
            None => false,
        })
        .unwrap_or(false)
    }

    /// The end of a viewport's ui, after berth drew its panes (root, immediate and deferred
    /// windows): the per-viewport scene passes (ARCH-SCENE R21). berth (7bd2d32) calls its host's
    /// `PaneHost::viewport_done(&mut self, ui, window)` once per window per pass; the adapter's impl
    /// is this call:
    ///
    /// ```ignore
    /// impl PaneHost for SvelteDock {
    ///     fn viewport_done(&mut self, ui: &mut egui::Ui, _window: WindowId) {
    ///         SvelteDock::viewport_done(ui); // = svelters_egui::embed::show_viewport_passes(ui)
    ///     }
    /// }
    /// ```
    ///
    /// Through the thread-local entry point, so it works inside a deferred viewport's closure (no
    /// `&Embed` there). Returns how many passes ran.
    pub fn viewport_done(ui: &mut egui::Ui) -> usize {
        crate::embed::show_viewport_passes(ui)
    }
}
