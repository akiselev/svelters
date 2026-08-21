# SVR23 - production hardening and release readiness

References: `plans/architecture/ecosystem-integrations.md`.

## Goal

Turn the completed architecture into a framework that can support real desktop applications and a stable first public compatibility promise.

## Correctness and safety

- fuzz graph mutations, serialized plugin inputs, binding argument decoding and native-value conversion;
- long-running QuickJS GC/native-handle cycle tests;
- callback-after-destroy and stale-generation fault injection;
- repeated Svelte HMR and renderer-generation reload stress;
- Svelte keyed-each/reorder stress against all renderers;
- toolkit-external deletion/ownership-transfer tests;
- ASan/UBSan generated Qt C++/FFI jobs; Miri where applicable;
- panic/exception boundaries and shutdown tests.

## Optional deterministic-rs adapter

Do not add deterministic-rs to production dependencies. Build a test-only adapter only after normal unit/property/model tests exist. Route the narrow nondeterministic surfaces that matter to Svelters through it where practical: virtual time/timers, queued event scheduling, async completion, candidate reload/failure decisions and shutdown ordering.

Use DecisionTape replay/reduction on concrete race classes such as:

- timer/async completion after component destruction;
- native event queued across a renderer generation swap;
- HMR update racing a callback or effect flush;
- Switchyard candidate failure at each staged-publication checkpoint;
- shutdown with pending JS jobs/events.

The adapter is retained only if it finds or reduces real bugs better than the existing harnesses. Record that evaluation.

## External accessibility/agent release gate

Use desktop-cli as an external executable on supported native environments to verify the reference application through platform accessibility APIs. Core interactions must be possible semantically, not only through Svelters' internal graph: observe, focus, activate, edit/select and scroll representative controls. Keep the GPL tool outside linked Svelters artifacts.

If the optional appfs platform service landed in SVR21, include its upstream provider/conformance and crash-recovery tests in the integration matrix.

## Performance budgets

Track at least:

- cold startup;
- first window/frame;
- event-to-JS and JS-to-native patch latency;
- large keyed update/reorder;
- 100k-row virtual list;
- HMR latency;
- dev fat-bridge startup;
- release build time;
- executable/package size broken down into app, QuickJS, Svelters and binding bridge where possible;
- idle CPU/repaint activity.

Set regression thresholds only after stable baselines exist, but require explanation for all material regressions.

## Compatibility matrix

Define supported Svelte 5 range/update policy, Rust MSRV, Node/Vite dev requirement, QuickJS version, GTK minimum/maximum tested range, Qt 6 tested range, and OS/architecture matrix. Do not promise untested static-toolkit combinations.

## Documentation

Write user-facing quick start, architecture, portable/native API guidance, HMR/devtools, custom widget SDK, deployment and troubleshooting. Mark browser-specific Svelte features and unsupported native semantics explicitly.

## Release gate

A release candidate must build and pass the editor-shell/reference application on GTK4, QtWidgets and egui; pass upstream runtime compatibility gates; pass binding-linker dead-code tests; pass external accessibility/agent smoke tests where platform environments exist; satisfy known size/performance budgets; and have no untriaged `Unsupported/Unknown` entries required by the documented core feature set.

At this point evaluate, with measurements, whether a small Svelte compiler native-target patch would reduce complexity or improve performance. Compiler forking is still optional, not a prerequisite for v2.
