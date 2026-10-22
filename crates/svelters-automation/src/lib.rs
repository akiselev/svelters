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

pub mod client;
pub mod clock;
pub mod driver;
#[cfg(feature = "run")]
pub mod e2e;
pub mod engine;
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
pub use selector::{Selector, TextMatch};
pub use session::{
    ActOptions, ActResult, Expect, Grants, NodeInfo, Ref, RootInfo, Session, Target, Tier,
    WaitState,
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
/// `automation.disabled`, `method.unknown`, `params.invalid`) and a message for people.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}

impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Error {
        Error {
            code,
            message: message.into(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "code": self.code, "message": self.message })
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}
