# SVR03 - logical UI graph and TestHost

References: `plans/architecture/native-api-ir.md` for stable handle concepts only.

## Goal

Implement the toolkit-neutral retained logical graph that is canonical for all backends.

## Work

Create `svelters-core`, `svelters-graph`, `svelters-backend-api`, and `svelters-backend-test`.

The graph supports element/text/comment/fragment/anchor nodes; parent/child/sibling traversal; stable NodeId and source identity; property values; event listener identities; native handle slots; and batched mutation patches.

Define deterministic operations for create, insert-before, append, move, remove, replace, set text/property, listener add/remove, and command enqueue. Invalid cycles, double removal, stale handles, and cross-backend handles are explicit errors.

TestHost applies batches to an in-memory realization and records normalized snapshots/events. It must be deterministic and usable without a GUI or QuickJS.

## Testing

- unit tests for all mutations;
- property tests comparing random operation sequences against a simple reference tree;
- stale-generation and invalid-parent tests;
- serialization/snapshot stability;
- benchmark large keyed reorder/insert/remove batches.

## Non-goals

No toolkit widgets, CSS, model/view virtualization, or Svelte runtime yet.

## Acceptance

At least one randomized property test executes thousands of valid/invalid operations without graph/reference divergence. The graph can rebuild a TestHost realization from scratch and produce the same normalized snapshot.
