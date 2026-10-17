# SVR03 - logical UI graph, semantic graph, and TestHost

References: `plans/architecture/native-api-ir.md` for stable handle concepts only, and `plans/architecture/semantic-graph.md` for the semantic/accessibility contract.

## Goal

Implement the toolkit-neutral retained logical graph that is canonical for all backends, including the user-facing semantic/accessibility data that later native renderers, devtools, tests, and agent integrations project from it.

## Work

Create `svelters-core`, `svelters-graph`, `svelters-backend-api`, and `svelters-backend-test`.

The graph supports element/text/comment/fragment/anchor nodes; parent/child/sibling traversal; stable runtime `NodeId`; deterministic `NodeIdentity` for HMR/rebuild/egui identity; optional application-supplied `DeveloperId`; source identity; property values; event listener identities; native handle slots; semantic records; and batched mutation patches.

Keep identity layers explicit:

- `NodeId` is the runtime generational graph handle;
- `NodeIdentity` correlates logical identity across renderer rebuilds/HMR where semantics permit;
- `DeveloperId` is an optional public semantic identifier supplied by the application;
- source location/component identity is development metadata and is not automatically exposed as accessibility identity.

Define the canonical `Semantics` model described in `plans/architecture/semantic-graph.md`: role, name, description, value, states, supported actions, relations, and optional developer ID. The representation may use compact/interned values, but it must not use backend-native GTK/Qt/egui enums as the source of truth.

Define deterministic operations for create, insert-before, append, move, remove, replace, set text/property, semantic update, listener add/remove, and command enqueue. Visual/property and semantic changes that describe one state transition must be publishable in the same batch. Invalid cycles, invalid semantic relations, double removal, stale handles, and cross-backend handles are explicit errors.

TestHost applies batches to an in-memory realization and records two normalized views:

1. the complete logical structural/property snapshot used for renderer correctness;
2. the bounded semantic projection used for accessibility/agent correctness.

TestHost must be deterministic and usable without a GUI or QuickJS. It is the first implementation proving that semantics exist independently of native accessibility frameworks.

Portable semantic defaults should be data-driven enough that later portable widget phases can extend them without changing graph identity. Custom/native widget semantics remain backend/API metadata concerns, but the graph representation is fixed here.

## Testing

- unit tests for all structural, property, semantic, and relation mutations;
- property tests comparing random operation sequences against a simple reference tree;
- semantic invariant tests for relations to deleted/stale nodes, contradictory states, and action/state transitions;
- stable-identity tests covering keyed reorder and logical rebuild correlation;
- stale-generation and invalid-parent tests;
- serialization/snapshot stability for both structural and semantic projections;
- tests proving semantic changes and corresponding visual/property changes cannot be observed as separate partially applied batches;
- benchmark large keyed reorder/insert/remove batches with semantic metadata enabled.

## Non-goals

No toolkit widgets, native accessibility implementation, CSS, model/view virtualization, Svelte runtime, or desktop-cli integration yet. SVR20 validates external native accessibility projection; it does not define semantics.

## Acceptance

At least one randomized property test executes thousands of valid/invalid operations without graph/reference divergence. The graph can rebuild a TestHost realization from scratch and produce the same normalized structural and semantic snapshots. A representative logical form with button/input/list/tree semantics can be queried entirely through TestHost without GTK, Qt, egui, or QuickJS, and deleting/reordering nodes cannot leave dangling semantic relations or identity aliases.
