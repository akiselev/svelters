# SVR10 - GTK4 renderer and thin release bindings

References: `plans/architecture/native-binding-linker.md`, `plans/architecture/native-api-ir.md`, and `plans/architecture/semantic-graph.md`.

## Goal

Deliver the first real retained native backend and prove the fat-development/thin-release split while also proving that the foundational semantic graph is realized into GTK's native accessibility surface from the start.

## Renderer

Implement a GTK4 backend over the graph/patch API with initial support for Application/Window, Box-style layout, Label, Button, Entry/Text, CheckButton, ScrolledWindow, menus/actions needed by the reference app, and focus/window lifecycle.

Use explicit structural policies rather than one universal append operation. Parent-owned GObjects and external destruction must invalidate Svelters native handles without use-after-free.

Events are queued and dispatched at runtime checkpoints; signal handlers do not reenter arbitrary JS while GTK internals hold unsafe transient state unless a specific synchronous contract requires it.

For every supported portable/native widget in this phase, map the canonical semantic role/name/description/value/state/actions/relations into GTK4's accessibility APIs so the resulting controls are AT-SPI-visible. Do not reconstruct semantics from GTK after rendering; the Svelters graph is the source of truth. Report unsupported semantic mappings as backend capability diagnostics.

## Release bindings

Feed SVR08's manifest into a GTK thin generator. GObject-reflected operations may remain generic. For live ordinary methods, generate direct gtk-rs `-sys`/C FFI wrappers where that improves code size, static-link visibility, or latency. Reuse canonical converters.

Development continues to use GIRepository for broad callable coverage.

## Fixtures

Port the old Svelters GTK application behavior: dynamic conditional children, buttons/signals, `bind:this`, imperative show/focus/destruction-equivalent operations, and basic list behavior (the real model/view subsystem lands later).

Add semantic fixtures for a labelled input, checkable control, focusable button, disabled state, and a list/tree-shaped logical sample. Compare the backend's intended mapping against the same TestHost semantic snapshot.

## Acceptance

- reference GTK app runs from Svelte 5/QuickJS;
- `.svelte` edits HMR without Cargo rebuild;
- a newly referenced GI-supported method works in dev without rebuilding Rust;
- release manifest contains only used binding roots/dependencies;
- thin release bridge is materially smaller than the fat dev bridge and its size is recorded;
- representative controls expose the canonical role/name/state/actions through GTK accessibility rather than visual-only widget state;
- an intentionally unsupported semantic field produces a capability diagnostic rather than being silently dropped;
- ASan/Valgrind-equivalent smoke verifies no obvious native lifetime leaks/UAF in create/remove/reorder stress.
