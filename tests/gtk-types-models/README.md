Native GTK list models feeding `@svelters/lists` on the real host (N3, `plans/milestones/N3-native-models.md`):
`crates/svelters-gtk/tests/lists_models.rs` clicks the buttons by label and watches the native view. One
`<ListView>` shows, by mode, a `Gio.ListStore` mutated from JS, a filter/sort chain over another store, a store of
`Gtk.Adjustment`s (reactive getters on items), a `Gtk.SingleSelection` used as is, a `Gtk.DirectoryList` of a
directory the test generates, `Gtk.StringList`s of 10k and 100k strings and, for comparison, JS arrays of the same
sizes. Not an op-log fixture (it needs a real model behind the handles).
