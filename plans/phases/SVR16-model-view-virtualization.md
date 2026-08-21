# SVR16 - model/view subsystem and virtualization

## Goal

Support serious lists, trees, tables, selections, sorting and filtering without representing every dataset row as a permanently instantiated native widget subtree.

## Data model

Create a renderer-neutral observable model API with stable row/item keys and incremental changes:

```text
Insert
Remove
Move
Replace
UpdateFields
Reset
```

Represent selection and expansion models separately from data. Define synchronous row access plus an async/paged extension for large datasets. Renderer callbacks must not retain temporary JS values outside explicit host-owned handles/snapshots.

## Backend mapping

- QtWidgets: `QAbstractItemModel`/selection models and native views/delegates.
- GTK4: `GListModel`, selection models and list/grid/column factories.
- egui: visible-range/virtualized traversal with stable IDs.

Use Svelte snippets/templates as row/cell renderer definitions where practical, but do not create 100k graph children to back a virtual list.

## Portable API

Add portable List, Tree, Table/Column definitions, Select/Combo model binding, selection, sorting/filtering hooks and row activation/context events. Native Qt/GTK model/view APIs remain available through backend packages.

## Acceptance

- 100k-row benchmark does not create 100k native widget trees;
- insert/remove/move/update preserve selection and stable row identity;
- Qt and GTK use native model/view infrastructure rather than manual child churn;
- egui renders only visible/needed rows;
- model changes triggered from JS and native selection changes round-trip without feedback loops.
