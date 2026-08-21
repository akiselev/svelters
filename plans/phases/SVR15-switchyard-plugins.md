# SVR15 - Switchyard renderer/plugin generations and native fast path

References: `plans/architecture/runtime-stack.md`, `plans/architecture/native-api-ir.md`, `plans/architecture/ecosystem-integrations.md`.

## Goal

Make renderer implementations and generated native binding packs real plugins with generation-safe hot reload, while preserving an efficient in-process/static production path. Evolve Switchyard generically where Svelters and egui-workbench share requirements rather than creating a Svelters-private reload system.

## Shared Switchyard substrate

Before adding Svelters-specific services, audit current Switchyard and the active egui-workbench extension plan. Generic substrate work belongs in Switchyard and should include, where not already present:

- unambiguous generation identity keyed by plugin plus generation;
- staged publication so a candidate generation and host-visible capability/contribution set can commit atomically;
- last-known-good rollback on failed validation/restore;
- real uninstall/retirement semantics;
- service/capability collision detection;
- structured build/watch diagnostics and artifact publication events;
- stable host-owned resources/callbacks/tasks with generation-safe invalidation.

Land and validate generic Switchyard changes before relying on them from Svelters. Do not encode Svelte graph types in Switchyard.

## Svelters service and native fast path

Define a versioned `svelters.renderer` service covering lifecycle, capability query, graph rebuild, checkpoint/restore, health, and generic serialized invocation for test/process/Wasm backends.

Do not send every patch through Switchyard's generic byte payload RPC. Add a generic optional schema-fingerprinted native-extension descriptor/vtable to Switchyard, usable only while the owning generation guard is pinned. Svelters defines a renderer extension on top with operations conceptually including:

```text
begin_batch
apply_patches
invoke_binding
end_batch
poll_events
rebuild_graph
```

All structures crossing the extension use explicit C-compatible layouts, version/size fields, stable IDs and host-owned allocation rules. Generic service invocation remains the compatibility fallback. The extension mechanism itself must be useful to non-Svelters consumers.

## Renderer publication

Convert TestHost, GTK4, QtWidgets and egui backends to the common renderer plugin contract. Support:

- statically linked renderer registration for release;
- trusted native generation loading for development;
- process backend for isolation/testing where latency is acceptable;
- capability/schema fingerprint negotiation.

## Reload semantics

The logical graph is canonical. On renderer generation replacement, do not preserve raw QObject/GObject/Rust widget references. Checkpoint semantic renderer state, stage and validate the candidate, publish the healthy new generation, rebuild native realization from the graph, then restore portable state such as focus, geometry, selections and scroll positions when supported.

Use Switchyard's conservative native generation-retention policy in development rather than pretending arbitrary native code is safely unloadable.

## Acceptance

- generic Switchyard changes have tests that do not depend on Svelters;
- each renderer runs through the same Switchyard service contract;
- native fast path and generic serialized path produce equivalent TestHost results;
- change backend Rust code, build/publish a new generation, and recover the running app graph without process restart;
- failed candidate health/restore leaves the previous renderer active with no partially published contribution set;
- uninstall/retirement leaves no callable stale renderer handles;
- static release mode has no dynamic-plugin requirement.
