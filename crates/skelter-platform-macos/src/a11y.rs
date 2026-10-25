//! The AccessKit host (`round2/ime-a11y-os.md` §2): our own view answers `accessibilityChildren`,
//! `accessibilityFocusedUIElement` and `accessibilityHitTest:` from an `accesskit_macos::Adapter`,
//! so no subclassing adapter swaps the view's class (the Touch Bar KVO crash).
//!
//! The app pushes tree updates with [`a11y_update`]. Until an assistive technology asks, the adapter
//! is inactive and only full trees are kept (the latest one answers the first request); the first
//! request without one sends the app [`A11yEvent::Activated`].

use crate::app::{self, Pending};
use accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
use accesskit_macos::{Adapter, QueuedEvents};
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use skelter_base::ids::WindowId;
use skelter_platform::A11yEvent;

pub(crate) struct A11y {
    adapter: Option<Adapter>,
    /// The latest full tree while inactive, dropped by any later incremental update.
    initial: Option<TreeUpdate>,
    /// Updates that arrived while the adapter was lent out.
    queued: Vec<TreeUpdate>,
    state: State,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// No assistive technology asked yet.
    Inactive,
    /// One asked and got AccessKit's placeholder; the next update must be a full tree.
    Placeholder,
    Active,
}

struct Activation<'a> {
    initial: &'a mut Option<TreeUpdate>,
    /// Some(true): served a tree; Some(false): had none.
    served: Option<bool>,
}

impl ActivationHandler for Activation<'_> {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        let tree = self.initial.take();
        self.served = Some(tree.is_some());
        tree
    }
}

struct Actions(WindowId);

impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        app::post(Pending::A11y(self.0, A11yEvent::Action(request)));
    }
}

impl A11y {
    pub(crate) fn new(view: &Retained<NSView>, window: WindowId) -> Self {
        let ptr = Retained::as_ptr(view) as *mut std::ffi::c_void;
        // SAFETY: `view` is a live NSView; the adapter keeps only a weak reference to it. Main thread.
        let adapter = unsafe { Adapter::new(ptr, false, Actions(window)) };
        A11y {
            adapter: Some(adapter),
            initial: None,
            queued: Vec::new(),
            state: State::Inactive,
        }
    }
}

/// Lends the window's adapter to `f` with no state borrowed (AccessKit calls back into the view and
/// the action handler), then puts it back and raises the resulting events.
pub(crate) fn with_adapter<R>(
    window: WindowId,
    f: impl FnOnce(&mut Adapter, &mut dyn ActivationHandler) -> (R, Option<QueuedEvents>),
) -> Option<R> {
    let (mut adapter, mut initial) =
        app::with_window(window, |w| (w.a11y.adapter.take(), w.a11y.initial.take()))?;
    let adapter_ref = adapter.as_mut()?;
    let mut activation = Activation {
        initial: &mut initial,
        served: None,
    };
    let (r, events) = f(adapter_ref, &mut activation);
    let served = activation.served;
    let queued = app::with_window(window, |w| {
        w.a11y.adapter = adapter.take();
        if w.a11y.initial.is_none() {
            w.a11y.initial = initial.take();
        }
        match served {
            Some(true) => w.a11y.state = State::Active,
            Some(false) => w.a11y.state = State::Placeholder,
            None => {}
        }
        std::mem::take(&mut w.a11y.queued)
    })
    .unwrap_or_default();
    if let Some(e) = events {
        e.raise();
    }
    if served == Some(false) {
        app::post(Pending::A11y(window, A11yEvent::Activated));
    }
    for update in queued {
        a11y_update(window, update);
    }
    Some(r)
}

/// Sends an AccessKit tree update for `window` (main thread). Before an assistive technology asks,
/// only the latest full tree (`update.tree` set) is kept, for its first request.
pub fn a11y_update(window: WindowId, update: TreeUpdate) {
    let apply = app::with_window(window, |w| {
        if w.a11y.adapter.is_none() {
            w.a11y.queued.push(update);
            return None;
        }
        let full = update.tree.is_some();
        match (w.a11y.state, full) {
            (State::Active, _) => Some(update),
            (State::Placeholder, true) => {
                w.a11y.state = State::Active;
                Some(update)
            }
            (State::Placeholder, false) => None,
            (State::Inactive, true) => {
                w.a11y.initial = Some(update);
                None
            }
            (State::Inactive, false) => {
                // The kept tree is stale now; the first request asks the app for a fresh one.
                w.a11y.initial = None;
                None
            }
        }
    })
    .flatten();
    if let Some(update) = apply {
        with_adapter(window, |a, _| ((), a.update_if_active(|| update)));
    }
}

pub(crate) fn focus_changed(window: WindowId, focused: bool) {
    with_adapter(window, |a, _| ((), a.update_view_focus_state(focused)));
}
