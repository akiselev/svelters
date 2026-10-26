//! AccessKit's own consumers on the app's `TreeUpdate`s (`plans/cloud/qc-a11y/PLAN-focus-and-text.md`
//! §5, Q1): what a platform adapter would compute, without a platform.
//!
//! [`Consumer`] folds every update into a real `accesskit_consumer::Tree` built with
//! `is_host_focused = true` (so `focus()` is the effective focus: followed into grafted subtrees, the
//! active descendant applied), and records each focus move its change handler reports. On Linux it
//! feeds the same updates to an `accesskit_atspi_common::Adapter` ([`Atspi`]) whose `AdapterCallback`
//! records `register_interfaces` and `emit_event`: AT-SPI's own logic (interfaces, states, events,
//! extents, text) without D-Bus.
//!
//! The consumers panic where a real adapter would (a focused graft without its subtree, a subtree
//! pushed before its graft node): a test on the [`Consumer`] fails exactly where the app would crash.
//!
//! On a [`Harness`](crate::Harness), turn it on with
//! [`HarnessBuilder::with_consumer`](crate::HarnessBuilder::with_consumer) and read it with
//! [`Harness::consumer`](crate::Harness::consumer). Anything else that has the updates (a skelter-core
//! document's `a11y_update`, `CoreIslands`) calls [`Consumer::apply`] itself, host update first, as the
//! app sends them.

use accesskit_consumer::{FullNodeId, NodeRef, Tree, TreeChangeHandler, TreeState};
use skelter_base::accesskit::{ActionRequest, NodeId, Rect, TreeId, TreeUpdate};
use std::sync::{Arc, Mutex};

/// A node's address in the consumer: its id and the tree it belongs to (`NodeRef::locate`).
pub type Located = (NodeId, TreeId);

/// One focus move the consumer's change handler reported (old and new effective focus).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusMove {
    pub from: Option<Located>,
    pub to: Option<Located>,
}

/// Records focus moves.
#[derive(Default)]
struct Moves(Vec<FocusMove>);

impl TreeChangeHandler for Moves {
    fn node_added(&mut self, _: &NodeRef) {}
    fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
    fn focus_moved(&mut self, old: Option<&NodeRef>, new: Option<&NodeRef>) {
        self.0.push(FocusMove {
            from: old.map(|n| n.locate()),
            to: new.map(|n| n.locate()),
        });
    }
    fn node_removed(&mut self, _: &NodeRef) {}
}

/// A real `accesskit_consumer::Tree` (and on Linux an AT-SPI adapter) on one window's updates.
pub struct Consumer {
    tree: Option<Tree>,
    /// Subtree updates that came before the first host update (applied right after it).
    early: Vec<TreeUpdate>,
    moves: Vec<FocusMove>,
    updates: usize,
    #[cfg(target_os = "linux")]
    atspi: Option<Atspi>,
    #[cfg(target_os = "linux")]
    with_atspi: bool,
    #[cfg(target_os = "linux")]
    bounds: accesskit_atspi_common::WindowBounds,
}

impl Default for Consumer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Consumer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Consumer")
            .field("updates", &self.updates)
            .field("focus", &self.focus_located())
            .field("moves", &self.moves.len())
            .finish()
    }
}

impl Consumer {
    /// A consumer tree, and on Linux the AT-SPI adapter beside it.
    pub fn new() -> Self {
        Consumer {
            tree: None,
            early: Vec::new(),
            moves: Vec::new(),
            updates: 0,
            #[cfg(target_os = "linux")]
            atspi: None,
            #[cfg(target_os = "linux")]
            with_atspi: true,
            #[cfg(target_os = "linux")]
            bounds: Default::default(),
        }
    }

    /// Only the consumer tree (no AT-SPI adapter, on any platform).
    pub fn tree_only() -> Self {
        #[allow(unused_mut)]
        let mut c = Self::new();
        #[cfg(target_os = "linux")]
        {
            c.with_atspi = false;
        }
        c
    }

