# SVR20 - native Svelte devtools, accessibility realization, and observability

References: `plans/architecture/ecosystem-integrations.md` and `plans/architecture/semantic-graph.md`.

## Goal

Exploit the foundational logical/semantic graph and canonical binding IDs to provide debugging capabilities that ordinary native GUI stacks usually lack, while validating that each renderer correctly realizes the already-defined semantic contract into externally observable accessibility/agent surfaces.

SVR20 does not invent portable accessibility semantics. Those are part of SVR03 and `plans/architecture/semantic-graph.md`. If this phase discovers that the foundational semantic model is insufficient, update that architecture explicitly rather than creating a second devtools-only model.

## Inspector

Expose a development inspector showing:

- Svelte component/source identity;
- logical graph tree and invisible anchors/fragments;
- semantic role/name/value/state/actions/relations and developer IDs;
- backend semantic capability/mapping diagnostics;
- backend native realization and native handle generation;
- live properties/listeners;
- native binding canonical IDs and invocation strategy;
- source callsites for native methods;
- model/view state;
- focus/selection/scroll state;
- HMR generations;
- Switchyard renderer generation;
- patch/event timings.

Support selecting a visible native widget and locating its logical node/source, plus selecting a logical node and highlighting its native realization where the backend can do so.

## Accessibility realization validation

Audit the semantic mappings implemented incrementally by GTK4, QtWidgets, and egui backend phases. Each backend must translate the canonical graph's role, name/description, value/state, focusability, selection, hierarchy, relations, and supported actions into its native accessibility integration.

Stable Svelters `NodeIdentity` is not automatically a public accessibility identifier. An explicitly supplied `DeveloperId` should survive into the backend accessibility surface where the toolkit/platform permits it without violating native conventions.

Add diagnostics for semantic information that is present in the canonical graph but not representable/exposed by the active backend. These gaps must be observable rather than silently dropped.

## External desktop-cli oracle

Use `desktop-cli` strictly as an external GPL test executable, never a linked library dependency. Against real running reference applications on supported platforms:

- `observe` the UI through Windows UIA, Linux AT-SPI2 or macOS AX;
- query buttons, inputs, lists/trees and focused/selected state by semantics;
- invoke activate/focus/set-value/select/scroll actions;
- verify the same user-visible semantics across GTK4, QtWidgets and egui;
- compare selected external observations with the internal canonical semantic graph to catch missing or incorrect native accessibility projection.

Keep deterministic snapshots/expected semantic assertions in Svelters where licensing permits; do not copy GPL implementation code.

## Profiling

Instrument Svelte effect flush, graph/semantic mutation, patch batching, native application, egui frame traversal, binding invocation and event-to-JS latency. Export trace events in a standard format where practical.

Add `svelters doctor` for runtime/toolkit/API DB/binding-pack and semantic-capability diagnostics.

## Acceptance

- use the inspector to diagnose an intentionally introduced stale property, stale semantic state, slow patch batch and native binding error;
- source navigation identifies the original `.svelte` callsite;
- internal TestHost semantic snapshots remain identical to the foundational SVR03 contract;
- desktop-cli can externally locate and operate representative controls in GTK4, QtWidgets and egui reference apps on platforms where the backend is available;
- an intentional backend omission of an accessibility label/role/action causes the external oracle test to fail even though the canonical internal semantic graph remains correct;
- unsupported semantic mappings are surfaced as diagnostics;
- devtools can be compiled out/disabled so release-size budgets are unaffected.
