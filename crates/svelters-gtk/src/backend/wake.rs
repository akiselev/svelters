//! `wake` (FU2): the thread-safe "run a host checkpoint soon".
//!
//! **Priority.** A wake is an idle source at `G_PRIORITY_DEFAULT`, the priority of GDK input events
//! and ordinary timeouts. GLib dispatches, in each iteration, only the ready sources of the best
//! ready priority, so a source below `DEFAULT` (the old `g_idle_add`, `DEFAULT_IDLE`) is starved
//! for as long as any default-priority source is ready at every iteration: a busy 1 ms timer kept
//! worker completions, store updates and other wake-driven work from ever being delivered (E1).
//! At `DEFAULT` the wake is dispatched in the same iteration as such a source. Nothing more urgent
//! is right either: `HIGH` would put JS ahead of input.
//!
//! **Redraw guard.** GTK lays out and paints from the frame clock at `GDK_PRIORITY_REDRAW`
//! (`HIGH_IDLE + 20`, below `DEFAULT`). A checkpoint that keeps re-waking (a JS-thread future that
//! yields, a completion backlog over the drain budget, a thread that wakes faster than checkpoints
//! run) would keep a default-priority source ready at every iteration and freeze the window. So after
//! [`STREAK`] wakes in a row dispatched at `DEFAULT`, the next one *yields*: it is queued just below
//! redraw (`GDK_PRIORITY_REDRAW + 1`), where it runs at the first iteration with no layout or paint
//! (or anything else above it) due, which ends the streak. That is free when the loop is calm. When
//! the loop is saturated at `DEFAULT` by someone else, a backup timeout at `DEFAULT` delivers the
//! yielded wake after [`AGE_MS`]: a wake is never starved, it is at most `AGE_MS` late.
//!
//! **Coalescing.** One wake is pending at a time: requests while one is pending are dropped (the
//! checkpoint they ask for has not run yet). The pending request is a ticket; each queued source
//! runs the checkpoint only if its ticket is still the pending one, so the loser of a yielded
//! wake's two sources does nothing. The ticket is cleared before the checkpoint runs: a wake
//! requested during the checkpoint gets a checkpoint of its own.
//!
//! **Threads.** `request` may run on any thread (atomics and `g_idle_add_full`/`g_timeout_add_full`
//! on the default main context, which GLib makes thread-safe). The host pointer is re-read at
//! dispatch: a host detached since (`start(NULL)`, `run` returned) is not called.

use gtk::glib;
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use svelters_abi::svl_host;

/// Wakes dispatched at `DEFAULT` in a row before the next one yields to redraw.
pub(super) const STREAK: u32 = 4;
/// How long a yielded wake waits at most when sources at or above redraw priority stay ready.
pub(super) const AGE_MS: u64 = 4;

pub(super) struct WakeQueue {
    host: Arc<AtomicPtr<svl_host>>,
    /// The pending request's ticket, 0 when none is pending.
    pending: AtomicU64,
    next_ticket: AtomicU64,
    /// Wakes dispatched at `DEFAULT` since one last yielded.
    streak: AtomicU32,
}

#[derive(Clone, Copy, PartialEq)]
enum Path {
    /// The `DEFAULT` idle of an ordinary wake.
    Default,
    /// The idle just below redraw of a yielded wake: everything at redraw priority had its turn.
    Yielded,
    /// The backup timeout of a yielded wake: the loop stayed busy above the yielded idle.
    Aged,
}

impl WakeQueue {
    pub(super) fn new(host: Arc<AtomicPtr<svl_host>>) -> Arc<WakeQueue> {
        Arc::new(WakeQueue { host, pending: AtomicU64::new(0), next_ticket: AtomicU64::new(1), streak: AtomicU32::new(0) })
    }

    /// Thread-safe: queues one host checkpoint unless one is already pending.
    pub(super) fn request(self: &Arc<Self>) {
        if self.host.load(Ordering::Acquire).is_null() || self.pending.load(Ordering::Acquire) != 0 {
            return;
        }
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        if self.pending.compare_exchange(0, ticket, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return;
        }
        if self.streak.load(Ordering::Acquire) < STREAK {
            let q = self.clone();
            glib::idle_add_full(glib::Priority::DEFAULT, move || {
                q.dispatch(ticket, Path::Default);
                glib::ControlFlow::Break
            });
        } else {
            let below_redraw = glib::Priority::from(gtk::gdk::PRIORITY_REDRAW as i32 + 1);
            let q = self.clone();
            glib::idle_add_full(below_redraw, move || {
                q.dispatch(ticket, Path::Yielded);
                glib::ControlFlow::Break
            });
            let q = self.clone();
            glib::timeout_add_full(Duration::from_millis(AGE_MS), glib::Priority::DEFAULT, move || {
                q.dispatch(ticket, Path::Aged);
                glib::ControlFlow::Break
            });
        }
    }

    /// On the main context's thread: runs the checkpoint if `ticket` is still the pending request.
    fn dispatch(&self, ticket: u64, path: Path) {
        if self.pending.compare_exchange(ticket, 0, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return;
        }
        match path {
            Path::Default => {
                self.streak.fetch_add(1, Ordering::AcqRel);
            }
            // Aged: the loop is saturated by others; yielding again would only delay the next wake.
            Path::Yielded | Path::Aged => self.streak.store(0, Ordering::Release),
        }
        let h = self.host.load(Ordering::Acquire);
        if !h.is_null() {
            unsafe { ((*h).checkpoint)((*h).self_) };
        }
    }
}
