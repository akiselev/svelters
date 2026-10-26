//! A window's overlay stack (architecture §14): skelter-core's [`skelter_core::overlay`], which the
//! document keeps per window (its top layer: paint order by band, hit routing with modal blocking,
//! light-dismiss chains, Escape one level, focus restore, `LayerDelta`s). Kinds open a surface by
//! recording an overlay part (`skelter_core::RecordCx::overlay`; hosted kinds through
//! [`crate::shim::core::Kind::record_overlay`]) and hear of its closing in `overlay_closed`.

pub use skelter_core::overlay::*;
