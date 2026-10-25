//! The svelters backend on presto (lane SVELTERS-PRESTO, `plans/cloud/presto-b2/LANE-svelters-presto.md`).
//!
//! | Module | What |
//! |---|---|
//! | [`app`] | the presto app runner's tree ([`app::PrestoDocument`]) and handler ([`app::SveltersApp`]): windows JS presents |
//! | [`backend`] | [`PrestoBackend`]: the backend ABI (`impl svelters_abi::Backend`) on presto's tree, and the [`Driver`] seam to whoever runs the loop |
//! | [`deliver`] | event-time delivery of what an input produced in the tree (architecture §11.1) |
//! | [`template`] | native templates: `instantiate` and `destroy_range` (compiler-and-js §3.2) |
//! | [`style`] | the `style` op: CSS engine inputs and the TR1 visual channels |
//! | [`kinds`] | the JS-facing kinds on presto-core's (presto-widgets' or synthesized), with the wire ids |
//! | [`motion`] | `OP_ANIMATE` on the compositor: presto-motion's plans, `animationend` |
//! | [`spec`] | the generated semantic spec `metadata/semantics/presto.json` |
//! | [`sync`] | cancelable events delivered inside presto-core's dispatch; JS ops made then, deferred |
//! | [`embed`] | many Svelte roots in one host: roots, slots, faults, settle (ARCH-EMBED, minus egui) |
//! | [`value`] | ABI values ↔ surface values ↔ presto-core values |
//! | [`surface`] | the JS-facing kind specs presto-core's `KindSpec` does not carry yet (Q3) |

pub mod app;
pub mod backend;
pub mod css;
pub mod deliver;
pub mod embed;
pub mod kinds;
pub mod motion;
pub mod spec;
pub mod style;
pub mod surface;
pub mod sync;
pub mod template;
pub mod value;

pub use backend::{Driver, PrestoBackend, SVL_LISTEN_PASSIVE, WakeState};
pub use deliver::{Delivered, Output};
pub use kinds::Kinds;
pub use template::{Template, TemplateNode};

/// The built-in kinds: presto-widgets' where it has them, synthesized from the Tier 0 surface table
/// otherwise. Built once per thread (the registry is shared by every document on it).
pub fn builtin_kinds() -> std::sync::Arc<Kinds> {
    thread_local! {
        static KINDS: std::sync::Arc<Kinds> = std::sync::Arc::new(Kinds::builtin().expect("the built-in kinds register"));
    }
    KINDS.with(|k| k.clone())
}
