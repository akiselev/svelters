//! The tree's storage: the slot table and the per-node record types. The columns themselves live in
//! [`crate::Document`], indexed by `NodeId::index()`; the edit paths are in `document.rs`.

mod node;
pub(crate) mod slots;

pub use node::{Dirty, ListenFlags, Listener, NodeFlags};
pub(crate) use node::{LayoutBox, NodeCore, PropStore};
