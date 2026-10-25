//! Stand-ins for APIs the views need that their owning crates do not have yet; the consolidation
//! replaces each and deletes this module.
//!
//! - [`fast`]: a kind's fast commits of native-owned properties (architecture §7.3). The crate's kind
//!   interface ([`crate::shim::core::EventCx`]) and presto-core's `EventCx` have no way to send one;
//!   the views queue them per thread and the app runner drains them for the compositor.

pub mod fast {
    //! The per-thread queue of fast commits the views made while handling input.

    use presto_base::commit::FastCommit;
    use std::cell::RefCell;

    thread_local! {
        static FAST: RefCell<Vec<FastCommit>> = const { RefCell::new(Vec::new()) };
    }

    /// Queues `f` for the compositor.
    pub fn push(f: FastCommit) {
        FAST.with(|q| q.borrow_mut().push(f));
    }

    /// The fast commits queued on this thread since the last call, in order.
    pub fn take() -> Vec<FastCommit> {
        FAST.with(|q| std::mem::take(&mut *q.borrow_mut()))
    }
}
