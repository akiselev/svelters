//! The change journal: what changed in the tree, in order, for svelters-presto (`bind:` reads,
//! observers, the inspector) and for tests. The frame phases do not read it; they read dirty bits.

use crate::ids::{KindId, PropId};
use presto_base::ids::{NodeId, Revision};
use svelters_style::StateMask;

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Created { node: NodeId, kind: KindId, revision: Revision },
    Inserted { parent: NodeId, child: NodeId, revision: Revision },
    Removed { parent: NodeId, child: NodeId, revision: Revision },
    Destroyed { node: NodeId, revision: Revision },
    /// `native`: written by the kind (native-owned state), which JS is told about.
    Prop { node: NodeId, prop: PropId, revision: Revision, native: bool },
    Style { node: NodeId, revision: Revision },
    State { node: NodeId, state: StateMask },
}

/// Bounded: when nobody drains it, the oldest records go (it is an observation aid, not a log).
#[derive(Default)]
pub struct Journal {
    changes: Vec<Change>,
    pub(crate) dropped: u64,
}

const CAP: usize = 1 << 16;

impl Journal {
    pub(crate) fn push(&mut self, c: Change) {
        if self.changes.len() >= CAP {
            let n = CAP / 2;
            self.changes.drain(..n);
            self.dropped += n as u64;
        }
        self.changes.push(c);
    }
    pub(crate) fn drain(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
    }
    pub fn len(&self) -> usize {
        self.changes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}
