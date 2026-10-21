//! Non-GUI WinRT types (N4, plans/research/NON-GUI-TYPES.md): every bound runtime class is a SHARED handle, and its
//! members are reached through the generated tables (`generated/types_tables.rs`, data only) and one dynamic invoker.
//!
//! Platform-neutral: [`model`] (table shapes and lookups), [`invoke`] (the Microsoft x64 call shapes), [`value`]
//! (scalars, enums, records). Windows only: the COM core (`rt`: identity, activation, strings, boxing, collections),
//! delegates (`delegate`: events, async completions with the hop to the UI thread) and the backend ops (`ops`).
pub mod invoke;
pub mod model;
#[path = "../generated/types_tables.rs"]
#[rustfmt::skip]
pub mod tables;
pub mod value;

#[cfg(windows)]
pub(crate) mod delegate;
#[cfg(windows)]
pub(crate) mod ops;
#[cfg(windows)]
pub(crate) mod rt;
