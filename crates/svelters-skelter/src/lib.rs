//! The svelters backend on skelter (lane SVELTERS-SKELTER, `plans/cloud/presto-b2/LANE-svelters-presto.md`).
//!
//! | Module | What |
//! |---|---|
//! | [`app`] | the skelter app runner's tree ([`app::SkelterDocument`]) and handler ([`app::SveltersApp`]): windows JS presents |
//! | [`backend`] | [`SkelterBackend`]: the backend ABI (`impl svelters_abi::Backend`) on skelter's tree, and the [`Driver`] seam to whoever runs the loop |
//! | [`behavior`] | `use:` behaviours routed to the app's Rust factories, and the event tap (A2) |
//! | [`designer`] | DZ0–DZ2b: a visual designer's reads (`scene().read`: `by_source`, `hit_stack`, matched rules) and forced states, in svelters-egui's shapes |
//! | [`dump`] | the app's `.svelters/app` dump: exports, `kinds.json`, `kinds.d.ts`, behaviour modules and typings |
//! | [`deliver`] | event-time delivery of what an input produced in the tree (architecture §11.1) |
//! | [`template`] | native templates: `instantiate` and `destroy_range` (compiler-and-js §3.2) |
//! | [`style`] | the `style` op: CSS engine inputs and the TR1 visual channels |
//! | [`kinds`] | the JS-facing kinds on skelter-core's (skelter-widgets' or synthesized), with the wire ids |
//! | [`motion`] | `OP_ANIMATE` on the compositor: skelter-motion's plans, `animationend` |
//! | [`spec`] | the generated semantic spec `metadata/semantics/skelter.json` |
//! | [`sync`] | cancelable events delivered inside skelter-core's dispatch; JS ops made then, deferred |
//! | [`embed`] | many Svelte roots in one host: roots, slots, faults, settle (ARCH-EMBED, minus egui) |
//! | [`value`] | ABI values ↔ surface values ↔ skelter-core values |
//! | [`surface`] | the JS-facing kind specs skelter-core's `KindSpec` does not carry yet (Q3) |

pub mod app;
pub mod backend;
pub mod behavior;
pub mod css;
pub mod deliver;
pub mod designer;
pub mod dump;
pub mod embed;
pub mod kinds;
pub mod motion;
pub mod spec;
pub mod style;
pub mod surface;
pub mod sync;
pub mod template;
pub mod value;

pub use backend::{Driver, SkelterBackend, SVL_LISTEN_PASSIVE, Traffic, WakeState};
pub use deliver::{Delivered, Output};
pub use dump::{dump, dump_with};
pub use kinds::Kinds;
pub use template::{Template, TemplateNode};

/// The built-in kinds: skelter-widgets' where it has them, synthesized from the Tier 0 surface table
/// otherwise. Built once per thread (the registry is shared by every document on it).
pub fn builtin_kinds() -> std::sync::Arc<Kinds> {
    thread_local! {
        static KINDS: std::sync::Arc<Kinds> = std::sync::Arc::new(Kinds::builtin().expect("the built-in kinds register"));
    }
    KINDS.with(|k| k.clone())
}
