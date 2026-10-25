//! skelter's style property table: egui's (`svelters_style::tables::EGUI`, the properties the Svelte
//! sources already use) plus the ones skelter renders that egui does not: `transform`,
//! `transform-origin`, `z-index`, `visibility`, `pointer-events` and `will-change` (SKELTER-core Q1,
//! decided by the coordinator 2026-10-25).
//!
//! Built once from the generated egui table, so it follows that table's changes. When the generated
//! `svelters_style::tables::SKELTER` (lane SVELTERS-SKELTER's row in `gen_props.py`, from skelter.json's
//! style section) carries these six too, [`table`] returns it instead and this file goes.

use std::sync::OnceLock;
use svelters_style::{Prop, Table, ValueType, Via};

const fn css(name: &'static str, ty: ValueType, values: &'static [&'static str]) -> Prop {
    Prop { name, via: Via::Css, target: "", ty, values, tags: &[], inline_only: false }
}

/// The properties skelter adds to egui's table.
pub const EXTRA: &[Prop] = &[
    css("transform", ValueType::Raw, &[]),
    css("transform-origin", ValueType::Raw, &[]),
    css("z-index", ValueType::Int, &[]),
    css("visibility", ValueType::Enum, &["visible", "hidden", "collapse"]),
    css("pointer-events", ValueType::Enum, &["auto", "none"]),
    css("will-change", ValueType::Raw, &[]),
];

/// The table skelter-core cascades with (sorted by name, as [`Table::props`] requires).
pub fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        let base = &svelters_style::tables::EGUI;
        let mut props: Vec<Prop> = base.props.iter().filter(|p| !EXTRA.iter().any(|e| e.name == p.name)).copied().collect();
        props.extend_from_slice(EXTRA);
        props.sort_by(|a, b| a.name.cmp(b.name));
        Table { toolkit: "skelter", props: Box::leak(props.into_boxed_slice()), ..*base }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_egui_plus_skelter_sorted() {
        let t = table();
        assert_eq!(t.toolkit, "skelter");
        assert!(t.props.windows(2).all(|w| w[0].name < w[1].name), "sorted, no duplicates");
        for e in EXTRA {
            assert!(t.props.iter().any(|p| p.name == e.name), "{}", e.name);
        }
        for p in svelters_style::tables::EGUI.props {
            assert!(t.props.iter().any(|q| q.name == p.name), "{}", p.name);
        }
    }
}
