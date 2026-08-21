# SVR20 - native Svelte devtools and observability

## Goal

Exploit the logical graph and canonical binding IDs to provide debugging capabilities that ordinary native GUI stacks usually lack.

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

## Profiling

Instrument Svelte effect flush, graph mutation, patch batching, native application, egui frame traversal, binding invocation and event-to-JS latency. Export trace events in a standard format where practical.

Add `svelters doctor` for runtime/toolkit/API DB/binding-pack diagnostics.

## Acceptance

Use the inspector to diagnose an intentionally introduced stale property, slow patch batch and native binding error in the reference app. Source navigation identifies the original `.svelte` callsite. Devtools can be compiled out/disabled so release-size budgets are unaffected.
