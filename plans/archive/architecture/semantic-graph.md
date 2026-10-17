# Semantic and accessibility graph

The canonical Svelters graph carries user-facing semantics from its first implementation. Accessibility, agent inspection, testing, and devtools are projections of the same semantic data; they are not late backend-specific annotations layered onto a visual widget tree.

## Identity layers

Keep three identities distinct:

- `NodeId`: runtime graph handle. Cheap, generational, and allowed to change when a logical node is destroyed/recreated.
- `NodeIdentity`: framework identity used for HMR/rebuild correlation and egui stable IDs. Derived from component instance/template source/keyed path or another deterministic construction.
- `DeveloperId`: optional application-supplied stable semantic identifier. This may be exposed to accessibility/devtools/agent surfaces where appropriate. It is never synthesized from a raw native pointer.

Source locations and HMR identity are debugging metadata, not automatically public accessibility identifiers.

## Semantic model

Every element may carry a toolkit-neutral semantic record independent of its visual/native properties:

```rust
struct Semantics {
    role: Option<SemanticRole>,
    developer_id: Option<DeveloperId>,
    name: Option<String>,
    description: Option<String>,
    value: Option<SemanticValue>,
    states: SemanticStates,
    actions: SemanticActions,
    relations: SemanticRelations,
}
```

The exact Rust representation may use interned IDs/bitsets, but the contract must represent at least:

- common roles such as window, dialog, button, checkbox, radio, textbox, label/text, list, list item, tree, tree item, table/grid, row/cell, menu/menuitem, toolbar, tab/tablist, slider/progress/spin, link and generic group;
- accessible name and description;
- scalar/text value where meaningful;
- enabled/disabled, focused/focusable, checked/mixed, selected/selectable, expanded/collapsed, readonly/editable, hidden/visible and modal states;
- actions including activate, focus, set value/text, toggle, select, expand/collapse, increment/decrement, show menu and scroll into view;
- labelled-by/described-by/control/ownership-style relations using logical NodeIds.

Do not model this as ARIA strings or Qt/GTK enum values. Backend adapters translate from the canonical model to their native accessibility APIs.

## Derivation and explicit overrides

Portable widgets provide default role/state/action semantics. Native widgets may contribute defaults through API/backend metadata. Application-authored semantics can override or enrich defaults through Svelte props/actions/attachments.

Semantics must remain inspectable before any native renderer exists. TestHost is therefore the first semantic backend.

A backend-specific native property such as `QAccessible::Role` or a GTK accessibility enum is not the source of truth. Native accessibility output is a realization of the canonical graph just like a native visual widget is.

## Mutation and batching

Semantic changes participate in the same graph transaction/patch batch as visual/property changes. A renderer must never observe a new visual state with stale semantic state because the two were published separately.

The graph API may represent this as dedicated semantic patches or as a typed semantic field update, but normalized snapshots must distinguish semantic changes from arbitrary toolkit properties.

## Backend obligations

Each renderer reports a semantic capability profile and must map the supported canonical semantics into its native accessibility surface:

```text
GTK4      -> GTK accessibility / AT-SPI-visible semantics
QtWidgets -> QAccessible interfaces/events
egui      -> egui/accessibility output
TestHost  -> normalized semantic snapshot
```

Unsupported semantics are explicit diagnostics/capability gaps, not silently discarded. Custom widgets/plugins must either provide semantics or explicitly declare themselves presentation-only.

## Agent/devtools projection

The internal semantic observation API exposes bounded, non-native data suitable for tests, devtools, and optional agent integrations:

```text
node identity
role/name/description/value
states/actions/relations
logical children
source/component metadata in development
backend capability/mapping diagnostics in development
```

Do not expose raw native handles, arbitrary host memory, or unbounded native accessibility dumps through the portable semantic API.

`desktop-cli` remains an external black-box oracle. It validates that the semantic graph was actually projected into UIA/AT-SPI/AX-visible native accessibility rather than merely being correct internally.

## Acceptance implications

SVR03 must establish semantic graph invariants before Svelte or any GUI toolkit lands. Later backend phases must include semantic mapping for every portable widget they introduce. SVR20 builds inspector and external accessibility/agent conformance tooling on top of this contract; it must not redefine the model.
