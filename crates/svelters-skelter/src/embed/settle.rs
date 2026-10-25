//! Settling (ARCH-EMBED §5.4, SV35, SVP4): [`Busy`] says *why* an embed is not idle, [`Settle`] adds
//! the next timer, and [`BusyProbe`] mirrors the bits for other threads (cadatomic's `IdleBarrier`
//! takes `Fn() -> usize + Send + Sync` busy sources). As svelters-egui's `embed/settle.rs`, plus
//! skelter's `COMMIT_IN_FLIGHT` (architecture §16.4).

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

/// Why an embed is busy, as a bit set. Empty means settled: no JS, layout or command is pending
/// and no timer is due within the horizon.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Busy(u32);

impl Busy {
    /// Native events queued for JS (or a wake that will run a checkpoint).
    pub const EVENTS: Busy = Busy(1);
    /// Observed props announced changed, not delivered yet.
    pub const CHANGES: Busy = Busy(2);
    /// Promise jobs queued.
    pub const JOBS: Busy = Busy(4);
    /// Offloaded export calls (E1) in flight.
    pub const OPS_IN_FLIGHT: Busy = Busy(8);
    /// E3 store fields or emitter events not delivered yet.
    pub const STORES: Busy = Busy(16);
    /// An animation frame was requested.
    pub const FRAME: Busy = Busy(32);
    /// Queued mount, props, unmount, close or restart commands, or root info not sent yet.
    pub const COMMANDS: Busy = Busy(64);
    /// A visible slot with a root has not been laid out yet.
    pub const LAYOUT: Busy = Busy(128);
    /// The tree produced events or changes not delivered yet.
    pub const OUTBOX: Busy = Busy(256);
    /// A root is mounting.
    pub const MOUNTING: Busy = Busy(512);
    /// A reload is pending (reserved: the module loader / HMR track).
    pub const RELOAD: Busy = Busy(1024);
    /// A timer is due within the horizon (`EmbedConfig::timer_horizon`).
    pub const TIMER_SOON: Busy = Busy(2048);
    /// A provider reports work (`Widget::busy`, a wanting, kicked or busy state provider; ARCH-EMBED
    /// R8, ARCH-SCENE R16; SG4).
    pub const PROVIDER: Busy = Busy(4096);
    /// A commit is on its way to the compositor and not applied yet (skelter, architecture §16.4).
    pub const COMMIT_IN_FLIGHT: Busy = Busy(8192);

    const NAMES: [(Busy, &'static str); 14] = [
        (Busy::EVENTS, "EVENTS"),
        (Busy::CHANGES, "CHANGES"),
        (Busy::JOBS, "JOBS"),
        (Busy::OPS_IN_FLIGHT, "OPS_IN_FLIGHT"),
        (Busy::STORES, "STORES"),
        (Busy::FRAME, "FRAME"),
        (Busy::COMMANDS, "COMMANDS"),
        (Busy::LAYOUT, "LAYOUT"),
        (Busy::OUTBOX, "OUTBOX"),
        (Busy::MOUNTING, "MOUNTING"),
        (Busy::RELOAD, "RELOAD"),
        (Busy::TIMER_SOON, "TIMER_SOON"),
        (Busy::PROVIDER, "PROVIDER"),
        (Busy::COMMIT_IN_FLIGHT, "COMMIT_IN_FLIGHT"),
    ];

    pub const fn empty() -> Busy {
        Busy(0)
    }
    pub const fn bits(self) -> u32 {
        self.0
    }
    pub const fn from_bits(bits: u32) -> Busy {
        Busy(bits)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, other: Busy) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: Busy) {
        self.0 |= other.0;
    }
    pub fn set(&mut self, other: Busy, on: bool) {
        if on {
            self.0 |= other.0;
        }
    }
}

impl std::ops::BitOr for Busy {
    type Output = Busy;
    fn bitor(self, o: Busy) -> Busy {
        Busy(self.0 | o.0)
    }
}

impl std::ops::BitOrAssign for Busy {
    fn bitor_assign(&mut self, o: Busy) {
        self.0 |= o.0;
    }
}

impl fmt::Debug for Busy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("Busy(settled)");
        }
        let names: Vec<&str> = Busy::NAMES.iter().filter(|(b, _)| self.contains(*b)).map(|(_, n)| *n).collect();
        write!(f, "Busy({})", names.join(" | "))
    }
}

/// What [`crate::embed::Embed::settled`] answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settle {
    pub busy: Busy,
    /// The earliest live JS timer, also one beyond the horizon (a test can advance a clock to it).
    pub next_timer: Option<Instant>,
}

impl Settle {
    pub fn is_settled(&self) -> bool {
        self.busy.is_empty()
    }
}

/// A `Send + Sync` mirror of [`Settle::busy`], written at the end of every `logic` (at worst one
/// frame stale; an idle barrier's N settled frames cover that).
///
/// ```ignore
/// barrier.add_busy_source({ let p = embed.busy_probe(); move || p.count() });
/// ```
#[derive(Clone, Default)]
pub struct BusyProbe(pub(crate) Arc<AtomicU32>);

impl BusyProbe {
    pub fn busy(&self) -> Busy {
        Busy(self.0.load(Ordering::Acquire))
    }
    /// The number of busy reasons (0 = settled), the shape of a cadatomic busy source.
    pub fn count(&self) -> usize {
        self.busy().0.count_ones() as usize
    }
    pub(crate) fn publish(&self, b: Busy) {
        self.0.store(b.0, Ordering::Release);
    }
}
