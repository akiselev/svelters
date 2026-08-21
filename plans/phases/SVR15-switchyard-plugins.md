# SVR15 - Switchyard renderer/plugin generations and native fast path

References: `plans/architecture/runtime-stack.md`, `plans/architecture/native-api-ir.md`.

## Goal

Make renderer implementations and generated native binding packs real plugins with generation-safe hot reload, while preserving an efficient in-process/static production path.

## Switchyard integration

Define a versioned `svelters.renderer` service covering lifecycle, capability query, graph rebuild, checkpoint/restore, health, and generic serialized invocation for test/process/Wasm backends.

Do not send every patch through Switchyard's generic byte payload RPC. Add an optional schema-fingerprinted native extension descriptor/vtable to Switchyard, usable only while the owning generation guard is pinned. Proposed renderer fast path includes:

```text
begin_batch
apply_patches
invoke_binding
end_batch
poll_events
rebuild_graph
```

All structures crossing this extension use explicit C-compatible layouts, version/size fields, stable IDs and host-owned allocation rules. Generic service invocation remains the compatibility fallback.

## Renderer publication

Convert TestHost, GTK4, QtWidgets and egui backends to the common renderer plugin contract. Support:

- statically linked renderer registration for release;
- trusted native generation loading for development;
- process backend for isolation/testing where latency is acceptable;
- capability/schema fingerprint negotiation.

## Reload semantics

The logical graph is canonical. On renderer generation replacement, do not preserve raw QObject/GObject/Rust widget references. Checkpoint semantic renderer state, publish the healthy new generation, rebuild native realization from the graph, then restore portable state such as focus, geometry, selections and scroll positions when supported.

Use Switchyard's conservative native generation-retention policy in development rather than pretending arbitrary native code is safely unloadable.

## Acceptance

- each renderer runs through the same Switchyard service contract;
- native fast path and generic serialized path produce equivalent TestHost results;
- change backend Rust code, build/publish a new generation, and recover the running app graph without process restart;
- failed candidate health/restore leaves the previous renderer active;
- static release mode has no dynamic-plugin requirement.
