# SVR19 - custom widget and plugin SDK

## Goal

Let applications and third parties add renderer-specific widgets, converters and commands without editing Svelters core.

## SDK

Provide Rust APIs/macros or code-generation helpers conceptually equivalent to:

```text
#[svelters_widget]
#[svelters_command]
#[svelters_converter]
```

A widget extension declares stable type/binding IDs, graph properties/events, backend compatibility, state checkpoint/restore hooks, and renderer implementation. egui extensions render immediate operations; GTK/Qt extensions may own native objects through the same handle/lifetime system.

Package extensions as Switchyard static or trusted-native plugins. Non-renderer services may use process/Wasm backends when isolation is valuable.

Generate TypeScript/Svelte declarations from extension manifests so custom widgets are typed exactly like built-in/native ones.

Define namespace/version conflict rules and deterministic capability resolution. An extension cannot shadow a canonical built-in type silently.

## Acceptance

Build a nontrivial example extension, preferably a plot or terminal-like widget, for at least egui and one retained backend. Hot-reload the native extension generation while a Svelte app remains running. Its TS declarations, events, properties and release binding roots are generated automatically.
