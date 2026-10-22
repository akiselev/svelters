//! Node handles: `generation << 24 | idx`, generations retired at 2^29.
//!
//! SG1 (ARCH-SCENE §4.2) replaces this with `NodeId(NonZeroU64)`, `SlotTable`, `VNodeId` and `Target`,
//! retiring generations at 2^28 (bit 52 belongs to app-export handles). Writer lane: SCENE.

use svelters_abi::svl_handle;

pub type Handle = svl_handle;

const IDX_BITS: u32 = 24;
const IDX_MASK: u64 = (1 << IDX_BITS) - 1;
pub(crate) const MAX_GEN: u32 = 1 << 29;

/// The slot index and the generation of a handle.
pub(crate) fn split(h: Handle) -> (usize, u32) {
    ((h & IDX_MASK) as usize, (h >> IDX_BITS) as u32)
}

/// The handle of slot `idx` at `generation`.
pub(crate) fn join(generation: u32, idx: usize) -> Handle {
    ((generation as u64) << IDX_BITS) | idx as u64
}
