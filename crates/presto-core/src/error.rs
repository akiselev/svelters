//! Errors of the tree and prop write paths.

use crate::ids::{PropId, SlotId};
use crate::value::ValueType;
use presto_base::ids::{NodeId, Revision};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeError {
    /// The node was destroyed (or the id was never a node of this document).
    Stale(NodeId),
    /// The parent's kind has no children.
    NotAContainer(NodeId),
    /// Inserting would make a node its own ancestor.
    Cycle { parent: NodeId, child: NodeId },
    /// `before` (or the removed node) is not a child of `parent`.
    NotAChild { parent: NodeId, child: NodeId },
    /// The parent's kind has no such slot.
    UnknownSlot { parent: NodeId, slot: SlotId },
    /// The slot takes one child and has one.
    SlotFull { parent: NodeId, slot: SlotId },
    /// A window root cannot be inserted under another node.
    IsRoot(NodeId),
}

impl std::fmt::Display for TreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TreeError::Stale(n) => write!(f, "stale node {n:?}"),
            TreeError::NotAContainer(n) => write!(f, "node {n:?} cannot have children"),
            TreeError::Cycle { parent, child } => write!(f, "inserting {child:?} under {parent:?} makes a cycle"),
            TreeError::NotAChild { parent, child } => write!(f, "{child:?} is not a child of {parent:?}"),
            TreeError::UnknownSlot { parent, slot } => write!(f, "{parent:?} has no slot {slot:?}"),
            TreeError::SlotFull { parent, slot } => write!(f, "slot {slot:?} of {parent:?} holds one child"),
            TreeError::IsRoot(n) => write!(f, "{n:?} is a window root"),
        }
    }
}

impl std::error::Error for TreeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropError {
    Stale(NodeId),
    /// Neither the node's kind nor the common props have this prop.
    Unknown(PropId),
    WrongType { prop: PropId, expected: ValueType, got: ValueType },
    /// The kind rejected the value (its message).
    Rejected(String),
    /// A JS write of a native-owned prop the kind changed after the event JS saw (architecture §4.5).
    /// The write is ignored.
    Superseded { prop: PropId, changed: Revision, seen: Revision },
}

impl std::fmt::Display for PropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropError::Stale(n) => write!(f, "stale node {n:?}"),
            PropError::Unknown(p) => write!(f, "unknown prop {p:?}"),
            PropError::WrongType { prop, expected, got } => write!(f, "prop {prop:?} takes {}, got {}", expected.name(), got.name()),
            PropError::Rejected(m) => write!(f, "{m}"),
            PropError::Superseded { prop, changed, seen } => {
                write!(f, "prop {prop:?} changed natively at revision {} after the write's revision {}", changed.0, seen.0)
            }
        }
    }
}

impl std::error::Error for PropError {}
