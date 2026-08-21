# SVR10 - GTK4 renderer and thin release bindings

References: `plans/architecture/native-binding-linker.md`, `plans/architecture/native-api-ir.md`.

## Goal

Deliver the first real retained native backend and prove the fat-development/thin-release split.

## Renderer

Implement a GTK4 backend over the graph/patch API with initial support for Application/Window, Box-style layout, Label, Button, Entry/Text, CheckButton, ScrolledWindow, menus/actions needed by the reference app, and focus/window lifecycle.

Use explicit structural policies rather than one universal append operation. Parent-owned GObjects and external destruction must invalidate Svelters native handles without use-after-free.

Events are queued and dispatched at runtime checkpoints; signal handlers do not reenter arbitrary JS while GTK internals hold unsafe transient state unless a specific synchronous contract requires it.

## Release bindings

Feed SVR08's manifest into a GTK thin generator. GObject-reflected operations may remain generic. For live ordinary methods, generate direct gtk-rs `-sys`/C FFI wrappers where that improves code size, static-link visibility, or latency. Reuse canonical converters.

Development continues to use GIRepository for broad callable coverage.

## Fixtures

Port the old Svelters GTK application behavior: dynamic conditional children, buttons/signals, `bind:this`, imperative show/focus/destruction-equivalent operations, and basic list behavior (the real model/view subsystem lands later).

## Acceptance

- reference GTK app runs from Svelte 5/QuickJS;
- `.svelte` edits HMR without Cargo rebuild;
- a newly referenced GI-supported method works in dev without rebuilding Rust;
- release manifest contains only used binding roots/dependencies;
- thin release bridge is materially smaller than the fat dev bridge and its size is recorded;
- ASan/Valgrind-equivalent smoke verifies no obvious native lifetime leaks/UAF in create/remove/reorder stress.
