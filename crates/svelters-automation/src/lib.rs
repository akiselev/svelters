//! svelters-automation (AU1, E2E-INSPECTOR §1): one automation protocol for end-to-end tests,
//! the inspector and agents, in process.
//!
//! - **Query** ([`selector`], [`engine`]): `role=`, `label=`, `text=`, `anchor=`, `root=`, `pos=`,
//!   `nth=` and CSS (the svelters-style matcher on the scene), chained with `>>`.
//! - **Read**: [`Session::query`], [`Session::node`], [`Session::snapshot`] (SG7's text snapshot in
//!   the aria-snapshot shape), [`Session::roots`], [`Session::a11y_tree`].
//! - **Act**: semantic actions through AccessKit action requests ([`Tier::Semantic`]) or synthetic
//!   egui input at the node ([`Tier::Synthetic`]), after the actionability checks (§1.5); every
//!   act replies after the app settles, with what changed.
//! - **Wait**: [`Session::wait_settled`], [`Session::wait_for`]: the embed's busy reasons, no
//!   pending input, and the host's idle predicate ([`Session::set_idle`]: cadatomic's
//!   IdleBarrier). Timeouts count virtual time (frames × the driver's step).
//! - **Capture**: [`Session::screenshot`] (drivers with a renderer), and the snapshot.
//! - **Journal**: [`Session::journal_subscribe`] / [`Session::journal_take`] over SG2's observers.
//! - **Waits on anything** (AU7, cadatomic WAITS §3): the app's event [`journal`] with cursors
//!   ([`Session::wait_event`], [`Session::never`], [`Session::arm_seen`]), named [`busy`] sources
//!   (foreground blocks settle, background never does), actionability checks named at the timeout,
//!   clock domains (a busy wall-clock source holds the virtual clock), [`replay`] of external
//!   sources, and one [`WaitReport`] on every wait timeout.
//! - [`Driver`]: what runs the frames; kittest's harnesses implement it (feature `kittest`).
//! - [`Session::call`]: the same methods as JSON (`scene.query {selector}` …), the schema AU2's
//!   IPC transport and cadatomic's socket server carry unchanged.
//!
//! - **Out of process** (AU2): [`server::Server`] carries [`Session::call`] unchanged as NDJSON
//!   JSON-RPC 2.0 ([`rpc`]) over a local socket, on IO threads the UI thread never waits for;
//!   [`client::Client`] and [`client::App`] are its Rust side, `svelters-run` (feature `run`) the
//!   headless reference host.
//!
//! **Off by default** (the user's decision, §6 Q1): nothing answers until the app calls
//! [`enable`] (cadatomic: its settings page; tests: [`Session::for_test`]), and each session gets
//! only the [`Grants`] it is given.

pub mod busy;
pub mod client;
pub mod clock;
pub mod driver;
#[cfg(feature = "run")]
pub mod e2e;
pub mod engine;
pub mod journal;
pub mod replay;
pub mod role;
pub mod rpc;
pub mod selector;
pub mod server;
mod session;
mod snapshot;

pub use driver::{A11yNode, Driver};
/// AU1b: `#[svelters_automation::test]`, one body in process or out of process (see [`e2e`]).
#[cfg(feature = "run")]
pub use svelters_macros::test;
pub use busy::{BusyKind, BusyRegistry, BusyReport, BusySource, BusyStatus};
pub use journal::{ClockDomain, Cursor, Event, EventKind, FieldType, Journal, Matcher};
pub use selector::{Selector, TextMatch};
pub use session::{
    ActOptions, ActResult, Actionability, At, Budget, ComputedInfo, Condition, Drag, DragTo, Expect, Grants, NeverId,
    NodeInfo, Ref, RootInfo, SeenId, Session, SourceInfo, Target, Tier, Unmet, WaitReport, WaitState, Wheel, rule_json,
};

use std::sync::atomic::{AtomicBool, Ordering};

/// The protocol version [`Session::hello`] reports.
pub const PROTOCOL: u32 = 1;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turns automation on for this process (off by default). Sessions created before stay as they are;
/// [`disable`] makes every session's calls fail with `automation.disabled`.
pub fn enable() {
    ENABLED.store(true, Ordering::SeqCst);
}

/// Turns automation off: every call of every session fails with `automation.disabled`.
pub fn disable() {
    ENABLED.store(false, Ordering::SeqCst);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// A protocol error: a stable `code` (`selector.invalid`, `node.not_found`, `node.ambiguous`,
/// `node.stale`, `node.not_actionable`, `timeout`, `capability.absent`, `grant.denied`,
/// `automation.disabled`, `method.unknown`, `params.invalid`; AU7: `journal.overflow`,
/// `event.unregistered`, `event.invalid`, `event.kind_conflict`, `wait.never`, `busy.invalid`,
/// `replay.*`) and a message for people. AU7: a wait's timeout carries its [`WaitReport`] as `data`.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
    /// Structured detail (a wait's [`WaitReport::to_json`]); `None` for most errors.
    pub data: Option<serde_json::Value>,
}

impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Error {
        Error {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Error {
        self.data = Some(data);
        self
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::json!({ "code": self.code, "message": self.message });
        if let Some(d) = &self.data {
            v["data"] = d.clone();
        }
        v
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}
