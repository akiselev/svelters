//! The root table: one mount into one slot, its state, its last props, its info, its faults
//! (ARCH-EMBED §4.1, §6.2), as svelters-egui's `embed/roots.rs` with skelter's window ids.

use super::{EntryId, Props, RootKey, SlotId};
use skelter_base::ids::WindowId;
use std::collections::VecDeque;
use std::time::Instant;
use svelters_abi::Value;
use svelters_host::JsError;

/// A mounted root's id: unique for the embed's lifetime, never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RootId(pub(crate) u32);

impl RootId {
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Who runs the root's JS. `Isolate` is reserved for SV15 (third-party code in its own runtime).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum RootOwner {
    #[default]
    InProcess,
    Isolate(u32),
}

/// Why a root faulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultCause {
    /// Its first render threw, or a render or effect error reached its boundary later.
    Render,
    /// Too many event-handler exceptions within the policy's window (`FaultPolicy`).
    Handler,
    /// One entry into its JS ran past the interrupt budget and was stopped.
    Interrupt,
    /// A runtime-wide failure (out of memory): every root faulted.
    Runtime,
}

/// A root's failure: what the slot's fault UI shows.
#[derive(Clone, Debug)]
pub struct RootError {
    pub cause: FaultCause,
    pub error: JsError,
}

#[derive(Clone, Debug)]
pub enum RootState {
    /// The slot has no root.
    Empty,
    /// Queued: mounts at the next `logic`.
    Mounting,
    Live,
    /// The root was disposed after a failure; the slot shows the fault UI. `restart_root` mounts it
    /// again with its last props.
    Faulted(RootError),
    /// Queued: unmounts at the next `logic`.
    Unmounting,
}

impl RootState {
    pub fn is_live(&self) -> bool {
        matches!(self, RootState::Live)
    }
    pub fn is_faulted(&self) -> bool {
        matches!(self, RootState::Faulted(_))
    }
}

/// What the host can read about a root ([`super::Embed::root_info`]).
#[derive(Clone, Debug)]
pub struct RootInfo {
    pub key: RootKey,
    pub slot: SlotId,
    pub state: RootState,
    /// Written by JS (`root().title`, `root().dirty`), for the pane's tab.
    pub title: String,
    pub dirty: bool,
    /// The slot's last laid-out size, in points.
    pub size: [f32; 2],
    pub window: Option<WindowId>,
    pub visible: bool,
    pub owner: RootOwner,
}

/// The `svelters:embed` fields the host owns, as last sent to JS.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SentInfo {
    pub key: String,
    pub visible: bool,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub viewport: String,
    /// Keyboard focus is inside the root (EM2).
    pub focused: bool,
}

impl SentInfo {
    /// The fields of `self` that differ from `old`, as a patch.
    pub fn diff(&self, old: Option<&SentInfo>) -> Vec<(String, Value)> {
        let mut p = Vec::new();
        let mut put = |k: &str, changed: bool, v: Value| {
            if changed {
                p.push((k.to_string(), v));
            }
        };
        put("key", old.is_none_or(|o| o.key != self.key), Value::Str(self.key.clone()));
        put("visible", old.is_none_or(|o| o.visible != self.visible), Value::Bool(self.visible));
        put("width", old.is_none_or(|o| o.width != self.width), Value::F64(self.width));
        put("height", old.is_none_or(|o| o.height != self.height), Value::F64(self.height));
        put("scale", old.is_none_or(|o| o.scale != self.scale), Value::F64(self.scale));
        put("viewport", old.is_none_or(|o| o.viewport != self.viewport), Value::Str(self.viewport.clone()));
        put("focused", old.is_none_or(|o| o.focused != self.focused), Value::Bool(self.focused));
        p
    }
}

pub(crate) struct Root {
    pub key: RootKey,
    pub entry: EntryId,
    pub export: String,
    pub slot: SlotId,
    pub owner: RootOwner,
    /// The props as last merged (for `restart_root`).
    pub props: Vec<(String, Value)>,
    pub state: RootState,
    pub title: String,
    pub dirty: bool,
    pub sent: Option<SentInfo>,
    /// Handler exceptions inside the fault policy's window.
    pub handler_errors: VecDeque<Instant>,
}

impl Root {
    pub fn new(key: RootKey, entry: EntryId, export: String, slot: SlotId, owner: RootOwner, props: &Props) -> Root {
        let mut r = Root {
            key,
            entry,
            export,
            slot,
            owner,
            props: Vec::new(),
            state: RootState::Mounting,
            title: String::new(),
            dirty: false,
            sent: None,
            handler_errors: VecDeque::new(),
        };
        props.merge_into(&mut r.props);
        r
    }
}
