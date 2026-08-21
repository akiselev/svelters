# SVR20 - native Svelte devtools, accessibility, and observability

References: `plans/architecture/ecosystem-integrations.md`.

## Goal

Exploit the logical graph and canonical binding IDs to provide debugging capabilities that ordinary native GUI stacks usually lack, while validating the externally observable accessibility/agent surface independently of Svelters internals.

## Inspector

Expose a development inspector showing:

- Svelte component/source identity;
- logical graph tree and invisible anchors/fragments;
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

## Accessibility semantics

Define the portable semantic/accessibility projection required of core widgets: role, name/label, value/state, focusability, enabled/disabled, selection, hierarchy and supported actions. Backends map these semantics to native GTK/AT-SPI, Qt accessibility and egui's accessibility integration rather than exposing only visual nodes.

Stable Svelters NodeIdentity is not automatically a public accessibility identifier, but where a stable developer/semantic ID is explicitly supplied it should survive into the backend accessibility surface when the toolkit permits it.

## External desktop-cli oracle

Use `desktop-cli` strictly as an external GPL test executable, never a linked library dependency. Against real running reference applications on supported platforms:

- `observe` the UI through Windows UIA, Linux AT-SPI2 or macOS AX;
- query buttons, inputs, lists/trees and focused/selected state by semantics;
- invoke activate/focus/set-value/select/scroll actions;
- verify the same user-visible semantics across GTK4, QtWidgets and egui;
- compare selected external observations with the internal Svelters inspector to catch missing native accessibility exposure.

Keep deterministic snapshots/expected semantic assertions in Svelters where licensing permits; do not copy GPL implementation code.

## Profiling

Instrument Svelte effect flush, graph mutation, patch batching, native application, egui frame traversal, binding invocation and event-to-JS latency. Export trace events in a standard format where practical.

Add `svelters doctor` for runtime/toolkit/API DB/binding-pack diagnostics.

## Acceptance

- use the inspector to diagnose an intentionally introduced stale property, slow patch batch and native binding error;
- source navigation identifies the original `.svelte` callsite;
- desktop-cli can externally locate and operate representative controls in GTK4, QtWidgets and egui reference apps on platforms where the backend is available;
- an intentional missing accessibility label/role causes the external oracle test to fail even if internal graph tests remain green;
- devtools can be compiled out/disabled so release-size budgets are unaffected.