    /// Fold one update in, as a platform adapter does (the first one must be the host tree's, with its
    /// `tree` info; a subtree update before it waits for it).
    pub fn apply(&mut self, u: &TreeUpdate) {
        self.updates += 1;
        let Some(tree) = &mut self.tree else {
            if u.tree_id != TreeId::ROOT {
                self.early.push(u.clone());
                return;
            }
            let tree = Tree::new(u.clone(), true);
            #[cfg(target_os = "linux")]
            if self.with_atspi {
                self.atspi = Some(Atspi::new(u.clone(), self.bounds));
            }
            self.tree = Some(tree);
            for e in std::mem::take(&mut self.early) {
                self.updates -= 1;
                self.apply(&e);
            }
            return;
        };
        let mut moves = Moves::default();
        tree.update_and_process_changes(u.clone(), &mut moves);
        self.moves.extend(moves.0);
        #[cfg(target_os = "linux")]
        if let Some(a) = &mut self.atspi {
            a.adapter.update(u.clone());
        }
    }

    /// The window's bounds the AT-SPI adapter places window coordinates in (screen extents add the
    /// inner origin). Before the first update it seeds the adapter's; after, it updates it.
    pub fn set_window_bounds(&mut self, outer: Rect, inner: Rect) {
        #[cfg(target_os = "linux")]
        {
            self.bounds = accesskit_atspi_common::WindowBounds::new(outer, inner);
            if let Some(a) = &mut self.atspi {
                a.adapter.set_root_window_bounds(self.bounds);
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (outer, inner);
    }

    /// Updates folded in so far.
    pub fn updates(&self) -> usize {
        self.updates
    }

    /// The consumer tree (`None` before the first host update).
    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    /// The tree's state.
    #[track_caller]
    pub fn state(&self) -> &TreeState {
        self.tree.as_ref().expect("no host update folded in yet").state()
    }

    /// The effective focus (`TreeState::focus`: through grafts, the active descendant applied).
    pub fn focus(&self) -> Option<NodeRef<'_>> {
        self.tree.as_ref()?.state().focus()
    }

    /// The effective focus's address.
    pub fn focus_located(&self) -> Option<Located> {
        self.focus().map(|n| n.locate())
    }

    /// The effective focus's author id.
    pub fn focus_author_id(&self) -> Option<&str> {
        self.focus().and_then(|n| n.data().author_id())
    }

    /// Every focus move since the tree was built (the initial focus is not a move).
    pub fn focus_moves(&self) -> &[FocusMove] {
        &self.moves
    }

    /// Forget the recorded focus moves.
    pub fn take_focus_moves(&mut self) -> Vec<FocusMove> {
        std::mem::take(&mut self.moves)
    }

    /// The node at an address.
    pub fn node(&self, at: Located) -> Option<NodeRef<'_>> {
        self.tree.as_ref()?.state().node_by_tree_local_id(at.0, at.1)
    }

