# SVR23 - production hardening and release readiness

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
- panic/exception boundaries and deterministic shutdown tests.

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

A release candidate must build and pass the editor-shell/reference application on GTK4, QtWidgets and egui; pass upstream runtime compatibility gates; pass binding-linker dead-code tests; satisfy known size/performance budgets; and have no untriaged `Unsupported/Unknown` entries required by the documented core feature set.

At this point evaluate, with measurements, whether a small Svelte compiler native-target patch would reduce complexity or improve performance. Compiler forking is still optional, not a prerequisite for v2.
