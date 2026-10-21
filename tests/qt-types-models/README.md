Native Qt item models feeding `@svelters/lists` on the real host (N3, `plans/milestones/N3-native-models.md`), the
twin of `tests/gtk-types-models`: `examples/counter-qt/tests/lists_models.rs` clicks the buttons by label and watches
the pooled `QListView`. Modes: a `QStringListModel` mutated from JS, a `QSortFilterProxyModel` chain,
`QStringListModel`s of 10k/100k strings and, for comparison, JS arrays of the same sizes.