    /// The first node, in tree order through grafts, whose author id is `id`.
    pub fn by_author_id(&self, id: &str) -> Option<NodeRef<'_>> {
        self.find(|n| n.data().author_id() == Some(id))
    }

    /// The first node, in tree order through grafts, that `f` accepts.
    pub fn find(&self, f: impl Fn(&NodeRef<'_>) -> bool) -> Option<NodeRef<'_>> {
        fn walk<'a>(n: NodeRef<'a>, f: &dyn Fn(&NodeRef<'_>) -> bool) -> Option<NodeRef<'a>> {
            if f(&n) {
                return Some(n);
            }
            n.children().find_map(|c| walk(c, f))
        }
        walk(self.tree.as_ref()?.state().root(), &f)
    }

    /// The AT-SPI adapter (Linux, unless built [`tree_only`](Self::tree_only); `None` before the first
    /// host update).
    #[cfg(target_os = "linux")]
    pub fn atspi(&self) -> Option<&Atspi> {
        self.atspi.as_ref()
    }

    #[cfg(target_os = "linux")]
    pub fn atspi_mut(&mut self) -> Option<&mut Atspi> {
        self.atspi.as_mut()
    }
}

#[cfg(target_os = "linux")]
pub use atspi::{Atspi, AtspiLog};

#[cfg(target_os = "linux")]
mod atspi {
    use super::*;
    use accesskit_atspi_common::{
        ActionHandlerNoMut, Adapter, AdapterCallback, AppContext, Event, InterfaceSet, NodeIdOrRoot,
        ObjectEvent, PlatformNode, State, WindowBounds,
    };
    use std::collections::HashMap;
    use std::sync::RwLock;

    pub use accesskit_atspi_common::{CoordType, Rect as AtspiRect};
    pub use atspi_common::Interface;

    /// What the adapter's callback saw.
    #[derive(Debug, Default)]
    pub struct AtspiLog {
        /// Every event, in order.
        pub events: Vec<Event>,
        /// The interfaces each node registered (minus those it unregistered since).
        pub interfaces: HashMap<FullNodeId, InterfaceSet>,
    }

    struct Recorder(Arc<Mutex<AtspiLog>>);

    impl AdapterCallback for Recorder {
        fn register_interfaces(&self, _: &Adapter, id: FullNodeId, interfaces: InterfaceSet) {
            let mut log = self.0.lock().unwrap();
            let e = log.interfaces.entry(id).or_insert_with(InterfaceSet::empty);
            for i in interfaces {
                e.insert(i);
            }
        }
        fn unregister_interfaces(&self, _: &Adapter, id: FullNodeId, interfaces: InterfaceSet) {
            let mut log = self.0.lock().unwrap();
            if let Some(e) = log.interfaces.get(&id) {
                let left: InterfaceSet = e.iter().filter(|i| !interfaces.contains(*i)).collect();
                if left.bits() == 0 {
                    log.interfaces.remove(&id);
                } else {
                    log.interfaces.insert(id, left);
                }
            }
        }
        fn emit_event(&self, _: &Adapter, event: Event) {
            self.0.lock().unwrap().events.push(event);
        }
    }

    struct Actions(Arc<Mutex<Vec<ActionRequest>>>);

    impl ActionHandlerNoMut for Actions {
        fn do_action(&self, request: ActionRequest) {
            self.0.lock().unwrap().push(request);
        }
    }

    /// An `accesskit_atspi_common::Adapter` on the same updates as the [`Consumer`]'s tree. Its node ids
    /// ([`FullNodeId`]) equal the consumer tree's: both number trees in the order their updates arrive.
    pub struct Atspi {
        pub(super) adapter: Adapter,
        log: Arc<Mutex<AtspiLog>>,
        actions: Arc<Mutex<Vec<ActionRequest>>>,
    }

    impl Atspi {
        pub(super) fn new(initial: TreeUpdate, bounds: WindowBounds) -> Self {
            let log = Arc::new(Mutex::new(AtspiLog::default()));
            let actions = Arc::new(Mutex::new(Vec::new()));
            let app: Arc<RwLock<AppContext>> = AppContext::new(Some("skelter-test".into()));
            let adapter = Adapter::with_wrapped_action_handler(
                accesskit_atspi_common::next_adapter_id(),
                &app,
                Recorder(log.clone()),
                initial,
                true,
                bounds,
                Arc::new(Actions(actions.clone())),
            );
            Atspi { adapter, log, actions }
        }

        /// The adapter.
        pub fn adapter(&self) -> &Adapter {
            &self.adapter
        }

        /// The AT-SPI object for a node (Text, Component, … methods as a screen reader calls them).
        pub fn node(&self, id: FullNodeId) -> PlatformNode {
            self.adapter.platform_node(id)
        }

        /// The callback's log.
        pub fn log(&self) -> std::sync::MutexGuard<'_, AtspiLog> {
            self.log.lock().unwrap()
        }

        /// The events since the last take, removed from the log.
        pub fn take_events(&self) -> Vec<Event> {
            std::mem::take(&mut self.log().events)
        }

        /// The interfaces a node has registered.
        pub fn interfaces(&self, id: FullNodeId) -> InterfaceSet {
            self.log().interfaces.get(&id).copied().unwrap_or_else(InterfaceSet::empty)
        }

        /// The targets of `StateChanged(Focused, true)`, in order (what a screen reader announces).
        pub fn focused_events(&self) -> Vec<FullNodeId> {
            self.log()
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::Object {
                        target: NodeIdOrRoot::Node(id),
                        event: ObjectEvent::StateChanged(State::Focused, true),
                    } => Some(*id),
                    _ => None,
                })
                .collect()
        }

        /// The action requests AT-SPI calls made (`grab_focus`, `set_selection`, …), removed; a harness
        /// test delivers them with `Harness::accesskit_action`.
        pub fn take_actions(&self) -> Vec<ActionRequest> {
            std::mem::take(&mut *self.actions.lock().unwrap())
        }
    }

    impl std::fmt::Debug for Atspi {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Atspi").field("log", &*self.log()).finish()
        }
    }
}

#[cfg(target_os = "linux")]
pub use atspi::{AtspiRect, CoordType, Interface};
