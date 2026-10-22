//! Deferred viewports (ARCH-EMBED §5.2, EM2): the thread-local registry, [`SlotPainter`],
//! [`with_current`] and the begin-pass plugin. Writer lane: EMBED.
//!
//! A deferred viewport's closure must be `Send + Sync + 'static`, and an [`Embed`] is `!Send`. So the
//! closure captures a [`SlotPainter`] (or only a pane id, and calls [`with_current`]), and on the UI
//! thread the embed is found through a `thread_local!` registry. On another thread nothing is drawn:
//! a debug build panics, a release build logs once. No `unsafe`, no fake `Send`.
//!
//! eframe calls `App::logic` for the root viewport only. The plugin registered on every context the
//! embed is attached to (`Embed::attach`) runs `Embed::logic` before a deferred viewport's pass when
//! `EmbedConfig::prepass_plugin` is set, so input in that window is handled in that window's next
//! pass with no root-viewport frame. With `prepass_plugin = false` (cadatomic, seam 4: the host owns
//! the per-frame order) the plugin only does the bookkeeping: pending work asks for a root-viewport
//! frame, whose logic routes repaints to the viewports it changed (one root frame per interaction).

use super::{Embed, EmbedInner, SlotId, SlotResponse};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread::ThreadId;

/// An embed's id in the registry of its thread: unique in the process, never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmbedId(u64);

impl EmbedId {
    pub(super) fn next() -> EmbedId {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        EmbedId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Default)]
struct Registry {
    /// Live embeds of this thread, oldest first.
    embeds: Vec<(EmbedId, Weak<EmbedInner>)>,
    /// What [`with_current`] resolves (`Embed::make_current`); else the oldest live one.
    current: Option<EmbedId>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

pub(super) fn register(id: EmbedId, inner: &Rc<EmbedInner>) {
    REGISTRY.with(|r| r.borrow_mut().embeds.push((id, Rc::downgrade(inner))));
}

pub(super) fn unregister(id: EmbedId) {
    // `try_with`: an embed dropped while the thread's locals are torn down
    let _ = REGISTRY.try_with(|r| {
        let mut r = r.borrow_mut();
        r.embeds.retain(|(x, w)| *x != id && w.strong_count() > 0);
        if r.current == Some(id) {
            r.current = None;
        }
    });
}

pub(super) fn make_current(id: EmbedId) {
    REGISTRY.with(|r| r.borrow_mut().current = Some(id));
}

fn lookup(id: Option<EmbedId>) -> Option<Embed> {
    REGISTRY
        .try_with(|r| {
            let r = r.borrow();
            let want = id.or(r.current);
            let found = match want {
                Some(w) => r
                    .embeds
                    .iter()
                    .find(|(x, _)| *x == w)
                    .and_then(|(_, e)| e.upgrade()),
                None => r.embeds.iter().find_map(|(_, e)| e.upgrade()),
            };
            found.map(|inner| Embed { inner })
        })
        .ok()
        .flatten()
}

/// Runs `f` with this thread's current embed (the one made current with `Embed::make_current`, else
/// the oldest live one). `None` when the thread has none. The registry is not borrowed while `f`
/// runs, so `f` may draw slots, mount roots or call `with_current` again.
///
/// ```ignore
/// // berth's draw callback, possibly inside a deferred viewport's closure (it needs only the pane id):
/// fn pane_ui(ui: &mut egui::Ui, pane: &PaneId) {
///     svelters_egui::embed::with_current(|e| if let Some(s) = e.slot(pane.as_str()) { e.show_slot(ui, s); });
/// }
/// ```
pub fn with_current<R>(f: impl FnOnce(&Embed) -> R) -> Option<R> {
    let e = lookup(None)?;
    Some(f(&e))
}

/// Runs `f` with the embed `id` of this thread, if it is still alive.
pub fn with_embed<R>(id: EmbedId, f: impl FnOnce(&Embed) -> R) -> Option<R> {
    let e = lookup(Some(id))?;
    Some(f(&e))
}

/// A `Copy + Send + Sync` token for one slot, for closures that must be `Send + Sync` (a deferred
/// viewport's). [`SlotPainter::show`] draws the slot on the thread that made the painter (the UI
/// thread); on any other thread it draws nothing (debug builds panic, release builds log once).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotPainter {
    embed: EmbedId,
    slot: SlotId,
    thread: ThreadId,
}

impl SlotPainter {
    pub(super) fn new(embed: EmbedId, slot: SlotId) -> SlotPainter {
        SlotPainter {
            embed,
            slot,
            thread: std::thread::current().id(),
        }
    }

    pub fn slot(&self) -> SlotId {
        self.slot
    }

    pub fn embed(&self) -> EmbedId {
        self.embed
    }

    /// `Embed::show_slot(ui, slot)`. `None`: the embed or the slot is gone (a window closing a pass
    /// after its pane), or this is not the UI thread.
    pub fn show(&self, ui: &mut egui::Ui) -> Option<SlotResponse> {
        if std::thread::current().id() != self.thread {
            off_thread();
            return None;
        }
        with_embed(self.embed, |e| {
            e.has_slot(self.slot).then(|| e.show_slot(ui, self.slot))
        })
        .flatten()
    }
}

fn off_thread() {
    static WARNED: AtomicBool = AtomicBool::new(false);
    debug_assert!(
        false,
        "svelters-egui: SlotPainter::show called off the UI thread (deferred viewports must run on it)"
    );
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("[svelters-egui] SlotPainter::show called off the UI thread: nothing drawn");
    }
}

/// Registers the embed's begin/end-pass callbacks on `ctx` (once per context and embed). They run
/// on the UI thread for every pass of every viewport; nested passes (immediate viewports) only
/// count. See the module doc.
pub(super) fn install(e: &Embed, ctx: &egui::Context) {
    let id = e.id();
    let key = egui::Id::unique(("svelters-embed-pass-plugin", id.0));
    if ctx.data(|d| d.get_temp::<bool>(key).is_some()) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, true));
    let thread = std::thread::current().id();
    let depth = Arc::new(AtomicU32::new(0));
    let d = depth.clone();
    ctx.on_begin_pass(
        "svelters-embed",
        Arc::new(move |ui: &mut egui::Ui| {
            if std::thread::current().id() != thread {
                return;
            }
            let outer = d.fetch_add(1, Ordering::Relaxed) == 0;
            with_embed(id, |e| e.begin_pass(ui.ctx(), outer));
        }),
    );
    ctx.on_end_pass(
        "svelters-embed",
        Arc::new(move |ui: &mut egui::Ui| {
            if std::thread::current().id() != thread {
                return;
            }
            let was = depth
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |d| {
                    Some(d.saturating_sub(1))
                })
                .unwrap_or(0);
            with_embed(id, |e| e.end_pass(ui.ctx(), was <= 1));
        }),
    );
}
