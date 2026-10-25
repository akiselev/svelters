//! Tier 0 kinds (EGUI.md §5): the layout containers (`box`, `frame`, `stack`, `sides`, `grid`), the
//! text and media leaves (`label` with rich runs, `hyperlink`, `icon`, `image`, `separator`,
//! `progress-bar`, `spinner`) and the controls (`button`, `toggle-button`, `checkbox`, `radio`,
//! `switch`, `slider`). Each implements [`Kind`](crate::shim::core::Kind) with a
//! [`KindSpec`](crate::shim::core::KindSpec), runs the `interact` units on presto events, and records
//! with the `paint` primitives.

pub mod common;
pub mod props;
#[cfg(test)]
pub(crate) mod testkit;

pub mod button;

pub use button::Button;
