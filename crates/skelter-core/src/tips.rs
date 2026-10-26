//! The `title` tooltip seam (SV61): the document finds the node with a `title` the pointer rests on
//! and tells the installed [`TitleTips`] what the input did (a press, a scroll, Escape) after each
//! dispatch, hover and due wake-up. The provider decides when the tip shows (skelter-widgets: the
//! `tooltip` surface's timer with the document's interaction policy, read on each step) and draws it
//! with [`Document::set_window_tip`], a click-through layer above every overlay surface and below
//! the drag image.

use crate::common;
use crate::document::Document;
use skelter_base::ids::{NodeId, WindowId};
use skelter_base::time::Time;
use std::sync::Arc;

/// What the input did since the last step of a window's tip ([`TitleTips::step`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TitleInput {
    /// The innermost hovered node with a non-empty `title` ([`Document::title_host`]), and its text.
    pub host: Option<(NodeId, Arc<str>)>,
    /// A pointer button went down.
    pub pressed: bool,
    /// A wheel or touchpad scroll arrived.
    pub scrolled: bool,
    /// Escape was pressed.
    pub escape: bool,
}

/// Shows the `title` tooltip of a window (the host's provider: skelter-widgets'
/// `views::tooltip::TitleTooltips`).
pub trait TitleTips {
    /// One step of `window`'s tip at `now`: show, move or hide it ([`Document::set_window_tip`]).
    /// Returns when to step again without input (the delay running out, a duration ending).
    fn step(&mut self, doc: &mut Document, window: WindowId, input: &TitleInput, now: Time) -> Option<Time>;
}

/// A tooltip surface: a display list in window points with the glyph runs it draws (its
/// `Prim::Glyphs` name them by position in `runs` plus one, as a [`crate::DragPreview::List`]'s).
pub type TipList = (skelter_base::display::DisplayList, Vec<Arc<skelter_base::text::GlyphRun>>);

impl Document {
    /// Installs the provider that shows `title` tooltips (`None`: none show).
    pub fn set_title_tips(&mut self, tips: Option<Box<dyn TitleTips>>) {
        self.title_tips = tips;
        self.tip_wakes.clear();
    }

    /// The innermost node with a non-empty `title` on a hovered path in `window`, with its text.
    pub fn title_host(&self, window: WindowId) -> Option<(NodeId, Arc<str>)> {
        self.input.hover.values().filter(|path| path.first().is_some_and(|&n| self.window_of(n) == Some(window))).find_map(|path| {
            path.iter().find_map(|&n| {
                let t = self.view(n)?.prop_str(common::TITLE).filter(|t| !t.is_empty())?;
                Some((n, Arc::from(t)))
            })
        })
    }

    /// Sets `window`'s tooltip surface (`None` hides it); the next frame sends it.
    pub fn set_window_tip(&mut self, window: WindowId, tip: Option<TipList>) {
        if let Some(w) = self.window_mut(window) {
            w.tip.pending = Some(tip);
            w.rebuild = true;
        }
    }

    /// The tooltip layer of `window` while it shows one.
    pub fn window_tip_layer(&self, window: WindowId) -> Option<skelter_base::ids::LayerId> {
        self.window(window)?.tip.layer
    }

    /// Steps `window`'s tip with what the input did.
    pub(crate) fn step_tip(&mut self, window: WindowId, pressed: bool, scrolled: bool, escape: bool) {
        let Some(mut tips) = self.title_tips.take() else { return };
        let input = TitleInput { host: self.title_host(window), pressed, scrolled, escape };
        let now = self.input.now;
        let wake = tips.step(self, window, &input, now);
        if self.title_tips.is_none() {
            self.title_tips = Some(tips);
        }
        match wake {
            Some(t) => self.tip_wakes.insert(window, t),
            None => self.tip_wakes.remove(&window),
        };
    }

    /// The earliest tip wake-up.
    pub(crate) fn next_tip_wake(&self) -> Option<Time> {
        self.tip_wakes.values().min().copied()
    }

    /// Steps the tips whose wake-up is due at `now`.
    pub(crate) fn run_tip_wakes(&mut self, now: Time) {
        let due: Vec<WindowId> = self.tip_wakes.iter().filter(|&(_, &t)| t <= now).map(|(&w, _)| w).collect();
        for w in due {
            self.tip_wakes.remove(&w);
            self.input.now = now;
            self.step_tip(w, false, false, false);
        }
    }
}
