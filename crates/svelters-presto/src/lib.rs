//! The svelters backend on presto (lane SVELTERS-PRESTO, `plans/cloud/presto-b2/LANE-svelters-presto.md`).
//!
//! | Module | What |
//! |---|---|
//! | [`backend`] | [`PrestoBackend`]: the backend ABI (`impl svelters_abi::Backend`) on presto's tree, and the [`Driver`] seam to whoever runs the loop |
//! | [`deliver`] | event-time delivery of what an input produced in the tree (architecture §11.1) |
//! | [`template`] | native templates: `instantiate` and `destroy_range` (compiler-and-js §3.2) |
//! | [`style`] | the `style` op: CSS engine inputs and the TR1 visual channels |
//! | [`ids`], [`wire`] | stable wire ids with fingerprints, and the registry seen through them |
//! | [`spec`] | the generated semantic spec `metadata/semantics/presto.json` |
//! | [`embed`] | many Svelte roots in one host: roots, slots, faults, settle (ARCH-EMBED, minus egui) |
//! | [`value`] | ABI values ↔ presto prop values |
//! | [`shim`] | stand-ins for presto-core and presto-widgets API not merged yet |

pub mod backend;
pub mod deliver;
pub mod embed;
pub mod ids;
pub mod shim;
pub mod spec;
pub mod style;
pub mod template;
pub mod value;
pub mod wire;

pub use backend::{Driver, PrestoBackend, SVL_LISTEN_PASSIVE, WakeState};
pub use deliver::Delivered;
pub use template::{Template, TemplateNode};

/// The built-in kinds: presto-core's base kinds and presto-widgets' (shim: the Tier 0 table).
pub fn builtin_registry() -> shim::KindRegistry {
    shim::tier0::registry()
}
